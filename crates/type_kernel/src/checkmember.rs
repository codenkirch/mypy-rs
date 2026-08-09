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

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `ArgKind.ARG_POS` = 0.
#[cfg(test)]
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

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
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
#[pyfunction]
pub(crate) fn rust_bind_self_fast(method_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(method_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
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
fn has_readable_member_by_ref(resolver: &TypeResolver, type_ref: &str, name: &str) -> Option<bool> {
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

/// `mypy.checkmember.analyze_instance_member_access` (checkmember.py:388-453),
/// the method branch (checkmember.py:415-453), Rust subset.
///
/// Ports the map-then-expand tail of the method path for a **static**,
/// non-overloaded method: `map_instance_to_supertype` +
/// `expand_type_by_instance` + `freeze_all_type_vars`. The Python caller
/// freshens the signature before dispatching, so method-level type vars are
/// freshened (raw_ids that are not class vars); Rust's
/// `expand_type_with_env` defers any result that still contains a TypeVar,
/// so methods generic over their own type vars fall through to Python.
///
/// The caller gates on `method.is_static`, which guarantees the signature is
/// not bound (non-static methods go through `bind_self`/`check_self_arg` on
/// the Python side); a bound callable is not representable here because
/// expand defers `is_bound`. Returns `None` (Python falls through) for:
///   * a non-Instance `typ`
///   * an Overloaded signature (the static overloaded path maps in Python)
///   * a missing resolver snapshot / unresolvable derivation path
///   * a mapped instance with empty args or a TVT class (expand defers)
///   * a bound callable or a ParamSpec/Unpack signature (expand defers)
///   * an expanded result that still carries a TypeVar
/// Python's `freeze_all_type_vars` is unported: the signature is already
/// frozen by this seam (expand produces only bound class vars), so nothing
/// remains to freeze when the Rust path fully succeeds.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_analyze_instance_member_access(
    resolver: &NativeTypeResolver,
    instance_bytes: &[u8],
    signature_bytes: &[u8],
    method_fullname: &str,
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let instance = decode_type(instance_bytes)?;
    let signature = decode_type(signature_bytes)?;
    let (left_ref, left_args) = match &instance {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    if !matches!(signature, Type::CallableType { .. }) {
        return None; // Overloaded defers to Python
    }
    // checkmember.py:450 `typ = map_instance_to_supertype(typ, method.info)`.
    let mapped_args = crate::subtypes::map_instance_to_supertype(
        left_ref,
        left_args,
        method_fullname,
        resolver.resolver(),
    )?;
    let mapped_instance = Type::Instance {
        type_ref: method_fullname.to_string(),
        args: mapped_args,
        last_known_value: None,
        extra_attrs: None,
    };
    // checkmember.py:451 `expand_type_by_instance(signature, typ)`.
    let expanded = crate::expandtype::expand_type_by_instance_core(
        &signature,
        &mapped_instance,
        resolver.resolver(),
        strict_optional,
    )?;
    encode_type(&expanded)
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
///   * Instance / UnionType / TypeType / TypedDictType / NoneType / DeletedType
///     → need analyzer state, `mx`, or error reporting.  Rust must not drop a
///     diagnostic (e.g. `deleted_as_rvalue`) or mis-answer (`__bool__`).
///   * TypeVarType with values → needs `make_simplified_union`.
///   * UnboundType / Parameters / UnpackType → needs `report_missing_attribute`.
#[pyfunction]
pub(crate) fn rust_analyze_member_access(
    resolver: &NativeTypeResolver,
    typ_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(analyze_member_access_inner(&typ, resolver.resolver()).and_then(|typ| encode_type(&typ)))
}

fn analyze_member_access_inner<'a>(typ: &'a Type, resolver: &'a TypeResolver) -> Option<Type> {
    match typ {
        // --- Instance ---
        Type::Instance { .. } => {
            // Needs analyze_instance_member_access (plugin hooks, method lookup).
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
        Type::TupleType {
            partial_fallback, ..
        } => {
            // Python: _analyze_member_access(name, tuple_fallback(typ), mx).
            // Fall back to the partial fallback.
            analyze_member_access_inner(partial_fallback, resolver)
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
                analyze_member_access_inner(upper_bound, resolver)
            }
        }
        // --- ParamSpecType ---
        Type::ParamSpecType { upper_bound, .. } => {
            // Python: TypeVarLikeType -> _analyze_member_access(name, typ.upper_bound, mx).
            analyze_member_access_inner(upper_bound, resolver)
        }
        // --- TypeVarTupleType ---
        Type::TypeVarTupleType { tuple_fallback, .. } => {
            // No upper_bound for TypeVarTuple; fall back to tuple_fallback.
            analyze_member_access_inner(tuple_fallback, resolver)
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
            analyze_member_access_inner(fallback, resolver)
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
                analyze_member_access_inner(fallback, resolver)
            }
        }
        // --- Overloaded ---
        Type::Overloaded { items } => {
            if items.is_empty() {
                // Degenerate; defer.
                None
            } else {
                // Python: check first item for is_type_obj().
                if let Some(Type::CallableType {
                    fallback, ret_type, ..
                }) = items.first()
                {
                    if is_type_obj(fallback, ret_type, resolver) {
                        // Type object — defer to Python's
                        // analyze_type_callable_member_access.
                        None
                    } else {
                        // Normal callable — recurse on fallback.
                        analyze_member_access_inner(fallback, resolver)
                    }
                } else {
                    // No CallableType first item (shouldn't happen); defer.
                    None
                }
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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
}
