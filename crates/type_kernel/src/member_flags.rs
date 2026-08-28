//! `mypy.subtypes.get_member_flags` (subtypes.py:1773-1812) + its helper
//! `is_descriptor` (subtypes.py:1815-1821), ported to Rust.
//!
//! The port reads the member graph off the LIVE `TypeInfo` via PyO3
//! (`info.get_method(name)`, `info.get(name)`, `info.mro`) exactly like
//! `rust_class_callable` reads `info.defn.type_vars`. Nodes are not
//! wire-serializable, so the method / node identity and the redefinition
//! fallback semantics stay in the Python `TypeInfo` methods; Rust only
//! classifies the node kind and reads scalar flags (`is_staticmethod`,
//! `is_classvar`, ...).
//!
//! Returns `None` (defer to the pure-Python body) whenever a live read
//! fails, a node kind is unexpected, or a `Var`/method structure cannot
//! be classified purely. The Python shim only catches
//! AssertionError/NotImplementedError/ValueError, so the inner body must
//! never raise: every `getattr` is `.ok()?` and every extract is `.ok()?`.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyType};

use crate::typeinfo::{serialize_type_to_bytes, NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type};

/// Flag constants, mirroring `mypy.subtypes` (subtypes.py:178-182).
pub(crate) const IS_SETTABLE: i64 = 1;
pub(crate) const IS_CLASSVAR: i64 = 2;
pub(crate) const IS_CLASS_OR_STATIC: i64 = 3;
const IS_VAR: i64 = 4;
pub(crate) const IS_EXPLICIT_SETTER: i64 = 5;

/// Fetch a class from a module.
fn py_class<'py>(py: Python<'py>, module: &str, name: &str) -> Option<&'py PyType> {
    py.import(module)
        .ok()?
        .getattr(name)
        .ok()?
        .downcast::<PyType>()
        .ok()
}

/// Read a boolean flag off a node. Any read failure defers (None).
fn get_bool_flag(node: &PyAny, name: &str) -> Option<bool> {
    read_bool_attr(node, name)
}

fn read_bool_attr(obj: &PyAny, attr: &str) -> Option<bool> {
    obj.getattr(attr)
        .and_then(|x| {
            if let Ok(b) = x.extract::<bool>() {
                Ok(b)
            } else if let Ok(b) = x.downcast::<pyo3::types::PyBool>() {
                Ok(b.is_true())
            } else {
                Err(pyo3::PyErr::fetch(x.py()))
            }
        })
        .ok()
}

/// `is_descriptor` (subtypes.py:1815-1821): an Instance with `__get__`,
/// or a Union whose relevant items are all descriptors. `typ` arrives as
/// wire bytes (the shim serializes `v.type`); TypeAliasType cannot be
/// expanded on the wire (`get_proper_type`) so it defers. `relevant_items`
/// drops NoneType items when strict_optional is off; `strict_optional`
/// lets Rust decide the same filtering.
fn is_descriptor(typ_bytes: &[u8], strict_optional: bool, resolver: &TypeResolver) -> Option<bool> {
    let mut buf = ReadBuffer::new(typ_bytes);
    let typ = wire::read_type(&mut buf, None).ok()?;
    match typ {
        Type::TypeAliasType { .. } => None,
        Type::Instance { type_ref, .. } => {
            let snap = resolver.get(&type_ref)?;
            Some(snap.member_definers.contains_key("__get__"))
        }
        Type::UnionType { items, .. } => {
            for item in &items {
                if !matches!(
                    is_descriptor(&encode(item)?, strict_optional, resolver),
                    Some(true)
                ) {
                    // all(): a non-descriptor item (or a defer) is False.
                    return Some(false);
                }
            }
            if strict_optional {
                return Some(true);
            }
            let keep = items
                .iter()
                .filter(|item| !matches!(item, Type::NoneType))
                .count();
            Some(keep == items.len())
        }
        _ => Some(false),
    }
}

