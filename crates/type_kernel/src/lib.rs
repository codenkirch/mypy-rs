//! Native type-kernel seam for mypy.
//!
//! Stage 1: implements `erase_type` as a PyO3 extension that walks live
//! Python `mypy.types.Type` objects and produces the erased `ProperType`,
//! mirroring `mypy.erasetype.EraseTypeVisitor`.
//!
//! The seam is intentionally narrow: one function, `erase_type`, with a
//! per-call fallback contract — if Rust does not recognise a type class or
//! cannot resolve a `TypeInfo` snapshot entry, it returns `None` and the
//! Python caller falls back to the pure-Python visitor. This is the
//! strangler-fig gate: no behavior changes unless `Options.native_type_kernel`
//! is set, and even then unsupported cases degrade gracefully.
//!
//! Why `erase_type` is the right first operation:
//!   * Pure visitor (`Type -> ProperType`), no plugin hooks, no input mutation.
//!   * The only `TypeInfo` dependency is `defn.type_vars` (count + kinds),
//!     passed in as a `dict[str, list[TypeVarLikeType]]` snapshot.
//!   * Well-tested in `mypy/test/testtypes.py` with a string-equality parity
//!     contract (`str(erase_type(t)) == str(expected)`).

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyType};

/// Cache of Python constructors and class objects, looked up once per call
/// (PyO3 cell borrow is cheap; the GIL-bound reference is what we hold).
struct TypeRefs<'py> {
    /// mypy.types.AnyType
    any_type: &'py PyType,
    /// mypy.types.NoneType
    none_type: &'py PyType,
    /// mypy.types.UninhabitedType
    uninhabited_type: &'py PyType,
    /// mypy.types.TypeVarType
    type_var_type: &'py PyType,
    /// mypy.types.ParamSpecType
    param_spec_type: &'py PyType,
    /// mypy.types.TypeVarTupleType
    type_var_tuple_type: &'py PyType,
    /// mypy.types.UnpackType
    unpack_type: &'py PyType,
    /// mypy.types.LiteralType
    literal_type: &'py PyType,
    /// mypy.types.DeletedType
    deleted_type: &'py PyType,
    /// mypy.types.Instance
    instance: &'py PyType,
    /// mypy.types.CallableType
    callable_type: &'py PyType,
    /// mypy.types.UnionType
    union_type: &'py PyType,
    /// mypy.types.TupleType
    tuple_type: &'py PyType,
    /// mypy.types.TypeType
    type_type: &'py PyType,
    /// mypy.types.Overloaded
    overloaded: &'py PyType,
    /// mypy.types.TypedDictType
    typed_dict_type: &'py PyType,
    /// mypy.types.TypeOfAny (the IntEnum)
    type_of_any: &'py PyType,
}

impl<'py> TypeRefs<'py> {
    fn try_new(py: Python<'py>) -> PyResult<Self> {
        let types_mod = py.import("mypy.types")?;
        macro_rules! class {
            ($name:literal) => {{
                types_mod.getattr($name)?.downcast::<PyType>()?
            }};
        }
        Ok(TypeRefs {
            any_type: class!("AnyType"),
            none_type: class!("NoneType"),
            uninhabited_type: class!("UninhabitedType"),
            type_var_type: class!("TypeVarType"),
            param_spec_type: class!("ParamSpecType"),
            type_var_tuple_type: class!("TypeVarTupleType"),
            unpack_type: class!("UnpackType"),
            literal_type: class!("LiteralType"),
            deleted_type: class!("DeletedType"),
            instance: class!("Instance"),
            callable_type: class!("CallableType"),
            union_type: class!("UnionType"),
            tuple_type: class!("TupleType"),
            type_type: class!("TypeType"),
            overloaded: class!("Overloaded"),
            typed_dict_type: class!("TypedDictType"),
            type_of_any: class!("TypeOfAny"),
        })
    }
}

/// Look up `TypeOfAny.special_form` once per call. Cheap, but avoids repeated
/// attribute resolution in the hot path.
fn type_of_any_special_form(type_of_any: &PyType) -> PyResult<PyObject> {
    // TypeOfAny is an IntEnum; special_form is an int value, not a callable.
    Ok(type_of_any.getattr("special_form")?.into())
}

/// Construct `AnyType(TypeOfAny.special_form)` with no source/missing-import.
fn make_any(_py: Python<'_>, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let type_of_any = type_of_any_special_form(refs.type_of_any)?;
    Ok(refs.any_type.call1((type_of_any,))?.into())
}

