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
use pyo3::types::{PyDict, PyList, PyString, PyTuple, PyType};

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
    /// mypy.types.TypeAliasType
    type_alias_type: &'py PyType,
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
            type_alias_type: class!("TypeAliasType"),
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

// ---------------------------------------------------------------------------
// Stage 2: remove_instance_last_known_values (LastKnownValueEraser)
// ---------------------------------------------------------------------------

/// Translate a single `Type` through the `LastKnownValueEraser` logic.
///
/// Mirrors `mypy.erasetype.LastKnownValueEraser`, which is a `TypeTranslator`
/// overriding `visit_instance`, `visit_type_alias_type`, and `visit_union_type`.
/// All other types use the `TypeTranslator` defaults (recursive translation
/// of children, identity on leaves).
///
/// Returns `None` (the fallback sentinel) for cases Rust does not handle —
/// the Python caller falls back to the pure-Python visitor.
fn lkv_translate_one(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    // --- Leaf types: TypeTranslator defaults are identity ---
    // AnyType, NoneType, UninhabitedType, ErasedType, DeletedType,
    // TypeVarType, ParamSpecType, TypeVarTupleType, PartialType, UnboundType
    // — all return `t` unchanged.
    if is_instance(obj, refs.any_type)
        || is_instance(obj, refs.none_type)
        || is_instance(obj, refs.uninhabited_type)
        || is_instance(obj, refs.deleted_type)
        || is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.type_var_tuple_type)
        || is_instance(obj, refs.literal_type)
    {
        return Ok(obj.into());
    }

    // --- TypeAliasType: return as-is (no recursion into alias target) ---
    if is_instance(obj, refs.type_alias_type) {
        return Ok(obj.into());
    }

    // --- Instance: the core override ---
    // Python:
    //   if not t.last_known_value and not t.args:
    //       return t
    //   return t.copy_modified(args=[a.accept(self) for a in t.args],
    //                          last_known_value=None)
    if is_instance(obj, refs.instance) {
        return lkv_visit_instance(py, obj, refs);
    }

    // --- UnionType: the dedup override ---
    // Python calls super().visit_union_type (translate all items), then
    // merges Instance items with the same fullname via make_simplified_union.
    if is_instance(obj, refs.union_type) {
        return lkv_visit_union(py, obj, refs);
    }

    // --- CallableType: TypeTranslator default ---
    // copy_modified(arg_types=translated, ret_type=translated)
    if is_instance(obj, refs.callable_type) {
        return lkv_visit_callable(py, obj, refs);
    }

    // --- TupleType: TypeTranslator default ---
    // TupleType(translated items, translated partial_fallback, line, column)
    if is_instance(obj, refs.tuple_type) {
        return lkv_visit_tuple(py, obj, refs);
    }

    // --- Overloaded: TypeTranslator default ---
    // Overloaded(items=[translated items])
    if is_instance(obj, refs.overloaded) {
        return lkv_visit_overloaded(py, obj, refs);
    }

    // --- TypeType: TypeTranslator default ---
    // TypeType.make_normalized(translated item, line, column, is_type_form)
    if is_instance(obj, refs.type_type) {
        return lkv_visit_type_type(py, obj, refs);
    }

    // --- LiteralType: TypeTranslator default ---
    // LiteralType(value, translated fallback, line, column)
    if is_instance(obj, refs.literal_type) {
        return lkv_visit_literal(py, obj, refs);
    }

    // --- UnpackType: TypeTranslator default ---
    // UnpackType(t.type.accept(self))
    if is_instance(obj, refs.unpack_type) {
        return lkv_visit_unpack(py, obj, refs);
    }

    // --- TypedDictType, Parameters, and anything else: fall back ---
    // TypedDictType has complex dict construction + caching; Parameters is
    // rare in this context. Fall back to Python for correctness.
    fallback_sentinel(py)
}

/// LastKnownValueEraser.visit_instance: strip last_known_value, recurse args.
fn lkv_visit_instance(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let has_lkv = !obj.getattr("last_known_value")?.is_none();
    let args = obj.getattr("args")?;

    // args is typed as tuple[Type, ...] but may arrive as a list in some
    // code paths; collect into a Vec to handle both uniformly. Treat
    // anything else as a fallback.
    let args_vec: Vec<&PyAny> = if let Ok(list) = args.downcast::<PyList>() {
        list.iter().collect()
    } else if let Ok(tuple) = args.downcast::<PyTuple>() {
        tuple.iter().collect()
    } else {
        return fallback_sentinel(py);
    };

    if !has_lkv && args_vec.is_empty() {
        return Ok(obj.into());
    }

    // Recurse on each arg. If any falls back, the whole instance falls back.
    // The Python visitor uses copy_modified(args=[a.accept(self) for a in t.args])
    // — it passes a list regardless of the input tuple/list shape.
    let mut translated_args: Vec<PyObject> = Vec::with_capacity(args_vec.len());
    for arg in &args_vec {
        let translated = lkv_translate_one(py, arg, refs)?;
        if is_fallback(&translated, py) {
            return fallback_sentinel(py);
        }
        translated_args.push(translated);
    }

    let kwargs = PyDict::new(py);
    kwargs.set_item("args", PyList::new(py, &translated_args))?;
    kwargs.set_item("last_known_value", py.None())?;
    let copy_modified = obj.getattr("copy_modified")?;
    let result = copy_modified.call((), Some(kwargs))?;
    Ok(result.into())
}

