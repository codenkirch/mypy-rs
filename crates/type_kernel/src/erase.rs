//! Stage 1: `erase_type` — mirrors `mypy.erasetype.EraseTypeVisitor`.
//!
//! Walks a live Python `mypy.types.Type` object and produces the erased
//! `ProperType`. Returns `None` for any type class Rust does not handle, so
//! the Python caller falls back to the pure-Python visitor (the
//! strangler-fig per-call gate).
//!
//! Why `erase_type` is the right first operation:
//!   * Pure visitor (`Type -> ProperType`), no plugin hooks, no input mutation.
//!   * The only `TypeInfo` dependency is `defn.type_vars` (count + kinds),
//!     read directly from the live object.
//!   * Well-tested in `mypy/test/testtypes.py` with a string-equality parity
//!     contract (`str(erase_type(t)) == str(expected)`).

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::refs::{fallback_sentinel, is_fallback, is_instance, make_any, TypeRefs};

/// Erase a single `Type` object. Returns the erased `ProperType`, or `None`
/// (the fallback sentinel) if Rust does not handle this case.
fn erase_one(py: Python<'_>, obj: &PyAny, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    // Class dispatch is by `isinstance` against the resolved class objects,
    // not by string compare, so plugin subclasses are handled correctly.
    //
    // Order mirrors the Python EraseTypeVisitor: leaf types first, then
    // the composite types that recurse.

    // --- Trivial leaves (return as-is) ---
    // ErasedType, AnyType, NoneType, UninhabitedType, DeletedType, LiteralType
    // all return `t` unchanged.
    if is_instance(obj, refs.any_type)
        || is_instance(obj, refs.none_type)
        || is_instance(obj, refs.uninhabited_type)
        || is_instance(obj, refs.deleted_type)
        || is_instance(obj, refs.literal_type)
    {
        return Ok(obj.into());
    }
    // ErasedType — Python: `return t` (identity; usually caught by
    // `get_proper_type` before a visitor sees it, but handled for safety).
    if let Ok(py_types) = py.import("mypy.types") {
        if let Ok(erased_type_obj) = py_types.getattr("ErasedType") {
            if let Ok(erased_type_cls) = erased_type_obj.downcast::<pyo3::types::PyType>() {
                if is_instance(obj, erased_type_cls) {
                    return Ok(obj.into());
                }
            }
        }
    }

    // --- TypeVar-like -> AnyType(special_form) ---
    if is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.unpack_type)
    {
        return make_any(py, refs);
    }

    // --- TypeVarTupleType -> t.tuple_fallback.copy_modified(args=[Any]) ---
    // Python: return t.tuple_fallback.copy_modified(args=[AnyType(TypeOfAny.special_form)])
    if is_instance(obj, refs.type_var_tuple_type) {
        return erase_type_var_tuple(py, obj, refs);
    }

    // --- Instance ---
    // Python visitor:
    //   args = erased_vars(t.type.defn.type_vars, TypeOfAny.special_form)
    //   return Instance(t.type, args, t.line)
    if is_instance(obj, refs.instance) {
        return erase_instance(py, obj, refs);
    }

    // --- CallableType ---
    // Python visitor: replace arg_types/arg_kinds/arg_names with the
    // `Callable[..., Any]` shape, preserve fallback.
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
    if is_instance(obj, refs.union_type) {
        return erase_union(py, obj, refs);
    }

    // --- Anything else ---
    // UnboundType, PartialType, PlaceholderType, Parameters, TypeGuardedType,
    // RawExpressionType, CallableArgument, TypeList, EllipsisType,
    // TypeAliasType (raises in Python visitor), etc. — either should not
    // leak past `get_proper_type` or the visitor raises. For safety we
    // fall back to Python.
    fallback_sentinel(py)
}

/// Erase a `TypeVarTupleType`: return
/// `t.tuple_fallback.copy_modified(args=[AnyType(TypeOfAny.special_form)])`.
fn erase_type_var_tuple(py: Python<'_>, obj: &PyAny, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let tuple_fallback = obj.getattr("tuple_fallback")?;
    let copy_modified = tuple_fallback.getattr("copy_modified")?;
    let any_type = make_any(py, refs)?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("args", vec![&any_type])?;
    let result = copy_modified.call((), Some(kwargs))?;
    Ok(result.into())
}

/// Erase an `Instance`: read `t.type.defn.type_vars` from the live TypeInfo
/// (same as the Python visitor), build `AnyType`/`UnpackType` erased args
/// mirroring `erased_vars`, construct a new `Instance(t.type, args, t.line)`.
fn erase_instance(py: Python<'_>, obj: &PyAny, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let typ = obj.getattr("type")?;
    let line = obj.getattr("line")?;

    // Read defn.type_vars directly from the live TypeInfo.
    let defn = match typ.getattr("defn") {
        Ok(d) => d,
        Err(_) => return fallback_sentinel(py),
    };
    let type_vars = match defn.getattr("type_vars") {
        Ok(tv) => match tv.downcast::<PyList>() {
            Ok(list) => list,
            Err(_) => return fallback_sentinel(py),
        },
        Err(_) => return fallback_sentinel(py),
    };

    let any_type = make_any(py, refs)?;
    let mut erased_args: Vec<PyObject> = Vec::with_capacity(type_vars.len());
    for tv in type_vars.iter() {
        if is_instance(tv, refs.type_var_tuple_type) {
            // Valid erasure for *Ts is *tuple[Any, ...].
            let tuple_fallback = tv.getattr("tuple_fallback")?;
            let copy_modified = tuple_fallback.getattr("copy_modified")?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("args", vec![&any_type])?;
            let erased_fallback = copy_modified.call((), Some(kwargs))?;
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
fn erase_callable(py: Python<'_>, obj: &PyAny, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let any_type = make_any(py, refs)?;
    let fallback = obj.getattr("fallback")?;

    let nodes_mod = py.import("mypy.nodes")?;
    let arg_star = nodes_mod.getattr("ARG_STAR")?;
    let arg_star2 = nodes_mod.getattr("ARG_STAR2")?;

    let arg_types = PyList::new(py, [&any_type, &any_type]);
    let arg_kinds = PyList::new(py, [arg_star, arg_star2]);
    let arg_names = PyList::new(py, [py.None(), py.None()]);

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
/// `mypy.typeops.make_simplified_union`.
fn erase_union(py: Python<'_>, obj: &PyAny, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;
    let mut erased_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let erased = erase_one(py, item, refs)?;
        if is_fallback(&erased, py) {
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
/// `EraseTypeVisitor`.
#[pyfunction]
pub(crate) fn erase_type(py: Python<'_>, typ: &PyAny) -> PyResult<PyObject> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return fallback_sentinel(py),
    };
    erase_one(py, typ, &refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyString;

    /// Helper: call erase_type on a constructed type, returning `str()` for comparison.
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

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn erase_any_is_identity() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let result = erase_to_str(py, "typ = fx.anyt");
            assert_eq!(result, "Any");
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn erase_type_var_becomes_any() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let result = erase_to_str(py, "typ = fx.t");
            assert_eq!(result, "Any");
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn erase_none_is_identity() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let result = erase_to_str(py, "typ = fx.nonet");
            assert_eq!(result, "None");
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn erase_instance_reads_live_typeinfo() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
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