/// Return true if `obj` is an instance of `cls` (PyType check).
fn is_instance(obj: &PyAny, cls: &PyType) -> bool {
    obj.is_instance(cls).unwrap_or(false)
}

/// Erase a single `Type` object. Returns the erased `ProperType`, or `None`
/// (the fallback sentinel) if Rust does not handle this case.
fn erase_one(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    // Class dispatch is by `isinstance` against the resolved class objects,
    // not by string compare, so plugin subclasses are handled correctly.
    //
    // Order mirrors the Python EraseTypeVisitor: leaf types first, then
    // the composite types that recurse.

    // --- Trivial leaves (return as-is) ---
    if is_instance(obj, refs.any_type)
        || is_instance(obj, refs.none_type)
        || is_instance(obj, refs.uninhabited_type)
        || is_instance(obj, refs.deleted_type)
        || is_instance(obj, refs.literal_type)
    {
        // These visitors all `return t` unchanged.
        return Ok(obj.into());
    }

    // --- TypeVar-like -> AnyType(special_form) ---
    if is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.unpack_type)
    {
        return make_any(py, refs);
    }

    // --- TypeVarTupleType -> fallback tuple with Any args ---
    // The Python visitor does:
    //   return t.tuple_fallback.copy_modified(args=[AnyType(special_form)])
    // `copy_modified` on an Instance is a Python method we'd need to call;
    // rather than special-case it, fall back. This is rare and the fallback
    // path handles it correctly.
    if is_instance(obj, refs.type_var_tuple_type) {
        return fallback_sentinel(py);
    }

    // --- Instance ---
    // Python visitor:
    //   args = erased_vars(t.type.defn.type_vars, TypeOfAny.special_form)
    //   return Instance(t.type, args, t.line)
    // Stage 1 reads `t.type.defn.type_vars` directly from the live TypeInfo.
    if is_instance(obj, refs.instance) {
        return erase_instance(py, obj, refs);
    }

    // --- CallableType ---
    // Python visitor: replace arg_types/arg_kinds/arg_names with the
    // `Callable[..., Any]` shape, preserve fallback, set is_ellipsis_args=True.
    if is_instance(obj, refs.callable_type) {
        return erase_callable(py, obj, refs);
    }

    // --- Overloaded ---
    // Python visitor: `return t.fallback.accept(self)` — recurse on fallback.
    if is_instance(obj, refs.overloaded) {
        let fallback = obj.getattr("fallback")?;
        return erase_one(py, fallback, refs);
    }

    // --- TupleType ---
    // Python visitor: `return t.partial_fallback.accept(self)` — recurse.
    if is_instance(obj, refs.tuple_type) {
        let fallback = obj.getattr("partial_fallback")?;
        return erase_one(py, fallback, refs);
    }

    // --- TypedDictType ---
    // Python visitor: `return t.fallback.accept(self)` — recurse.
    if is_instance(obj, refs.typed_dict_type) {
        let fallback = obj.getattr("fallback")?;
        return erase_one(py, fallback, refs);
    }

    // --- TypeType ---
    // Python visitor:
    //   return TypeType.make_normalized(t.item.accept(self), line=t.line,
    //                                  is_type_form=t.is_type_form)
    if is_instance(obj, refs.type_type) {
        let item = obj.getattr("item")?;
        let erased_item = erase_one(py, item, refs)?;
        if is_fallback(&erased_item, py) {
            return Ok(erased_item);
        }
        let line = obj.getattr("line")?;
        let is_type_form = obj.getattr("is_type_form")?;
        let type_type_cls = refs.type_type;
        let make_normalized = type_type_cls.getattr("make_normalized")?;
        // make_normalized(item, *, line=-1, column=-1, is_type_form=False)
        // — line and is_type_form are keyword-only.
        let kwargs = PyDict::new(py);
        kwargs.set_item("line", line)?;
        kwargs.set_item("is_type_form", is_type_form)?;
        let result = make_normalized.call((erased_item,), Some(kwargs))?;
        return Ok(result.into());
    }

    // --- UnionType ---
    // Python visitor:
    //   erased_items = [erase_type(item) for item in t.items]
    //   return make_simplified_union(erased_items)
    // We recurse on each item; if any item falls back, we fall back the whole
    // union (conservative — Python path is unchanged).
    if is_instance(obj, refs.union_type) {
        return erase_union(py, obj, refs);
    }

    // Anything else (UnboundType, ErasedType, PartialType, PlaceholderType,
    // Parameters, RawExpressionType, CallableArgument, TypeList, EllipsisType,
    // TypeAliasType which the visitor raises on, TypeGuardedType which is
    // unwrapped by get_proper_type before we see it) — fall back.
    fallback_sentinel(py)
}

