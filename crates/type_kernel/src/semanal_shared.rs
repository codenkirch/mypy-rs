//! Native port of pure helper functions from `mypy/semanal_shared.py` and
//! `mypy/sharedparse.py`.
//!
//! These are pure helpers used during semantic analysis and parsing. Each
//! mirrors the Python implementation and operates on live Python objects via
//! PyO3, following the same strangler-fig pattern: Rust handles the common
//! case and returns `None` / `false` for anything it cannot handle, so
//! Python falls back gracefully.
//!
//! Ported functions:
//! - `special_function_elide_names` / `argument_elide_name` — pure magic-method
//!   set membership checks from `mypy/sharedparse.py`.
//! - `set_callable_name` — resolve the display name of a callable
//!   (semanal_shared.py:266-279).
//! - `calculate_tuple_fallback` — compute the union fallback for a
//!   `TupleType` (semanal_shared.py:282-316).
//! - `has_placeholder` — check whether a type tree contains a
//!   `PlaceholderType` (semanal_shared.py:379).
//! - `find_dataclass_transform_spec` — unwrap decorators/calls to reach a
//!   `DataclassTransformSpec` (semanal_shared.py:384-454).

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple, PyType};

// ---------------------------------------------------------------------------
// Magic-method constant sets (from mypy/sharedparse.py)
// ---------------------------------------------------------------------------

/// The set of non-binary magic method names (sharedparse.py:8-41).
const NON_BINARY_MAGIC_METHODS: &[&str] = &[
    "__abs__",
    "__call__",
    "__complex__",
    "__contains__",
    "__buffer__",
    "__del__",
    "__delattr__",
    "__delitem__",
    "__enter__",
    "__exit__",
    "__float__",
    "__getattr__",
    "__getattribute__",
    "__getitem__",
    "__hex__",
    "__init__",
    "__init_subclass__",
    "__int__",
    "__invert__",
    "__iter__",
    "__len__",
    "__long__",
    "__neg__",
    "__new__",
    "__oct__",
    "__pos__",
    "__release_buffer__",
    "__repr__",
    "__reversed__",
    "__setattr__",
    "__setitem__",
    "__str__",
];

/// Magic methods that allow keyword arguments (sharedparse.py:43-49).
const MAGIC_METHODS_ALLOWING_KWARGS: &[&str] = &[
    "__init__",
    "__init_subclass__",
    "__new__",
    "__call__",
    "__setattr__",
];

/// Binary magic method names (sharedparse.py:51-100).
const BINARY_MAGIC_METHODS: &[&str] = &[
    "__add__",
    "__and__",
    "__divmod__",
    "__eq__",
    "__floordiv__",
    "__ge__",
    "__gt__",
    "__iadd__",
    "__iand__",
    "__idiv__",
    "__ifloordiv__",
    "__ilshift__",
    "__imatmul__",
    "__imod__",
    "__imul__",
    "__ior__",
    "__ipow__",
    "__irshift__",
    "__isub__",
    "__itruediv__",
    "__ixor__",
    "__le__",
    "__lshift__",
    "__lt__",
    "__matmul__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__or__",
    "__pow__",
    "__radd__",
    "__rand__",
    "__rdiv__",
    "__rfloordiv__",
    "__rlshift__",
    "__rmatmul__",
    "__rmod__",
    "__rmul__",
    "__ror__",
    "__rpow__",
    "__rrshift__",
    "__rshift__",
    "__rsub__",
    "__rtruediv__",
    "__rxor__",
    "__sub__",
    "__truediv__",
    "__xor__",
];

/// `TPDICT_FB_NAMES` from mypy/types.py:126-130. Used by `set_callable_name`
/// to detect internal `_TypedDict` names and replace them with "TypedDict".
const TPDICT_FB_NAMES: &[&str] = &[
    "typing._TypedDict",
    "typing_extensions._TypedDict",
    "mypy_extensions._TypedDict",
];

/// Build the `MAGIC_METHODS_POS_ARGS_ONLY` set at runtime: the union of
/// `NON_BINARY_MAGIC_METHODS` and `BINARY_MAGIC_METHODS` minus
/// `MAGIC_METHODS_ALLOWING_KWARGS` (sharedparse.py:106).
fn magic_methods_pos_args_only() -> HashSet<&'static str> {
    let mut set: HashSet<&str> = HashSet::new();
    for &n in NON_BINARY_MAGIC_METHODS {
        set.insert(n);
    }
    for &n in BINARY_MAGIC_METHODS {
        set.insert(n);
    }
    for &n in MAGIC_METHODS_ALLOWING_KWARGS {
        set.remove(n);
    }
    set
}