/// LastKnownValueEraser.visit_union_type: translate items, then dedup
/// Instance items with the same fullname via make_simplified_union.
fn lkv_visit_union(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;

    // Translate all items (TypeTranslator.visit_union_type default).
    let mut translated: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated.push(t);
    }

    // Python logic:
    //   instances = [item for item in new.items if isinstance(get_proper_type(item), Instance)]
    //   if len(instances) > 1:
    //       ... group by fullname, merge groups >1 via make_simplified_union ...
    //   return new  (if <=1 instance)
    //
    // We replicate the dedup: collect proper-type Instance items with no args,
    // group by fullname, and call make_simplified_union for groups with >1.

    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;

    // First pass: resolve proper types and check if dedup is needed.
    let proper: Vec<PyObject> = translated
        .iter()
        .map(|t| get_proper_type.call1((t,)).map(|r| r.into()))
        .collect::<PyResult<Vec<_>>>()?;

    // Count Instance items (with no args) per fullname.
    let mut groups_by_name: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    let mut fullnames: Vec<Option<String>> = Vec::with_capacity(proper.len());
    for p in &proper {
        let is_inst = is_instance(p.as_ref(py), refs.instance);
        if is_inst {
            // args is typed as tuple[Type, ...] but may be a list; accept both.
            let args_obj = p.as_ref(py).getattr("args")?;
            let no_args = if let Ok(list) = args_obj.downcast::<PyList>() {
                list.is_empty()
            } else if let Ok(tuple) = args_obj.downcast::<PyTuple>() {
                tuple.is_empty()
            } else {
                // Unexpected args type — be conservative, skip grouping.
                false
            };
            if no_args {
                let fullname_obj = p
                    .as_ref(py)
                    .getattr("type")?
                    .getattr("fullname")?;
                let fullname: String = fullname_obj
                    .downcast::<PyString>()?
                    .to_str()?
                    .to_string();
                fullnames.push(Some(fullname.clone()));
                let idx = fullnames.len() - 1;
                groups_by_name
                    .entry(fullname)
                    .or_insert_with(Vec::new)
                    .push(idx);
                continue;
            }
        }
        fullnames.push(None);
    }

    // If no group has >1 member, no dedup needed — construct the union.
    let needs_dedup = groups_by_name.values().any(|v| v.len() > 1);
    if !needs_dedup {
        let translated_list = PyList::new(py, &translated);
        let result = types_mod
            .getattr("UnionType")?
            .getattr("make_union")?
            .call1((translated_list,))?;
        return Ok(result.into());
    }

    // Dedup: build merged list, calling make_simplified_union for groups.
    let typeops = py.import("mypy.typeops")?;
    let make_simplified = typeops.getattr("make_simplified_union")?;

    // Track which indices have been consumed by a merge.
    let mut consumed: Vec<bool> = vec![false; translated.len()];
    let mut merged: Vec<PyObject> = Vec::with_capacity(translated.len());

    for (i, p) in proper.iter().enumerate() {
        if consumed[i] {
            continue;
        }
        match &fullnames[i] {
            None => {
                // Not an Instance with no args — keep original translated item.
                merged.push(translated[i].clone_ref(py));
            }
            Some(name) => {
                let group = groups_by_name.get(name).unwrap();
                if group.len() <= 1 {
                    // Single instance — keep as-is (use proper type).
                    merged.push(p.clone_ref(py));
                } else {
                    // Merge the group via make_simplified_union.
                    let group_items: Vec<PyObject> = group
                        .iter()
                        .map(|&idx| {
                            consumed[idx] = true;
                            proper[idx].clone_ref(py)
                        })
                        .collect();
                    let group_list = PyList::new(py, &group_items);
                    let simplified = make_simplified.call1((group_list,))?;
                    merged.push(simplified.into());
                }
            }
        }
    }

    let merged_list = PyList::new(py, &merged);
    let result = types_mod
        .getattr("UnionType")?
        .getattr("make_union")?
        .call1((merged_list,))?;
    Ok(result.into())
}