/// Build the fallback sentinel as a Python `None`. The Python caller treats
/// `None` as "Rust did not handle this; use the Python visitor".
fn fallback_sentinel(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

/// True if `obj` is the Python `None` sentinel (our fallback marker).
fn is_fallback(obj: &PyObject, py: Python<'_>) -> bool {
    obj.is(&py.None())
}

/// Erase an `Instance`: read `t.type.defn.type_vars` from the live TypeInfo
/// (same as the Python visitor), build `AnyType`/`UnpackType` erased args
/// mirroring `erased_vars`, construct a new `Instance(t.type, args, t.line)`.
///
/// Stage 1 reads `defn.type_vars` directly from the live object — no snapshot
/// cache needed because we hold a Python `Type` object. Stage 3 (Rust-owned
/// Type enum on the bytes seam) will introduce a snapshot protocol since Rust
/// won't have the live TypeInfo graph.
fn erase_instance(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let typ = obj.getattr("type")?;
    let line = obj.getattr("line")?;

    // Read defn.type_vars directly from the live TypeInfo, mirroring the
    // Python visitor's `t.type.defn.type_vars`.
    let defn = match typ.getattr("defn") {
        Ok(d) => d,
        Err(_) => return fallback_sentinel(py),
    };
    let type_vars = match defn.getattr("type_vars") {
        Ok(tv) => match tv.downcast::<PyList>() {
            Ok(list) => list,
            // type_vars is typed as Sequence, so could be a tuple; fall back
            // rather than coerce — the Python path handles any sequence.
            Err(_) => return fallback_sentinel(py),
        },
        Err(_) => return fallback_sentinel(py),
    };

    let any_type = make_any(py, refs)?;
    let mut erased_args: Vec<PyObject> = Vec::with_capacity(type_vars.len());
    for tv in type_vars.iter() {
        if is_instance(tv, refs.type_var_tuple_type) {
            // Valid erasure for *Ts is *tuple[Any, ...], not just Any.
            // Python: UnpackType(tv.tuple_fallback.copy_modified(args=[Any]))
            // We call copy_modified via PyO3 to avoid reconstructing the
            // tuple_fallback Instance ourselves.
            let tuple_fallback = tv.getattr("tuple_fallback")?;
            let copy_modified = tuple_fallback.getattr("copy_modified")?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("args", vec![&any_type])?;
            let erased_fallback = copy_modified.call((), Some(kwargs))?;
            // UnpackType(tv) copies name/id/etc from tv; we want the erased
            // fallback, so construct UnpackType(type=erased_fallback).
            // UnpackType.__init__ signature: (self, typ, *, name=None, line=-1, column=-1)
            // The first positional arg is the type to unpack.
            let unpack = refs.unpack_type.call1((erased_fallback,))?;
            erased_args.push(unpack.into());
        } else {
            // TypeVar or ParamSpec -> AnyType(special_form)
            erased_args.push(any_type.clone_ref(py));
        }
    }

    let args_pylist = PyList::new(py, &erased_args);
    let result = refs.instance.call1((typ, args_pylist, line))?;
    Ok(result.into())
}

/// Erase a `CallableType`: produce `Callable[..., Any]` preserving the fallback.
/// Python visitor:
///   any_type = AnyType(TypeOfAny.special_form)
///   return CallableType(
///     arg_types=[any_type, any_type],
///     arg_kinds=[ARG_STAR, ARG_STAR2],
///     arg_names=[None, None],
///     ret_type=any_type,
///     fallback=t.fallback,
///     is_ellipsis_args=True,
///     implicit=True,
///   )
fn erase_callable(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let any_type = make_any(py, refs)?;
    let fallback = obj.getattr("fallback")?;

    // ARG_STAR, ARG_STAR2 are module-level constants in mypy.nodes.
    let nodes_mod = py.import("mypy.nodes")?;
    let arg_star = nodes_mod.getattr("ARG_STAR")?;
    let arg_star2 = nodes_mod.getattr("ARG_STAR2")?;

    let arg_types = PyList::new(py, [&any_type, &any_type]);
    let arg_kinds = PyList::new(py, [arg_star, arg_star2]);
    let arg_names = PyList::new(py, [py.None(), py.None()]);

    // CallableType constructor uses keyword args for everything except
    // arg_types/arg_kinds/arg_names/ret_type. We pass fallback,
    // is_ellipsis_args, and implicit via kwargs to match the Python visitor.
    let kwargs = PyDict::new(py);
    kwargs.set_item("ret_type", &any_type)?;
    kwargs.set_item("fallback", fallback)?;
    kwargs.set_item("is_ellipsis_args", true)?;
    kwargs.set_item("implicit", true)?;
    let result = refs
        .callable_type
        .call((arg_types, arg_kinds, arg_names), Some(kwargs))?;
    Ok(result.into())
}

/// Erase a `UnionType`: recurse on each item, then call
/// `mypy.typeops.make_simplified_union`. Falls back if any item falls back.
fn erase_union(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?
        .downcast::<PyList>()?;
    let mut erased_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let erased = erase_one(py, item, refs)?;
        if is_fallback(&erased, py) {
            // Conservative: if any item falls back, the whole union falls back.
            return fallback_sentinel(py);
        }
        erased_items.push(erased);
    }
    let erased_list = PyList::new(py, &erased_items);
    let typeops = py.import("mypy.typeops")?;
    let make_simplified = typeops.getattr("make_simplified_union")?;
    let result = make_simplified.call1((erased_list,))?;
    Ok(result.into())
}

