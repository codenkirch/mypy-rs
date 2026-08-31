//! Native port of `mypy/checkmember.py` helpers (Stage M20).
//!
//! Ports the pure-logic subset of `analyze_member_access` that operates on
//! the wire-format `Type` enum without needing live Python checker state.
//! Each `#[pyfunction]` returns `None` for cases Rust cannot handle, so the
//! Python caller falls through to the pure-Python implementation (the
//! strangler-fig per-call gate).
//!
//! Ported functions:
//!   * `bind_self_fast` — strips the first positional argument from a
//!     CallableType or Overloaded and sets `is_bound=True`. Pure type
//!     manipulation, no checker state. Called on every trivial-self method
//!     access (hot path). Star-args, empty-args and non-callable types are
//!     handled exactly like Python: the first two return the method
//!     unchanged, only non-callable types defer.
//!   * `classify_member_access` — classifies the `_analyze_member_access`
//!     dispatch branch from a wire-format type. Returns an int code so
//!     Python can skip the isinstance chain. Defers on TypeAliasType
//!     (needs alias expansion via `get_proper_type`).
//!   * `instance_fallback` — the Instance fallback for a proper type.
//!   * `has_operator` / `meta_has_operator` — operator-presence checks that
//!     walk the resolver's mro + member metadata instead of consulting live
//!     checker state.
//!   * `defined_in_superclass` — whether a variable has an explicit value at
//!     class level in any superclass.
//!   * `rust_analyze_instance_member_dispatch` (issue #805) — the method
//!     branch head of `analyze_instance_member_access`: live `get_method`
//!     lookup via the resolver's live TypeInfo map, flag reads, freshen,
//!     and the static / trivial-self / non-trivial tail.
//!   * `rust_analyze_union_member_access` (issue #805) — per-item mapping
//!     of `analyze_union_member_access`; Instance items dispatch, the
//!     Python shim joins via `make_simplified_union`.
//!
//! Deferred (return None):
//!   * `TypeAliasType` — the wire format carries no resolved alias target,
//!     so `get_proper_type` cannot expand it.
//!   * Overloaded with zero items — degenerate; defer to Python.
//!   * `has_operator` on a TypeVarType with a non-empty value restriction,
//!     or on ParamSpec/TypeVarTuple — `values_or_bound()` needs union
//!     construction or the objects bound; defer.
//!   * Any member or metaclass lookup that hits a snapshot missing from the
//!     resolver — we cannot distinguish "absent" from "unknown", so we
//!     return None rather than risk a wrong boolean.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyType};
use std::collections::HashSet;