// ---------------------------------------------------------------------------
// special_function_elide_names
// ---------------------------------------------------------------------------

/// `mypy.sharedparse.special_function_elide_names` — is `name` a magic
/// method that must be marked pos-only?
///
/// Mirrors sharedparse.py:109-110. Pure set membership check against
/// `MAGIC_METHODS_POS_ARGS_ONLY`. No Python objects involved.
#[pyfunction]
pub(crate) fn rust_special_function_elide_names(name: &str) -> bool {
    special_function_elide_names_inner(name)
}

/// Pure-logic core of `rust_special_function_elide_names`, testable
/// without a Python interpreter.
fn special_function_elide_names_inner(name: &str) -> bool {
    let set = magic_methods_pos_args_only();
    set.contains(name)
}

// ---------------------------------------------------------------------------
// argument_elide_name
// ---------------------------------------------------------------------------

/// `mypy.sharedparse.argument_elide_name` — should `name` be elided?
///
/// Mirrors sharedparse.py:113-114. Returns `true` when `name` is not
/// `None`, starts with `"__"`, and does not end with `"__"`.
#[pyfunction]
pub(crate) fn rust_argument_elide_name(name: Option<&str>) -> bool {
    argument_elide_name_inner(name)
}

/// Pure-logic core of `rust_argument_elide_name`.
fn argument_elide_name_inner(name: Option<&str>) -> bool {
    match name {
        Some(s) => s.starts_with("__") && !s.ends_with("__"),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// set_callable_name
// ---------------------------------------------------------------------------

/// `mypy.semanal_shared.set_callable_name` — resolve the display name of a
/// callable type.
///
/// Mirrors semanal_shared.py:266-279. Resolves `sig` via
/// `get_proper_type`; if the result is a `FunctionLike` and `fdef.info`
/// is truthy, uses `fdef.info.fullname` (replaced with "TypedDict" if it's
/// an internal `_TypedDict` name) or `fdef.info.name` to build
/// `"{fdef.name} of {class_name}"`. Otherwise uses just `fdef.name`. For
/// non-`FunctionLike` types, returns the proper type unchanged.
///
/// The class-context test mirrors Python's `if fdef.info:` truthiness, not
/// an `is None` check: non-method `FuncDef`s carry a `FakeInfo` placeholder
/// (`FUNC_NO_INFO`) whose `__getattribute__` raises `AssertionError`, and
/// `TypeInfo.__bool__` returns `False` for it. Testing `is None` alone
/// deferred every such call to the pure-Python body.
///
/// Returns `None` when any attribute access fails (conservative fallback:
/// Python would never raise here on a well-formed input).
#[pyfunction]
pub(crate) fn rust_set_callable_name(
    py: Python<'_>,
    sig: &PyAny,
    fdef: &PyAny,
) -> PyResult<Option<PyObject>> {
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let function_like_cls: &PyType = types_mod.getattr("FunctionLike")?.downcast()?;

    // Resolve to proper type first.
    let proper = match get_proper_type.call1((sig,)) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !proper.is_instance(function_like_cls)? {
        // Not a FunctionLike: return the proper type unchanged.
        return Ok(Some(proper.into_py(py)));
    }

    let info = match fdef.getattr("info") {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    // Mirror Python's `if fdef.info:` truthiness: `TypeInfo.__bool__`
    // returns False for the FakeInfo placeholder (FUNC_NO_INFO), and None
    // (no class context) is falsy too.
    let has_info = match info.is_true() {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    if !has_info {
        // No class context: sig.with_name(fdef.name).
        let fdef_name = match fdef.getattr("name") {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        let result = match proper.call_method1("with_name", (fdef_name,)) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        return Ok(Some(result.into_py(py)));
    }

    // fdef.info is a TypeInfo; check fullname against TPDICT_FB_NAMES.
    let fullname = match info.getattr("fullname") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let fullname_str: &str = match fullname.downcast::<PyString>() {
        Ok(s) => match s.to_str() {
            Ok(st) => st,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let class_name: String = if TPDICT_FB_NAMES.contains(&fullname_str) {
        "TypedDict".to_string()
    } else {
        // Use info.name.
        let info_name = match info.getattr("name") {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        match info_name.downcast::<PyString>() {
            Ok(s) => match s.to_str() {
                Ok(st) => st.to_string(),
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        }
    };

    let fdef_name = match fdef.getattr("name") {
        Ok(n) => n,
        Err(_) => return Ok(None),
    };
    let fdef_name_str = match fdef_name.str() {
        Ok(s) => match s.to_str() {
            Ok(st) => st.to_string(),
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let display_name = format!("{} of {}", fdef_name_str, class_name);
    let result = match proper.call_method1("with_name", (display_name,)) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    Ok(Some(result.into_py(py)))
}

// ---------------------------------------------------------------------------
// has_placeholder
// ---------------------------------------------------------------------------

/// `mypy.semanal_shared.has_placeholder` — does `typ` contain a
/// `PlaceholderType` recursively?
///
/// Mirrors `HasPlaceholders` (semanal_shared.py) on top of
/// `BoolTypeQuery` (type_visitor.py, ANY_STRATEGY with `default=False`):
/// `visit_placeholder_type` returns True and every other visit method
/// follows the BoolTypeQuery table. `TypeAliasType` gets the same
/// identity-based `seen_aliases` cycle detection as the Python visitor,
/// and new-style (PEP 695) aliases also query their type arguments even
/// when the alias expansion finds nothing (unused type variables).
///
/// Returns `Some(true)` when a placeholder is found, `Some(false)` when
/// the type tree is fully understood and contains no placeholder, and
/// `None` when Rust encounters a type class it cannot fully traverse
/// (so Python should run the `HasPlaceholders` visitor instead).
#[pyfunction]
pub(crate) fn rust_has_placeholder(py: Python<'_>, typ: &PyAny) -> PyResult<Option<bool>> {
    // A fresh visitor per call, matching `HasPlaceholders()` in
    // has_placeholder(); seen_aliases is not reset mid-walk.
    let mut seen_aliases: HashSet<*mut pyo3::ffi::PyObject> = HashSet::new();
    has_placeholder_walk(py, typ, 0, &mut seen_aliases)
}

/// `mypy.types.get_proper_type(typ)`; `Ok(None)` defers to Python when the
/// call raised (e.g. an unfixed alias asserting on `alias is None`).
fn get_proper_type_of<'a>(types_mod: &'a PyModule, typ: &'a PyAny) -> PyResult<Option<&'a PyAny>> {
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    match get_proper_type.call1((typ,)) {
        Ok(r) => Ok(Some(r)),
        Err(_) => Ok(None),
    }
}

/// ANY-strategy query over a `list`/`tuple` of types, mirroring
/// `BoolTypeQuery.query_types`. `Ok(None)` defers when the attribute is
/// neither a list nor a tuple.
fn query_type_items(
    py: Python<'_>,
    depth: u32,
    seen_aliases: &mut HashSet<*mut pyo3::ffi::PyObject>,
    items: &PyAny,
) -> PyResult<Option<bool>> {
    let vals: Option<Vec<&PyAny>> = if let Ok(l) = items.downcast::<PyList>() {
        Some(l.iter().collect())
    } else if let Ok(t) = items.downcast::<PyTuple>() {
        Some(t.iter().collect())
    } else {
        None
    };
    let vals = match vals {
        Some(v) => v,
        None => return Ok(None),
    };
    for v in vals {
        match has_placeholder_walk(py, v, depth + 1, seen_aliases)? {
            Some(true) => return Ok(Some(true)),
            None => return Ok(None),
            Some(false) => {}
        }
    }
    Ok(Some(false))
}

/// Recursive walk for `has_placeholder`. Returns `None` for unsupported
/// type shapes so Python's `HasPlaceholders` visitor handles them. The
/// identity set tracks visited `TypeAliasType` objects (cycle detection,
/// mirroring `seen_aliases`); a depth limit additionally guards against
/// non-alias self-reference, deferring to Python's visitor.
fn has_placeholder_walk(
    py: Python<'_>,
    typ: &PyAny,
    depth: u32,
    seen_aliases: &mut HashSet<*mut pyo3::ffi::PyObject>,
) -> PyResult<Option<bool>> {
    if depth > 64 {
        return Ok(None);
    }
    let types_mod = py.import("mypy.types")?;

    // TypeAliasType must be visited *before* get_proper_type(): expansion
    // discards the alias object, which the Python visitor still needs for
    // cycle detection and for querying t.args on new-style aliases.
    let alias_type_cls: &PyType = types_mod.getattr("TypeAliasType")?.downcast()?;
    if typ.is_instance(alias_type_cls)? {
        // Identity-based: TypeAliasType does not override __eq__/__hash__,
        // so Python's seen_aliases set also keys on object identity.
        let key = typ.as_ptr();
        if seen_aliases.contains(&key) {
            return Ok(Some(false));
        }
        seen_aliases.insert(key);
        let alias = typ.getattr("alias")?;
        if alias.is_none() {
            return Ok(None);
        }
        let proper = match get_proper_type_of(types_mod, typ)? {
            Some(p) => p,
            None => return Ok(None),
        };
        let res = match has_placeholder_walk(py, proper, depth + 1, seen_aliases)? {
            Some(r) => r,
            None => return Ok(None),
        };
        if res {
            return Ok(Some(true));
        }
        // visit_type_alias_type: res or (python_3_12_type_alias and
        // query_types(t.args)).
        let new_style = alias.getattr("python_3_12_type_alias")?.is_true()?;
        if new_style {
            return query_type_items(py, depth, seen_aliases, typ.getattr("args")?);
        }
        return Ok(Some(false));
    }

    let proper = match get_proper_type_of(types_mod, typ)? {
        Some(p) => p,
        None => return Ok(None),
    };

    let placeholder_cls: &PyType = types_mod.getattr("PlaceholderType")?.downcast()?;
    if proper.is_instance(placeholder_cls)? {
        return Ok(Some(true));
    }

    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;
    if proper.is_instance(instance_cls)? {
        return query_type_items(py, depth, seen_aliases, proper.getattr("args")?);
    }

    let union_cls: &PyType = types_mod.getattr("UnionType")?.downcast()?;
    if proper.is_instance(union_cls)? {
        return query_type_items(py, depth, seen_aliases, proper.getattr("items")?);
    }

    let tuple_cls: &PyType = types_mod.getattr("TupleType")?.downcast()?;
    if proper.is_instance(tuple_cls)? {
        // query_types([partial_fallback] + items).
        match has_placeholder_walk(
            py,
            proper.getattr("partial_fallback")?,
            depth + 1,
            seen_aliases,
        )? {
            Some(true) => return Ok(Some(true)),
            None => return Ok(None),
            Some(false) => {}
        }
        return query_type_items(py, depth, seen_aliases, proper.getattr("items")?);
    }

    let callable_cls: &PyType = types_mod.getattr("CallableType")?.downcast()?;
    if proper.is_instance(callable_cls)? {
        // query_types(arg_types) or ret_type or instance_type.
        match query_type_items(py, depth, seen_aliases, proper.getattr("arg_types")?)? {
            Some(true) => return Ok(Some(true)),
            None => return Ok(None),
            Some(false) => {}
        }
        match has_placeholder_walk(py, proper.getattr("ret_type")?, depth + 1, seen_aliases)? {
            Some(true) => return Ok(Some(true)),
            None => return Ok(None),
            Some(false) => {}
        }
        let instance_type = proper.getattr("instance_type")?;
        if !instance_type.is_none() {
            match has_placeholder_walk(py, instance_type, depth + 1, seen_aliases)? {
                Some(true) => return Ok(Some(true)),
                None => return Ok(None),
                Some(false) => {}
            }
        }
        return Ok(Some(false));
    }

    // BoolTypeQuery leaves all return self.default (False for ANY).
    for leaf in [
        "AnyType",
        "NoneType",
        "UninhabitedType",
        "ErasedType",
        "DeletedType",
        "PartialType",
        "RawExpressionType",
        "LiteralType",
        "EllipsisType",
    ] {
        let cls: &PyType = types_mod.getattr(leaf)?.downcast()?;
        if proper.is_instance(cls)? {
            return Ok(Some(false));
        }
    }

    let typevar_cls: &PyType = types_mod.getattr("TypeVarType")?.downcast()?;
    if proper.is_instance(typevar_cls)? {
        // query_types([upper_bound, default] + values). Crucial for
        // recursive generic aliases: the TypeVar's default may be the
        // alias itself, whose target is a PlaceholderType.
        for sub in [proper.getattr("upper_bound")?, proper.getattr("default")?] {
            match has_placeholder_walk(py, sub, depth + 1, seen_aliases)? {
                Some(true) => return Ok(Some(true)),
                None => return Ok(None),
                Some(false) => {}
            }
        }
        return query_type_items(py, depth, seen_aliases, proper.getattr("values")?);
    }

    let paramspec_cls: &PyType = types_mod.getattr("ParamSpecType")?.downcast()?;
    if proper.is_instance(paramspec_cls)? {
        // query_types([upper_bound, default, prefix]).
        for sub in [
            proper.getattr("upper_bound")?,
            proper.getattr("default")?,
            proper.getattr("prefix")?,
        ] {
            match has_placeholder_walk(py, sub, depth + 1, seen_aliases)? {
                Some(true) => return Ok(Some(true)),
                None => return Ok(None),
                Some(false) => {}
            }
        }
        return Ok(Some(false));
    }

    let tvtuple_cls: &PyType = types_mod.getattr("TypeVarTupleType")?.downcast()?;
    if proper.is_instance(tvtuple_cls)? {
        // query_types([upper_bound, default]).
        for sub in [proper.getattr("upper_bound")?, proper.getattr("default")?] {
            match has_placeholder_walk(py, sub, depth + 1, seen_aliases)? {
                Some(true) => return Ok(Some(true)),
                None => return Ok(None),
                Some(false) => {}
            }
        }
        return Ok(Some(false));
    }

    let unpack_cls: &PyType = types_mod.getattr("UnpackType")?.downcast()?;
    if proper.is_instance(unpack_cls)? {
        // query_types([t.type]).
        return has_placeholder_walk(py, proper.getattr("type")?, depth + 1, seen_aliases);
    }

    let parameters_cls: &PyType = types_mod.getattr("Parameters")?.downcast()?;
    if proper.is_instance(parameters_cls)? {
        return query_type_items(py, depth, seen_aliases, proper.getattr("arg_types")?);
    }

    let overloaded_cls: &PyType = types_mod.getattr("Overloaded")?.downcast()?;
    if proper.is_instance(overloaded_cls)? {
        return query_type_items(py, depth, seen_aliases, proper.getattr("items")?);
    }

    let typeddict_cls: &PyType = types_mod.getattr("TypedDictType")?.downcast()?;
    if proper.is_instance(typeddict_cls)? {
        let items = match proper.getattr("items")?.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        for value in items.values() {
            match has_placeholder_walk(py, value, depth + 1, seen_aliases)? {
                Some(true) => return Ok(Some(true)),
                None => return Ok(None),
                Some(false) => {}
            }
        }
        return Ok(Some(false));
    }

    let type_type_cls: &PyType = types_mod.getattr("TypeType")?.downcast()?;
    if proper.is_instance(type_type_cls)? {
        // t.item.accept(self).
        return has_placeholder_walk(py, proper.getattr("item")?, depth + 1, seen_aliases);
    }

    let unbound_cls: &PyType = types_mod.getattr("UnboundType")?.downcast()?;
    if proper.is_instance(unbound_cls)? {
        // query_types(t.args); entries may be PartialType (leaf False).
        return query_type_items(py, depth, seen_aliases, proper.getattr("args")?);
    }

    let typelist_cls: &PyType = types_mod.getattr("TypeList")?.downcast()?;
    if proper.is_instance(typelist_cls)? {
        return query_type_items(py, depth, seen_aliases, proper.getattr("items")?);
    }

    let callable_arg_cls: &PyType = types_mod.getattr("CallableArgument")?.downcast()?;
    if proper.is_instance(callable_arg_cls)? {
        // t.typ.accept(self).
        return has_placeholder_walk(py, proper.getattr("typ")?, depth + 1, seen_aliases);
    }

    // Any other type: defer to Python. Returning Some(false) would be
    // unsafe if the type contains a placeholder we don't know about.
    Ok(None)
}

// ---------------------------------------------------------------------------
// calculate_tuple_fallback
// ---------------------------------------------------------------------------

/// `mypy.semanal_shared.calculate_tuple_fallback` — compute the union of
/// tuple items and return it so Python can set `fallback.args = (result,)`.
///
/// Mirrors semanal_shared.py:282-316. Flattens nested tuples, unwraps
/// `UnpackType` (resolving `TypeVarTupleType.upper_bound` and
/// `builtins.tuple` args), builds the item list, and calls
/// `make_simplified_union(items)` to produce the union type.
///
/// Returns `Some(union_type)` (the computed union, to be assigned to
/// `fallback.args` by the Python caller) or `None` to defer to pure Python.
/// This function does NOT mutate `typ` — the caller is responsible for
/// `typ.partial_fallback.args = (result,)`.
#[pyfunction]
pub(crate) fn rust_calculate_tuple_fallback(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<Option<PyObject>> {
    let types_mod = py.import("mypy.types")?;
    let tuple_cls: &PyType = types_mod.getattr("TupleType")?.downcast()?;
    if !typ.is_instance(tuple_cls)? {
        return Ok(None);
    }

    let flatten_nested_tuples = match types_mod.getattr("flatten_nested_tuples") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let items_attr = match typ.getattr("items") {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    let flat = match flatten_nested_tuples.call1((items_attr,)) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let flat_list = match flat.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let unpack_cls: &PyType = types_mod.getattr("UnpackType")?.downcast()?;
    let typevar_tuple_cls: &PyType = types_mod.getattr("TypeVarTupleType")?.downcast()?;
    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;

    // AnyType(TypeOfAny.from_error) — construct via Python.
    let any_type_cls = match types_mod.getattr("AnyType") {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let type_of_any_from_error = match types_mod.getattr("TypeOfAny") {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    let from_error = match type_of_any_from_error.getattr("from_error") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };

    let mut collected: Vec<PyObject> = Vec::new();
    for item in flat_list.iter() {
        if item.is_instance(unpack_cls)? {
            // UnpackType: unwrap the inner type.
            let unpacked = item.getattr("type")?;
            let proper = match get_proper_type.call1((unpacked,)) {
                Ok(r) => r,
                Err(_) => return Ok(None),
            };
            // If TypeVarTupleType, resolve upper_bound.
            let resolved = if proper.is_instance(typevar_tuple_cls)? {
                let upper = proper.getattr("upper_bound")?;
                match get_proper_type.call1((upper,)) {
                    Ok(r) => r,
                    Err(_) => return Ok(None),
                }
            } else {
                proper
            };
            if resolved.is_instance(instance_cls)? {
                // Check type.fullname == "builtins.tuple".
                let type_obj = match resolved.getattr("type") {
                    Ok(t) => t,
                    Err(_) => return Ok(None),
                };
                let fullname = match type_obj.getattr("fullname") {
                    Ok(f) => f,
                    Err(_) => return Ok(None),
                };
                let fullname_str: &str = match fullname.downcast::<PyString>() {
                    Ok(s) => match s.to_str() {
                        Ok(st) => st,
                        Err(_) => return Ok(None),
                    },
                    Err(_) => return Ok(None),
                };
                if fullname_str == "builtins.tuple" {
                    let args = match resolved.getattr("args") {
                        Ok(a) => a,
                        Err(_) => return Ok(None),
                    };
                    let args_list = match args.downcast::<PyList>() {
                        Ok(l) => l,
                        Err(_) => return Ok(None),
                    };
                    if args_list.len() != 1 {
                        return Ok(None);
                    }
                    collected.push(args_list.get_item(0)?.into_py(py));
                } else {
                    // Not builtins.tuple: append AnyType(from_error).
                    let any_t = match any_type_cls.call1((from_error,)) {
                        Ok(r) => r,
                        Err(_) => return Ok(None),
                    };
                    collected.push(any_t.into_py(py));
                }
            } else {
                let any_t = match any_type_cls.call1((from_error,)) {
                    Ok(r) => r,
                    Err(_) => return Ok(None),
                };
                collected.push(any_t.into_py(py));
            }
        } else {
            collected.push(item.into_py(py));
        }
    }

    // make_simplified_union(items) — call the Python typeops function.
    let typeops_mod = match py.import("mypy.typeops") {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    let make_simplified_union = match typeops_mod.getattr("make_simplified_union") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let items_tuple = PyTuple::new(py, collected.iter());
    let result = match make_simplified_union.call1((items_tuple,)) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    Ok(Some(result.into_py(py)))
}

// ---------------------------------------------------------------------------
// find_dataclass_transform_spec
// ---------------------------------------------------------------------------

/// `mypy.semanal_shared.find_dataclass_transform_spec` — find the
/// `DataclassTransformSpec` for a node, if any.
///
/// Mirrors semanal_shared.py:384-454. Unwraps `CallExpr` → callee,
/// `RefExpr` → node, `Decorator` → func, `OverloadedFuncDef` → search items
/// + impl, then checks `FuncDef.dataclass_transform_spec` or searches the
/// `TypeInfo` MRO and metaclass.
///
/// Returns `Some(spec)` (the `DataclassTransformSpec` object) or `None`
/// (no spec found, or a node shape Rust cannot handle — Python should
/// fall back).
#[pyfunction]
pub(crate) fn rust_find_dataclass_transform_spec(
    py: Python<'_>,
    node: &PyAny,
) -> PyResult<Option<PyObject>> {
    if node.is_none() {
        return Ok(None);
    }
    find_dataclass_transform_spec_inner(py, node)
}

/// Inner recursive walk for `find_dataclass_transform_spec`.
fn find_dataclass_transform_spec_inner(py: Python<'_>, node: &PyAny) -> PyResult<Option<PyObject>> {
    let nodes_mod = py.import("mypy.nodes")?;

    // CallExpr → unwrap callee (semanal_shared.py:395-405).
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    let decorator_cls: &PyType = nodes_mod.getattr("Decorator")?.downcast()?;
    let overloaded_cls: &PyType = nodes_mod.getattr("OverloadedFuncDef")?.downcast()?;
    let func_def_cls: &PyType = nodes_mod.getattr("FuncDef")?.downcast()?;
    let class_def_cls: &PyType = nodes_mod.getattr("ClassDef")?.downcast()?;
    let type_info_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;

    let mut current: &PyAny = node;

    if current.is_instance(call_expr_cls)? {
        current = current.getattr("callee")?;
    }

    if current.is_instance(ref_expr_cls)? {
        let node_attr = current.getattr("node")?;
        if !node_attr.is_none() {
            current = node_attr;
        }
    }

    if current.is_instance(decorator_cls)? {
        current = current.getattr("func")?;
    }

    if current.is_instance(overloaded_cls)? {
        // Search all items + impl (semanal_shared.py:415-424).
        let items = current.getattr("items")?;
        let items_list = match items.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        for candidate in items_list.iter() {
            if let Some(spec) = find_dataclass_transform_spec_inner(py, candidate)? {
                return Ok(Some(spec));
            }
        }
        let impl_attr = current.getattr("impl")?;
        if !impl_attr.is_none() {
            return find_dataclass_transform_spec_inner(py, impl_attr);
        }
        return Ok(None);
    }

    if current.is_instance(func_def_cls)? {
        // FuncDef: return node.dataclass_transform_spec (semanal_shared.py:427-428).
        let spec = current.getattr("dataclass_transform_spec")?;
        if spec.is_none() {
            return Ok(None);
        }
        return Ok(Some(spec.into_py(py)));
    }

    if current.is_instance(class_def_cls)? {
        // ClassDef → unwrap to info (semanal_shared.py:430-431).
        let info = current.getattr("info")?;
        if info.is_none() {
            return Ok(None);
        }
        current = info;
    }

    if current.is_instance(type_info_cls)? {
        // Search MRO[1:] for dataclass_transform_spec
        // (semanal_shared.py:433-436).
        let mro = current.getattr("mro")?;
        let mro_list = match mro.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        if mro_list.len() <= 1 {
            // Only the type itself; check metaclass.
        } else {
            for base in mro_list.iter().skip(1) {
                let spec = base.getattr("dataclass_transform_spec")?;
                if !spec.is_none() {
                    return Ok(Some(spec.into_py(py)));
                }
            }
        }

        // Check metaclass (semanal_shared.py:438-452).
        let metaclass_type = current.getattr("metaclass_type")?;
        if !metaclass_type.is_none() {
            let type_obj = metaclass_type.getattr("type")?;
            let spec = type_obj.getattr("dataclass_transform_spec")?;
            if !spec.is_none() {
                return Ok(Some(spec.into_py(py)));
            }
        }
        return Ok(None);
    }

    // Unknown node shape: defer to Python.
    Ok(None)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- special_function_elide_names ---

    #[test]
    fn test_elide_pos_only_magic_methods() {
        // Methods in MAGIC_METHODS_POS_ARGS_ONLY should elide.
        assert!(special_function_elide_names_inner("__add__"));
        assert!(special_function_elide_names_inner("__eq__"));
        assert!(special_function_elide_names_inner("__contains__"));
        assert!(special_function_elide_names_inner("__repr__"));
        assert!(special_function_elide_names_inner("__getitem__"));
    }

    #[test]
    fn test_elide_rejects_kwargs_allowed_methods() {
        // Methods in MAGIC_METHODS_ALLOWING_KWARGS should NOT elide.
        assert!(!special_function_elide_names_inner("__init__"));
        assert!(!special_function_elide_names_inner("__init_subclass__"));
        assert!(!special_function_elide_names_inner("__new__"));
        assert!(!special_function_elide_names_inner("__call__"));
        assert!(!special_function_elide_names_inner("__setattr__"));
    }

    #[test]
    fn test_elide_rejects_non_magic_names() {
        assert!(!special_function_elide_names_inner("foo"));
        assert!(!special_function_elide_names_inner("__init_subclass__"));
        assert!(!special_function_elide_names_inner("regular_method"));
        assert!(!special_function_elide_names_inner("__dunder__"));
    }

    #[test]
    fn test_elide_all_binary_methods() {
        // All binary magic methods should elide (none are in ALLOWING_KWARGS).
        for &name in BINARY_MAGIC_METHODS {
            assert!(
                special_function_elide_names_inner(name),
                "{} should elide",
                name
            );
        }
    }

    #[test]
    fn test_elide_non_binary_except_kwargs() {
        // Non-binary methods except those allowing kwargs should elide.
        for &name in NON_BINARY_MAGIC_METHODS {
            if MAGIC_METHODS_ALLOWING_KWARGS.contains(&name) {
                assert!(
                    !special_function_elide_names_inner(name),
                    "{} should NOT elide (allows kwargs)",
                    name
                );
            } else {
                assert!(
                    special_function_elide_names_inner(name),
                    "{} should elide",
                    name
                );
            }
        }
    }

    // --- argument_elide_name ---

    #[test]
    fn test_argument_elide_dunder_prefix() {
        assert!(argument_elide_name_inner(Some("__init")));
        assert!(argument_elide_name_inner(Some("__private")));
        assert!(argument_elide_name_inner(Some("__x")));
    }

    #[test]
    fn test_argument_elide_rejects_dunder_suffix() {
        assert!(!argument_elide_name_inner(Some("__init__")));
        assert!(!argument_elide_name_inner(Some("__dunder__")));
        assert!(!argument_elide_name_inner(Some("____")));
    }

    #[test]
    fn test_argument_elide_rejects_no_dunder_prefix() {
        assert!(!argument_elide_name_inner(Some("init")));
        assert!(!argument_elide_name_inner(Some("_init")));
        assert!(!argument_elide_name_inner(Some("foo")));
    }

    #[test]
    fn test_argument_elide_none() {
        assert!(!argument_elide_name_inner(None));
    }

    #[test]
    fn test_argument_elide_empty_string() {
        // Empty string: starts_with("__") is false.
        assert!(!argument_elide_name_inner(Some("")));
    }

    #[test]
    fn test_argument_elide_single_char() {
        assert!(!argument_elide_name_inner(Some("_")));
        assert!(!argument_elide_name_inner(Some("__")));
        // "__" starts with "__" and ends with "__" → false.
    }

    // --- magic_methods_pos_args_only set ---

    #[test]
    fn test_pos_args_only_set_disjoint_from_kwargs() {
        let set = magic_methods_pos_args_only();
        for &name in MAGIC_METHODS_ALLOWING_KWARGS {
            assert!(
                !set.contains(name),
                "{} should not be in POS_ARGS_ONLY",
                name
            );
        }
    }

    #[test]
    fn test_pos_args_only_set_contains_all_magic() {
        let set = magic_methods_pos_args_only();
        // All magic methods except kwargs-allowed should be present.
        for &name in NON_BINARY_MAGIC_METHODS {
            if !MAGIC_METHODS_ALLOWING_KWARGS.contains(&name) {
                assert!(set.contains(name), "missing {}", name);
            }
        }
        for &name in BINARY_MAGIC_METHODS {
            if !MAGIC_METHODS_ALLOWING_KWARGS.contains(&name) {
                assert!(set.contains(name), "missing {}", name);
            }
        }
    }
}