/// TypeTranslator.visit_callable_type default: copy_modified with translated
/// arg_types and ret_type. variables are passed through unchanged
/// (translate_variables returns them as-is).
fn lkv_visit_callable(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let arg_types = obj.getattr("arg_types")?.downcast::<PyList>()?;
    let mut translated_args: Vec<PyObject> = Vec::with_capacity(arg_types.len());
    for arg in arg_types.iter() {
        let t = lkv_translate_one(py, arg, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_args.push(t);
    }

    let ret_type = obj.getattr("ret_type")?;
    let translated_ret = lkv_translate_one(py, ret_type, refs)?;
    if is_fallback(&translated_ret, py) {
        return fallback_sentinel(py);
    }

    // instance_type: translate if present (TypeTranslator default).
    let instance_type = obj.getattr("instance_type")?;
    let translated_instance_type: Option<PyObject> = if !instance_type.is_none() {
        let t = lkv_translate_one(py, instance_type, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        Some(t)
    } else {
        None
    };

    let kwargs = PyDict::new(py);
    kwargs.set_item("arg_types", PyList::new(py, &translated_args))?;
    kwargs.set_item("ret_type", &translated_ret)?;
    if let Some(it) = &translated_instance_type {
        kwargs.set_item("instance_type", it)?;
    }
    let copy_modified = obj.getattr("copy_modified")?;
    let result = copy_modified.call((), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_tuple_type default: TupleType(translated items,
/// translated partial_fallback, line, column).
fn lkv_visit_tuple(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;
    let mut translated_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_items.push(t);
    }

    let partial_fallback = obj.getattr("partial_fallback")?;
    let translated_fallback = lkv_translate_one(py, partial_fallback, refs)?;
    if is_fallback(&translated_fallback, py) {
        return fallback_sentinel(py);
    }

    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let items_list = PyList::new(py, &translated_items);
    let result = refs.tuple_type.call1((items_list, translated_fallback, line, column))?;
    Ok(result.into())
}

/// TypeTranslator.visit_overloaded default: Overloaded(items=[translated items]).
fn lkv_visit_overloaded(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;
    let mut translated_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_items.push(t);
    }
    let items_list = PyList::new(py, &translated_items);
    let kwargs = PyDict::new(py);
    kwargs.set_item("items", items_list)?;
    let result = refs.overloaded.call((), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_type_type default: TypeType.make_normalized(
/// translated item, line, column, is_type_form).
fn lkv_visit_type_type(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let item = obj.getattr("item")?;
    let translated = lkv_translate_one(py, item, refs)?;
    if is_fallback(&translated, py) {
        return fallback_sentinel(py);
    }
    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let is_type_form = obj.getattr("is_type_form")?;
    let make_normalized = refs.type_type.getattr("make_normalized")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("line", line)?;
    kwargs.set_item("column", column)?;
    kwargs.set_item("is_type_form", is_type_form)?;
    let result = make_normalized.call((translated,), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_literal_type default: LiteralType(value,
/// translated fallback, line, column).
fn lkv_visit_literal(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let value = obj.getattr("value")?;
    let fallback = obj.getattr("fallback")?;
    let translated_fallback = lkv_translate_one(py, fallback, refs)?;
    if is_fallback(&translated_fallback, py) {
        return fallback_sentinel(py);
    }
    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let result = refs.literal_type.call1((value, translated_fallback, line, column))?;
    Ok(result.into())
}

/// TypeTranslator.visit_unpack_type default: UnpackType(t.type.accept(self)).
fn lkv_visit_unpack(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let typ = obj.getattr("type")?;
    let translated = lkv_translate_one(py, typ, refs)?;
    if is_fallback(&translated, py) {
        return fallback_sentinel(py);
    }
    let result = refs.unpack_type.call1((translated,))?;
    Ok(result.into())
}

/// Native `remove_instance_last_known_values(typ) -> Type | None`.
///
/// Returns `None` when the Rust path does not handle `typ` or one of its
/// sub-components; the Python caller falls back to the pure-Python
/// `LastKnownValueEraser`. Stage 2 of the type-kernel migration.
#[pyfunction]
fn remove_instance_last_known_values(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<PyObject> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return fallback_sentinel(py),
    };
    lkv_translate_one(py, typ, &refs)
}

#[pymodule]
fn type_kernel(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(erase_type, module)?)?;
    module.add_function(wrap_pyfunction!(remove_instance_last_known_values, module)?)?;
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

    /// Stage 2 parity: `remove_instance_last_known_values` on an Instance with
    /// a `last_known_value` strips it, matching the Python visitor. Compares
    /// Rust output against `mypy.erasetype.remove_instance_last_known_values`.
    #[test]
    fn lkv_strips_last_known_value_from_instance() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.erasetype import remove_instance_last_known_values as py_lkv
fx = TypeFixture(COVARIANT)
# fx.lit1_inst is an Instance with a last_known_value (Literal[1]).
typ = fx.lit1_inst
expected = str(py_lkv(typ))
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
            let result = super::remove_instance_last_known_values(py, typ).unwrap();
            assert!(!result.is_none(py), "Rust path should not fall back here");
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

    /// Stage 2 parity: union dedup — `make_union([lit1_inst, lit2_inst, lit4_inst])`
    /// collapses to a single Instance after LKV erasure, matching the Python path.
    #[test]
    fn lkv_merges_union_of_same_fullname_instances() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.types import UnionType
from mypy.erasetype import remove_instance_last_known_values as py_lkv
fx = TypeFixture(COVARIANT)
typ = UnionType.make_union([fx.lit1_inst, fx.lit2_inst, fx.lit4_inst])
expected = str(py_lkv(typ))
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
            let result = super::remove_instance_last_known_values(py, typ).unwrap();
            assert!(!result.is_none(py), "Rust path should not fall back here");
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