use crate::freshen::freshen_type;
use crate::setops::make_simplified_union;
use crate::subtypes::SubtypeContext;
use crate::typeinfo::{serialize_type_to_bytes, NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, Parameters, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `ArgKind.ARG_POS` = 0.
const ARG_POS: i64 = 0;
/// `ArgKind.ARG_STAR` = 2.
const ARG_STAR: i64 = 2;
/// `ArgKind.ARG_STAR2` = 4.
const ARG_STAR2: i64 = 4;

/// Dispatch codes for `classify_member_access`. Mirror the `isinstance`
/// chain in `_analyze_member_access` (checkmember.py:242-281).
pub(crate) const MA_INSTANCE: i64 = 0;
pub(crate) const MA_ANY: i64 = 1;
pub(crate) const MA_UNION: i64 = 2;
pub(crate) const MA_TYPE_CALLABLE: i64 = 3;
pub(crate) const MA_TYPE_TYPE: i64 = 4;
pub(crate) const MA_TUPLE: i64 = 5;
pub(crate) const MA_LITERAL_OR_FUNC: i64 = 6;
pub(crate) const MA_TYPEDDICT: i64 = 7;
pub(crate) const MA_NONE: i64 = 8;
pub(crate) const MA_TYPEVAR: i64 = 9;
pub(crate) const MA_DELETED: i64 = 10;
pub(crate) const MA_UNINHABITED: i64 = 11;
pub(crate) const MA_MISSING: i64 = 12;

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

pub(crate) fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

pub(crate) fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// `get_proper_type` for the wire format. Expands `TypeAliasType` by
/// returning `None` (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// Whether a CallableType is a type object (i.e. its fallback is a
/// metaclass). Mirrors `CallableType.is_type_obj()` (types.py:2343).
/// Checks `fallback.type.is_metaclass()` via the resolver snapshot's
/// `metaclass_fullname` field. Also requires `ret_type` not to be
/// `UninhabitedType`.
fn is_type_obj(fallback: &Type, ret_type: &Type, resolver: &TypeResolver) -> bool {
    if matches!(ret_type, Type::UninhabitedType { .. }) {
        return false;
    }
    let type_ref = match fallback {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return false,
    };
    // is_metaclass() checks if the TypeInfo or any of its MRO bases is
    // builtins.type or abc.ABCMeta. The snapshot stores metaclass_fullname
    // only when metaclass_type is set. We check if the fallback's type_ref

    // appears as a metaclass in any snapshot (i.e. its own
    // metaclass_fullname is set and it has builtins.type in its MRO).
    // Simplified: check if type_ref has builtins.type in its MRO.
    if type_ref == "builtins.type" {
        return true;
    }
    if let Some(snap) = resolver.get(type_ref) {
        snap.mro.iter().any(|m| m == "builtins.type")
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// bind_self_fast
// ---------------------------------------------------------------------------

/// `mypy.checkmember.bind_self_fast` — strip the first positional argument
/// from a CallableType or Overloaded and set `is_bound=True`.
///
/// Mirrors `bind_self_fast` (checkmember.py:1503-1525). Returns `None` (Python
/// `None`) when Rust cannot handle the case so the Python caller falls
/// through. Deferred cases:
///   * Non-callable types (Instance, AnyType, etc.)
///   * Overloaded with zero items
///
/// A CallableType with no args, or whose first arg is `*args`/`**kwargs`, is
/// returned unchanged (mirroring Python), not deferred.
///
/// The `original_type` parameter from Python is unused here: `bind_self_fast`
/// only strips the first arg and sets `is_bound`; it does NOT substitute
/// type variables (that's `bind_self` in typeops.py).
///
/// Deferral guard: returns true when `typ` contains an ErasedType anywhere.
/// Before ErasedType gained a wire tag, `decode_type` could not reconstruct
/// an ErasedType, so `rust_bind_self_fast` returned `None` (deferred to
/// Python) for any ErasedType-carrying signature. That implicit deferral is
/// now made explicit so inference semantics stay identical: binding a
/// self/arg from a method whose signature still holds an ErasedType
/// placeholder must go through the pure-Python `copy_modified` path.
fn contains_erased(typ: &Type) -> bool {
    match typ {
        Type::ErasedType => true,
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            args.iter().any(contains_erased)
                || last_known_value.as_deref().is_some_and(contains_erased)
        }
        Type::TypeAliasType { args, .. } => args.iter().any(contains_erased),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            values.iter().any(contains_erased)
                || contains_erased(upper_bound)
                || contains_erased(default)
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            // `prefix` is a boxed `Parameters`; only its arg_types carry types.
            prefix.arg_types.iter().any(contains_erased)
                || contains_erased(upper_bound)
                || contains_erased(default)
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            contains_erased(tuple_fallback)
                || contains_erased(upper_bound)
                || contains_erased(default)
        }
        Type::UnboundType { args, .. } => args.iter().any(contains_erased),
        Type::UnpackType { typ } => contains_erased(typ),
        Type::AnyType { source_any, .. } => source_any.as_deref().is_some_and(contains_erased),
        Type::CallableType {
            fallback,
            instance_type,
            arg_types,
            ret_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            contains_erased(fallback)
                || instance_type.as_deref().is_some_and(contains_erased)
                || arg_types.iter().any(contains_erased)
                || contains_erased(ret_type)
                || variables.iter().any(contains_erased)
                || type_guard.as_deref().is_some_and(contains_erased)
                || type_is.as_deref().is_some_and(contains_erased)
        }
        Type::Overloaded { items } => items.iter().any(contains_erased),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => items.iter().any(contains_erased) || contains_erased(partial_fallback),
        Type::TypedDictType {
            fallback, items, ..
        } => contains_erased(fallback) || items.iter().any(|(_, v)| contains_erased(v)),
        Type::LiteralType { fallback, .. } => contains_erased(fallback),
        Type::UnionType { items, .. } => items.iter().any(contains_erased),
        Type::TypeType { item, .. } => contains_erased(item),
        Type::Parameters(parameters) => parameters.arg_types.iter().any(contains_erased),
        Type::NoneType | Type::UninhabitedType { .. } | Type::DeletedType { .. } => false,
    }
}

#[pyfunction]
pub(crate) fn rust_bind_self_fast(method_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(method_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    if contains_erased(&typ) {
        return Ok(None);
    }
    match bind_self_fast_inner(&typ) {
        Some(bound) => Ok(encode_type(&bound)),
        None => Ok(None),
    }
}

fn bind_self_fast_inner(typ: &Type) -> Option<Type> {
    match typ {
        Type::Overloaded { items } => {
            if items.is_empty() {
                return None;
            }
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(bind_self_fast_inner(item)?);
            }
            Some(Type::Overloaded { items: new_items })
        }
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound: _,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            if arg_types.is_empty() {
                // Nothing to strip — Python returns the method unchanged.
                return Some(typ.clone());
            }
            // Python indexes arg_kinds[0]; guard a length mismatch so a
            // pathological CallableType returns unchanged instead of panicking.
            match arg_kinds.first() {
                Some(&kind) if kind == ARG_STAR || kind == ARG_STAR2 => {
                    // *args / **kwargs — Python returns the method unchanged.
                    Some(typ.clone())
                }
                Some(_) => Some(Type::CallableType {
                    fallback: fallback.clone(),
                    instance_type: instance_type.clone(),
                    is_ellipsis_args: *is_ellipsis_args,
                    implicit: *implicit,
                    is_bound: true,
                    from_concatenate: *from_concatenate,
                    imprecise_arg_kinds: *imprecise_arg_kinds,
                    unpack_kwargs: *unpack_kwargs,
                    from_type_type: *from_type_type,
                    arg_types: arg_types[1..].to_vec(),
                    arg_kinds: arg_kinds[1..].to_vec(),
                    arg_names: arg_names[1..].to_vec(),
                    ret_type: ret_type.clone(),
                    name: name.clone(),
                    variables: variables.clone(),
                    type_guard: type_guard.clone(),
                    type_is: type_is.clone(),
                }),
                None => Some(typ.clone()),
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// instance_fallback
// ---------------------------------------------------------------------------

/// `mypy.checkmember.instance_fallback` — the Instance fallback of a proper
/// type: Instance→self, TupleType→tuple_fallback, Literal/TypedDict→fallback,
/// anything else→`builtins.object`.
///
/// Mirrors `instance_fallback` (checkmember.py:1625-1635). Returns `None` for
/// a TupleType whose partial fallback is not an Instance (variadic edge), so
/// the caller falls through to Python. Returns the (possibly non-Instance)
/// fallback for LiteralType/TypedDictType exactly like Python; the Python
/// shim re-checks `isinstance(decoded, Instance)` before trusting it.
#[pyfunction]
pub(crate) fn rust_instance_fallback(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let fb = match instance_fallback_inner(&typ) {
        Some(fb) => fb,
        None => return Ok(None),
    };
    Ok(encode_type(&fb))
}

fn instance_fallback_inner(typ: &Type) -> Option<Type> {
    match typ {
        Type::Instance { .. } => Some(typ.clone()),
        Type::TupleType {
            partial_fallback, ..
        } => {
            // tuple_fallback: return the partial fallback when it is an
            // Instance, else defer (variadic edge). For builtins.tuple the
            // Instance is rebuilt from the items with the same type_ref.
            match &**partial_fallback {
                Type::Instance { .. } => Some((**partial_fallback).clone()),
                _ => None,
            }
        }
        Type::LiteralType { fallback, .. } | Type::TypedDictType { fallback, .. } => {
            Some((**fallback).clone())
        }
        _ => Some(Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// has_operator / meta_has_operator
// ---------------------------------------------------------------------------

/// `mypy.checkmember.has_operator` — whether a proper type has the given
/// operator method, mirroring checkmember.py:1590-1622.
///
/// Resolver-based: member presence is read from the resolver snapshots (the
/// Rust `Type` wire format carries no `TypeInfo.mro`/`names`/metaclass), so
/// this returns `None` whenever the relevant snapshots are missing from the
/// resolver, letting Python fall through. `strict_optional` mirrors
/// `state.strict_optional` for `relevant_items()` filtering of `NoneType`
/// union items.
#[pyfunction]
pub(crate) fn rust_has_operator(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    op_method: &str,
    strict_optional: bool,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_operator_inner(
        &typ,
        op_method,
        strict_optional,
        resolver.resolver(),
    ))
}

fn has_operator_inner(
    typ: &Type,
    op_method: &str,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    match typ {
        Type::TypeAliasType { .. } => None, // needs get_proper_type alias expansion
        Type::AnyType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            let mut acc = true;
            for item in items {
                let is_none = matches!(item, Type::NoneType);
                if !strict_optional && is_none {
                    continue; // relevant_items(): skip NoneType when not strict
                }
                let b = has_operator_inner(item, op_method, strict_optional, resolver)?;
                if !b {
                    acc = false;
                }
            }
            Some(acc)
        }
        Type::TypeVarType {
            values,
            upper_bound,
            ..
        } => {
            if !values.is_empty() {
                None // values_or_bound() would need union construction
            } else {
                // values_or_bound(): a TypeVarType with no value restriction
                // resolves to its upper bound.
                has_operator_inner(upper_bound, op_method, strict_optional, resolver)
            }
        }
        Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => None,
        Type::CallableType {
            fallback, ret_type, ..
        } => {
            if is_type_obj(fallback, ret_type, resolver) {
                // FunctionLike.is_type_obj() -> fallback.type.has_readable_member
                match &**fallback {
                    Type::Instance { type_ref, .. } => {
                        has_readable_member_by_ref(resolver, type_ref, op_method)
                    }
                    _ => None,
                }
            } else {
                // Not a type object — falls through to the generic path below.
                match instance_fallback_inner(typ) {
                    Some(Type::Instance { type_ref, .. }) => {
                        has_readable_member_by_ref(resolver, &type_ref, op_method)
                    }
                    _ => None,
                }
            }
        }
        Type::Overloaded { items } => {
            if items.is_empty() {
                return None; // degenerate — defer to Python
            }
            // FunctionLike.is_type_obj() checks the first overload item.
            if let Some(Type::CallableType {
                fallback, ret_type, ..
            }) = items.first()
            {
                if is_type_obj(fallback, ret_type, resolver) {
                    match &**fallback {
                        Type::Instance { type_ref, .. } => {
                            has_readable_member_by_ref(resolver, type_ref, op_method)
                        }
                        _ => None,
                    }
                } else {
                    match instance_fallback_inner(typ) {
                        Some(Type::Instance { type_ref, .. }) => {
                            has_readable_member_by_ref(resolver, &type_ref, op_method)
                        }
                        _ => None,
                    }
                }
            } else {
                None
            }
        }
        Type::TypeType { item, .. } => {
            // Python: item = typ.item; if TypeVarType -> values_or_bound();
            // if Union -> all(meta_has_operator per relevant item); else
            // meta_has_operator(item).
            match &**item {
                Type::TypeVarType {
                    values,
                    upper_bound,
                    ..
                } => {
                    if values.is_empty() {
                        meta_has_operator_inner(upper_bound, op_method, resolver)
                    } else {
                        None
                    }
                }
                Type::UnionType { items, .. } => {
                    let mut acc = true;
                    for x in items {
                        let is_none = matches!(x, Type::NoneType);
                        if !strict_optional && is_none {
                            continue;
                        }
                        let b = meta_has_operator_inner(x, op_method, resolver)?;
                        if !b {
                            acc = false;
                        }
                    }
                    Some(acc)
                }
                _ => meta_has_operator_inner(item, op_method, resolver),
            }
        }
        // Generic path: instance_fallback(typ).type.has_readable_member(op).
        _ => match instance_fallback_inner(typ) {
            Some(Type::Instance { type_ref, .. }) => {
                has_readable_member_by_ref(resolver, &type_ref, op_method)
            }
            _ => None,
        },
    }
}

/// `mypy.checkmember.meta_has_operator` — operator presence on a type's
/// metaclass, mirroring checkmember.py:1638-1647.
#[pyfunction]
pub(crate) fn rust_meta_has_operator(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    op_method: &str,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(meta_has_operator_inner(
        &typ,
        op_method,
        resolver.resolver(),
    ))
}

fn meta_has_operator_inner(item: &Type, op_method: &str, resolver: &TypeResolver) -> Option<bool> {
    match item {
        Type::TypeAliasType { .. } => None,
        Type::AnyType { .. } => Some(true),
        _ => {
            let fb = instance_fallback_inner(item)?;
            let type_ref = match &fb {
                Type::Instance { type_ref, .. } => type_ref.as_str(),
                _ => return None,
            };
            // meta = item.type.metaclass_type or Instance(builtins.type)
            let snap = resolver.get(type_ref)?;
            let meta_ref = snap
                .metaclass_fullname
                .as_deref()
                .unwrap_or("builtins.type");
            has_readable_member_by_ref(resolver, meta_ref, op_method)
        }
    }
}

/// `TypeInfo.has_readable_member` for a resolver-resolved class ref.
///
/// Mirrors `TypeInfo.get` (mypy/nodes.py): walks `self.mro`, returning the
/// first class whose own `names` dict contains the name (existence-only;
/// implicit or explicit). Defer (None) when any class in the mro is missing
/// from the resolver — we cannot distinguish "absent" from "unknown".
pub(crate) fn has_readable_member_by_ref(
    resolver: &TypeResolver,
    type_ref: &str,
    name: &str,
) -> Option<bool> {
    let snap = resolver.get(type_ref)?;
    for base in &snap.mro {
        let b = resolver.get(base)?;
        if b.member_info.contains_key(name) {
            return Some(true);
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// defined_in_superclass
// ---------------------------------------------------------------------------

/// `mypy.checkmember.defined_in_superclass` — whether `name` is defined as an
/// explicitly-valued variable in any superclass of `fullname`, mirroring
/// checkmember.py:1650-1655. Defer (None) when the class or any superclass
/// snapshot is missing from the resolver.
#[pyfunction]
pub(crate) fn rust_defined_in_superclass(
    resolver: &NativeTypeResolver,
    fullname: &str,
    name: &str,
) -> PyResult<Option<bool>> {
    let r = resolver.resolver();
    let snap = match r.get(fullname) {
        Some(s) => s,
        None => return Ok(None),
    };
    for base in snap.mro.iter().skip(1) {
        let b = match r.get(base) {
            Some(b) => b,
            None => return Ok(None),
        };
        if let Some((implicit, var_explicit)) = b.member_info.get(name) {
            if !*implicit && *var_explicit {
                return Ok(Some(true));
            }
        }
    }
    Ok(Some(false))
}

// ---------------------------------------------------------------------------
// analyze_instance_member_access (method path)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// The `maptype.py:326-345` `builtins.tuple` special case inside
/// `map_instance_to_direct_supertypes`, decided from the live receiver
/// `TypeInfo`. Returns `Some(Some(inst))` when the special case produces
/// the mapped supertype instance, `Some(None)` when it does not apply and
/// the generic `map_instance_to_supertype` walk is parity-correct, and
/// `None` (defer) when a live fact is unreadable or the expand defers.
///
/// `tuple_type` and `special_alias._is_recursive` are read LIVE (not from
/// the snapshot): `_is_recursive` is a mutable raw cache (types.py:488)
/// that Python reads raw at maptype time, so a snapshot could go stale.
fn tuple_special_map(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    instance: &Type,
    strict_optional: bool,
) -> Option<Option<Type>> {
    let left_ref = match instance {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return None,
    };
    if left_ref == "builtins.tuple" {
        // maptype.py:178 `if instance.type == superclass: return instance`.
        return Some(Some(instance.clone()));
    }
    // The special case needs a DIRECT `builtins.tuple` base in the
    // receiver's `bases` (the NamedTuple shape); a generic NamedTuple
    // reached via an intermediate class defers to Python.
    let info = resolver.live_typeinfo(py, left_ref)?;
    if info.is_none() {
        return None;
    }
    let has_tuple_base = {
        let bases = info.getattr("bases").ok()?;
        let mut found = false;
        for b in bases.iter().ok()? {
            let full = b
                .ok()?
                .getattr("type")
                .ok()?
                .getattr("fullname")
                .ok()?
                .extract::<String>()
                .ok()?;
            if full == "builtins.tuple" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_tuple_base {
        return None;
    }
    // maptype.py:327 `and instance.type.tuple_type`: falsy tuple_type
    // skips the special case.
    let tt = match info.getattr("tuple_type") {
        Ok(t) if !t.is_none() => t,
        _ => return Some(None),
    };
    let tt_bytes = serialize_type_to_bytes(py, tt)?;
    let tt_wire = decode_type(&tt_bytes)?;
    if !crate::expandtype::result_has_typevar(&tt_wire) {
        // maptype.py:328 `if has_type_vars(...)`: false means the generic
        // expand of the direct tuple base is parity-correct.
        return Some(None);
    }
    // maptype.py:330-332 `alias = instance.type.special_alias; assert
    // alias is not None`. A missing alias asserts in Python, so defer.
    let alias = match info.getattr("special_alias") {
        Ok(a) if !a.is_none() => a,
        _ => return None,
    };
    // maptype.py:334 `if not alias._is_recursive` reads the RAW cached
    // value: `None` (not yet computed) is falsy and proceeds.
    let is_recursive = match alias.getattr("_is_recursive") {
        Ok(v) if v.is_none() => false,
        Ok(v) => match v.extract::<bool>() {
            Ok(b) => b,
            Err(_) => return None,
        },
        Err(_) => return None,
    };
    if is_recursive {
        // "Unfortunately we can't support this for generic recursive
        // tuples": Python skips the special casing and falls back to the
        // generic walk of the direct tuple base.
        return Some(None);
    }
    // maptype.py:335-336 `tuple_type = expand_type_by_instance(
    // instance.type.tuple_type, instance)`.
    let expanded = crate::expandtype::expand_type_by_instance_core(
        &tt_wire,
        instance,
        resolver.resolver(),
        strict_optional,
    )?;
    match expanded {
        Type::TupleType { .. } => {
            // maptype.py:338-342 `tuple_fallback(tuple_type)`.
            let fb = crate::typeops::tuple_fallback(&expanded, resolver.resolver())?;
            Some(Some(fb))
        }
        // maptype.py:343-345 "This can happen after normalizing variadic
        // tuples."
        Type::Instance { .. } => Some(Some(expanded)),
        // Any other shape falls through to the generic expand of the
        // direct tuple base.
        _ => Some(None),
    }
}

/// Value-level port of `FreezeTypeVarsVisitor` (typeops.py:2107-2113),
/// in two steps: `collect_freeze_ids` gathers the ids of every callable's
/// `variables`, and `apply_freeze` sets `meta_level = 0` on matching
/// typevars anywhere in the tree. Python freezes by shared-object mutation
/// (`freshen_function_type_vars` shares one typevar object per variable
/// between `variables` and every occurrence, expandtype.py:550); the wire
/// round-trip breaks object sharing, so we rewrite by id instead. Meta
/// typevars that appear only outside `variables` (the caller's own
/// unification variables in the receiver args) stay untouched, exactly like
/// Python.
fn collect_freeze_ids(typ: &mut Type, ids: &mut Vec<(i64, i64, String)>) {
    if let Type::CallableType { variables, .. } = typ {
        for v in variables.iter_mut() {
            if let Type::TypeVarType {
                raw_id,
                namespace,
                meta_level,
                ..
            } = v
            {
                let key = (*raw_id, *meta_level, namespace.clone());
                if !ids.contains(&key) {
                    ids.push(key);
                }
            }
        }
    }
    freeze_children(typ, &mut |c| collect_freeze_ids(c, ids));
}

fn apply_freeze(typ: &mut Type, ids: &[(i64, i64, String)]) {
    if let Type::TypeVarType {
        raw_id,
        namespace,
        meta_level,
        ..
    } = typ
    {
        if ids
            .iter()
            .any(|(r, m, ns)| *r == *raw_id && *m == *meta_level && ns == namespace)
        {
            *meta_level = 0;
        }
    }
    freeze_children(typ, &mut |c| apply_freeze(c, ids));
}

fn key_in(keys: &[BindTVarKey], raw_id: i64, meta_level: i64, namespace: &str) -> bool {
    keys.iter()
        .any(|(r, m, ns)| *r == raw_id && *m == meta_level && ns == namespace)
}

/// Collect the (raw_id, meta_level, namespace) triple of every typevar-like
/// node in the tree.
fn collect_tvar_keys(typ: &Type, keys: &mut Vec<BindTVarKey>) {
    match typ {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => keys.push((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id, namespace, ..
        }
        | Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => keys.push((*raw_id, -1, namespace.clone())),
        _ => {}
    }
    for_each_child(typ, &mut |c| collect_tvar_keys(c, keys));
}

/// Narrowed survivor gate (issue #1277): a tvar leftover in the expanded tree
/// rides through only when it is a `variables` entry or an occurrence among
/// the mapped receiver's args; anything else, or an UnpackType, defers.
///
/// A receiver-arg residual must additionally be bound (`meta_level == 0`):
/// Python returns the caller's fresh (meta_level > 0) unification variable
/// object itself, and downstream solve-freshening fuses var identities that
/// live in one object; the wire round-trip can re-share identity only inside
/// one decoded tree, never across seams, so a bare fresh var decoded from the
/// IAMA tail arrives as a doppelganger and the callee solve collapses the
/// inference target to `Never` (issue #1286). Defer those to Python.
fn survivors_allowed(typ: &Type, ids: &[BindTVarKey], recv: &[BindTVarKey]) -> bool {
    let ok = match typ {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => {
            key_in(ids, *raw_id, *meta_level, namespace)
                || (key_in(recv, *raw_id, *meta_level, namespace) && *meta_level == 0)
        }
        Type::ParamSpecType {
            raw_id, namespace, ..
        }
        | Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => key_in(recv, *raw_id, -1, namespace),
        Type::UnpackType { .. } => false,
        _ => true,
    };
    if !ok {
        return false;
    }
    let mut all = true;
    for_each_child(typ, &mut |c| {
        if all {
            all = survivors_allowed(c, ids, recv);
        }
    });
    all
}

/// Read-only mirror of `freeze_children`, used by the survivor-gate walks
/// (which must consult exactly the tree freeze would reach).
fn for_each_child<F: FnMut(&Type)>(typ: &Type, f: &mut F) {
    match typ {
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            for v in values.iter() {
                f(v);
            }
            f(upper_bound);
            f(default);
        }
        Type::Instance { args, .. }
        | Type::TypeAliasType { args, .. }
        | Type::UnboundType { args, .. } => {
            for a in args.iter() {
                f(a);
            }
        }
        Type::AnyType { source_any, .. } => {
            if let Some(src) = source_any {
                f(src);
            }
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            let Parameters {
                arg_types,
                variables,
                ..
            } = &**prefix;
            for a in arg_types.iter() {
                f(a);
            }
            for v in variables.iter() {
                f(v);
            }
            f(upper_bound);
            f(default);
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            f(tuple_fallback);
            f(upper_bound);
            f(default);
        }
        Type::UnpackType { typ } => f(typ),
        Type::CallableType {
            fallback,
            instance_type,
            arg_types,
            ret_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            f(fallback);
            if let Some(it) = instance_type {
                f(it);
            }
            for a in arg_types.iter() {
                f(a);
            }
            f(ret_type);
            for v in variables.iter() {
                f(v);
            }
            if let Some(g) = type_guard {
                f(g);
            }
            if let Some(i) = type_is {
                f(i);
            }
        }
        Type::Overloaded { items } => {
            for it in items.iter() {
                f(it);
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            f(partial_fallback);
            for it in items.iter() {
                f(it);
            }
        }
        Type::TypedDictType {
            fallback, items, ..
        } => {
            f(fallback);
            for (_, it) in items.iter() {
                f(it);
            }
        }
        Type::LiteralType { fallback, .. } => f(fallback),
        Type::UnionType { items, .. } => {
            for it in items.iter() {
                f(it);
            }
        }
        Type::TypeType { item, .. } => f(item),
        Type::Parameters(params) => {
            for a in params.arg_types.iter() {
                f(a);
            }
            for v in params.variables.iter() {
                f(v);
            }
        }
        Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. } => {}
    }
}

fn freeze_children<F: FnMut(&mut Type)>(typ: &mut Type, f: &mut F) {
    match typ {
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            for v in values.iter_mut() {
                f(v);
            }
            f(upper_bound);
            f(default);
        }
        Type::Instance { args, .. }
        | Type::TypeAliasType { args, .. }
        | Type::UnboundType { args, .. } => {
            for a in args.iter_mut() {
                f(a);
            }
        }
        Type::AnyType { source_any, .. } => {
            if let Some(src) = source_any {
                f(src);
            }
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            let Parameters {
                arg_types,
                variables,
                ..
            } = prefix.as_mut();
            for a in arg_types.iter_mut() {
                f(a);
            }
            for v in variables.iter_mut() {
                f(v);
            }
            f(upper_bound);
            f(default);
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            f(tuple_fallback);
            f(upper_bound);
            f(default);
        }
        Type::UnpackType { typ } => f(typ),
        Type::CallableType {
            fallback,
            instance_type,
            arg_types,
            ret_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            f(fallback);
            if let Some(it) = instance_type {
                f(it);
            }
            for a in arg_types.iter_mut() {
                f(a);
            }
            f(ret_type);
            for v in variables.iter_mut() {
                f(v);
            }
            if let Some(g) = type_guard {
                f(g);
            }
            if let Some(i) = type_is {
                f(i);
            }
        }
        Type::Overloaded { items } => {
            for it in items.iter_mut() {
                f(it);
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            f(partial_fallback);
            for it in items.iter_mut() {
                f(it);
            }
        }
        Type::TypedDictType {
            fallback, items, ..
        } => {
            f(fallback);
            for (_, it) in items.iter_mut() {
                f(it);
            }
        }
        Type::LiteralType { fallback, .. } => f(fallback),
        Type::UnionType { items, .. } => {
            for it in items.iter_mut() {
                f(it);
            }
        }
        Type::TypeType { item, .. } => f(item),
        Type::Parameters(params) => {
            for a in params.arg_types.iter_mut() {
                f(a);
            }
            for v in params.variables.iter_mut() {
                f(v);
            }
        }
        Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. } => {}
    }
}

/// Shared map-then-expand tail of `analyze_instance_member_access`
/// (checkmember.py:773-775) for static and trivial-self methods, and the
/// member-access dispatch: `map_instance_to_supertype` +
/// `expand_type_by_instance`, with an optional trivial-self bind.
///
/// A static signature is never bound; a trivial-self signature is bound via
/// `bind_self_fast` (checkmember.py:704-705), which only drops the first
/// argument and sets `is_bound` — no `__self__`/`__cls__` identity is
/// involved, so Rust can mirror it after expansion instead of before.
/// Expanding first on the unbound callable avoids the `is_bound` deferral
/// in `expand_type_inner`; binding then only trims `arg_types[1:]`, which
/// expansion does not touch semantically (a trivial self carries no type
/// variables by construction). Returns `None` (Python falls through) for:
///   * a non-Instance `typ`
///   * an Overloaded signature when `allow_overloaded` is false (the
///     legacy static seam maps overloads in Python)
///   * a missing resolver snapshot / unresolvable derivation path
///   * a mapped instance with empty args or a TVT class (expand defers)
///   * a ParamSpec/Unpack signature (expand defers)
///   * an expansion whose result still carries a TypeAliasType (wire
///     round-trip decodes it with `alias=None`, types.py:397)
///   * a non-Callable trivial-self signature (bind_self_fast_inner's None:
///     Overloaded with zero items); a zero-arg or *args/**kwargs callable is
///     returned unchanged by bind_self_fast, and Rust mirrors that as the
///     unchanged callable, not a deferral
///
/// Python's `freeze_all_type_vars` (typeops.py:2102) sets `meta_level = 0`
/// on the `variables` entries of every callable in the result; shared-object
/// mutation freezes all occurrences. The wire round-trip breaks sharing, so
/// the tail rewrites by id: collect the ids of every callable's `variables`,
/// then set `meta_level = 0` on the matching occurrences. Leftover tvars
/// ride through only when they are bound (`meta_level == 0`) occurrences in
/// the mapped receiver's args (the caller-tvar substitution class the cold
/// corpus showed); the narrowed survivor gate (#1277) defers everything
/// else, whose decoded round-trip broke downstream inference in 28 testcheck
/// defenses, and a bare fresh var decoded from the tail cannot be re-linked
/// to the caller's live unification variable across seams (#1286).
#[allow(clippy::too_many_arguments)]
fn static_member_tail(
    instance: &Type,
    signature: &Type,
    method_fullname: &str,
    strict_optional: bool,
    resolver: &TypeResolver,
    is_trivial: bool,
    allow_overloaded: bool,
    tuple_special: Option<Option<&Type>>,
) -> Option<Type> {
    let (left_ref, left_args) = match instance {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    // `map_instance_to_supertype` walks map_derivation_path and returns
    // None for a non-base receiver, deferring to Python exactly as the
    // old exact-class guard did; subclass receivers now map natively.
    let ok = match signature {
        Type::CallableType { .. } => true,
        Type::Overloaded { .. } => allow_overloaded,
        _ => false,
    };
    if !ok {
        return None;
    }
    // Defer `builtins.tuple` methods unless the dispatch head already
    // decided the maptype.py:326-345 special case. A `None` here is the
    // legacy seam path, which maps overloads in Python anyway.
    if method_fullname == "builtins.tuple" && tuple_special.is_none() {
        return None;
    }
    // checkmember.py:450 `typ = map_instance_to_supertype(typ, method.info)`.
    let mapped_instance = match tuple_special {
        Some(Some(pre)) => pre.clone(),
        _ => {
            let mapped_args = match crate::subtypes::map_instance_to_supertype(
                left_ref,
                left_args,
                method_fullname,
                resolver,
            ) {
                Some(a) => a,
                None => {
                    return None;
                }
            };
            Type::Instance {
                type_ref: method_fullname.to_string(),
                args: mapped_args,
                last_known_value: None,
                extra_attrs: None,
            }
        }
    };
    // checkmember.py:451 `expand_type_by_instance(signature, typ)`. Expand
    // the unbound callable first (binding would defer the expand). The free-result
    // variant mirrors Python (`freeze_all_type_vars` on return).
    let mut expanded = crate::expandtype::expand_type_by_instance_free(
        signature,
        &mapped_instance,
        resolver,
        strict_optional,
    )?;
    // checkmember.py:503 `freeze_all_type_vars(member_type)`. First the
    // narrowed survivor gate (#1277): bound receiver-arg tvars ride through,
    // any other leftover defers (fresh riders cannot cross seams, #1286).
    let mut recv_keys: Vec<BindTVarKey> = Vec::new();
    if let Type::Instance { args, .. } = &mapped_instance {
        for a in args {
            collect_tvar_keys(a, &mut recv_keys);
        }
    }
    let mut ids: Vec<(i64, i64, String)> = Vec::new();
    collect_freeze_ids(&mut expanded, &mut ids);
    if !survivors_allowed(&expanded, &ids, &recv_keys) {
        return None;
    }
    apply_freeze(&mut expanded, &ids);
    if is_trivial {
        bind_self_fast_inner(&expanded)
    } else {
        Some(expanded)
    }
}

/// Legacy M20 seam for a static or trivial-self `FuncDef` method
/// (checkmember.py:670-721). Overloaded signatures defer to Python,
/// preserving the seam's pre-dispatch behavior.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_analyze_instance_member_access(
    resolver: &NativeTypeResolver,
    instance_bytes: &[u8],
    signature_bytes: &[u8],
    method_fullname: &str,
    strict_optional: bool,
    is_trivial_self: bool,
) -> Option<Vec<u8>> {
    let instance = decode_type(instance_bytes)?;
    let signature = decode_type(signature_bytes)?;
    let result = static_member_tail(
        &instance,
        &signature,
        method_fullname,
        strict_optional,
        resolver.resolver(),
        is_trivial_self,
        false,
        None,
    )?;
    encode_type(&result)
}

// ---------------------------------------------------------------------------
// bind_self generic self-solve (issue #1214)
// ---------------------------------------------------------------------------

type BindTVarKey = (i64, i64, String);

fn contains_tvar_like(typ: &Type) -> bool {
    match typ {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            true
        }
        Type::Instance { args, .. } => args.iter().any(contains_tvar_like),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            arg_types.iter().any(contains_tvar_like)
                || contains_tvar_like(ret_type)
                || instance_type.as_deref().is_some_and(contains_tvar_like)
                || type_guard.as_deref().is_some_and(contains_tvar_like)
                || type_is.as_deref().is_some_and(contains_tvar_like)
                || variables.iter().any(contains_tvar_like)
        }
        Type::Overloaded { items } => items.iter().any(contains_tvar_like),
        Type::UnionType { items, .. } => items.iter().any(contains_tvar_like),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => items.iter().any(contains_tvar_like) || contains_tvar_like(partial_fallback),
        Type::TypedDictType {
            items, fallback, ..
        } => items.iter().any(|(_, t)| contains_tvar_like(t)) || contains_tvar_like(fallback),
        Type::TypeType { item, .. } => contains_tvar_like(item),
        Type::UnpackType { typ } => contains_tvar_like(typ),
        Type::Parameters(p) => p.arg_types.iter().any(contains_tvar_like),
        _ => false,
    }
}

/// Per-item plan of `bind_self`'s generic solve branch (typeops.py:1062):
/// `self_vars` are the method tvars in the item's self param, `bare` the
/// subset replaced by the receiver; None defers, star-self/empty empty.
struct ItemBindPlan {
    self_vars: Vec<BindTVarKey>,
    bare: Vec<BindTVarKey>,
}

fn plan_item_bind(item: &Type) -> Option<ItemBindPlan> {
    let (arg_types, arg_kinds, variables) = match item {
        Type::CallableType {
            arg_types,
            arg_kinds,
            variables,
            ..
        } => (arg_types, arg_kinds, variables),
        _ => return None,
    };
    let empty = ItemBindPlan {
        self_vars: vec![],
        bare: vec![],
    };
    let Some(self_t) = arg_types.first() else {
        return Some(empty);
    };
    match arg_kinds.first() {
        Some(&kind) if kind == ARG_STAR || kind == ARG_STAR2 => return Some(empty),
        _ => {}
    }
    if crate::expandtype::result_contains_typealias(self_t) {
        return None;
    }
    let mut self_ids: HashSet<BindTVarKey> = HashSet::new();
    collect_bind_self_ids(self_t, &mut self_ids);
    if self_ids.is_empty() {
        return Some(empty);
    }
    let mut self_vars: Vec<BindTVarKey> = vec![];
    for v in variables {
        match v {
            Type::TypeVarType {
                raw_id,
                meta_level,
                namespace,
                ..
            } => {
                let key = (*raw_id, *meta_level, namespace.clone());
                if self_ids.contains(&key) {
                    self_vars.push(key);
                }
            }
            Type::ParamSpecType {
                raw_id, namespace, ..
            }
            | Type::TypeVarTupleType {
                raw_id, namespace, ..
            } => {
                let key = (*raw_id, -1, namespace.clone());
                if self_ids.contains(&key) {
                    self_vars.push(key);
                }
            }
            _ => {}
        }
    }
    let bare = match self_t {
        // Only a plain TypeVarType self param is substituted with the
        // receiver; nested tvars were bound by the expand from the mapped
        // receiver args, exactly as Python's solve inference derives them.
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => {
            let key = (*raw_id, *meta_level, namespace.clone());
            if self_vars.contains(&key) {
                vec![key]
            } else {
                vec![]
            }
        }
        _ => vec![],
    };
    Some(ItemBindPlan { self_vars, bare })
}

/// Tvar id collection for the self-solve plan (TypeVarExtractor semantics
/// over the wire walk; ParamSpec/TypeVarTuple entries use the -1
/// meta-level sentinel, matching the plan's variable-key build).
fn collect_bind_self_ids(typ: &Type, out: &mut HashSet<BindTVarKey>) {
    match typ {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => {
            out.insert((*raw_id, *meta_level, namespace.clone()));
        }
        Type::ParamSpecType {
            raw_id, namespace, ..
        }
        | Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => {
            out.insert((*raw_id, -1, namespace.clone()));
        }
        Type::Instance { args, .. } => {
            for a in args {
                collect_bind_self_ids(a, out);
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            for a in arg_types {
                collect_bind_self_ids(a, out);
            }
            collect_bind_self_ids(ret_type, out);
            if let Some(it) = instance_type {
                collect_bind_self_ids(it, out);
            }
        }
        Type::Overloaded { items } => {
            for i in items {
                collect_bind_self_ids(i, out);
            }
        }
        Type::UnionType { items, .. } => {
            for i in items {
                collect_bind_self_ids(i, out);
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            collect_bind_self_ids(partial_fallback, out);
            for i in items {
                collect_bind_self_ids(i, out);
            }
        }
        Type::TypedDictType {
            fallback, items, ..
        } => {
            collect_bind_self_ids(fallback, out);
            for (_, t) in items {
                collect_bind_self_ids(t, out);
            }
        }
        Type::TypeType { item, .. } => collect_bind_self_ids(item, out),
        Type::UnpackType { typ } => collect_bind_self_ids(typ, out),
        Type::Parameters(p) => {
            for a in &p.arg_types {
                collect_bind_self_ids(a, out);
            }
        }
        _ => {}
    }
}

/// Substitute the bare self-var keys with `val` everywhere Python's solve
/// expand_type reaches: arg/ret types, type_guard/type_is, instance_type,
/// nested callables; variables/fallback excluded (expandtype.py:1158-1171).
fn subst_tvar_keys(typ: &mut Type, keys: &[BindTVarKey], val: &Type) {
    if let Type::TypeVarType {
        raw_id,
        meta_level,
        namespace,
        ..
    } = typ
    {
        if keys
            .iter()
            .any(|(r, m, ns)| *r == *raw_id && *m == *meta_level && ns == namespace)
        {
            *typ = val.clone();
        }
        return;
    }
    match typ {
        Type::Instance { args, .. } => {
            for a in args {
                subst_tvar_keys(a, keys, val);
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            type_guard,
            type_is,
            ..
        } => {
            for a in arg_types {
                subst_tvar_keys(a, keys, val);
            }
            subst_tvar_keys(ret_type, keys, val);
            if let Some(it) = instance_type.as_deref_mut() {
                subst_tvar_keys(it, keys, val);
            }
            if let Some(g) = type_guard.as_deref_mut() {
                subst_tvar_keys(g, keys, val);
            }
            if let Some(i) = type_is.as_deref_mut() {
                subst_tvar_keys(i, keys, val);
            }
        }
        Type::Overloaded { items } => {
            for item in items {
                subst_tvar_keys(item, keys, val);
            }
        }
        Type::UnionType { items, .. } => {
            for item in items {
                subst_tvar_keys(item, keys, val);
            }
        }
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            subst_tvar_keys(partial_fallback, keys, val);
            for item in items {
                subst_tvar_keys(item, keys, val);
            }
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            subst_tvar_keys(fallback, keys, val);
            for (_, t) in items.iter_mut() {
                subst_tvar_keys(t, keys, val);
            }
        }
        Type::TypeType { item, .. } => subst_tvar_keys(item, keys, val),
        Type::UnpackType { typ } => subst_tvar_keys(typ, keys, val),
        Type::Parameters(p) => {
            for a in &mut p.arg_types {
                subst_tvar_keys(a, keys, val);
            }
        }
        _ => {}
    }
}

/// Remove the solved self-var entries from the item's `variables` list
/// (Python: `variables = [v for v in func.variables if v not in self_vars]`).
fn remove_self_vars(item: &mut Type, keys: &[BindTVarKey]) {
    if let Type::CallableType { variables, .. } = item {
        variables.retain(|v| match v {
            Type::TypeVarType {
                raw_id,
                meta_level,
                namespace,
                ..
            } => {
                let key = (*raw_id, *meta_level, namespace.clone());
                !keys.contains(&key)
            }
            Type::ParamSpecType {
                raw_id, namespace, ..
            }
            | Type::TypeVarTupleType {
                raw_id, namespace, ..
            } => {
                let key = (*raw_id, -1, namespace.clone());
                !keys.contains(&key)
            }
            _ => true,
        });
    }
}

/// Freeze the freshened method typevars of one item (typeops.py:2102
/// `freeze_all_type_vars` mirror): collect the `variables`-declared ids,
/// zero their meta levels everywhere in the tree; leftovers are gated by
/// `survivors_allowed` against the mapped receiver's args.
fn freeze_item(mut item: Type, recv_keys: &[BindTVarKey]) -> Option<Type> {
    let mut freeze_ids: Vec<(i64, i64, String)> = Vec::new();
    collect_freeze_ids(&mut item, &mut freeze_ids);
    if !survivors_allowed(&item, &freeze_ids, recv_keys) {
        return None;
    }
    apply_freeze(&mut item, &freeze_ids);
    Some(item)
}

/// Non-trivial-instance-method tail of `analyze_instance_member_access`
/// (checkmember.py:717-731): receiver-validated bind + map + expand in one
/// wire call, deferring on Python's object semantics. `suppress_self_fail`
/// is set in the error-suppressed protocol member-fetch contexts where a
/// zero-match self filter keeps binding Python-side.
#[allow(clippy::too_many_arguments)]
pub(crate) fn member_method_inner(
    instance: &Type,
    signature: &Type,
    method_fullname: &str,
    self_type: &Type,
    name: &str,
    resolver: &TypeResolver,
    strict_optional: bool,
    is_class: bool,
    suppress_self_fail: bool,
) -> Option<Type> {
    // Class methods: bind_self's non-generic strip path is
    // is_classmethod-agnostic, so the same strip is valid here; the
    // generic path needs the TypeType-wrap, so defer.
    let has_vars = matches!(
        signature,
        Type::CallableType { variables, .. } if !variables.is_empty()
    ) || matches!(
        signature,
        Type::Overloaded { items } if items.iter().any(
            |it| matches!(it, Type::CallableType { variables, .. } if !variables.is_empty())
        )
    );
    if is_class && has_vars {
        return None;
    }
    let (left_ref, left_args) = match instance {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => {
            return None;
        }
    };
    // checkmember.py:769 self filter: a zero match defers so Python emits
    // `incompatible_self_argument`; in error-suppressed find_member contexts
    // Python keeps the original functype (callers pass suppress_self_fail=true).
    let filtered = match check_self_arg_inner(
        signature,
        self_type,
        is_class,
        name,
        strict_optional,
        resolver,
        suppress_self_fail,
    ) {
        Some(f) => f,
        None => {
            return None;
        }
    };
    // checkmember.py:728 `typ = map_instance_to_supertype(typ, method.info)`.
    let mapped_args = match crate::subtypes::map_instance_to_supertype(
        left_ref,
        left_args,
        method_fullname,
        resolver,
    ) {
        Some(a) => a,
        None => {
            return None;
        }
    };
    let mapped_instance = Type::Instance {
        type_ref: method_fullname.to_string(),
        args: mapped_args,
        last_known_value: None,
        extra_attrs: None,
    };
    // checkmember.py:729-731 runs bind_self first (generic solve,
    // typeops.py:1062-1108) and expands afterwards; the plans below gate
    // the solve outcome on the expand tail and defer unbuildable items.
    let bind_plans: Vec<ItemBindPlan> = match &filtered {
        Type::CallableType { .. } => vec![plan_item_bind(&filtered)?],
        Type::Overloaded { items } => {
            if items.is_empty() {
                return None;
            }
            let mut plans = Vec::with_capacity(items.len());
            for it in items {
                plans.push(plan_item_bind(it)?);
            }
            plans
        }
        _ => return None,
    };
    // A bare-Self plan injects the receiver wholesale, while Python solves
    // against the receiver first; when the receiver itself carries type
    // vars the two orders can diverge, so defer.
    if bind_plans.iter().any(|p| !p.bare.is_empty()) && contains_tvar_like(self_type) {
        return None;
    }
    let expanded = match crate::expandtype::expand_type_by_instance_free(
        &filtered,
        &mapped_instance,
        resolver,
        strict_optional,
    ) {
        Some(e) => e,
        None => {
            return None;
        }
    };
    let mut recv_keys: Vec<BindTVarKey> = Vec::new();
    if let Type::Instance { args, .. } = &mapped_instance {
        for a in args {
            collect_tvar_keys(a, &mut recv_keys);
        }
    }
    // bind_self (strip tail): the self-arg filter kept only receiver-
    // compatible items, so `bind_self_fast_inner` strips and sets
    // `is_bound`; leftover typevars after bare-subst + freeze defer.
    let bound_items: Vec<Type> = match expanded {
        Type::Overloaded { items } => {
            if items.len() != bind_plans.len() {
                return None;
            }
            let mut new_items = Vec::with_capacity(items.len());
            for (mut item, plan) in items.into_iter().zip(bind_plans) {
                subst_tvar_keys(&mut item, &plan.bare, self_type);
                remove_self_vars(&mut item, &plan.self_vars);
                let item = match freeze_item(item, &recv_keys) {
                    Some(i) => i,
                    None => {
                        return None;
                    }
                };
                new_items.push(bind_self_fast_inner(&item)?);
            }
            new_items
        }
        item @ Type::CallableType { .. } => {
            let plan = bind_plans.into_iter().next()?;
            let mut item = item;
            subst_tvar_keys(&mut item, &plan.bare, self_type);
            remove_self_vars(&mut item, &plan.self_vars);
            let item = match freeze_item(item, &recv_keys) {
                Some(i) => i,
                None => {
                    return None;
                }
            };
            vec![bind_self_fast_inner(&item)?]
        }
        _ => return None,
    };
    if bound_items.len() == 1 && !matches!(filtered, Type::Overloaded { .. }) {
        return bound_items.into_iter().next();
    }
    Some(Type::Overloaded { items: bound_items })
}

/// `#[pyfunction]` entry: `rust_analyze_member_method`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_analyze_member_method(
    resolver: &NativeTypeResolver,
    instance_bytes: &[u8],
    signature_bytes: &[u8],
    method_fullname: &str,
    self_type_bytes: &[u8],
    name: &str,
    strict_optional: bool,
    is_class: bool,
) -> Option<Vec<u8>> {
    let instance = decode_type(instance_bytes)?;
    let signature = decode_type(signature_bytes)?;
    let self_type = decode_type(self_type_bytes)?;
    let result = member_method_inner(
        &instance,
        &signature,
        method_fullname,
        &self_type,
        name,
        resolver.resolver(),
        strict_optional,
        is_class,
        false,
    )?;
    encode_type(&result)
}

// analyze_instance_member_access (method-branch dispatch, issue #805)
// ---------------------------------------------------------------------------

/// Live `get_method` result: a function node (FuncBase) or a `Decorator`.
/// The dispatch only handles FuncBase; decorated methods defer to Python.
enum LiveMethod {
    FuncBase(Py<PyAny>),
    Decorator,
}

/// Mirror `TypeInfo.get_method` (nodes.py:4167-4183) on a live `TypeInfo`:
/// walk its MRO, prefer the exact name, then the last sorted
/// `name}-redefinition` entry, and return the node only when it is a
/// `FuncDef`/`OverloadedFuncDef` (SYMBOL_FUNCBASE_TYPES) or a `Decorator`.
/// A found-but-non-function node stops the walk, matching Python. Returns
/// `None` for both "not a method" and any read failure — the dispatch
/// defers either way.
fn get_method_live(py: Python<'_>, info: &PyAny, name: &str) -> Option<LiveMethod> {
    let nodes_mod = py.import("mypy.nodes").ok()?;
    let func_def_cls = nodes_mod.getattr("FuncDef").ok()?;
    let overloaded_cls = nodes_mod.getattr("OverloadedFuncDef").ok()?;
    let decorator_cls = nodes_mod.getattr("Decorator").ok()?;
    let mro = info.getattr("mro").ok()?.downcast::<PyList>().ok()?;
    let redefinition_prefix = format!("{name}-redefinition");
    for cls in mro.iter() {
        let names = cls.getattr("names").ok()?.downcast::<PyDict>().ok()?;
        let node = if let Some(entry) = names.get_item(name).ok()? {
            entry.getattr("node").ok()?
        } else {
            // sorted([n for n in cls.names.keys()
            //         if n.startswith(f"{name}-redefinition")])[-1]
            let mut redefs: Vec<String> = Vec::new();
            for key in names.keys() {
                if let Ok(key) = key.extract::<String>() {
                    if key.starts_with(&redefinition_prefix) {
                        redefs.push(key);
                    }
                }
            }
            if redefs.is_empty() {
                continue;
            }
            redefs.sort();
            names
                .get_item(&redefs[redefs.len() - 1])
                .ok()??
                .getattr("node")
                .ok()?
        };
        if node.is_none() {
            return None;
        }
        // isinstance(node, SYMBOL_FUNCBASE_TYPES) — two explicit checks
        // instead of a tuple arg (PyO3 has no tuple `isinstance`).
        if node.is_instance(func_def_cls).ok()? || node.is_instance(overloaded_cls).ok()? {
            return Some(LiveMethod::FuncBase(node.into()));
        }
        if node.is_instance(decorator_cls).ok()? {
            return Some(LiveMethod::Decorator);
        }
        return None; // found-but-non-func node stops the walk
    }
    None
}

/// Live-attribute read that defers (None) on any failure, mirroring the
/// checker_helpers helper. `None` values read as `false` (Python treats
/// None/absent flags as False in the method branch).
fn get_bool_flag(py: Python<'_>, node: &PyAny, name: &str) -> Option<bool> {
    let v = node.getattr(name).ok()?;
    if v.is_none() {
        return Some(false);
    }
    if let Ok(b) = v.extract::<bool>() {
        return Some(b);
    }
    if let Ok(b) = v.downcast::<pyo3::types::PyBool>() {
        return Some(b.is_true());
    }
    if let Ok(i) = v.extract::<i64>() {
        return Some(i != 0);
    }
    // A non-bool, non-int object: Python truthiness is not decidable here.
    let _ = py;
    None
}

/// Freshen a method signature: `freshen_type` handles CallableType;
/// Overloaded is freshened per item with the shared raw-id counter
/// (mirrors FreshenCallableVisitor over an Overloaded). Any item that
/// defers defers the whole signature.
fn freshen_signature(
    signature: &Type,
    next_raw_id: &mut i64,
    changed: &mut bool,
    strict_optional: bool,
) -> Option<Type> {
    match signature {
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                let it = match freshen_type(item, next_raw_id, changed, strict_optional) {
                    Some(t) => t,
                    None => {
                        return None;
                    }
                };
                new_items.push(it);
            }
            Some(Type::Overloaded { items: new_items })
        }
        _ => freshen_type(signature, next_raw_id, changed, strict_optional),
    }
}

/// Wire-portable head of `analyze_instance_member_access`
/// (checkmember.py:607-776): live `get_method` lookup, flag reads,
/// `freshen_all_functions_type_vars`, and the static / trivial-self /
/// non-trivial tail dispatch. Returns `None` when any step needs Python
/// (decorated methods, properties, lvalues, `__init__` guard failures,
/// unresolvable live reads, unanalyzable signatures).
///
/// A `get_method` miss or a `Decorator` head falls through to the var arm
/// (`dispatch_var_member_inner`), mirroring Python's
/// `analyze_member_var_access` continuation.
#[allow(clippy::too_many_arguments)]
fn dispatch_instance_member_inner(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    instance: &Type,
    name: &str,
    override_info: Option<&str>,
    self_type: &Type,
    is_operator: bool,
    is_self: bool,
    preserve_type_var_ids: bool,
    next_raw_id: &mut i64,
    changed: &mut bool,
    strict_optional: bool,
    plugin: Option<&PyAny>,
) -> Option<Type> {
    let type_ref = match instance {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => {
            return None;
        }
    };
    // checkmember.py:610-612 `info = typ.type; if override_info: info = ...`
    let lookup_fullname = override_info.unwrap_or(type_ref);
    let info = match resolver.live_typeinfo(py, lookup_fullname) {
        Some(i) => i,
        None => {
            return None;
        }
    };
    if info.is_none() {
        return None; // present-but-None map entry
    }
    let method = match get_method_live(py, info, name) {
        Some(LiveMethod::FuncBase(node)) => node,
        // checkmember.py:951: a get_method miss (or a Decorator head)
        // continues into analyze_member_var_access; the var arm mirrors it.
        Some(LiveMethod::Decorator) | None => {
            return dispatch_var_member_inner(
                py,
                resolver,
                instance,
                name,
                info,
                is_operator,
                is_self,
                preserve_type_var_ids,
                next_raw_id,
                changed,
                strict_optional,
                plugin,
            );
        }
    };
    let method = method.as_ref(py);
    match get_bool_flag(py, method, "is_property") {
        Some(true) => {
            return None;
        }
        Some(false) => {}
        None => {
            return None;
        }
    }
    let is_static = match get_bool_flag(py, method, "is_static") {
        Some(b) => b,
        None => {
            return None;
        }
    };
    let is_trivial_self = match get_bool_flag(py, method, "is_trivial_self") {
        Some(b) => b,
        None => {
            return None;
        }
    };
    let is_class = match get_bool_flag(py, method, "is_class") {
        Some(b) => b,
        None => {
            return None;
        }
    };
    // checkmember.py:616-621 `__init__` guard: non-final class and method
    // (and not via super, already gated) defers so Python emits the error.
    if name == "__init__" {
        let info_final = match get_bool_flag(py, info, "is_final") {
            Some(b) => b,
            None => {
                return None;
            }
        };
        let method_final = match get_bool_flag(py, method, "is_final") {
            Some(b) => b,
            None => {
                return None;
            }
        };
        if !info_final && !method_final {
            return None;
        }
    }
    // checkmember.py:648-658 `method.info.fullname` + `function_type`
    // typed passthrough (`method.type`). A None type is a not-ready
    // overload or an unanalyzed function — defer to Python.
    let method_fullname: String = match method
        .getattr("info")
        .ok()
        .and_then(|i| i.getattr("fullname").ok())
        .and_then(|f| f.extract().ok())
    {
        Some(f) => f,
        None => {
            return None;
        }
    };
    let type_attr = match method.getattr("type") {
        Ok(t) => t,
        Err(_) => {
            return None;
        }
    };
    if type_attr.is_none() {
        return None;
    }
    let sig_bytes = match serialize_type_to_bytes(py, type_attr) {
        Some(b) => b,
        None => {
            return None;
        }
    };
    let mut signature = match decode_type(&sig_bytes) {
        Some(s) => s,
        None => {
            return None;
        }
    };
    if !matches!(
        signature,
        Type::CallableType { .. } | Type::Overloaded { .. }
    ) {
        return None;
    }
    // checkmember.py:659-660 `freshen_all_functions_type_vars(signature)`
    // unless `preserve_type_var_ids`.
    if !preserve_type_var_ids {
        signature = freshen_signature(&signature, next_raw_id, changed, strict_optional)?;
    }
    // checkmember.py:722-775 tail: static never binds, trivial-self binds
    // via bind_self_fast, otherwise the validated member_method_inner. A
    // `builtins.tuple` method defers if the map special case was undecided.
    let tuple_special = if method_fullname == "builtins.tuple" && (is_static || is_trivial_self) {
        match tuple_special_map(py, resolver, instance, strict_optional) {
            Some(r) => Some(r),
            None => {
                return None;
            }
        }
    } else {
        None
    };
    if is_static {
        static_member_tail(
            instance,
            &signature,
            &method_fullname,
            strict_optional,
            resolver.resolver(),
            false,
            true,
            tuple_special.as_ref().map(|r| r.as_ref()),
        )
    } else if is_trivial_self {
        static_member_tail(
            instance,
            &signature,
            &method_fullname,
            strict_optional,
            resolver.resolver(),
            true,
            true,
            tuple_special.as_ref().map(|r| r.as_ref()),
        )
    } else {
        member_method_inner(
            instance,
            &signature,
            &method_fullname,
            self_type,
            name,
            resolver.resolver(),
            strict_optional,
            is_class,
            false,
        )
    }
}

/// `#[pyfunction]` entry for the method-branch dispatch (issue #805).
#[pyfunction]
#[pyo3(signature = (resolver, instance_bytes, name, override_info, self_type_bytes,
                    _no_deferral, preserve_type_var_ids, start_raw_id, strict_optional))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_analyze_instance_member_dispatch(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    instance_bytes: &[u8],
    name: &str,
    override_info: Option<String>,
    self_type_bytes: &[u8],
    _no_deferral: bool,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
) -> Option<(i64, bool, Vec<u8>)> {
    let instance = match decode_type(instance_bytes) {
        Some(t) => t,
        None => {
            return None;
        }
    };
    let self_type = match decode_type(self_type_bytes) {
        Some(t) => t,
        None => {
            return None;
        }
    };
    let mut next_raw_id = start_raw_id;
    let mut changed = false;
    let result = dispatch_instance_member_inner(
        py,
        resolver,
        &instance,
        name,
        override_info.as_deref(),
        &self_type,
        // The Python gate requires a found non-property method, so the
        // var arm is unreachable from this entry point.
        false,
        false,
        preserve_type_var_ids,
        &mut next_raw_id,
        &mut changed,
        strict_optional,
        None,
    );
    let result = result?;
    Some((next_raw_id, changed, encode_type(&result)?))
}

/// Wire-portable var arm of the Instance member dispatch:
/// `analyze_member_var_access` + `analyze_var` (checkmember.py:1235,
/// 1759), the path Python takes when `get_method` misses or finds a
/// `Decorator` head. Called from `dispatch_instance_member_inner` at both
/// sites.
///
/// Handled by Rust: a plain `Var` hit and a non-deprecated `Decorator`
/// head (unwrapped to `.var`), the `expand_without_binding` expansion, the
/// class-var `call_type` arbitration for a single unbound CallableType,
/// the `plugin_hook_known_absent` fast-path, and the non-descriptor
/// pass-through of `analyze_descriptor_access`. Everything else defers
/// (`None`) to the pure-Python body: `__init__` (guard + fail),
/// deprecated decorators (`warn_deprecated` side effect), non-Var /
/// non-Decorator nodes (module refs, synthesized static-reference Vars),
/// enum classes (literal wrap / `enum.nonmember` unwrap), union or
/// Overloaded `call_type` item loops, the non-trivial bind-self path,
/// property-bearing `call_type` (Python's property-extract tail in
/// `expand_and_bind_callable`), self-type expansion, partial or not-ready
/// vars, a possible plugin hook, and descriptor `__get__`-bearing accesses.
#[allow(clippy::too_many_arguments)]
fn dispatch_var_member_inner(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    instance: &Type,
    name: &str,
    info: &PyAny,
    is_operator: bool,
    is_self: bool,
    preserve_type_var_ids: bool,
    next_raw_id: &mut i64,
    changed: &mut bool,
    strict_optional: bool,
    plugin: Option<&PyAny>,
) -> Option<Type> {
    // `mx.is_self` routes through expand_self_type / bind_self with live
    // Var state (checkmember.py:1898): defer to the Python body.

    // `mx.is_super` is excluded by the caller's non-super gate; keep
    // the guard symmetric.
    if is_self {
        return None;
    }
    // Python fails `__init__` access on a non-final class before the var
    // path runs (checkmember.py:738); keep the whole name in Python.
    if name == "__init__" {
        return None;
    }
    let nodes_mod = py.import("mypy.nodes").ok()?;
    let decorator_cls = nodes_mod.getattr("Decorator").ok()?;
    let var_cls = nodes_mod.getattr("Var").ok()?;
    // analyze_member_var_access: `node = info.get(name)`.
    let entry = info.call_method1("get", (name,)).ok()?;
    if entry.is_none() {
        return None;
    }
    let implicit = entry.getattr("implicit").ok()?.extract::<bool>().ok()?;
    let node = entry.getattr("node").ok()?;
    if node.is_none() {
        return None;
    }
    let is_trivial_self;
    let var = if node.is_instance(decorator_cls).ok()? {
        // Decorator head: `warn_deprecated` fires on `vv.func` and
        // defers; `v = vv.var`, `is_trivial_self = vv.func.is_trivial_self
        // and not vv.decorators` (checkmember.py:1252-1255).
        let func = node.getattr("func").ok()?;
        if !func.getattr("deprecated").ok()?.is_none() {
            return None;
        }
        is_trivial_self = get_bool_flag(py, func, "is_trivial_self")?
            && node.getattr("decorators").ok()?.len().ok()? == 0;
        node.getattr("var").ok()?
    } else if node.is_instance(var_cls).ok()? {
        is_trivial_self = false;
        node
    } else {
        return None;
    };
    if !var.is_instance(var_cls).ok()? {
        return None;
    }

    // analyze_var scalar facts (checkmember.py:1801-1823).
    let is_property = get_bool_flag(py, var, "is_property")?;
    let is_initialized_in_class = get_bool_flag(py, var, "is_initialized_in_class")?;
    let is_staticmethod = get_bool_flag(py, var, "is_staticmethod")?;
    let var_info = var.getattr("info").ok()?;
    let var_info_fullname: String = var_info.getattr("fullname").ok()?.extract().ok()?;
    let var_info_is_enum = get_bool_flag(py, var_info, "is_enum")?;
    let var_info_is_protocol = get_bool_flag(py, var_info, "is_protocol")?;
    if var_info_is_enum {
        return None;
    }
    // expand_without_binding native gate: `var.info.self_type is None or
    // var.is_property` (checkmember.py:1902); otherwise expand_self_type
    // needs the live Var.
    if !var_info.getattr("self_type").ok()?.is_none() && !is_property {
        return None;
    }

    // checkmember.py:1802 `itype = map_instance_to_supertype(itype,
    // var.info)`; a snapshot miss defers.
    let (inst_ref, inst_args) = match instance {
        Type::Instance { type_ref, args, .. } => (type_ref, args),
        _ => return None,
    };
    let mapped_args = crate::subtypes::map_instance_to_supertype(
        inst_ref,
        inst_args,
        &var_info_fullname,
        resolver.resolver(),
    )?;
    let itype = Type::Instance {
        type_ref: var_info_fullname.clone(),
        args: mapped_args,
        last_known_value: None,
        extra_attrs: None,
    };

    // `typ = var.type`; a None type (not-ready / implicit Any) and
    // PartialType inference defer (checkmember.py:1810-1812).
    let typ_obj = var.getattr("type").ok()?;
    if typ_obj.is_none() {
        return None;
    }
    let partial_cls = py.import("mypy.types").ok()?.getattr("PartialType").ok()?;
    if typ_obj.is_instance(partial_cls).ok()? {
        return None;
    }
    let typ = decode_type(&serialize_type_to_bytes(py, typ_obj)?)?;

    // checkmember.py:1819 `result = expand_without_binding(typ, var, ...)`.
    let (nri, ch, mut result) = expand_without_binding_inner(
        &typ,
        &itype,
        preserve_type_var_ids,
        *next_raw_id,
        strict_optional,
        resolver.resolver(),
    )?;
    *next_raw_id = nri;
    if ch {
        *changed = true;
    }

    // `is_instance_var(var)` (checkmember.py:1627) is consulted for the
    // class-var arbitration and the descriptor gate; computed lazily so
    // non-class, non-protocol hits do not pay for the interop call.
    let is_instance_var_flag = if is_initialized_in_class || var_info_is_protocol {
        Some(
            py.import("mypy.checkmember")
                .ok()?
                .getattr("is_instance_var")
                .ok()?
                .call1((var,))
                .ok()?
                .extract::<bool>()
                .ok()?,
        )
    } else {
        None
    };

    // checkmember.py:1822-1831 class-var callable arbitration.
    let mut call_type: Option<Type> = None;
    if is_initialized_in_class && (!is_instance_var_flag.unwrap_or(false) || is_operator) {
        let proper = get_proper_or_none(&typ)?;
        let is_func_non_typeobj = match proper {
            Type::CallableType {
                fallback, ret_type, ..
            } => !is_type_obj(fallback, ret_type, resolver.resolver()),
            Type::Overloaded { items } => match items.first() {
                Some(Type::CallableType {
                    fallback, ret_type, ..
                }) => !is_type_obj(fallback, ret_type, resolver.resolver()),
                _ => false,
            },
            _ => false,
        };
        if is_func_non_typeobj {
            call_type = Some(proper.clone());
        } else if is_property {
            // `__call__` recursion on the property type (checkmember.py:1827).
            return None;
        } else {
            call_type = Some(proper.clone());
        }
    }

    // checkmember.py:1840-1849 bound-method-alias loop. A UnionType
    // call_type iterates items with per-item bind decisions; an Overloaded
    // call_type iterates its items. Both defer.

    // A CallableType item binds only on the trivial-self path; the
    // non-trivial bind_self defers.
    if let Some(ct) = call_type {
        if !is_staticmethod {
            match &ct {
                Type::CallableType { is_bound, .. } => {
                    // `p_ct.bound()` for a CallableType is its `is_bound`
                    // flag (types.py:2048; items = [self]).
                    if is_property {
                        // Python's expand_and_bind_callable binds, then
                        // returns the getter ret_type or the setter arg;
                        // the inner port returns the raw CallableType, so defer.
                        return None;
                    }
                    let (nri, ch, t) = if !is_bound {
                        if !is_trivial_self {
                            return None;
                        }
                        expand_and_bind_callable_inner(
                            &ct,
                            &itype,
                            preserve_type_var_ids,
                            *next_raw_id,
                            strict_optional,
                            resolver.resolver(),
                        )?
                    } else {
                        expand_without_binding_inner(
                            &ct,
                            &itype,
                            preserve_type_var_ids,
                            *next_raw_id,
                            strict_optional,
                            resolver.resolver(),
                        )?
                    };
                    *next_raw_id = nri;
                    if ch {
                        *changed = true;
                    }
                    result = t;
                }
                _ => return None,
            }
        }
    }

    // checkmember.py:1862-1870 plugin hook gate. The registry fast path proves
    // absence for config-static DefaultPlugin names; otherwise the live plugin
    // chain is authoritative (user plugins are not enumerable); no handle defers.
    let fullname = format!("{}.{}", var_info_fullname, name);
    let fast_absent = py
        .import("mypy.checkexpr")
        .ok()?
        .getattr("plugin_hook_known_absent")
        .ok()?
        .call1(("get_attribute_hook", &fullname))
        .ok()?
        .extract::<bool>()
        .ok()?;
    if !fast_absent {
        let p = plugin?;
        match p.call_method1("get_attribute_hook", (&fullname,)) {
            Ok(hook) if hook.is_none() => {}
            _ => return None,
        }
    }

    // checkmember.py:1876-1877 descriptor pass-through: `not (implicit or
    // var.info.is_protocol and is_instance_var(var))`.
    if !(implicit || (var_info_is_protocol && is_instance_var_flag.unwrap_or(false))) {
        match analyze_descriptor_access_inner(&result, false, strict_optional, resolver.resolver())?
        {
            DescriptorDecision::Orig => {}
            DescriptorDecision::Value(t) => result = t,
        }
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// classify_member_access
// ---------------------------------------------------------------------------

/// Classify the `_analyze_member_access` dispatch branch from a wire-format
/// type. Returns an int code (MA_* constant) so Python can skip the
/// isinstance chain. Returns `None` (Python `None`) for `TypeAliasType`
/// (needs alias expansion) or decode failure.
///
/// Mirrors `_analyze_member_access` (checkmember.py:242-281). The
/// `resolver` is used to check `is_type_obj()` for FunctionLike (CallableType
/// / Overloaded whose fallback is a metaclass).
#[pyfunction]
pub(crate) fn rust_classify_member_access(
    resolver: &NativeTypeResolver,
    typ_bytes: &[u8],
) -> PyResult<Option<i64>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let proper = match get_proper_or_none(&typ) {
        Some(p) => p,
        None => return Ok(None),
    };
    Ok(Some(classify_member_access_inner(
        proper,
        resolver.resolver(),
    )))
}

fn classify_member_access_inner(typ: &Type, resolver: &TypeResolver) -> i64 {
    match typ {
        Type::Instance { .. } => MA_INSTANCE,
        Type::AnyType { .. } => MA_ANY,
        Type::UnionType { .. } => MA_UNION,
        Type::TypeType { .. } => MA_TYPE_TYPE,
        Type::TupleType { .. } => MA_TUPLE,
        Type::TypedDictType { .. } => MA_TYPEDDICT,
        Type::NoneType => MA_NONE,
        Type::DeletedType { .. } => MA_DELETED,
        Type::UninhabitedType { .. } => MA_UNINHABITED,
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            MA_TYPEVAR
        }
        Type::LiteralType { .. } => MA_LITERAL_OR_FUNC,
        Type::CallableType {
            fallback, ret_type, ..
        } => {
            if is_type_obj(fallback, ret_type, resolver) {
                MA_TYPE_CALLABLE
            } else {
                MA_LITERAL_OR_FUNC
            }
        }
        Type::Overloaded { items } => {
            // FunctionLike.is_type_obj() checks the first item.
            if let Some(Type::CallableType {
                fallback, ret_type, ..
            }) = items.first()
            {
                if is_type_obj(fallback, ret_type, resolver) {
                    MA_TYPE_CALLABLE
                } else {
                    MA_LITERAL_OR_FUNC
                }
            } else {
                MA_LITERAL_OR_FUNC
            }
        }
        // TypeAliasType is deferred in get_proper_or_none; unreachable here.
        // Parameters, UnpackType, UnboundType: fall through to MISSING.
        _ => MA_MISSING,
    }
}

// ---------------------------------------------------------------------------
// analyze_member_access (GENERAL dispatch path)
// ---------------------------------------------------------------------------

/// GENERAL dispatch for `_analyze_member_access` (checkmember.py:311-350).
///
/// Ports the pure-type-transform branches that do not need live Python
/// checker state, plugin hooks, or error reporting.  Returns `None`
/// (Python `None`) for cases that need Python state — the Python caller
/// falls through to the pure-Python implementation.
///
/// Handled by Rust:
///   * AnyType → AnyType(from_another_any, source_any=typ).  Pure
///     reconstruction; `from_another_any` = 7 (TypeOfAny value in types.py).
///   * UninhabitedType → new UninhabitedType preserving `ambiguous`.
///   * TupleType → recurse on the fallback instance.
///   * LiteralType / CallableType / Overloaded → recurse on fallback,
///     deferring when `is_type_obj` (needs class-level lookup).
///   * TypeVarType (no values) / ParamSpecType / TypeVarTupleType → recurse
///     on `upper_bound` / `tuple_fallback` (matches Python's
///     TypeVarLikeType branch).
///   * TypeAliasType → defer (wire format carries no alias target).
///
/// Deferred to Python (return None):
///   * Instance → dispatched through the native method + var arms, which
///     defer back on any fact they cannot decide.
///   * UnionType / TypeType / TypedDictType / NoneType / DeletedType
///     → need analyzer state, `mx`, or error reporting.  Rust must not drop a
///     diagnostic (e.g. `deleted_as_rvalue`) or mis-answer (`__bool__`).
///   * TypeVarType with values → needs `make_simplified_union`.
///   * UnboundType / Parameters / UnpackType → needs `report_missing_attribute`.
#[pyfunction]
#[pyo3(signature = (resolver, name, typ_bytes, self_type_bytes, is_lvalue, is_super,
                    is_operator, is_self, preserve_type_var_ids, start_raw_id,
                    strict_optional, plugin=None))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_analyze_member_access(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    name: &str,
    typ_bytes: &[u8],
    self_type_bytes: &[u8],
    is_lvalue: bool,
    is_super: bool,
    is_operator: bool,
    is_self: bool,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
    plugin: Option<&PyAny>,
) -> Option<(i64, bool, Vec<u8>)> {
    let typ = decode_type(typ_bytes)?;
    let self_type = decode_type(self_type_bytes)?;
    let mut next_raw_id = start_raw_id;
    let mut changed = false;
    let mut ctx = MemberAccessCtx {
        py,
        resolver,
        name,
        self_type: &self_type,
        is_lvalue,
        is_super,
        is_operator,
        is_self,
        preserve_type_var_ids,
        next_raw_id: &mut next_raw_id,
        changed: &mut changed,
        strict_optional,
        plugin,
    };
    let result = analyze_member_access_inner(&typ, Some(&mut ctx), resolver.resolver())?;
    Some((next_raw_id, changed, encode_type(&result)?))
}

/// Call-site context for `analyze_member_access_inner`: the dispatch data
/// needed to route an `Instance` fallback target through the native method
/// branch. Carried as a token so the pure type-transform branches do not
/// pay for it. When the caller only has a resolver (the union-item and
/// NoneType recursion paths, which Python keeps in Python), the context is
/// `None` and an `Instance` fallback defers as before.
struct MemberAccessCtx<'a> {
    py: Python<'a>,
    resolver: &'a NativeTypeResolver,
    name: &'a str,
    self_type: &'a Type,
    is_lvalue: bool,
    is_super: bool,
    is_operator: bool,
    is_self: bool,
    preserve_type_var_ids: bool,
    next_raw_id: &'a mut i64,
    changed: &'a mut bool,
    strict_optional: bool,
    /// Live plugin chain (`mx.chk.plugin`) for the attribute-hook verdict;
    /// `None` when the caller has no plugin (suite calls, union/none
    /// seams), in which case the var hook gate defers as before.
    plugin: Option<&'a PyAny>,
}

fn analyze_member_access_inner<'a>(
    typ: &Type,
    mut ctx: Option<&mut MemberAccessCtx<'a>>,
    resolver: &TypeResolver,
) -> Option<Type> {
    match typ {
        // --- Instance ---
        Type::Instance { .. } => {
            // Python: analyze_instance_member_access. With a ctx, route
            // rvalue/non-super Instance operands (incl. fallback recursion)

            // through the native method branch (with the var-arm fallback
            // on a get_method miss); lvalue/super stay in Python.
            if let Some(ctx) = ctx.as_deref_mut() {
                if !ctx.is_lvalue && !ctx.is_super {
                    let r = dispatch_instance_member_inner(
                        ctx.py,
                        ctx.resolver,
                        typ,
                        ctx.name,
                        None,
                        ctx.self_type,
                        ctx.is_operator,
                        ctx.is_self,
                        ctx.preserve_type_var_ids,
                        ctx.next_raw_id,
                        ctx.changed,
                        ctx.strict_optional,
                        ctx.plugin,
                    )?;
                    return Some(r);
                }
            }
            None
        }
        // --- AnyType ---
        Type::AnyType {
            type_of_any: _,
            source_any: _,
            missing_import_name: _,
        } => {
            // Python: AnyType(TypeOfAny.from_another_any, source_any=typ)
            // from_another_any = 7 per types.py; always set regardless of
            // the input's type_of_any (Python hardcodes it, does not copy).
            Some(Type::AnyType {
                type_of_any: 7, // TypeOfAny.from_another_any
                source_any: Some(Box::new(typ.clone())),
                missing_import_name: None,
            })
        }
        // --- UnionType ---
        Type::UnionType { .. } => {
            // Needs analyze_union_member_access (needs mx for disable_type_names).
            None
        }
        // --- TypeType ---
        Type::TypeType { .. } => {
            // Needs analyze_type_type_member_access (needs mx, override_info).
            None
        }
        // --- TupleType ---
        Type::TupleType { .. } => {
            // Python: _analyze_member_access(name, tuple_fallback(typ), mx).
            // tuple_fallback recomputes args from the items when the partial
            // fallback is builtins.tuple; the wire's `tuple[Any, ...]` erased it.
            let target = match crate::typeops::tuple_fallback(typ, resolver) {
                Some(t @ Type::Instance { .. }) => t,
                _ => return None,
            };
            analyze_member_access_inner(&target, ctx.as_deref_mut(), resolver)
        }
        // --- TypedDictType ---
        Type::TypedDictType { .. } => {
            // Needs analyze_typeddict_access.
            None
        }
        // --- NoneType ---
        Type::NoneType => {
            // Defer to Python: analyze_none_member_access special-cases
            // `__bool__` -> Literal[False]; non-bool names recurse on
            // builtins.object.  Both need mx / named_type.  Returning

            // builtins.object unconditionally would mis-answer `__bool__`.
            None
        }
        // --- TypeVarType ---
        Type::TypeVarType {
            values,
            upper_bound,
            ..
        } => {
            if !values.is_empty() {
                // Python: make_simplified_union(typ.values), mx.
                // We cannot build a union without knowing how to join; defer.
                None
            } else {
                // Python: _analyze_member_access(name, typ.upper_bound, mx).
                analyze_member_access_inner(upper_bound, ctx.as_deref_mut(), resolver)
            }
        }
        // --- ParamSpecType ---
        Type::ParamSpecType { upper_bound, .. } => {
            // Python: TypeVarLikeType -> _analyze_member_access(name, typ.upper_bound, mx).
            analyze_member_access_inner(upper_bound, ctx.as_deref_mut(), resolver)
        }
        // --- TypeVarTupleType ---
        Type::TypeVarTupleType { tuple_fallback, .. } => {
            // No upper_bound for TypeVarTuple; fall back to tuple_fallback.
            analyze_member_access_inner(tuple_fallback, ctx.as_deref_mut(), resolver)
        }
        // --- DeletedType ---
        Type::DeletedType { .. } => {
            // Defer to Python: Python reports `deleted_as_rvalue` unless
            // mx.suppress_errors, then returns AnyType(from_error).  Rust
            // cannot know suppress_errors and must not drop the diagnostic.
            None
        }
        // --- UninhabitedType ---
        Type::UninhabitedType { ambiguous } => {
            // Python: new UninhabitedType with same ambiguous flag.
            Some(Type::UninhabitedType {
                ambiguous: *ambiguous,
            })
        }
        // --- LiteralType ---
        Type::LiteralType { fallback, .. } => {
            // Python: _analyze_member_access(name, typ.fallback, mx).
            analyze_member_access_inner(fallback, ctx.as_deref_mut(), resolver)
        }
        // --- CallableType ---
        Type::CallableType {
            fallback, ret_type, ..
        } => {
            // Python: analyze_type_callable_member_access when is_type_obj(),
            // else recurse on fallback. Type objects need class-level
            // lookup (mx, override_info), so defer when is_type_obj.
            if is_type_obj(fallback, ret_type, resolver) {
                None
            } else {
                // Python: _analyze_member_access(name, typ.fallback, mx).
                analyze_member_access_inner(fallback, ctx.as_deref_mut(), resolver)
            }
        }
        // --- Overloaded ---
        Type::Overloaded { items } => {
            if items.is_empty() {
                // Degenerate; defer.
                None
            } else {
                // Python iterates items, returns first non-None result.
                for item in items {
                    if let Type::CallableType {
                        fallback, ret_type, ..
                    } = item
                    {
                        if is_type_obj(fallback, ret_type, resolver) {
                            continue;
                        }
                        if matches!(&**fallback, Type::Instance { .. }) {
                            // Instance target would defer inside the
                            // recursion for this item; keep looking.
                            continue;
                        }
                        if let Some(r) =
                            analyze_member_access_inner(fallback, ctx.as_deref_mut(), resolver)
                        {
                            return Some(r);
                        }
                    }
                }
                None
            }
        }
        // --- TypeAliasType ---
        Type::TypeAliasType { .. } => {
            // Wire format carries no resolved alias target.
            None
        }
        // --- UnboundType, Parameters, UnpackType ---
        _ => {
            // Needs report_missing_attribute (needs mx).
            None
        }
    }
}