fn encode(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = crate::wire::WriteBuffer::new();
    wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

fn is_decorator(py: Python<'_>, obj: &PyAny) -> bool {
    let cls = py_class(py, "mypy.nodes", "Decorator");
    match cls {
        Some(cls) => obj.is_instance(cls).unwrap_or(false),
        None => false,
    }
}

fn is_overloaded(py: Python<'_>, obj: &PyAny) -> bool {
    let cls = py_class(py, "mypy.nodes", "OverloadedFuncDef");
    match cls {
        Some(cls) => obj.is_instance(cls).unwrap_or(false),
        None => false,
    }
}

fn is_var(py: Python<'_>, obj: &PyAny) -> bool {
    let cls = py_class(py, "mypy.nodes", "Var");
    match cls {
        Some(cls) => obj.is_instance(cls).unwrap_or(false),
        None => false,
    }
}

/// `info.get_method(name)`: a FuncBase/Decorator node, or None. The
/// redefinition fallback and the "found a non-func node" stop live in the
/// Python `TypeInfo.get_method`; we call it on the live object and only
/// classify the result.
fn get_method_node(info: &PyAny, name: &str) -> Option<PyObject> {
    let method = info.getattr("get_method").ok()?.call1((name,)).ok()?;
    if method.is_none() {
        return None;
    }
    Some(method.to_object(method.py()))
}

/// `info.get(name)`: the SymbolTableNode (already resolved through the
/// full mro by the Python `TypeInfo.get`).
fn get_node(info: &PyAny, name: &str) -> Option<PyObject> {
    let node = info.getattr("get").ok()?.call1((name,)).ok()?;
    if node.is_none() {
        return None;
    }
    Some(node.to_object(node.py()))
}

/// `info.get_method("__setattr__")` present? Mirrors the Python body's
/// `setattr_meth` truthiness.
fn has_setattr(info: &PyAny) -> Option<bool> {
    let method = info
        .getattr("get_method")
        .ok()?
        .call1(("__setattr__",))
        .ok()?;
    Some(!method.is_none())
}

/// `get_member_flags` (subtypes.py:1773-1812). `info` is the live
/// `itype.type` TypeInfo; `extra_attrs` is the live `itype.extra_attrs`
/// (or None): its `attrs` dict keys and `immutable`/`mutable` sets are
/// read via PyO3.
fn get_member_flags_inner(
    py: Python<'_>,
    info: &PyAny,
    name: &str,
    class_obj: bool,
    extra_attrs: Option<&PyAny>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<i64>> {
    get_member_flags_inner_impl(
        py,
        info,
        name,
        class_obj,
        extra_attrs,
        strict_optional,
        resolver,
    )
}

/// Re-exported for `protocols::is_protocol_implementation_inner` (the
/// Rust member-flag loop). Delegates to the same impl as
/// `get_member_flags_inner`.
pub(crate) fn get_member_flags_inner_pub(
    py: Python<'_>,
    info: &PyAny,
    name: &str,
    class_obj: bool,
    extra_attrs: Option<&PyAny>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<i64>> {
    get_member_flags_inner_impl(
        py,
        info,
        name,
        class_obj,
        extra_attrs,
        strict_optional,
        resolver,
    )
}

fn get_member_flags_inner_impl(
    py: Python<'_>,
    info: &PyAny,
    name: &str,
    class_obj: bool,
    extra_attrs: Option<&PyAny>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<i64>> {
    let method = get_method_node(info, name);
    let setattr_meth = has_setattr(info)?.then_some(());
    if let Some(method) = method {
        let method = method.as_ref(py);
        if is_decorator(py, method) {
            let var = method.getattr("var").ok()?;
            if get_bool_flag(var, "is_staticmethod")? || get_bool_flag(var, "is_classmethod")? {
                return Some(vec![IS_CLASS_OR_STATIC]);
            }
            if get_bool_flag(var, "is_property")? {
                return Some(vec![IS_VAR]);
            }
            return Some(vec![]);
        }
        if is_overloaded(py, method) {
            if get_bool_flag(method, "is_property")? {
                let items = method.getattr("items").ok()?;
                let list = items.downcast::<PyList>().ok()?;
                let first = list.iter().next()?;
                if !is_decorator(py, first) {
                    return Some(vec![]);
                }
                let var = first.getattr("var").ok()?;
                if get_bool_flag(var, "is_settable_property")? || setattr_meth.is_some() {
                    let mut flags = vec![IS_VAR, IS_SETTABLE];
                    if !var.getattr("setter_type").ok()?.is_none() {
                        flags.push(IS_EXPLICIT_SETTER);
                    }
                    return Some(flags);
                }
                return Some(vec![IS_VAR]);
            }
            return Some(vec![]);
        }
        return Some(vec![]);
    }
    let node = get_node(info, name);
    match node {
        None => {
            if setattr_meth.is_some() {
                return Some(vec![IS_SETTABLE]);
            }
            if let Some(extra_attrs) = extra_attrs {
                let attrs = extra_attrs.getattr("attrs").ok()?;
                let attrs_dict = attrs.downcast::<PyDict>().ok()?;
                if attrs_dict.contains(name).ok()? {
                    let mut flags: Vec<i64> = Vec::new();
                    let immutable = extra_attrs.getattr("immutable").ok()?;
                    let immutable_set = immutable.downcast::<PySet>().ok()?;
                    if !immutable_set.contains(name).ok()? {
                        flags.push(IS_SETTABLE);
                    }
                    let mutable = extra_attrs.getattr("mutable").ok()?;
                    let mutable_set = mutable.downcast::<PySet>().ok()?;
                    if mutable_set.contains(name).ok()? {
                        flags.push(IS_CLASSVAR);
                    }
                    return Some(flags);
                }
            }
            Some(vec![])
        }
        Some(node) => {
            // v = node.node
            let v = node.as_ref(py).getattr("node").ok()?;
            if !is_var(py, v) {
                return Some(vec![]);
            }
            if get_bool_flag(v, "is_property")? {
                return Some(vec![IS_VAR]);
            }
            let mut flags = vec![IS_VAR];
            if !get_bool_flag(v, "is_final")? {
                flags.push(IS_SETTABLE);
            }
            if get_bool_flag(v, "is_classvar")? {
                // `v.type` can be None; `is_descriptor(None)` is False in
                // Python, so a None type must NOT add IS_CLASSVAR.
                let type_obj = v.getattr("type").ok()?;
                if !type_obj.is_none() {
                    let bytes = serialize_type_to_bytes(py, type_obj)?;
                    match is_descriptor(&bytes, strict_optional, resolver) {
                        Some(false) => flags.push(IS_CLASSVAR),
                        // Python computes `not is_descriptor(v.type)` with
                        // a total predicate; an undecidable wire answer
                        // must defer the flag set so the shim recomputes
                        // it with the exact IS_CLASSVAR membership.
                        None => return None,
                        Some(true) => {}
                    }
                }
            }
            if class_obj && get_bool_flag(v, "is_inferred")? {
                flags.push(IS_CLASSVAR);
            }
            Some(flags)
        }
    }
}

/// `#[pyfunction]` entry: `rust_get_member_flags(info, name, class_obj,
/// extra_attrs, strict_optional, resolver)`. Returns the flag list, or
/// `None` to defer to the pure-Python body.
#[pyfunction]
#[pyo3(signature = (info, name, class_obj, extra_attrs, strict_optional, resolver))]
pub(crate) fn rust_get_member_flags(
    py: Python<'_>,
    info: &PyAny,
    name: &str,
    class_obj: bool,
    extra_attrs: Option<&PyAny>,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> Option<Vec<i64>> {
    get_member_flags_inner(
        py,
        info,
        name,
        class_obj,
        extra_attrs,
        strict_optional,
        resolver.resolver(),
    )
}