/// Native `erase_type(typ) -> ProperType | None`.
///
/// Returns `None` when the Rust path does not handle `typ` or one of its
/// sub-components; the Python caller must then fall back to the pure-Python
/// `EraseTypeVisitor`. This is the strangler-fig per-call gate.
#[pyfunction]
fn erase_type(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<PyObject> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return fallback_sentinel(py),
    };
    erase_one(py, typ, &refs)
}

#[pymodule]
fn type_kernel(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(erase_type, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: import mypy.types fixtures and call erase_type on a constructed
    /// type, returning the `str()` of the result for comparison.
    fn erase_to_str(py: Python<'_>, type_expr: &str) -> String {
        let locals = PyDict::new(py);
        let setup = format!(
            r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.types import AnyType, TypeOfAny
fx = TypeFixture(COVARIANT)
{type_expr}
"#,
            type_expr = type_expr,
        );
        py.run(&setup, None, Some(locals)).unwrap();
        let typ = locals.get_item("typ").unwrap().unwrap();
        let result = super::erase_type(py, typ).unwrap();
        if result.is_none(py) {
            return "__fallback__".to_string();
        }
        // The result is a Type object; call Python str() on it for comparison.
        let builtins = py.import("builtins").unwrap();
        let result_str = builtins
            .getattr("str")
            .unwrap()
            .call1((&result,))
            .unwrap()
            .downcast::<PyString>()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        result_str
    }

    #[test]
    fn erase_any_is_identity() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // typ = fx.anyt
            let result = erase_to_str(py, "typ = fx.anyt");
            assert_eq!(result, "Any");
        });
    }

    #[test]
    fn erase_type_var_becomes_any() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // typ = fx.t  (a TypeVarType)
            let result = erase_to_str(py, "typ = fx.t");
            assert_eq!(result, "Any");
        });
    }

    #[test]
    fn erase_none_is_identity() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let result = erase_to_str(py, "typ = fx.nonet");
            assert_eq!(result, "None");
        });
    }

    #[test]
    fn erase_instance_reads_live_typeinfo() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // typ = fx.ga  (Instance with one TypeVar arg)
            // After erase: Instance(fx.gi, [Any])  ->  "G[Any]"
            // Compare against the Python erase_type output for parity.
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.erasetype import erase_type as py_erase
fx = TypeFixture(COVARIANT)
typ = fx.ga
expected = str(py_erase(typ))
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let expected: String = locals
                .get_item("expected")
                .unwrap()
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let typ = locals.get_item("typ").unwrap().unwrap();
            let result = super::erase_type(py, typ).unwrap();
            let builtins = py.import("builtins").unwrap();
            let result_str: String = builtins
                .getattr("str")
                .unwrap()
                .call1((&result,))
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            assert_eq!(result_str, expected);
        });
    }
}