// ---------------------------------------------------------------------------
// analyze_union_member_access
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_union_member_access` (checkmember.py:892-925),
/// per-item Rust subset (issue #805, extended by #1170). Returns per-item
/// results, not the joined union: the Python shim joins via
/// `make_simplified_union` after restoring each item's `definition` link,
/// mirroring the pure-Python loop (checkmember.py:920-925). An Instance
/// item is dispatched through `dispatch_instance_member_inner`
/// (self_type = item, override_info = None); any other item goes through
/// `analyze_member_access_inner` without a dispatch context. A slot is a
/// `None` when the item defers (dispatch failure, non-Instance singleton,
/// undecodable result): the Python shim fills exactly those slots through
/// the pure-Python per-item loop instead of discarding whole-union work.
/// The whole call defers (None) for `is_lvalue` / `is_super` (the Python
/// shim pre-gates those for the instance path; per-item lvalue/super
/// semantics stay in Python) or a union-shape mismatch.
#[pyfunction]
#[pyo3(signature = (resolver, union_bytes, name, is_lvalue, is_super,
                    _no_deferral, preserve_type_var_ids, start_raw_id,
                    strict_optional))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn rust_analyze_union_member_access(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    union_bytes: &[u8],
    name: &str,
    is_lvalue: bool,
    is_super: bool,
    _no_deferral: bool,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
) -> Option<(i64, bool, Vec<Option<Vec<u8>>>)> {
    if is_lvalue || is_super {
        return None;
    }
    let typ = decode_type(union_bytes)?;
    let items = match typ {
        Type::UnionType { items, .. } => items,
        _ => {
            return None;
        }
    };
    // relevant_items(): skip NoneType when not strict_optional.
    let relevant: Vec<&Type> = if strict_optional {
        items.iter().collect()
    } else {
        items
            .iter()
            .filter(|i| !matches!(i, Type::NoneType))
            .collect()
    };
    let mut next_raw_id = start_raw_id;
    let mut changed = false;
    let mut results: Vec<Option<Vec<u8>>> = Vec::with_capacity(relevant.len());
    for item in relevant.iter() {
        let item_result: Option<Type> = match item {
            Type::Instance { .. } => {
                // Python binds self_type per union item
                // (mx.copy_modified(self_type=subtype) at
                // checkmember.py:923).
                dispatch_instance_member_inner(
                    py,
                    resolver,
                    item,
                    name,
                    None,
                    item,
                    // Conservative: the var arm's own guards keep these
                    // facts safe (is_operator only widens deferral;
                    // is_self is re-guarded via the self_type check).
                    false,
                    false,
                    preserve_type_var_ids,
                    &mut next_raw_id,
                    &mut changed,
                    strict_optional,
                    // Union-item shims have no plugin handle; the var
                    // hook gate defers exactly as before.
                    None,
                )
            }
            _ => {
                // Non-Instance union item: route through the general path
                // without a dispatch context (Python keeps Instance operands
                // in Python here via `_analyze_member_access` on the item).
                analyze_member_access_inner(item, None, resolver.resolver())
            }
        };
        results.push(item_result.as_ref().and_then(encode_type));
    }
    Some((next_raw_id, changed, results))
}

// ---------------------------------------------------------------------------
// analyze_none_member_access
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_none_member_access` (checkmember.py:666-677).
///
/// `__bool__` returns a pure CallableType ret=Literal[False]. Any other
/// name recurses on `builtins.object` through the live-method dispatch
/// with the caller's mx facts: `self_type` (bound per union item on the
/// `analyze_union_member_access` fill path or the narrowed receiver;
/// `MemberContext.__init__` falls back to `original_type`, so Python
/// never carries None here), `preserve_type_var_ids`, and the raw-id
/// counter shared with the caller's `TypeVarId.next_raw_id`. Defer
/// (None) for `is_lvalue` / `is_super` (per-item lvalue/super semantics
/// stay in Python) or when the dispatch defers.
#[pyfunction]
#[pyo3(signature = (resolver, name, typ_bytes, self_type_bytes, is_lvalue, is_super,
                    preserve_type_var_ids, start_raw_id, strict_optional))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_analyze_none_member_access(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    name: &str,
    typ_bytes: &[u8],
    self_type_bytes: Option<Vec<u8>>,
    is_lvalue: bool,
    is_super: bool,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
) -> PyResult<Option<(i64, bool, Vec<u8>)>> {
    // We only accept NoneType input; any other type is a caller bug — defer.
    if !matches!(decode_type(typ_bytes), Some(Type::NoneType)) {
        return Ok(None);
    }
    if name == "__bool__" {
        return Ok(Some((
            start_raw_id,
            false,
            match encode_type(&analyze_none_bool_type()) {
                Some(b) => b,
                None => return Ok(None),
            },
        )));
    }
    if is_lvalue || is_super {
        return Ok(None);
    }
    let self_type = match self_type_bytes {
        Some(b) => match decode_type(&b) {
            Some(t) => t,
            None => {
                return Ok(None);
            }
        },
        None => return Ok(None),
    };
    // _analyze_member_access(name, builtins.object, mx)
    let object_inst = Type::Instance {
        type_ref: "builtins.object".to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    let mut next_raw_id = start_raw_id;
    let mut changed = false;
    let result = match dispatch_instance_member_inner(
        py,
        resolver,
        &object_inst,
        name,
        None,
        &self_type,
        // Conservative: same rationale as the union-item call site.
        false,
        false,
        preserve_type_var_ids,
        &mut next_raw_id,
        &mut changed,
        strict_optional,
        // NoneType shims have no plugin handle; the var hook gate
        // defers exactly as before.
        None,
    ) {
        Some(r) => r,
        None => {
            return Ok(None);
        }
    };
    Ok(encode_type(&result).map(|b| (next_raw_id, changed, b)))
}

fn analyze_none_bool_type() -> Type {
    // LiteralType(False, fallback=builtins.bool)
    let bool_inst = Type::Instance {
        type_ref: "builtins.bool".to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    let literal_false = Type::LiteralType {
        fallback: Box::new(bool_inst.clone()),
        value: crate::wire::LiteralValue::Bool(false),
    };
    let func_inst = Type::Instance {
        type_ref: "builtins.function".to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    Type::CallableType {
        fallback: Box::new(func_inst),
        instance_type: None,
        is_ellipsis_args: false,
        implicit: false,
        is_bound: false,
        from_concatenate: false,
        imprecise_arg_kinds: false,
        unpack_kwargs: false,
        from_type_type: false,
        arg_types: vec![],
        arg_kinds: vec![],
        arg_names: vec![],
        ret_type: Box::new(literal_false),
        name: None,
        variables: vec![],
        type_guard: None,
        type_is: None,
    }
}

// ---------------------------------------------------------------------------
// analyze_typeddict_access (__delitem__ only)
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_typeddict_access` (checkmember.py:1532-1568),
/// `__delitem__` branch only. The `__setitem__` branch needs live checker
/// state (`visit_typeddict_index_expr`, `readonly_keys_mutated`) and is
/// deferred. The fallback branch recurses on `typ.fallback` (an Instance),
/// which `analyze_member_access_inner` defers, so it is also deferred here.
#[pyfunction]
pub(crate) fn rust_analyze_typeddict_access(
    resolver: &NativeTypeResolver,
    name: &str,
    typ_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<Vec<u8>>> {
    // Accept only TypedDictType input.
    if !matches!(decode_type(typ_bytes), Some(Type::TypedDictType { .. })) {
        return Ok(None);
    }
    Ok(
        analyze_typeddict_access_inner(name, strict_optional, resolver.resolver())
            .and_then(|t| encode_type(&t)),
    )
}

fn analyze_typeddict_access_inner(
    name: &str,
    _strict_optional: bool,
    _resolver: &TypeResolver,
) -> Option<Type> {
    if name == "__delitem__" {
        let str_inst = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let func_inst = Type::Instance {
            type_ref: "builtins.function".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        Some(Type::CallableType {
            fallback: Box::new(func_inst),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![str_inst],
            arg_kinds: vec![ARG_POS],
            arg_names: vec![None],
            ret_type: Box::new(Type::NoneType),
            name: Some("__delitem__".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        })
    } else {
        // __setitem__ needs checker state; fallback branch recurses on
        // an Instance, which defers. Both defer to Python.
        None
    }
}

// ---------------------------------------------------------------------------
// analyze_enum_class_attribute_access (enum_literal tail only)
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_enum_class_attribute_access`
/// (checkmember.py:1507-1529), the enum-literal tail only.
///
/// Ports the final branch: when `name` is in `itype.type.enum_members`,
/// construct `LiteralType(name, fallback=itype)` and return
/// `itype.copy_modified(last_known_value=enum_literal)`. Defer (None)
/// for the `EXCLUDED_ENUM_ATTRIBUTES` path (needs `report_missing_attribute`
/// / `mx`) and the `nonmember` path (needs `node.type` from the resolver,
/// which the snapshot does not carry). Also defer when the class snapshot
/// is missing from the resolver.
#[pyfunction]
pub(crate) fn rust_analyze_enum_class_attribute_access(
    resolver: &NativeTypeResolver,
    instance_bytes: &[u8],
    name: &str,
) -> PyResult<Option<Vec<u8>>> {
    let inst = match decode_type(instance_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(
        analyze_enum_class_attribute_access_inner(&inst, name, resolver.resolver())
            .and_then(|t| encode_type(&t)),
    )
}

/// `EXCLUDED_ENUM_ATTRIBUTES` (nodes.py:3620): the set of attribute names
/// that Enum strips. We defer these to Python (needs `report_missing_attribute`).
const EXCLUDED_ENUM_ATTRIBUTES: &[&str] = &["_ignore_", "_order_", "__order__"];

fn analyze_enum_class_attribute_access_inner(
    inst: &Type,
    name: &str,
    resolver: &TypeResolver,
) -> Option<Type> {
    let (type_ref, args, _last_known_value, extra_attrs) = match inst {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => (type_ref.as_str(), args, last_known_value, extra_attrs),
        _ => return None,
    };
    // EXCLUDED path needs report_missing_attribute (mx) — defer.
    if EXCLUDED_ENUM_ATTRIBUTES.contains(&name) {
        return None;
    }
    let snap = resolver.get(type_ref)?;
    if !snap.is_enum {
        return None;
    }
    // The nonmember path needs node.type from the resolver snapshot,
    // which is not carried — defer. We only handle the enum_literal tail.
    if !snap.enum_members.iter().any(|m| m == name) {
        return None;
    }
    let enum_literal = Type::LiteralType {
        fallback: Box::new(inst.clone()),
        value: crate::wire::LiteralValue::Str(name.to_string()),
    };
    Some(Type::Instance {
        type_ref: type_ref.to_string(),
        args: args.clone(),
        last_known_value: Some(Box::new(enum_literal)),
        extra_attrs: extra_attrs.clone(),
    })
}

// ---------------------------------------------------------------------------
// analyze_descriptor_access (guards + union map; __get__ tail stays Python)
// ---------------------------------------------------------------------------

/// Wire tags for the seam result. Tag 0 (ORIG) carries no bytes: Python
/// returns its live `orig_descriptor_type`. Tag 1 (VALUE) carries the
/// computed result type (union simplification).
const DA_ORIG: u8 = 0;
const DA_VALUE: u8 = 1;

/// Decision of the pure head of `analyze_descriptor_access`
/// (checkmember.py:1120-1162).
enum DescriptorDecision {
    /// Python returns `orig_descriptor_type` unchanged; the live object
    /// is authoritative, so the seam only carries the decision.
    Orig,
    /// Return this type (the simplified-union result of the union map).
    Value(Type),
}

/// `mypy.checkmember.analyze_descriptor_access` (checkmember.py:1120-1162),
/// the pure-type-transform head.
///
/// Ports the early-return guards:
///   * Not an `Instance` → `Orig` (checkmember.py:1416-1417: every
///     non-Instance proper type — CallableType, NoneType, TupleType,
///     Overloaded, AnyType, LiteralType, TypeType, ... — returns
///     `orig_descriptor_type` unconditionally). This is the dominant
///     shape: `analyze_var` routes every non-descriptor attribute
///     result through here.
///   * `Instance` + no readable `__get__` (non-lvalue, :1189-1190) or
///     neither `__get__` nor `__set__` (lvalue, :1204-1206) → `Orig`.
///   * `UnionType` → map each item through the same decision and
///     `make_simplified_union` → `Value`; a `__get__`/`__set__`-bearing
///     Instance item defers the whole union.
///
/// The `__get__`-bearing Instance tail (bound `__get__` lookup,
/// `map_instance_to_supertype` + expand, `transform_callee_type`,
/// `check_call`, `warn_deprecated`) needs `mx.chk` checker state, so it
/// defers (`None`), as does the lvalue `__set__` assign path.
#[pyfunction]
pub(crate) fn rust_analyze_descriptor_access(
    resolver: &NativeTypeResolver,
    descriptor_bytes: &[u8],
    is_lvalue: bool,
    strict_optional: bool,
) -> PyResult<Option<(u8, Vec<u8>)>> {
    let typ = match decode_type(descriptor_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(
        analyze_descriptor_access_inner(&typ, is_lvalue, strict_optional, resolver.resolver())
            .and_then(|d| match d {
                DescriptorDecision::Orig => Some((DA_ORIG, Vec::new())),
                // An unencodable union defers the whole call so Python
                // re-runs the original body.
                DescriptorDecision::Value(t) => encode_type(&t).map(|b| (DA_VALUE, b)),
            }),
    )
}

fn analyze_descriptor_access_inner(
    typ: &Type,
    is_lvalue: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<DescriptorDecision> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::UnionType { items, .. } => {
            // Map the access over union types, then make_simplified_union.
            let mut results = Vec::with_capacity(items.len());
            for item in items {
                match analyze_descriptor_access_inner(item, is_lvalue, strict_optional, resolver)? {
                    DescriptorDecision::Orig => results.push(item.clone()),
                    DescriptorDecision::Value(t) => results.push(t),
                }
            }
            let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            let joined = make_simplified_union(&results, &ctx, resolver, true, false)?;
            Some(DescriptorDecision::Value(joined))
        }
        Type::Instance { type_ref, .. } => {
            // Member presence rides the resolver snapshots; a missing
            // snapshot defers the whole call.
            let has_get = has_readable_member_by_ref(resolver, type_ref, "__get__")?;
            if !is_lvalue {
                // Non-lvalue __get__ short-circuit (checkmember.py:1189):
                // no-readable-__get__ passes the descriptor through; a
                // __get__-bearing descriptor defers (heavy-path analysis).
                if has_get {
                    return None;
                }
                Some(DescriptorDecision::Orig)
            } else {
                // Lvalue: a readable __set__ routes to analyze_descriptor_assign
                // and a readable __get__ to the heavy tail (both checker
                // state); a plain non-descriptor passes through (1204-1206).
                if has_get {
                    return None;
                }
                let has_set = has_readable_member_by_ref(resolver, type_ref, "__set__")?;
                if has_set {
                    return None;
                }
                Some(DescriptorDecision::Orig)
            }
        }
        // Every non-Instance proper type returns orig_descriptor_type
        // unconditionally (checkmember.py:1416-1417), regardless of
        // is_lvalue.
        _ => Some(DescriptorDecision::Orig),
    }
}

// ---------------------------------------------------------------------------
// check_self_arg
// ---------------------------------------------------------------------------

/// `mypy.checkmember.check_self_arg` (checkmember.py:1294-1370).
///
/// Filters overload items by checking that the dispatched argument type is
/// compatible with each item's first (self/cls) parameter. Returns the
/// filtered signature as a single CallableType (one match), an Overloaded
/// (multiple matches), or the original signature (no match, error reported
/// by Python). Defer (`None`) for cases Rust cannot handle: ParamSpec/
/// TypeVarTuple selfargs, TypeAliasType expansion, is_subtype deferral,
/// or `is_overlapping_types` deferral in the Instance special-case.
///
/// Mirrors the two-pass filter in Python:
///   1. Instance special-case: drop items where both selfarg and dispatched
///      arg are Instances of the same type but with non-overlapping args.
///   2. Subtype check: keep items where `dispatched_arg_type <: erase_typevars(
///      erase_to_bound(selfarg))` with `always_covariant` + `ignore_pos_arg_names`.
///
/// `is_classmethod` wraps `dispatched_arg_type` in `TypeType.make_normalized`.
/// The `name` parameter is used only for the `__call__` special-case detection
/// of a callable selfarg.
#[pyfunction]
pub(crate) fn rust_check_self_arg(
    resolver: &NativeTypeResolver,
    functype_bytes: &[u8],
    dispatched_arg_type_bytes: &[u8],
    is_classmethod: bool,
    name: &str,
    strict_optional: bool,
) -> PyResult<Option<(i64, bool, Vec<u8>)>> {
    let functype = match decode_type(functype_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let dispatched = match decode_type(dispatched_arg_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = check_self_arg_inner(
        &functype,
        &dispatched,
        is_classmethod,
        name,
        strict_optional,
        resolver.resolver(),
        false,
    );
    // check_self_arg only filters overload items — it does not freshen.
    // Return (next_raw_id=0, changed=false, wire_bytes) so the Python
    // gate unpacks the 3-tuple but skips the TypeVarId.next_raw_id update.
    Ok(result.and_then(|t| encode_type(&t).map(|bytes| (0i64, false, bytes))))
}

fn check_self_arg_inner(
    functype: &Type,
    dispatched_arg_type: &Type,
    is_classmethod: bool,
    name: &str,
    strict_optional: bool,
    resolver: &TypeResolver,
    suppress_self_fail: bool,
) -> Option<Type> {
    let items = match functype {
        Type::CallableType { .. } => vec![functype.clone()],
        Type::Overloaded { items } => items.clone(),
        _ => return None,
    };
    if items.is_empty() {
        return Some(functype.clone());
    }

    // classmethod: wrap dispatched_arg_type in TypeType.make_normalized.
    let dispatched = if is_classmethod {
        make_type_type_normalized(dispatched_arg_type)
    } else {
        dispatched_arg_type.clone()
    };

    // Pass 1: Instance special-case filtering.
    // Drop items where both selfarg and dispatched are Instances of the
    // same type but args don't overlap. Also defer if is_overlapping_types

    // cannot decide.
    let mut pass1 = Vec::with_capacity(items.len());
    for item in &items {
        let (arg_types, arg_kinds) = match item {
            Type::CallableType {
                arg_types,
                arg_kinds,
                ..
            } => (arg_types, arg_kinds),
            _ => return None,
        };
        if arg_types.is_empty() {
            // Python: msg.no_formal_self, then `return functype`. Defer in
            // production (Python reports the error); the error-suppressed
            // find_member contexts keep the original functype, as Python.
            if suppress_self_fail {
                return Some(functype.clone());
            }
            return None;
        }
        // Python checks item.arg_kinds[0] not in (ARG_POS, ARG_STAR).
        // Guard a length mismatch; if arg_kinds[0] is not ARG_POS/ARG_STAR,
        // Python reports no_formal_self and returns functype.

        // Defer in production; suppressed contexts keep functype.
        match arg_kinds.first() {
            Some(&k) if k == ARG_POS || k == ARG_STAR => {}
            _ => {
                if suppress_self_fail {
                    return Some(functype.clone());
                }
                return None;
            }
        }
        let selfarg = get_proper_or_none(&arg_types[0])?;
        match (&dispatched, selfarg) {
            (
                Type::Instance {
                    type_ref: d_ref, ..
                },
                Type::Instance {
                    type_ref: s_ref,
                    args: s_args,
                    ..
                },
            ) if d_ref == s_ref && !s_args.is_empty() => {
                let overlap = crate::meet::overlap(
                    &dispatched,
                    selfarg,
                    strict_optional,
                    false, // ignore_promotions
                    false, // overlap_for_overloads
                    resolver,
                    0,
                )?;
                if !overlap {
                    continue;
                }
            }
            _ => {}
        }
        pass1.push(item.clone());
    }

    // Pass 2: subtype check.
    let working = if pass1.is_empty() { &items } else { &pass1 };
    let mut pass2 = Vec::with_capacity(working.len());
    for item in working {
        let arg_types = match item {
            Type::CallableType { arg_types, .. } => arg_types,
            _ => return None,
        };
        let selfarg = get_proper_or_none(&arg_types[0])?;
        // __call__ special-case: callable selfarg is always accepted.
        let self_callable = name == "__call__" && matches!(selfarg, Type::CallableType { .. });

        if self_callable {
            pass2.push(item.clone());
            continue;
        }

        // erase_to_bound(selfarg), then erase_typevars.
        let erased_bound = crate::typeops::erase_to_bound(selfarg)?;
        let erased = crate::erase_typevars::erase_typevars_inner(
            &erased_bound,
            None,
            &crate::erase_typevars::make_any(),
        )?;

        // always_covariant: True if any type var in get_all_type_vars(selfarg)
        // is NOT a TypeVarType (i.e. ParamSpec or TypeVarTuple).
        let mut tvs = Vec::new();
        crate::typeops::collect_type_vars(selfarg, false, &mut tvs)?;
        let always_covariant = tvs.iter().any(|tv| !matches!(tv, Type::TypeVarType { .. }));

        let ctx = SubtypeContext::new(
            false, // ignore_type_params
            false, // ignore_declared_variance
            always_covariant,
            false, // ignore_promotions
            false, // proper_subtype
            strict_optional,
        );

        let subtype_result = crate::subtypes::is_subtype(&dispatched, &erased, &ctx, resolver);

        match subtype_result {
            Some(true) => pass2.push(item.clone()),
            Some(false) => continue,
            None => {
                // Defer: ParamSpecType or TypeVarTupleType selfarg.
                if matches!(selfarg, Type::ParamSpecType { .. }) {
                    pass2.push(item.clone());
                } else if matches!(selfarg, Type::TypeVarTupleType { .. }) {
                    // Python raises NotImplementedError; defer entire call.
                    return None;
                } else {
                    return None;
                }
            }
        }
    }

    if pass2.is_empty() {
        // Python reports incompatible_self_argument, then returns functype.
        // Defer in production so Python can report the error; suppressed
        // find_member contexts keep the original functype, as Python.
        if suppress_self_fail {
            return Some(functype.clone());
        }
        return None;
    }
    if pass2.len() == 1 {
        return Some(pass2.into_iter().next().unwrap());
    }
    Some(Type::Overloaded { items: pass2 })
}

/// `TypeType.make_normalized` (types.py:3710-3724): wraps a type in TypeType,
/// expanding UnionType items into a Union of TypeType items.
fn make_type_type_normalized(item: &Type) -> Type {
    let proper = get_proper_or_none(item).unwrap_or(item);
    match proper {
        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true,
            can_be_false,
        } => {
            let mapped: Vec<Type> = items.iter().map(make_type_type_normalized).collect();
            Type::UnionType {
                items: mapped,
                uses_pep604_syntax: *uses_pep604_syntax,
                can_be_true: *can_be_true,
                can_be_false: *can_be_false,
            }
        }
        other => Type::TypeType {
            item: Box::new(other.clone()),
            is_type_form: false,
        },
    }
}

// ---------------------------------------------------------------------------
// expand_without_binding
// ---------------------------------------------------------------------------

/// `mypy.checkmember.expand_without_binding` (checkmember.py:1192-1200).
///
/// Expands a Var's type for instance access without binding self:
/// `freshen_all_functions_type_vars` + `expand_self_type_if_needed` +
/// `expand_type_by_instance` + `freeze_all_type_vars`.
///
/// Rust handles the common pure path: when `preserve_type_var_ids` is False
/// and there is no Self type to expand (no `var.info.self_type`), the result
/// is just `freshen + expand_type_by_instance`. Defer (`None`) when:
///   * `preserve_type_var_ids` is True (freshen skipped, but expand_self_type
///     still needs the Var's self_type, which the wire format does not carry)
///   * `expand_self_type` would need to run (needs `var.info.self_type`)
///   * `freshen` or `expand_type_by_instance` defers
///
/// The caller passes `has_self_type=False` from the Python side to indicate
/// that `var.info.self_type is None`, enabling the pure path.
#[pyfunction]
pub(crate) fn rust_expand_without_binding(
    typ_bytes: &[u8],
    itype_bytes: &[u8],
    preserve_type_var_ids: bool,
    has_self_type: bool,
    start_raw_id: i64,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> PyResult<Option<(i64, bool, Vec<u8>)>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let itype = match decode_type(itype_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    // If the var has a self_type, expand_self_type needs the Var node;
    // defer to Python.
    if has_self_type {
        return Ok(None);
    }
    let result = expand_without_binding_inner(
        &typ,
        &itype,
        preserve_type_var_ids,
        start_raw_id,
        strict_optional,
        resolver.resolver(),
    );
    match result {
        Some((next_raw_id, changed, t)) => {
            let bytes = match encode_type(&t) {
                Some(b) => b,
                None => return Ok(None),
            };
            Ok(Some((next_raw_id, changed, bytes)))
        }
        None => Ok(None),
    }
}

fn expand_without_binding_inner(
    typ: &Type,
    itype: &Type,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<(i64, bool, Type)> {
    let mut current = typ.clone();
    let mut next_raw_id = start_raw_id;
    let mut changed = false;

    if !preserve_type_var_ids {
        let freshened = crate::freshen::freshen_type(
            &current,
            &mut next_raw_id,
            &mut changed,
            strict_optional,
        )?;
        current = freshened;
    }

    // expand_self_type_if_needed: when has_self_type is False and not
    // is_self/is_super, expand_self_type returns typ unchanged. We only
    // handle this case (caller sets has_self_type=False).
    let expanded = crate::expandtype::expand_type_by_instance_core(
        &current,
        itype,
        resolver,
        strict_optional,
    )?;

    // freeze_all_type_vars: no-op in Rust (the result is already frozen;
    // expand produces only bound class vars when it fully succeeds).
    Some((next_raw_id, changed, expanded))
}

// ---------------------------------------------------------------------------
// expand_and_bind_callable
// ---------------------------------------------------------------------------

/// `mypy.checkmember.expand_and_bind_callable` (checkmember.py:1203-1247),
/// the `is_trivial_self=True` path only.
///
/// Ports: `freshen_all_functions_type_vars` + `bind_self_fast` +
/// `expand_type_by_instance` + `freeze_all_type_vars`. The non-trivial path
/// (`check_self_arg` + `bind_self`) and the property extraction tail are
/// deferred to Python.
///
/// Defer (`None`) when:
///   * Not `is_trivial_self` (needs check_self_arg + bind_self)
///   * `var.is_property` (property extraction needs the Var node)
///   * freshen or bind_self_fast or expand defers
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_and_bind_callable(
    functype_bytes: &[u8],
    itype_bytes: &[u8],
    is_trivial_self: bool,
    is_property: bool,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> PyResult<Option<(i64, bool, Vec<u8>)>> {
    if !is_trivial_self || is_property {
        return Ok(None);
    }
    let functype = match decode_type(functype_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let itype = match decode_type(itype_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = expand_and_bind_callable_inner(
        &functype,
        &itype,
        preserve_type_var_ids,
        start_raw_id,
        strict_optional,
        resolver.resolver(),
    );
    match result {
        Some((next_raw_id, changed, t)) => {
            let bytes = match encode_type(&t) {
                Some(b) => b,
                None => return Ok(None),
            };
            Ok(Some((next_raw_id, changed, bytes)))
        }
        None => Ok(None),
    }
}

fn expand_and_bind_callable_inner(
    functype: &Type,
    itype: &Type,
    preserve_type_var_ids: bool,
    start_raw_id: i64,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<(i64, bool, Type)> {
    let mut current = functype.clone();
    let mut next_raw_id = start_raw_id;
    let mut changed = false;

    if !preserve_type_var_ids {
        let freshened = crate::freshen::freshen_type(
            &current,
            &mut next_raw_id,
            &mut changed,
            strict_optional,
        )?;
        current = freshened;
    }

    // expand_self_type: needs var.info.self_type; for trivial_self the
    // caller ensures has_self_type=False, so expand_self_type returns t.
    // bind_self_fast for trivial_self.
    let bound = bind_self_fast_inner(&current)?;
    let expanded =
        crate::expandtype::expand_type_by_instance_core(&bound, itype, resolver, strict_optional)?;

    Some((next_raw_id, changed, expanded))
}

// ---------------------------------------------------------------------------
// add_class_tvars
// ---------------------------------------------------------------------------

/// `mypy.checkmember.add_class_tvars` (checkmember.py:1707-1774), the
/// `is_classmethod + is_trivial_self` path for CallableType, plus the
/// Overloaded recursion.
///
/// Ports: `freshen_all_functions_type_vars` + `bind_self_fast` +
/// `expand_type_by_instance(isuper)` + `freeze_all_type_vars` +
/// `copy_modified(variables=original_vars + t.variables)`.
///
/// Defer (`None`) when:
///   * Not `is_classmethod` or not `is_trivial_self` (needs bind_self)
///   * CallableType is already `is_bound`
///   * freshen or bind_self_fast or expand defers
///   * Overloaded with non-CallableType items
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_add_class_tvars(
    resolver: &NativeTypeResolver,
    t_bytes: &[u8],
    isuper_bytes: &[u8],
    is_classmethod: bool,
    is_trivial_self: bool,
    preserve_type_var_ids: bool,
    original_vars_bytes: &[u8],
    start_raw_id: i64,
    strict_optional: bool,
) -> PyResult<Option<(i64, bool, Vec<u8>)>> {
    let t = match decode_type(t_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let isuper = if isuper_bytes.is_empty() {
        None
    } else {
        decode_type(isuper_bytes)
    };
    let original_vars = if original_vars_bytes.is_empty() {
        Vec::new()
    } else {
        match decode_type_list_blob(original_vars_bytes) {
            Some(v) => v,
            None => return Ok(None),
        }
    };
    let result = add_class_tvars_inner(
        &t,
        isuper.as_ref(),
        is_classmethod,
        is_trivial_self,
        preserve_type_var_ids,
        &original_vars,
        start_raw_id,
        strict_optional,
        resolver.resolver(),
    );
    match result {
        Some((next_raw_id, changed, typ)) => {
            let bytes = match encode_type(&typ) {
                Some(b) => b,
                None => return Ok(None),
            };
            Ok(Some((next_raw_id, changed, bytes)))
        }
        None => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
fn add_class_tvars_inner(
    t: &Type,
    isuper: Option<&Type>,
    is_classmethod: bool,
    is_trivial_self: bool,
    preserve_type_var_ids: bool,
    original_vars: &[Type],
    start_raw_id: i64,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<(i64, bool, Type)> {
    match t {
        Type::CallableType { is_bound, .. } => {
            // Only handle classmethod + trivial_self + not already bound.
            if !is_classmethod || !is_trivial_self || *is_bound {
                return None;
            }
            let mut current = t.clone();
            let mut next_raw_id = start_raw_id;
            let mut changed = false;

            if !preserve_type_var_ids {
                let freshened = crate::freshen::freshen_type(
                    &current,
                    &mut next_raw_id,
                    &mut changed,
                    strict_optional,
                )?;
                current = freshened;
            }

            // bind_self_fast for trivial_self classmethod.
            let bound = bind_self_fast_inner(&current)?;

            // expand_type_by_instance(bound, isuper) if isuper is Some.
            let expanded = if let Some(isup) = isuper {
                crate::expandtype::expand_type_by_instance_core(
                    &bound,
                    isup,
                    resolver,
                    strict_optional,
                )?
            } else {
                bound
            };

            // copy_modified(variables=list(original_vars) + list(t.variables))
            // After freshen + expand, the variables field may have changed.
            // Python reads t.variables from the *post-expand* result.
            let new_vars: Vec<Type> = match &expanded {
                Type::CallableType {
                    variables: exp_vars,
                    ..
                } => {
                    let mut combined = original_vars.to_vec();
                    combined.extend(exp_vars.iter().cloned());
                    combined
                }
                _ => return None,
            };

            let result = match &expanded {
                Type::CallableType {
                    fallback: ef,
                    instance_type: ei,
                    is_ellipsis_args: ee,
                    implicit: eimp,
                    is_bound: eb,
                    from_concatenate: ec,
                    imprecise_arg_kinds: eik,
                    unpack_kwargs: ew,
                    from_type_type: eftt,
                    arg_types: eat,
                    arg_kinds: eak,
                    arg_names: ean,
                    ret_type: ert,
                    name: en,
                    type_guard: etg,
                    type_is: eti,
                    ..
                } => Type::CallableType {
                    fallback: ef.clone(),
                    instance_type: ei.clone(),
                    is_ellipsis_args: *ee,
                    implicit: *eimp,
                    is_bound: *eb,
                    from_concatenate: *ec,
                    imprecise_arg_kinds: *eik,
                    unpack_kwargs: *ew,
                    from_type_type: *eftt,
                    arg_types: eat.clone(),
                    arg_kinds: eak.clone(),
                    arg_names: ean.clone(),
                    ret_type: ert.clone(),
                    name: en.clone(),
                    variables: new_vars,
                    type_guard: etg.clone(),
                    type_is: eti.clone(),
                },
                _ => return None,
            };
            Some((next_raw_id, changed, result))
        }
        Type::Overloaded { items } => {
            if items.is_empty() {
                return None;
            }
            let mut new_items = Vec::with_capacity(items.len());
            let mut next_raw_id = start_raw_id;
            let mut changed = false;
            for item in items {
                let r = add_class_tvars_inner(
                    item,
                    isuper,
                    is_classmethod,
                    is_trivial_self,
                    preserve_type_var_ids,
                    original_vars,
                    next_raw_id,
                    strict_optional,
                    resolver,
                )?;
                next_raw_id = r.0;
                if r.1 {
                    changed = true;
                }
                // Python casts each item to CallableType.
                if !matches!(r.2, Type::CallableType { .. }) {
                    return None;
                }
                new_items.push(r.2);
            }
            Some((next_raw_id, changed, Type::Overloaded { items: new_items }))
        }
        _ => {
            // Non-callable: expand_type_by_instance(t, isuper) if isuper.
            let expanded = if let Some(isup) = isuper {
                crate::expandtype::expand_type_by_instance_core(t, isup, resolver, strict_optional)?
            } else {
                t.clone()
            };
            Some((start_raw_id, false, expanded))
        }
    }
}

/// Decode a wire-format list of Type blobs (LIST_GEN tag + bare count + types).
fn decode_type_list_blob(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    crate::wire::read_type_list(&mut buf).ok()
}

// ---------------------------------------------------------------------------
// descriptor_has_get / descriptor_has_set
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_descriptor_access` (checkmember.py:924-941),
/// the `__get__`/`__set__` presence checks.
///
/// Returns `(has_get, has_set)` for a descriptor Instance type, reading
/// member presence from the resolver snapshots. Defer (`None`) when the
/// class or any MRO class snapshot is missing from the resolver.
#[pyfunction]
pub(crate) fn rust_descriptor_has_get_set(
    resolver: &NativeTypeResolver,
    descriptor_bytes: &[u8],
) -> PyResult<Option<(bool, bool)>> {
    let typ = match decode_type(descriptor_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let proper = match get_proper_or_none(&typ) {
        Some(p) => p,
        None => return Ok(None),
    };
    let type_ref = match proper {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return Ok(None),
    };
    let has_get = has_readable_member_by_ref(resolver.resolver(), type_ref, "__get__");
    let has_set = has_readable_member_by_ref(resolver.resolver(), type_ref, "__set__");
    match (has_get, has_set) {
        (Some(g), Some(s)) => Ok(Some((g, s))),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// classify_type_type_member_access (issue #957)
// ---------------------------------------------------------------------------

/// Dispatch tags for `analyze_type_type_member_access`
/// (checkmember.py:965). Values must match `NATIVE_TT_*` in
/// mypy/checkmember.py.
const TT_NONE: i64 = 0;
const TT_ITEM_INSTANCE: i64 = 1;
const TT_ITEM_ANY: i64 = 2;
const TT_TV_UB_INSTANCE: i64 = 3;
const TT_TV_UB_UNION: i64 = 4;
const TT_TV_UB_TUPLE: i64 = 5;
const TT_TV_UB_ANY: i64 = 6;
const TT_TV_UB_OTHER: i64 = 7;
const TT_ITEM_TUPLE: i64 = 8;
const TT_ITEM_FUNC_TYPEOBJ: i64 = 9;
const TT_ITEM_FUNC_NOT_TYPEOBJ: i64 = 10;
const TT_ITEM_TYPE_TYPE_INSTANCE: i64 = 11;
const TT_ITEM_TYPE_TYPE_OTHER: i64 = 12;

/// The `typ.item` kind in the top-level isinstance dispatch.
#[derive(Clone, Copy, PartialEq)]
enum TtItemKind {
    Instance,
    AnyType,
    TypeVarType,
    TupleType,
    FunctionLike,
    TypeType,
    Other,
}

/// The `get_proper_type(typ.item.upper_bound)` kind in the
/// TypeVarType sub-dispatch.
#[derive(Clone, Copy, PartialEq)]
enum TtUbKind {
    Instance,
    UnionType,
    TupleType,
    AnyType,
    Other,
}

/// Pure decision mirroring `analyze_type_type_member_access` dispatch
/// head (checkmember.py:971-999). Returns a branch tag; Python applies
/// the terminal branches (`_analyze_member_access`, `filter_errors`,
/// `tuple_fallback`, `TypeType.make_normalized`, `metaclass_type`).
fn classify_type_type_member_access(
    item_kind: TtItemKind,
    is_type_obj: bool,
    inner_item_is_instance: bool,
    ub_kind: TtUbKind,
) -> i64 {
    match item_kind {
        TtItemKind::Instance => TT_ITEM_INSTANCE,
        TtItemKind::AnyType => TT_ITEM_ANY,
        TtItemKind::TypeVarType => match ub_kind {
            TtUbKind::Instance => TT_TV_UB_INSTANCE,
            TtUbKind::UnionType => TT_TV_UB_UNION,
            TtUbKind::TupleType => TT_TV_UB_TUPLE,
            TtUbKind::AnyType => TT_TV_UB_ANY,
            TtUbKind::Other => TT_TV_UB_OTHER,
        },
        TtItemKind::TupleType => TT_ITEM_TUPLE,
        TtItemKind::FunctionLike => {
            if is_type_obj {
                TT_ITEM_FUNC_TYPEOBJ
            } else {
                TT_ITEM_FUNC_NOT_TYPEOBJ
            }
        }
        TtItemKind::TypeType => {
            if inner_item_is_instance {
                TT_ITEM_TYPE_TYPE_INSTANCE
            } else {
                TT_ITEM_TYPE_TYPE_OTHER
            }
        }
        TtItemKind::Other => TT_NONE,
    }
}

/// `#[pyfunction]` entry for
/// `mypy.checkmember.analyze_type_type_member_access`
/// (checkmember.py:965-1018). Reads the live `TypeType` via PyO3,
/// classifies the 9-way dispatch head (plus a nested 4-way
/// sub-dispatch on `get_proper_type(typ.item.upper_bound)` for the
/// TypeVarType arm), and returns a branch tag. Python applies the
/// terminal branches. Returns `None` on any read failure.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_type_type_member_access(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<Option<i64>> {
    match classify_type_type_member_access_inner(py, typ) {
        Ok(tag) => Ok(Some(tag)),
        Err(_) => Ok(None),
    }
}

fn classify_type_type_member_access_inner(py: Python<'_>, typ: &PyAny) -> PyResult<i64> {
    let types_mod = py.import("mypy.types")?;
    let item = typ.getattr("item")?;
    let instance_cls = types_mod.getattr("Instance")?;
    let any_cls = types_mod.getattr("AnyType")?;
    let typevar_cls = types_mod.getattr("TypeVarType")?;
    let tuple_cls = types_mod.getattr("TupleType")?;
    let funclike_cls = types_mod.getattr("FunctionLike")?;
    let typetype_cls = types_mod.getattr("TypeType")?;
    let union_cls = types_mod.getattr("UnionType")?;

    let item_kind = if item.is_instance(instance_cls)? {
        TtItemKind::Instance
    } else if item.is_instance(any_cls)? {
        TtItemKind::AnyType
    } else if item.is_instance(typevar_cls)? {
        TtItemKind::TypeVarType
    } else if item.is_instance(tuple_cls)? {
        TtItemKind::TupleType
    } else if item.is_instance(funclike_cls)? {
        TtItemKind::FunctionLike
    } else if item.is_instance(typetype_cls)? {
        TtItemKind::TypeType
    } else {
        TtItemKind::Other
    };

    let mut is_type_obj = false;
    let mut inner_item_is_instance = false;
    let mut ub_kind = TtUbKind::Other;
    match item_kind {
        TtItemKind::FunctionLike => {
            is_type_obj = item.call_method0("is_type_obj")?.extract()?;
        }
        TtItemKind::TypeType => {
            let inner = item.getattr("item")?;
            inner_item_is_instance = inner.is_instance(instance_cls)?;
        }
        TtItemKind::TypeVarType => {
            let upper_bound = item.getattr("upper_bound")?;
            let proper = types_mod
                .getattr("get_proper_type")?
                .call1((upper_bound,))?;
            ub_kind = if proper.is_instance(instance_cls)? {
                TtUbKind::Instance
            } else if proper.is_instance(union_cls)? {
                TtUbKind::UnionType
            } else if proper.is_instance(tuple_cls)? {
                TtUbKind::TupleType
            } else if proper.is_instance(any_cls)? {
                TtUbKind::AnyType
            } else {
                TtUbKind::Other
            };
        }
        _ => {}
    }
    Ok(classify_type_type_member_access(
        item_kind,
        is_type_obj,
        inner_item_is_instance,
        ub_kind,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `is_instance_var` (checkmember.py:1502-1511): the PEP 526
/// instance-variable predicate. Reads `var.name`, `var.info.names`,
/// `var.is_classvar`, and `var.is_inferred` via PyO3 and mirrors the
/// four-clause conjunction exactly. Defers (`None`) when any attribute
/// is unreadable, so the Python caller falls back to the pure-Python
/// predicate.
#[pyfunction]
pub(crate) fn rust_is_instance_var(var: &PyAny) -> PyResult<Option<bool>> {
    is_instance_var_inner(var).map(Some).or(Ok(None))
}

fn is_instance_var_inner(var: &PyAny) -> PyResult<bool> {
    let info = var.getattr("info")?;
    let name: String = var.getattr("name")?.extract()?;
    let names = info.getattr("names")?;
    // Clause 1: `var.name in var.info.names`. A missing key raises
    // KeyError, which short-circuits to False (clause 1 fails).
    let entry = match names.get_item(name.as_str()) {
        Ok(e) => e,
        Err(_) => return Ok(false),
    };
    // Clause 2: `var.info.names[var.name].node is var`.
    let node = entry.getattr("node")?;
    if !node.is(var) {
        return Ok(false);
    }
    // Clause 3: `not var.is_classvar`.
    let is_classvar: bool = var.getattr("is_classvar")?.extract()?;
    if is_classvar {
        return Ok(false);
    }
    // Clause 4: `not var.is_inferred`.
    let is_inferred: bool = var.getattr("is_inferred")?.extract()?;
    Ok(!is_inferred)
}

// ---------------------------------------------------------------------------
// classify_analyze_var (issue #1056)
// ---------------------------------------------------------------------------

/// Decision tags for `analyze_var`; must match `NATIVE_AV_*` in
/// mypy/checkmember.py.
pub(crate) const ANALYZE_VAR_SETTER: i64 = 0;
pub(crate) const ANALYZE_VAR_GETTER: i64 = 1;
pub(crate) const ANALYZE_VAR_PARTIAL: i64 = 2;
pub(crate) const ANALYZE_VAR_NOT_READY: i64 = 3;
pub(crate) const ANALYZE_VAR_ENUM_LITERAL: i64 = 4;
pub(crate) const ANALYZE_VAR_UNBOUND_ANY: i64 = 5;

/// Live `Var` facts gathered via PyO3; pure scalars so the decision core
/// is unit-testable without an interpreter.
struct AnalyzeVarFacts {
    is_settable_property: bool,
    setter_type_present: bool,
    setter_type_is_partial: bool,
    var_type_present: bool,
    var_type_is_partial: bool,
    is_ready: bool,
    is_initialized_in_class: bool,
    is_instance_var: bool,
    info_fullname: String,
    is_enum: bool,
    enum_has_name: bool,
}

fn live_bool(obj: &PyAny, attr: &str) -> Option<bool> {
    obj.getattr(attr).ok()?.extract::<bool>().ok()
}

/// Read the `Var` scalars the decision core consumes. Any unreadable
/// attribute (including a `FakeInfo` `info`, whose `__getattribute__`
/// raises) defers, mirroring `rust_is_instance_var`'s deferral story.
fn gather_analyze_var_facts(
    py: Python<'_>,
    var: &PyAny,
    name: &str,
    is_lvalue: bool,
) -> Option<AnalyzeVarFacts> {
    let info = var.getattr("info").ok()?;
    let info_fullname: String = info.getattr("fullname").ok()?.extract().ok()?;
    let is_settable_property = live_bool(var, "is_settable_property")?;
    let is_ready = live_bool(var, "is_ready")?;
    let is_initialized_in_class = live_bool(var, "is_initialized_in_class")?;
    let setter_type = var.getattr("setter_type").ok()?;
    let var_type = var.getattr("type").ok()?;
    let partial_cls: &PyType = py
        .import("mypy.types")
        .ok()?
        .getattr("PartialType")
        .ok()?
        .downcast()
        .ok()?;
    let setter_type_present = !setter_type.is_none();
    let setter_type_is_partial =
        setter_type_present && setter_type.is_instance(partial_cls).ok()?;
    let var_type_present = !var_type.is_none();
    let var_type_is_partial = var_type_present && var_type.is_instance(partial_cls).ok()?;
    let is_instance_var = is_instance_var_inner(var).ok();
    let is_enum = live_bool(info, "is_enum")?;
    // Python short-circuits `var.info.is_enum and not mx.is_lvalue` before
    // touching `enum_members` (checkmember.py:1704); the property rebuilds
    // the member list from scratch, so mirror the guard exactly.
    let enum_has_name = if is_enum && !is_lvalue {
        info.getattr("enum_members")
            .ok()?
            .call_method1("__contains__", (name,))
            .ok()?
            .extract::<bool>()
            .ok()?
    } else {
        false
    };
    Some(AnalyzeVarFacts {
        is_settable_property,
        setter_type_present,
        setter_type_is_partial,
        var_type_present,
        var_type_is_partial,
        is_ready,
        is_initialized_in_class,
        is_instance_var: is_instance_var?,
        info_fullname,
        is_enum,
        enum_has_name,
    })
}

/// Pure decision core of `analyze_var` (checkmember.py:1599-1700): the
/// typ-selection head, the PartialType / not-ready / unbound-Any dispatch,
/// and the enum-literal tail arm, reduced to a single outcome tag.
///
/// * typ selection (checkmember.py:1622-1628): a settable property read as
///   an lvalue takes `setter_type`, falling back to `var.type` when the
///   synthetic setter type is missing and the var is ready; everything
///   else takes `var.type`.
/// * PARTIAL wins: `handle_partial_var_type` returns before the tail can
///   run, so it beats the enum-literal arm.
/// * NOT_READY beats the enum-literal arm: the not-ready callback is a
///   head-body side effect the shim must apply.
/// * ENUM_LITERAL collapses the head body only when that body is
///   side-effect free under a non-lvalue access: the lvalue-only msg
///   gates cannot fire, and the bind tail (whose property `__call__`
///   re-analysis can emit errors) must not engage. The nonmember unwrap
///   arm consumes the computed result, so those accesses stay GETTER and
///   the shim re-runs the tail check.
fn classify_analyze_var_inner(
    name: &str,
    facts: &AnalyzeVarFacts,
    is_lvalue: bool,
    no_deferral: bool,
    is_operator: bool,
) -> i64 {
    let setter_path = facts.is_settable_property && is_lvalue;
    let (typ_present, selected_is_partial) = if setter_path {
        if facts.setter_type_present {
            (true, facts.setter_type_is_partial)
        } else {
            (
                facts.is_ready && facts.var_type_present,
                facts.var_type_is_partial,
            )
        }
    } else {
        (facts.var_type_present, facts.var_type_is_partial)
    };

    if typ_present && selected_is_partial {
        return ANALYZE_VAR_PARTIAL;
    }
    // Not-ready callback fires before the enum tail can overwrite the
    // result, so NOT_READY cannot collapse into ENUM_LITERAL.
    if !typ_present && !facts.is_ready && !no_deferral {
        return ANALYZE_VAR_NOT_READY;
    }
    let bind_tail_engages =
        facts.is_initialized_in_class && (!facts.is_instance_var || is_operator);
    if facts.is_enum
        && !is_lvalue
        && facts.enum_has_name
        && name != "name"
        && name != "value"
        && !bind_tail_engages
    {
        return ANALYZE_VAR_ENUM_LITERAL;
    }
    if typ_present {
        if setter_path {
            ANALYZE_VAR_SETTER
        } else {
            ANALYZE_VAR_GETTER
        }
    } else {
        ANALYZE_VAR_UNBOUND_ANY
    }
}

/// `#[pyfunction]` entry for the `analyze_var` decision head (issue
/// #1056). Reads the live `Var` scalars via PyO3 (rust_is_magic_base
/// pattern) and maps the wire receiver instance through the
/// resolver-backed `map_instance_to_supertype` to prove the native tail
/// will engage (a snapshot miss defers, so Python's total
/// `map_instance_to_supertype` handles the access). Returns a single
/// outcome tag (`ANALYZE_VAR_*`); Python applies the branch's side
/// effects.
#[pyfunction]
#[pyo3(signature = (name, var, itype_bytes, is_lvalue, no_deferral, is_operator, resolver))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_classify_analyze_var(
    py: Python<'_>,
    name: &str,
    var: &PyAny,
    itype_bytes: &[u8],
    is_lvalue: bool,
    no_deferral: bool,
    is_operator: bool,
    resolver: &NativeTypeResolver,
) -> PyResult<Option<i64>> {
    let itype = match decode_type(itype_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let (type_ref, args) = match &itype {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return Ok(None),
    };
    let facts = match gather_analyze_var_facts(py, var, name, is_lvalue) {
        Some(f) => f,
        None => return Ok(None),
    };
    // checkmember.py:1621 `itype = map_instance_to_supertype(itype,
    // var.info)`: the mapped instance feeds every non-partial tail. Rust
    // gates on resolver coverage; the shim still maps its own instance.
    if crate::subtypes::map_instance_to_supertype(
        type_ref,
        args,
        &facts.info_fullname,
        resolver.resolver(),
    )
    .is_none()
    {
        return Ok(None);
    }
    Ok(Some(classify_analyze_var_inner(
        name,
        &facts,
        is_lvalue,
        no_deferral,
        is_operator,
    )))
}

// ---------------------------------------------------------------------------
// check_final_member
// ---------------------------------------------------------------------------

/// The fold over per-MRO-entry facts, kept separate from the PyO3 entry so
/// the MRO ordering is unit-testable without a Python runtime. Each entry
/// is whether that base declares `name` final (`false` includes "name not
/// present" and "present but not a final-capable node kind"); `None` is an
/// unreadable entry, which defers the whole walk (Python would have raised
/// mid-loop, so the shim falls back to the pure body). Python scans the
/// full MRO without breaking, so the fold answers "any entry final".
fn check_final_member_fold(entries: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    for entry in entries {
        match entry {
            None => return None,
            Some(true) => return Some(true),
            Some(false) => {}
        }
    }
    Some(false)
}

/// `mypy.checkmember.check_final_member` (checkmember.py:1358-1363): the
/// pure MRO fold deciding whether any base of `info` declares `name`
/// final. Live-PyO3-object seam, zero wire bytes: for each `TypeInfo` in
/// `info.mro`, read `names.get(name)` and decide finality from the node
/// kind (`is_final_node`: `Var`/`FuncBase`/`Decorator` with `is_final`;
/// `FuncDef`/`OverloadedFuncDef` are `FuncBase` subclasses). Returns
/// `Some(bool)`; any unreadable attribute defers (`None`) so the shim
/// re-runs the untouched pure-Python body.
#[pyfunction]
pub(crate) fn rust_check_final_member(info: &PyAny, name: &str) -> Option<bool> {
    let py = info.py();
    let mro = info.getattr("mro").ok()?;
    let nodes_mod = py.import("mypy.nodes").ok()?;
    let var_cls = nodes_mod.getattr("Var").ok()?.downcast::<PyType>().ok()?;
    let func_base_cls = nodes_mod
        .getattr("FuncBase")
        .ok()?
        .downcast::<PyType>()
        .ok()?;
    let decorator_cls = nodes_mod
        .getattr("Decorator")
        .ok()?
        .downcast::<PyType>()
        .ok()?;
    let entries = mro.iter().ok()?.map(|base| {
        let base = match base {
            Ok(b) => b,
            Err(_) => return None,
        };
        let names = match base.getattr("names") {
            Ok(n) => n,
            Err(_) => return None,
        };
        let sym = match names.call_method1("get", (name,)) {
            Ok(s) => s,
            Err(_) => return None,
        };
        if sym.is_none() {
            return Some(false);
        }
        let node = match sym.getattr("node") {
            Ok(n) => n,
            Err(_) => return None,
        };
        // is_final_node(None) is False; a non-matching kind (TypeInfo,
        // TypeAlias, ...) can never be final, so keep walking.
        if node.is_none() {
            return Some(false);
        }
        let is_kind = match node.is_instance(var_cls) {
            Ok(v) => v,
            Err(_) => return None,
        } || match node.is_instance(func_base_cls) {
            Ok(v) => v,
            Err(_) => return None,
        } || match node.is_instance(decorator_cls) {
            Ok(v) => v,
            Err(_) => return None,
        };
        if !is_kind {
            return Some(false);
        }
        node.getattr("is_final").ok()?.extract::<bool>().ok()
    });
    check_final_member_fold(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use std::collections::HashSet;

    fn make_callable(arg_kinds: Vec<i64>, is_bound: bool) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }],
            arg_kinds,
            arg_names: vec![Some("self".to_string())],
            ret_type: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            name: Some("method".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn make_instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// A resolver with self-only MRO snapshots for the builtin classes the
    /// tuple tests consult (mirrors checkcall.rs's test_resolver).
    fn snap_resolver() -> TypeResolver {
        let mut r = TypeResolver::new();
        for fullname in [
            "builtins.int",
            "builtins.str",
            "builtins.object",
            "builtins.tuple",
            "builtins.function",
        ] {
            let mut s = crate::typeinfo::TypeInfoSnapshot {
                fullname: fullname.to_string(),
                name: fullname.to_string(),
                ..Default::default()
            };
            s.mro.push(fullname.to_string());
            s.has_base.insert(fullname.to_string());
            if fullname != "builtins.object" {
                s.mro.push("builtins.object".to_string());
                s.has_base.insert("builtins.object".to_string());
            }
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn make_overloaded(items: Vec<Type>) -> Type {
        Type::Overloaded { items }
    }

    #[test]
    fn test_bind_self_fast_strips_first_arg() {
        let method = make_callable(vec![ARG_POS], false);
        let result = bind_self_fast_inner(&method).expect("expected bound callable");
        match result {
            Type::CallableType {
                arg_types,
                arg_kinds,
                arg_names,
                is_bound,
                ..
            } => {
                assert!(arg_types.is_empty());
                assert!(arg_kinds.is_empty());
                assert!(arg_names.is_empty());
                assert!(is_bound);
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_bind_self_fast_overloaded() {
        let item1 = make_callable(vec![ARG_POS], false);
        let item2 = make_callable(vec![ARG_POS], false);
        let overloaded = make_overloaded(vec![item1, item2]);
        let result = bind_self_fast_inner(&overloaded).expect("expected bound overloaded");
        match result {
            Type::Overloaded { items } => {
                assert_eq!(items.len(), 2);
                for item in &items {
                    match item {
                        Type::CallableType {
                            is_bound,
                            arg_types,
                            ..
                        } => {
                            assert!(*is_bound);
                            assert!(arg_types.is_empty());
                        }
                        _ => panic!("expected CallableType"),
                    }
                }
            }
            _ => panic!("expected Overloaded"),
        }
    }

    #[test]
    fn test_bind_self_fast_keeps_star_args() {
        let method = make_callable(vec![ARG_STAR], false);
        let result = bind_self_fast_inner(&method).expect("star-arg method returned unchanged");
        match result {
            Type::CallableType {
                arg_types,
                is_bound,
                ..
            } => {
                assert_eq!(arg_types.len(), 1);
                assert!(!is_bound);
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_bind_self_fast_keeps_star2() {
        let method = make_callable(vec![ARG_STAR2], false);
        let result = bind_self_fast_inner(&method).expect("star2-arg method returned unchanged");
        match result {
            Type::CallableType {
                arg_types,
                is_bound,
                ..
            } => {
                assert_eq!(arg_types.len(), 1);
                assert!(!is_bound);
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_bind_self_fast_keeps_empty_args() {
        let method = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int")),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let result = bind_self_fast_inner(&method).expect("empty-arg method returned unchanged");
        match result {
            Type::CallableType { is_bound, .. } => assert!(!is_bound),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_bind_self_fast_defers_non_callable() {
        let inst = make_instance("builtins.int");
        assert!(bind_self_fast_inner(&inst).is_none());
    }

    #[test]
    fn test_bind_self_fast_defers_empty_overloaded() {
        let overloaded = make_overloaded(vec![]);
        assert!(bind_self_fast_inner(&overloaded).is_none());
    }

    #[test]
    fn test_bind_self_fast_round_trip() {
        let method = make_callable(vec![ARG_POS], false);
        let encoded = encode_type(&method).unwrap();
        let decoded = decode_type(&encoded).unwrap();
        let result = bind_self_fast_inner(&decoded).expect("expected bound callable");
        match result {
            Type::CallableType { is_bound, .. } => assert!(is_bound),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_classify_instance() {
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        assert_eq!(classify_member_access_inner(&inst, &resolver), MA_INSTANCE);
    }

    #[test]
    fn test_classify_any() {
        let resolver = TypeResolver::new();
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(classify_member_access_inner(&any, &resolver), MA_ANY);
    }

    #[test]
    fn test_classify_union() {
        let resolver = TypeResolver::new();
        let union = Type::UnionType {
            items: vec![make_instance("builtins.int"), make_instance("builtins.str")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(classify_member_access_inner(&union, &resolver), MA_UNION);
    }

    #[test]
    fn test_classify_none_type() {
        let resolver = TypeResolver::new();
        assert_eq!(
            classify_member_access_inner(&Type::NoneType, &resolver),
            MA_NONE
        );
    }

    #[test]
    fn test_classify_deleted() {
        let resolver = TypeResolver::new();
        let deleted = Type::DeletedType {
            source: Some("x".to_string()),
        };
        assert_eq!(
            classify_member_access_inner(&deleted, &resolver),
            MA_DELETED
        );
    }

    #[test]
    fn test_classify_uninhabited() {
        let resolver = TypeResolver::new();
        let uninh = Type::UninhabitedType { ambiguous: false };
        assert_eq!(
            classify_member_access_inner(&uninh, &resolver),
            MA_UNINHABITED
        );
    }

    #[test]
    fn test_classify_type_type() {
        let resolver = TypeResolver::new();
        let tt = Type::TypeType {
            item: Box::new(make_instance("builtins.int")),
            is_type_form: false,
        };
        assert_eq!(classify_member_access_inner(&tt, &resolver), MA_TYPE_TYPE);
    }

    #[test]
    fn test_classify_typed_dict() {
        let resolver = TypeResolver::new();
        let td = Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict")),
            items: vec![],
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        assert_eq!(classify_member_access_inner(&td, &resolver), MA_TYPEDDICT);
    }

    #[test]
    fn test_classify_typevar() {
        let resolver = TypeResolver::new();
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(make_instance("builtins.object")),
            default: Box::new(make_instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(classify_member_access_inner(&tv, &resolver), MA_TYPEVAR);
    }

    #[test]
    fn test_classify_tuple() {
        let resolver = TypeResolver::new();
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple")),
            items: vec![make_instance("builtins.int")],
            implicit: false,
        };
        assert_eq!(classify_member_access_inner(&tup, &resolver), MA_TUPLE);
    }

    #[test]
    fn test_classify_literal() {
        let resolver = TypeResolver::new();
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int")),
            value: crate::wire::LiteralValue::Int(42),
        };
        assert_eq!(
            classify_member_access_inner(&lit, &resolver),
            MA_LITERAL_OR_FUNC
        );
    }

    #[test]
    fn test_classify_plain_callable_not_type_obj() {
        let resolver = TypeResolver::new();
        let callable = make_callable(vec![ARG_POS], false);
        // fallback is builtins.function, not a metaclass -> MA_LITERAL_OR_FUNC
        assert_eq!(
            classify_member_access_inner(&callable, &resolver),
            MA_LITERAL_OR_FUNC
        );
    }

    // --- freshen_signature / static_member_tail ---

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    #[test]
    fn test_freshen_signature_overloaded_per_item() {
        // Overloaded freshens per item with the shared counter. Non-generic
        // items translate in place and leave the counter untouched.
        let sig = Type::Overloaded {
            items: vec![
                make_callable(vec![ARG_POS], false),
                make_callable(vec![ARG_POS], true),
            ],
        };
        let mut next_raw_id = 5;
        let mut changed = false;
        let result = freshen_signature(&sig, &mut next_raw_id, &mut changed, false);
        let Type::Overloaded { items } = result.expect("non-generic overload freshens") else {
            panic!("expected Overloaded");
        };
        assert_eq!(items.len(), 2);
        assert!(!changed);
        assert_eq!(next_raw_id, 5);
    }

    #[test]
    fn test_freshen_signature_overloaded_any_item_defers() {
        // A non-wire item (Parameters) defers the whole Overloaded.
        let sig = Type::Overloaded {
            items: vec![Type::Parameters(crate::wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            })],
        };
        let mut next_raw_id = 5;
        let mut changed = false;
        assert!(freshen_signature(&sig, &mut next_raw_id, &mut changed, false).is_none());
        assert_eq!(next_raw_id, 5);
    }

    #[test]
    fn test_static_member_tail_overloaded_signature_gate() {
        // The legacy seam defers an Overloaded at the signature gate; the
        // dispatch may allow it. Signature-gate check runs before any map,
        // so an empty resolver still shows the allow=false distinction.
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        let sig = Type::Overloaded {
            items: vec![make_callable(vec![ARG_POS], false)],
        };
        assert!(static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None
        )
        .is_none());
    }

    #[test]
    fn test_static_member_tail_non_instance_defers() {
        let resolver = TypeResolver::new();
        let sig = make_callable(vec![ARG_POS], false);
        assert!(static_member_tail(
            &Type::NoneType,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None
        )
        .is_none());
    }

    fn make_meta_tvar(raw_id: i64, namespace: &str, meta_level: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: namespace.to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level,
        }
    }

    #[test]
    fn test_static_member_tail_freezes_meta_type_vars() {
        // Python's tail runs freeze_all_type_vars after the expand: empty-args
        // expand returns the signature unchanged, so the freeze is the only
        // rewrite; it must hit ret type and variables (wire broke sharing).
        let resolver = snap_resolver();
        let tvar = make_meta_tvar(50, "", 1);
        let mut sig = make_callable(vec![], false);
        if let Type::CallableType {
            ret_type,
            variables,
            ..
        } = &mut sig
        {
            **ret_type = tvar.clone();
            variables.push(tvar);
        }
        let inst = make_instance("builtins.int");
        let result = static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None,
        )
        .expect("expected Some result");
        match result {
            Type::CallableType {
                ret_type,
                variables,
                ..
            } => {
                match *ret_type {
                    Type::TypeVarType {
                        raw_id, meta_level, ..
                    } => {
                        assert_eq!(raw_id, 50);
                        assert_eq!(meta_level, 0);
                    }
                    other => panic!("expected TypeVarType ret, got {other:?}"),
                }
                assert_eq!(variables.len(), 1);
                match &variables[0] {
                    Type::TypeVarType {
                        raw_id, meta_level, ..
                    } => {
                        assert_eq!(*raw_id, 50);
                        assert_eq!(*meta_level, 0);
                    }
                    other => panic!("expected TypeVarType variable, got {other:?}"),
                }
            }
            other => panic!("expected CallableType result, got {other:?}"),
        }
    }

    #[test]
    fn test_static_member_tail_defers_foreign_leftover_tvar() {
        // A tvar outside every `variables` list AND outside the receiver's
        // args broke downstream inference when returned decoded (28 testcheck
        // defenses, #1277): the narrowed survivor gate defers to Python.
        let resolver = snap_resolver();
        let mut sig = make_callable(vec![], false);
        if let Type::CallableType { ret_type, .. } = &mut sig {
            **ret_type = make_meta_tvar(7, "__main__.B@3", 1);
        }
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![make_instance("builtins.str")],
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None,
        )
        .is_none());
    }

    #[test]
    fn test_static_member_tail_leaves_bound_receiver_arg_tvar_untouched() {
        // A caller tvar among the receiver's args rides through when it is
        // bound (meta_level == 0): Python's freeze only rewrites `variables`
        // entries, so it keeps its original meta level (#1277).
        let resolver = snap_resolver();
        let mut sig = make_callable(vec![], false);
        if let Type::CallableType { ret_type, .. } = &mut sig {
            **ret_type = make_meta_tvar(7, "__main__.B@3", 0);
        }
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![make_meta_tvar(7, "__main__.B@3", 0)],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None,
        );
        match result {
            Some(Type::CallableType { ret_type, .. }) => match *ret_type {
                Type::TypeVarType {
                    raw_id, meta_level, ..
                } => {
                    assert_eq!(raw_id, 7);
                    assert_eq!(meta_level, 0);
                }
                other => panic!("expected TypeVarType ret, got {other:?}"),
            },
            other => panic!("expected frozen callable, got {other:?}"),
        }
    }

    #[test]
    fn test_static_member_tail_defers_meta1_receiver_arg_tvar() {
        // A caller's fresh (meta_level > 0) unification variable must NOT
        // ride through the decoded IAMA tail even when it is among the
        // receiver's args: the wire round-trip arrives here as a doppelganger
        // with no object-identity link to anything Python-side, and the
        // solve's freshening fuses identities that live in one object, so
        // the declared-var association breaks and the target collapses to
        // `Never` (issue #1286). Defer to the pure-Python body.
        let resolver = snap_resolver();
        let mut sig = make_callable(vec![], false);
        if let Type::CallableType { ret_type, .. } = &mut sig {
            **ret_type = make_meta_tvar(7, "__main__.B@3", 1);
        }
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![make_meta_tvar(7, "__main__.B@3", 1)],
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None,
        )
        .is_none());
    }

    #[test]
    fn test_static_member_tail_defers_unpack_occurrence() {
        // An UnpackType survivor defers outright, like the pre-port gate's
        // special-shape class (#1277).
        let resolver = snap_resolver();
        let mut sig = make_callable(vec![], false);
        if let Type::CallableType { ret_type, .. } = &mut sig {
            **ret_type = Type::UnpackType {
                typ: Box::new(Type::NoneType {}),
            };
        }
        let inst = make_instance("builtins.int");
        assert!(static_member_tail(
            &inst,
            &sig,
            "builtins.int",
            false,
            &resolver,
            false,
            false,
            None,
        )
        .is_none());
    }

    // --- analyze_member_access_inner pass-through recursion ---

    #[test]
    fn test_memacc_instance_no_ctx_defers() {
        // Instance operand with no dispatch context (the union-item and
        // NoneType recursion callers) defers; those paths keep Instance
        // operands in Python.
        let resolver = TypeResolver::new();
        assert!(
            analyze_member_access_inner(&make_instance("builtins.int"), None, &resolver).is_none()
        );
    }

    #[test]
    fn test_memacc_tuple_instance_fallback_no_ctx_defers() {
        // A TupleType whose partial fallback is an Instance recurses into
        // the Instance branch; without a dispatch context that defers.
        let fb = make_instance("builtins.int");
        let tup = Type::TupleType {
            partial_fallback: Box::new(fb),
            items: vec![make_instance("builtins.int")],
            implicit: false,
        };
        let resolver = TypeResolver::new();
        assert!(analyze_member_access_inner(&tup, None, &resolver).is_none());
    }

    #[test]
    fn test_memacc_typevar_instance_bound_no_ctx_defers() {
        // TypeVarType with no value restriction and an Instance upper bound
        // recurses on the bound; without a dispatch context it defers.
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: String::new(),
            values: vec![],
            upper_bound: Box::new(make_instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 12,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        let resolver = TypeResolver::new();
        assert!(analyze_member_access_inner(&tv, None, &resolver).is_none());
    }

    #[test]
    fn test_memacc_typevar_any_bound_recurses() {
        // TypeVarType with an AnyType upper bound: the recursion resolves
        // AnyType -> AnyType(from_another_any), proving the fallback
        // threshold passes control to the pure transform.
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: String::new(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 2,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 12,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        let resolver = TypeResolver::new();
        let result = analyze_member_access_inner(&tv, None, &resolver)
            .expect("Any upper bound transforms natively");
        match result {
            Type::AnyType { type_of_any: 7, .. } => {}
            other => panic!("expected from_another_any, got {other:?}"),
        }
    }

    // --- analyze_none_bool_type ---

    #[test]
    fn test_none_access_bool_returns_callable_literal_false() {
        let result = analyze_none_bool_type();
        match result {
            Type::CallableType {
                arg_types,
                ret_type,
                ..
            } => {
                assert!(arg_types.is_empty());
                match &*ret_type {
                    Type::LiteralType {
                        value: crate::wire::LiteralValue::Bool(false),
                        ..
                    } => {}
                    other => panic!("expected LiteralType(False), got {other:?}"),
                }
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    // --- analyze_typeddict_access_inner ---

    #[test]
    fn test_typeddict_delitem_returns_callable() {
        let resolver = TypeResolver::new();
        let result = analyze_typeddict_access_inner("__delitem__", true, &resolver)
            .expect("__delitem__ returns a callable");
        match result {
            Type::CallableType {
                arg_types,
                arg_kinds,
                ret_type,
                name,
                ..
            } => {
                assert_eq!(arg_types.len(), 1);
                assert_eq!(arg_kinds, vec![ARG_POS]);
                assert!(matches!(&*ret_type, Type::NoneType));
                assert_eq!(name.as_deref(), Some("__delitem__"));
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    #[test]
    fn test_typeddict_setitem_defers() {
        let resolver = TypeResolver::new();
        assert!(analyze_typeddict_access_inner("__setitem__", true, &resolver).is_none());
    }

    #[test]
    fn test_typeddict_other_name_defers() {
        let resolver = TypeResolver::new();
        assert!(analyze_typeddict_access_inner("foo", true, &resolver).is_none());
    }

    // --- analyze_enum_class_attribute_access_inner ---

    #[test]
    fn test_enum_access_non_instance_defers() {
        let resolver = TypeResolver::new();
        let any_t = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        assert!(analyze_enum_class_attribute_access_inner(&any_t, "FOO", &resolver).is_none());
    }

    #[test]
    fn test_enum_access_excluded_name_defers() {
        let resolver = TypeResolver::new();
        let inst = make_instance("mypy_types.Color");
        assert!(analyze_enum_class_attribute_access_inner(&inst, "_ignore_", &resolver).is_none());
    }

    #[test]
    fn test_enum_access_missing_snapshot_defers() {
        let resolver = TypeResolver::new();
        let inst = make_instance("mypy_types.Color");
        // No snapshot in an empty resolver -> defer.
        assert!(analyze_enum_class_attribute_access_inner(&inst, "RED", &resolver).is_none());
    }

    // --- analyze_descriptor_access_inner ---

    #[test]
    fn test_descriptor_access_lvalue_non_descriptor_returns_orig() {
        // Lvalue access to a plain non-descriptor Instance passes through
        // (checkmember.py:1204-1206: neither __get__ nor __set__).
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.has_base.insert("desc.D".to_string());
        resolver.insert("desc.D".to_string(), snap);
        resolver.insert(
            "builtins.object".to_string(),
            TypeInfoSnapshot {
                fullname: "builtins.object".to_string(),
                name: "object".to_string(),
                mro: vec!["builtins.object".to_string()],
                ..Default::default()
            },
        );
        let inst = make_instance("desc.D");
        let r = analyze_descriptor_access_inner(&inst, true, true, &resolver);
        assert!(matches!(r, Some(DescriptorDecision::Orig)));
    }

    #[test]
    fn test_descriptor_access_lvalue_with_get_defers() {
        // Lvalue + readable __get__ (no __set__) needs the heavy tail ->
        // defer.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.has_base.insert("desc.D".to_string());
        snap.member_info.insert("__get__".to_string(), (true, true));
        resolver.insert("desc.D".to_string(), snap);
        let inst = make_instance("desc.D");
        assert!(analyze_descriptor_access_inner(&inst, true, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_lvalue_with_set_defers() {
        // Lvalue + readable __set__ routes to analyze_descriptor_assign
        // (checker state) -> defer.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.has_base.insert("desc.D".to_string());
        snap.member_info.insert("__set__".to_string(), (true, true));
        resolver.insert("desc.D".to_string(), snap);
        let inst = make_instance("desc.D");
        assert!(analyze_descriptor_access_inner(&inst, true, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_missing_snapshot_defers() {
        // No class snapshot in an empty resolver -> the member-presence
        // read defers.
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        assert!(analyze_descriptor_access_inner(&inst, false, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_type_alias_defers() {
        let resolver = TypeResolver::new();
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "some.Alias".to_string(),
        };
        assert!(analyze_descriptor_access_inner(&alias, true, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_union_with_lvalue_instance_defers() {
        let resolver = TypeResolver::new();
        let union = make_union(vec![make_instance("builtins.int")]);
        // lvalue Instance item defers (missing snapshot) -> union defers.
        assert!(analyze_descriptor_access_inner(&union, true, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_union_all_orig_returns_value() {
        // A union whose items all decide Orig maps to a simplified union
        // Value (tag 1), not Orig.
        let mut resolver = TypeResolver::new();
        for name in ["desc.D", "builtins.object"] {
            let mut snap = TypeInfoSnapshot {
                fullname: name.to_string(),
                name: name.rsplit('.').next().unwrap().to_string(),
                mro: vec![name.to_string(), "builtins.object".to_string()],
                ..Default::default()
            };
            snap.has_base.insert(name.to_string());
            if name == "builtins.object" {
                snap.mro = vec!["builtins.object".to_string()];
            }
            resolver.insert(name.to_string(), snap);
        }
        let union = make_union(vec![make_instance("desc.D"), make_instance("desc.D")]);
        let r = analyze_descriptor_access_inner(&union, false, true, &resolver);
        assert!(matches!(r, Some(DescriptorDecision::Value(_))));
    }

    #[test]
    fn test_descriptor_access_union_with_get_item_defers() {
        // One __get__-bearing item defers the whole union (the item needs
        // the checker-state tail).
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.has_base.insert("desc.D".to_string());
        snap.member_info.insert("__get__".to_string(), (true, true));
        resolver.insert("desc.D".to_string(), snap);
        let union = make_union(vec![make_instance("desc.D"), make_instance("desc.D")]);
        assert!(analyze_descriptor_access_inner(&union, false, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_non_lvalue_no_get_returns_orig() {
        // Class with no readable __get__: non-lvalue access returns the
        // descriptor unchanged (checkmember.py:1189-1190) — as an Orig
        // decision, the live object stays Python-side.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        // Default member_info is empty (no __get__); is_base for itself.
        snap.has_base.insert("desc.D".to_string());
        resolver.insert("desc.D".to_string(), snap);
        // mro walk requires every entry resolvable; provide object.
        resolver.insert(
            "builtins.object".to_string(),
            TypeInfoSnapshot {
                fullname: "builtins.object".to_string(),
                name: "object".to_string(),
                mro: vec!["builtins.object".to_string()],
                ..Default::default()
            },
        );
        let inst = make_instance("desc.D");
        let r = analyze_descriptor_access_inner(&inst, false, true, &resolver);
        assert!(matches!(r, Some(DescriptorDecision::Orig)));
    }

    #[test]
    fn test_descriptor_access_non_lvalue_with_get_defers() {
        // Class with a readable __get__: non-lvalue access needs the heavy
        // __get__-analysis path -> defer.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "desc.D".to_string(),
            name: "D".to_string(),
            mro: vec!["desc.D".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.has_base.insert("desc.D".to_string());
        snap.member_info.insert("__get__".to_string(), (true, true));
        resolver.insert("desc.D".to_string(), snap);
        let inst = make_instance("desc.D");
        assert!(analyze_descriptor_access_inner(&inst, false, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_non_instance_returns_orig() {
        // Every non-Instance proper type returns orig unconditionally
        // (checkmember.py:1416-1417), both access kinds.
        let resolver = TypeResolver::new();
        let any_t = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        for typ in [Type::NoneType, any_t] {
            assert!(matches!(
                analyze_descriptor_access_inner(&typ, false, true, &resolver),
                Some(DescriptorDecision::Orig)
            ));
            assert!(matches!(
                analyze_descriptor_access_inner(&typ, true, true, &resolver),
                Some(DescriptorDecision::Orig)
            ));
        }
    }

    #[test]
    fn test_descriptor_access_tuple_returns_orig() {
        // TupleType is a non-Instance proper type: Python returns
        // orig_descriptor_type unconditionally (checkmember.py:1416),
        // regardless of the fallback's __get__ presence or is_lvalue.
        let resolver = TypeResolver::new();
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple")),
            items: vec![make_instance("builtins.str")],
            implicit: false,
        };
        for lvalue in [false, true] {
            assert!(matches!(
                analyze_descriptor_access_inner(&tup, lvalue, true, &resolver),
                Some(DescriptorDecision::Orig)
            ));
        }
    }

    // --- check_self_arg_inner ---

    /// Build a resolver snapshot for a class with is_base set for itself.
    fn snap_with_self_base(fullname: &str) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_owned(),
            mro: vec![fullname.to_owned(), "builtins.object".to_owned()],
            ..Default::default()
        };
        s.has_base.insert(fullname.to_owned());
        s.has_base.insert("builtins.object".to_owned());
        s
    }

    /// Resolver with builtins.int and builtins.type snapshots.
    fn int_resolver() -> TypeResolver {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.int".to_owned(),
            snap_with_self_base("builtins.int"),
        );
        r
    }

    fn make_callable_with_selfarg(selfarg: Type, arg_kind: i64) -> Type {
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![selfarg, make_instance("builtins.int")],
            arg_kinds: vec![arg_kind, ARG_POS],
            arg_names: vec![Some("self".to_string()), None],
            ret_type: Box::new(make_instance("builtins.int")),
            name: Some("f".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn test_check_self_arg_matching_selfarg() {
        let resolver = int_resolver();
        let selfarg = make_instance("builtins.int");
        let item1 = make_callable_with_selfarg(selfarg.clone(), ARG_POS);
        let result = check_self_arg_inner(
            &item1,
            &make_instance("builtins.int"),
            false,
            "f",
            true,
            &resolver,
            false,
        )
        .expect("matching selfarg returns item");
        match result {
            Type::CallableType { .. } => {}
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    #[test]
    fn test_check_self_arg_empty_items_returns_functype() {
        let resolver = TypeResolver::new();
        assert!(check_self_arg_inner(
            &make_instance("builtins.int"),
            &make_instance("builtins.int"),
            false,
            "f",
            true,
            &resolver,
            false
        )
        .is_none());
    }

    #[test]
    fn test_check_self_arg_empty_args_defers() {
        let resolver = TypeResolver::new();
        let empty = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int")),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        // Empty arg_types -> Python reports no_formal_self -> defer.
        assert!(check_self_arg_inner(
            &empty,
            &make_instance("builtins.int"),
            false,
            "f",
            true,
            &resolver,
            false
        )
        .is_none());
    }

    #[test]
    fn test_check_self_arg_classmethod_wraps_type_type() {
        let resolver = TypeResolver::new();
        let selfarg = make_instance("builtins.type");
        let item = make_callable_with_selfarg(selfarg.clone(), ARG_POS);
        // dispatched is an Instance; classmethod wraps in TypeType, so the
        // is_subtype check against Instance selfarg may defer. Verify it
        // either returns a callable or defers — but never panics.
        let _ = check_self_arg_inner(
            &item,
            &make_instance("builtins.int"),
            true,
            "f",
            true,
            &resolver,
            false,
        );
    }

    // --- expand_without_binding_inner ---

    #[test]
    fn test_expand_without_binding_preserves_type() {
        let resolver = int_resolver();
        let typ = make_instance("builtins.int");
        let itype = make_instance("builtins.int");
        let result = expand_without_binding_inner(&typ, &itype, false, 100, true, &resolver)
            .expect("simple type expands");
        assert!(!result.1); // no freshening of non-generic type
        match result.2 {
            Type::Instance { type_ref, .. } => assert_eq!(type_ref, "builtins.int"),
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn test_expand_without_binding_freshens_generic_callable() {
        let resolver = int_resolver();
        // A generic callable def[T](x: T) -> T
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(make_instance("builtins.object")),
            default: Box::new(make_instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        };
        let callable = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![tvar.clone()],
            arg_kinds: vec![ARG_POS],
            arg_names: vec![Some("x".to_string())],
            ret_type: Box::new(tvar.clone()),
            name: Some("f".to_string()),
            variables: vec![tvar],
            type_guard: None,
            type_is: None,
        };
        let itype = make_instance("builtins.int");
        let result = expand_without_binding_inner(&callable, &itype, false, 100, true, &resolver);
        // Freshening a generic callable succeeds; expand may defer due to
        // unbound T, but freshen itself must have run (changed=true).
        assert!(result.is_some());
        let (next_raw_id, changed, _t) = result.unwrap();
        assert!(changed);
        assert!(next_raw_id > 100);
    }

    // --- expand_and_bind_callable_inner ---

    #[test]
    fn test_expand_and_bind_trivial_self() {
        let resolver = int_resolver();
        let selfarg = make_instance("builtins.int");
        let callable = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![selfarg, make_instance("builtins.int")],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("self".to_string()), None],
            ret_type: Box::new(make_instance("builtins.int")),
            name: Some("f".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let itype = make_instance("builtins.int");
        let result = expand_and_bind_callable_inner(&callable, &itype, false, 100, true, &resolver)
            .expect("trivial self callable binds");
        match result.2 {
            Type::CallableType {
                is_bound,
                arg_types,
                ..
            } => {
                assert!(is_bound);
                assert_eq!(arg_types.len(), 1);
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    // --- add_class_tvars_inner ---

    #[test]
    fn test_add_class_tvars_callable_classmethod() {
        let resolver = int_resolver();
        let cls = make_instance("builtins.type");
        let callable = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![cls, make_instance("builtins.int")],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("cls".to_string()), None],
            ret_type: Box::new(make_instance("builtins.int")),
            name: Some("foo".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let result = add_class_tvars_inner(
            &callable,
            None,  // isuper: None avoids the resolver lookup
            true,  // is_classmethod
            true,  // is_trivial_self
            false, // preserve_type_var_ids
            &[],
            100,
            true,
            &resolver,
        );
        // Callable is non-generic; freshen leaves it unchanged; bind_self_fast
        // strips cls; isuper=None skips expand. All succeed.
        match result {
            Some((next_raw_id, changed, t)) => {
                assert!(!changed);
                assert_eq!(next_raw_id, 100);
                match t {
                    Type::CallableType {
                        is_bound,
                        arg_types,
                        ..
                    } => {
                        assert!(is_bound);
                        assert_eq!(arg_types.len(), 1);
                    }
                    other => panic!("expected CallableType, got {other:?}"),
                }
            }
            None => panic!("expected success"),
        }
    }

    #[test]
    fn test_add_class_tvars_not_classmethod_defers() {
        let resolver = TypeResolver::new();
        let callable = make_callable(vec![ARG_POS], false);
        assert!(add_class_tvars_inner(
            &callable,
            None,
            false, // is_classmethod
            true,  // is_trivial_self
            false,
            &[],
            100,
            true,
            &resolver,
        )
        .is_none());
    }

    #[test]
    fn test_add_class_tvars_already_bound_defers() {
        let resolver = TypeResolver::new();
        let callable = make_callable(vec![ARG_POS], true);
        assert!(add_class_tvars_inner(
            &callable,
            None,
            true, // is_classmethod
            true, // is_trivial_self
            false,
            &[],
            100,
            true,
            &resolver,
        )
        .is_none());
    }

    #[test]
    fn test_add_class_tvars_overloaded_recursion() {
        let resolver = int_resolver();
        let callable = make_callable(vec![ARG_POS], false);
        let overloaded = make_overloaded(vec![callable.clone(), callable]);
        let result = add_class_tvars_inner(
            &overloaded,
            None, // isuper: None avoids the resolver lookup
            true,
            true,
            false,
            &[],
            100,
            true,
            &resolver,
        );
        match result {
            Some((_, _, Type::Overloaded { items })) => assert_eq!(items.len(), 2),
            other => panic!("expected Overloaded, got {other:?}"),
        }
    }

    // --- descriptor_has_get_set helpers ---

    #[test]
    fn test_descriptor_has_get_set_missing_snapshot_defers() {
        let resolver = TypeResolver::new();
        // has_readable_member_by_ref defers when the snapshot is missing.
        assert!(has_readable_member_by_ref(&resolver, "descriptor.D", "__get__").is_none());
    }

    // --- rust_analyze_member_method / member_method_inner ---

    fn make_g_resolver() -> TypeResolver {
        // G[T] with one type var T (raw_id 1); the snapshot must carry
        // type_vars_with_variance + type_var_raw_ids, and A + builtins.object
        // so check_self_arg subtype checks against `A` resolve.
        let mut r = TypeResolver::new();
        for fullname in ["A", "builtins.object"] {
            r.insert(
                fullname.to_string(),
                TypeInfoSnapshot {
                    fullname: fullname.to_string(),
                    name: fullname.to_string(),
                    mro: vec![fullname.to_string(), "builtins.object".to_string()],
                    has_base: [fullname.to_string(), "builtins.object".to_string()]
                        .into_iter()
                        .collect(),
                    ..Default::default()
                },
            );
        }
        let mut snap = TypeInfoSnapshot {
            fullname: "G".to_string(),
            name: "G".to_string(),
            mro: vec!["G".to_string(), "builtins.object".to_string()],
            has_base: ["G".to_string(), "builtins.object".to_string()]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        snap.type_vars_with_variance = vec![("T".to_string(), 0, 0)];
        snap.type_var_raw_ids = vec![1];
        r.insert("G".to_string(), snap);
        r
    }

    fn make_ga_instance() -> Type {
        Type::Instance {
            type_ref: "G".to_string(),
            args: vec![make_instance("A")],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_g_method() -> Type {
        // def foo(self: G[T], x: A) -> G[T]
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![make_ga_instance(), make_instance("A")],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("self".to_string()), Some("x".to_string())],
            ret_type: Box::new(make_ga_instance()),
            name: Some("foo".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn test_member_method_bind_and_expand() {
        let resolver = make_g_resolver();
        let method = make_g_method();
        // Step 1: map_instance_to_supertype fast path (G == G).
        let mapped =
            crate::subtypes::map_instance_to_supertype("G", &[make_instance("A")], "G", &resolver);
        assert_eq!(mapped, Some(vec![make_instance("A")]));
        let mapped_instance = Type::Instance {
            type_ref: "G".to_string(),
            args: vec![make_instance("A")],
            last_known_value: None,
            extra_attrs: None,
        };
        // Step 1b: check_self_arg filter passes for the generic `self: G[T]`
        // receiver `G[A]` (the TypeVar overlaps with the concrete arg).
        let filtered = check_self_arg_inner(
            &method,
            &make_ga_instance(),
            false,
            "foo",
            true,
            &resolver,
            false,
        );
        assert!(filtered.is_some(), "TypeVar self must survive the filter");
        // Step 2: expand runs on the unbound callable; the Callable arm of
        // expand_type_inner defers on is_bound, so bind must come after.
        let expanded = crate::expandtype::expand_type_by_instance_core(
            &method,
            &mapped_instance,
            &resolver,
            true,
        )
        .expect("expand should succeed");
        assert!(!crate::expandtype::result_has_typevar(&expanded));
        // Step 3: bind_self_fast_inner strips self (G[T]).
        let bound = bind_self_fast_inner(&expanded).expect("bind should succeed");
        match &bound {
            Type::CallableType {
                arg_types,
                is_bound,
                ..
            } => {
                assert!(is_bound);
                assert_eq!(arg_types.len(), 1);
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
        // Step 4: member_method_inner composes check_self_arg (receiver `G[A]`
        // with `self: G[A]` passes the subtype filter), then expands and
        // binds; an incompatible self defers so Python reports the error.
        let concrete_self_method = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![make_ga_instance(), make_instance("A")],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("self".to_string()), Some("x".to_string())],
            ret_type: Box::new(make_instance("A")),
            name: Some("foo".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let result = member_method_inner(
            &make_ga_instance(),
            &concrete_self_method,
            "G",
            &make_ga_instance(),
            "foo",
            &resolver,
            true,
            false, // is_class
            false, // suppress_self_fail
        );
        match result {
            Some(Type::CallableType { is_bound, .. }) => assert!(is_bound),
            other => panic!("expected bound CallableType, got {other:?}"),
        }
        // An incompatible self arg defers so Python reports "Invalid self
        // argument" (mirrors check_self_arg zero-match).
        let bad_self_method = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![make_instance("C"), make_instance("A")],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("self".to_string()), Some("x".to_string())],
            ret_type: Box::new(make_instance("A")),
            name: Some("foo".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let result = member_method_inner(
            &make_ga_instance(),
            &bad_self_method,
            "G",
            &make_ga_instance(),
            "foo",
            &resolver,
            true,
            false, // is_class
            false, // suppress_self_fail
        );
        assert!(result.is_none(), "incompatible self must defer");
    }

    #[test]
    fn test_member_method_classmethod_defers() {
        let resolver = make_g_resolver();
        let method = make_g_method();
        let result = member_method_inner(
            &make_ga_instance(),
            &method,
            "G",
            &make_ga_instance(),
            "foo",
            &resolver,
            true,
            true,  // is_class
            false, // suppress_self_fail
        );
        assert!(result.is_none(), "classmethod must defer");
    }

    fn make_ab_resolver() -> TypeResolver {
        // A (plain) with subclass B: map_instance_to_supertype needs B's
        // snapshot to carry A as a reachable base.
        let mut r = TypeResolver::new();
        let mut snap = |fullname: &str, mro: &[&str]| {
            r.insert(
                fullname.to_string(),
                TypeInfoSnapshot {
                    fullname: fullname.to_string(),
                    name: fullname.to_string(),
                    mro: mro.iter().map(|s| s.to_string()).collect(),
                    has_base: mro.iter().map(|s| s.to_string()).collect(),
                    ..Default::default()
                },
            );
        };
        snap(
            "builtins.function",
            &["builtins.function", "builtins.object"],
        );
        snap("builtins.object", &["builtins.object"]);
        snap("A", &["A", "builtins.object"]);
        // B's snapshot needs the serialized base Instance blob: the map
        // derivation walk decodes `bases` blobs, it does not use `mro`.
        let base_a = encode_type(&Type::Instance {
            type_ref: "A".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        })
        .expect("encode base A");
        r.insert(
            "B".to_string(),
            TypeInfoSnapshot {
                fullname: "B".to_string(),
                name: "B".to_string(),
                mro: vec![
                    "B".to_string(),
                    "A".to_string(),
                    "builtins.object".to_string(),
                ],
                has_base: [
                    "B".to_string(),
                    "A".to_string(),
                    "builtins.object".to_string(),
                ]
                .into_iter()
                .collect(),
                bases: vec![base_a],
                ..Default::default()
            },
        );
        r
    }

    fn make_ab_method(variables: Vec<Type>) -> Type {
        // def foo(self: A, x: A) -> A, optionally `def foo[T](...)` via
        // `variables`.
        let (x_type, ret_type, variables) = if variables.is_empty() {
            (
                Box::new(make_instance("A")),
                Box::new(make_instance("A")),
                variables,
            )
        } else {
            let tvar = Type::TypeVarType {
                name: "T".to_string(),
                fullname: "A.T".to_string(),
                raw_id: 1,
                namespace: "A".to_string(),
                values: vec![],
                upper_bound: Box::new(make_instance("builtins.object")),
                default: Box::new(make_instance("builtins.object")),
                variance: 0,
                meta_level: 0,
            };
            (Box::new(tvar.clone()), Box::new(tvar), variables)
        };
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![make_instance("A"), *x_type],
            arg_kinds: vec![ARG_POS, ARG_POS],
            arg_names: vec![Some("self".to_string()), Some("x".to_string())],
            ret_type,
            name: Some("foo".to_string()),
            variables,
            type_guard: None,
            type_is: None,
        }
    }

    fn make_b_receiver() -> Type {
        Type::Instance {
            type_ref: "B".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn test_member_method_subclass_receiver_nongeneric_completes() {
        // B receiver, method on A, variable-free signature: Python's full
        // bind_self takes only the strip path, so the filter + map +
        // expand + strip composition below decides the case natively.
        let resolver = make_ab_resolver();
        let method = make_ab_method(vec![]);
        let b = make_b_receiver();
        let result = member_method_inner(
            &b, &method, "A", &b, "foo", &resolver, true, false, // is_class
            false, // suppress_self_fail
        );
        match result {
            Some(Type::CallableType {
                arg_types,
                is_bound,
                ..
            }) => {
                assert!(is_bound);
                assert_eq!(arg_types.len(), 1);
            }
            other => panic!("expected bound CallableType, got {other:?}"),
        }
    }

    #[test]
    fn test_member_method_subclass_receiver_generic_completes() {
        // B receiver, `def [T] foo(self: A, x: T) -> T` on A: the self param
        // carries no method typevar, so the plan is the plain strip and the
        // receiver completes like the nongeneric case (bind plan).
        let resolver = make_ab_resolver();
        let method = make_ab_method(vec![Type::TypeVarType {
            name: "T".to_string(),
            fullname: "A.T".to_string(),
            raw_id: 1,
            namespace: "A".to_string(),
            values: vec![],
            upper_bound: Box::new(make_instance("builtins.object")),
            default: Box::new(make_instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        }]);
        let b = make_b_receiver();
        let result = member_method_inner(
            &b, &method, "A", &b, "foo", &resolver, true, false, // is_class
            false, // suppress_self_fail
        );
        match result {
            Some(Type::CallableType {
                arg_types,
                is_bound,
                ..
            }) => {
                assert!(is_bound);
                assert_eq!(arg_types.len(), 1);
            }
            other => panic!("expected bound CallableType, got {other:?}"),
        }
    }

    // --- classify_type_type_member_access (issue #957) ---

    #[test]
    fn test_tt_item_instance() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::Instance, false, false, TtUbKind::Other),
            TT_ITEM_INSTANCE
        );
    }

    #[test]
    fn test_tt_item_any() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::AnyType, false, false, TtUbKind::Other),
            TT_ITEM_ANY
        );
    }

    #[test]
    fn test_tt_tv_ub_instance() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::TypeVarType,
                false,
                false,
                TtUbKind::Instance
            ),
            TT_TV_UB_INSTANCE
        );
    }

    #[test]
    fn test_tt_tv_ub_union() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::TypeVarType,
                false,
                false,
                TtUbKind::UnionType
            ),
            TT_TV_UB_UNION
        );
    }

    #[test]
    fn test_tt_tv_ub_tuple() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::TypeVarType,
                false,
                false,
                TtUbKind::TupleType
            ),
            TT_TV_UB_TUPLE
        );
    }

    #[test]
    fn test_tt_tv_ub_any() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::TypeVarType,
                false,
                false,
                TtUbKind::AnyType
            ),
            TT_TV_UB_ANY
        );
    }

    #[test]
    fn test_tt_tv_ub_other() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::TypeVarType,
                false,
                false,
                TtUbKind::Other
            ),
            TT_TV_UB_OTHER
        );
    }

    #[test]
    fn test_tt_item_tuple() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::TupleType, false, false, TtUbKind::Other),
            TT_ITEM_TUPLE
        );
    }

    #[test]
    fn test_tt_item_func_typeobj() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::FunctionLike,
                true,
                false,
                TtUbKind::Other
            ),
            TT_ITEM_FUNC_TYPEOBJ
        );
    }

    #[test]
    fn test_tt_item_func_not_typeobj() {
        assert_eq!(
            classify_type_type_member_access(
                TtItemKind::FunctionLike,
                false,
                false,
                TtUbKind::Other
            ),
            TT_ITEM_FUNC_NOT_TYPEOBJ
        );
    }

    #[test]
    fn test_tt_item_type_type_instance() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::TypeType, false, true, TtUbKind::Other),
            TT_ITEM_TYPE_TYPE_INSTANCE
        );
    }

    #[test]
    fn test_tt_item_type_type_other() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::TypeType, false, false, TtUbKind::Other),
            TT_ITEM_TYPE_TYPE_OTHER
        );
    }

    #[test]
    fn test_tt_none() {
        assert_eq!(
            classify_type_type_member_access(TtItemKind::Other, false, false, TtUbKind::Other),
            TT_NONE
        );
    }

    #[test]
    fn is_instance_var_defers_on_non_var() {
        // A plain int has no `info`/`name` attrs, so the seam defers
        // (returns None) instead of raising, mirroring the strangler-fig
        // per-call gate.
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let obj = py.eval("1", None, None).unwrap();
            let result = rust_is_instance_var(obj).unwrap();
            assert_eq!(result, None);
        });
    }

    #[test]
    fn test_tuple_fallback_recomputes_args_from_items() {
        // issue #1112: a tuple literal's partial fallback is `tuple[Any, ...]`;
        // tuple_fallback (typeops.py:339-375) must recompute the element from
        // the items, which the TupleType arm now feeds the method branch.
        let tt = Type::TupleType {
            items: vec![make_instance("builtins.int"), make_instance("builtins.str")],
            partial_fallback: Box::new(Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![Type::AnyType {
                    type_of_any: 6, // TypeOfAny.special_form
                    source_any: None,
                    missing_import_name: None,
                }],
                last_known_value: None,
                extra_attrs: None,
            }),
            implicit: false,
        };
        let resolver = snap_resolver();
        let fb = crate::typeops::tuple_fallback(&tt, &resolver).expect("fallback computed");
        match fb {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.tuple");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Type::UnionType { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected union of items, got {other:?}"),
                }
            }
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn test_tuple_fallback_keeps_non_tuple_partial_fallback() {
        // typeops.py:342-343: a named-tuple fallback is returned verbatim.
        let tt = Type::TupleType {
            items: vec![make_instance("builtins.int")],
            partial_fallback: Box::new(make_instance("collections.OrderedDict")),
            implicit: false,
        };
        let resolver = TypeResolver::new();
        let fb = crate::typeops::tuple_fallback(&tt, &resolver).expect("fallback computed");
        assert_eq!(fb, make_instance("collections.OrderedDict"));
    }

    #[test]
    fn test_member_access_tuple_arm_defers_without_ctx() {
        // The TupleType arm routes through the computed fallback and then
        // the method branch; without a dispatch context the Instance arm
        // defers to Python (no panics, no partial results).
        let tt = Type::TupleType {
            items: vec![make_instance("builtins.int")],
            partial_fallback: Box::new(Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![Type::AnyType {
                    type_of_any: 6,
                    source_any: None,
                    missing_import_name: None,
                }],
                last_known_value: None,
                extra_attrs: None,
            }),
            implicit: false,
        };
        let resolver = TypeResolver::new();
        assert!(analyze_member_access_inner(&tt, None, &resolver).is_none());
    }

    #[test]
    fn test_static_member_tail_tuple_guard_legacy_seam() {
        // The legacy seam passes tuple_special=None: builtins.tuple
        // methods defer to Python (the maptype.py:326-345 special case
        // is not mirrored on that path).
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.tuple");
        let sig = make_callable(vec![ARG_POS], false);
        assert!(static_member_tail(
            &inst,
            &sig,
            "builtins.tuple",
            true,
            &resolver,
            false,
            true,
            None
        )
        .is_none());
    }

    #[test]
    fn test_static_member_tail_tuple_special_uses_decided_map() {
        // With a decided tuple_special (Some(Some(mapped))) the tail uses
        // it directly, skipping map_instance_to_supertype.
        let inst = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }],
            last_known_value: None,
            extra_attrs: None,
        };
        let sig = make_callable(vec![ARG_POS], false);
        let result = static_member_tail(
            &inst,
            &sig,
            "builtins.tuple",
            true,
            &snap_resolver(),
            false,
            true,
            Some(Some(&inst)),
        );
        // The decided map engages: the expand of a var-less callable is
        // identity, so the signature comes back unchanged (vs the legacy
        // guard's deferral above).
        assert!(result.is_some());
    }
}

/// Pure decision-table tests for `classify_analyze_var_inner` (issue
/// #1056). Live-Var coverage (gate differential, deferral) lives in
/// `NativeAnalyzeVarSuite` in mypy/test/testtypes.py.
#[cfg(test)]
mod classify_analyze_var_tests {
    use super::*;

    /// Plain data var: `x: int` on a non-enum class, accessed non-lvalue.
    fn plain_var_facts() -> AnalyzeVarFacts {
        AnalyzeVarFacts {
            is_settable_property: false,
            setter_type_present: false,
            setter_type_is_partial: false,
            var_type_present: true,
            var_type_is_partial: false,
            is_ready: true,
            is_initialized_in_class: true,
            is_instance_var: true,
            info_fullname: "mod.A".to_string(),
            is_enum: false,
            enum_has_name: false,
        }
    }

    fn facts_with(f: impl FnOnce(&mut AnalyzeVarFacts)) -> AnalyzeVarFacts {
        let mut facts = plain_var_facts();
        f(&mut facts);
        facts
    }

    #[test]
    fn plain_data_var_is_getter() {
        let facts = plain_var_facts();
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn lvalue_plain_var_is_still_getter() {
        let facts = plain_var_facts();
        assert_eq!(
            classify_analyze_var_inner("x", &facts, true, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn settable_property_lvalue_is_setter() {
        let facts = facts_with(|f| {
            f.is_settable_property = true;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, true, false, false),
            ANALYZE_VAR_SETTER
        );
    }

    #[test]
    fn settable_property_non_lvalue_is_getter() {
        let facts = facts_with(|f| {
            f.is_settable_property = true;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn setter_falls_back_to_ready_var_type() {
        // Synthetic property with no setter type: the ready var.type
        // fallback keeps the SETTER head (checkmember.py:1624-1625).
        let facts = facts_with(|f| {
            f.is_settable_property = true;
            f.var_type_present = true;
            f.is_ready = true;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, true, false, false),
            ANALYZE_VAR_SETTER
        );
    }

    #[test]
    fn setter_unready_without_setter_type_is_not_ready() {
        // setter_type missing and the var not ready: the fallback picks
        // nothing, and the not-ready callback must fire.
        let facts = facts_with(|f| {
            f.is_settable_property = true;
            f.is_initialized_in_class = false;
            f.is_instance_var = true;
            f.var_type_present = true;
            f.is_ready = false;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, true, false, false),
            ANALYZE_VAR_NOT_READY
        );
    }

    #[test]
    fn partial_var_type_is_partial() {
        let facts = facts_with(|f| {
            f.var_type_is_partial = true;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, false, false),
            ANALYZE_VAR_PARTIAL
        );
    }

    #[test]
    fn setter_fallback_partial_var_type_is_partial() {
        let facts = facts_with(|f| {
            f.is_settable_property = true;
            f.var_type_is_partial = true;
            f.is_ready = true;
        });
        assert_eq!(
            classify_analyze_var_inner("x", &facts, true, false, false),
            ANALYZE_VAR_PARTIAL
        );
    }

    #[test]
    fn unbound_var_not_ready() {
        let mut facts = plain_var_facts();
        facts.var_type_present = false;
        facts.is_ready = false;
        facts.is_initialized_in_class = false;
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, false, false),
            ANALYZE_VAR_NOT_READY
        );
    }

    #[test]
    fn unbound_var_ready_is_unbound_any() {
        let mut facts = plain_var_facts();
        facts.var_type_present = false;
        facts.is_initialized_in_class = false;
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, false, false),
            ANALYZE_VAR_UNBOUND_ANY
        );
    }

    #[test]
    fn unbound_var_no_deferral_is_unbound_any() {
        // no_deferral suppresses the not-ready callback, so the head
        // reduces to the implicit Any.
        let mut facts = plain_var_facts();
        facts.var_type_present = false;
        facts.is_ready = false;
        facts.is_initialized_in_class = false;
        assert_eq!(
            classify_analyze_var_inner("x", &facts, false, true, false),
            ANALYZE_VAR_UNBOUND_ANY
        );
    }

    #[test]
    fn enum_member_is_enum_literal() {
        let facts = facts_with(|f| {
            f.is_enum = true;
            f.enum_has_name = true;
        });
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_ENUM_LITERAL
        );
    }

    #[test]
    fn enum_name_value_excluded() {
        // `name`/`value` are real attributes, not enum literals; the
        // plain GETTER body must run.
        let facts = facts_with(|f| {
            f.is_enum = true;
            f.enum_has_name = true;
        });
        assert_eq!(
            classify_analyze_var_inner("name", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
        assert_eq!(
            classify_analyze_var_inner("value", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn enum_lvalue_not_literal() {
        let facts = facts_with(|f| {
            f.is_enum = true;
            f.enum_has_name = true;
        });
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, true, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn enum_not_in_members_not_literal() {
        let facts = facts_with(|f| {
            f.is_enum = true;
        });
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn partial_beats_enum_literal() {
        // handle_partial_var_type returns before the enum tail runs.
        let facts = facts_with(|f| {
            f.is_enum = true;
            f.enum_has_name = true;
            f.var_type_is_partial = true;
        });
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_PARTIAL
        );
    }

    #[test]
    fn not_ready_beats_enum_literal() {
        // The not-ready callback is a head side effect; the tail
        // overwrite happens after it, so the tag must stay NOT_READY.
        let mut facts = plain_var_facts();
        facts.is_enum = true;
        facts.enum_has_name = true;
        facts.var_type_present = false;
        facts.is_ready = false;
        facts.is_initialized_in_class = false;
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_NOT_READY
        );
    }

    #[test]
    fn enum_literal_requires_no_bind_tail() {
        // A class-level non-instance var (method alias-like) with a
        // callable type engages the bind tail; the property `__call__`
        // re-analysis can emit errors, so the head body must run.
        let facts = facts_with(|f| {
            f.is_enum = true;
            f.enum_has_name = true;
            f.is_instance_var = false;
        });
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_GETTER
        );
        // mx.is_operator widens the bind-tail gate the same way.
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, true),
            ANALYZE_VAR_GETTER
        );
    }

    #[test]
    fn enum_literal_allows_ready_unbound() {
        // An enum member without a declared type: the implicit Any head
        // has no side effects once ready, so the literal still collapses.
        let mut facts = plain_var_facts();
        facts.is_enum = true;
        facts.enum_has_name = true;
        facts.var_type_present = false;
        facts.is_initialized_in_class = false;
        assert_eq!(
            classify_analyze_var_inner("RED", &facts, false, false, false),
            ANALYZE_VAR_ENUM_LITERAL
        );
    }
}

/// Pure fold tests for `check_final_member_fold` (issue #1078). Live
/// MRO-walk coverage (gate differential, deferral) lives in
/// `NativeCheckFinalMemberSuite` in mypy/test/testtypes.py.
#[cfg(test)]
mod check_final_member_tests {
    use super::check_final_member_fold;

    #[test]
    fn test_fold_empty_mro_is_not_final() {
        assert_eq!(check_final_member_fold(std::iter::empty()), Some(false));
    }

    #[test]
    fn test_fold_name_not_in_mro() {
        // `names.get(name)` misses on every base: not final.
        let entries = vec![Some(false), Some(false)];
        assert_eq!(check_final_member_fold(entries.into_iter()), Some(false));
    }

    #[test]
    fn test_fold_final_var_in_first_base() {
        let entries = vec![Some(true)];
        assert_eq!(check_final_member_fold(entries.into_iter()), Some(true));
    }

    #[test]
    fn test_fold_final_in_later_base() {
        // Only a base deeper in the MRO declares the name final.
        let entries = vec![Some(false), Some(false), Some(true)];
        assert_eq!(check_final_member_fold(entries.into_iter()), Some(true));
    }

    #[test]
    fn test_fold_any_entry_final_wins() {
        // Python scans the full MRO without breaking on non-final hits,
        // so an overridden name is still final when a base declares it.
        let entries = vec![Some(false), Some(false), Some(true), Some(false)];
        assert_eq!(check_final_member_fold(entries.into_iter()), Some(true));
    }

    #[test]
    fn test_fold_unreadable_entry_defers() {
        let entries = vec![Some(false), None];
        assert_eq!(check_final_member_fold(entries.into_iter()), None);
    }

    #[test]
    fn test_fold_unreadable_after_final_still_answers() {
        // The early-true short-circuit answers before the defer is seen;
        // the shim's fallback reproduces the same output either way.
        let entries = vec![Some(true), None];
        assert_eq!(check_final_member_fold(entries.into_iter()), Some(true));
    }
}
