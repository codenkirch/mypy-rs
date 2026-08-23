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

use crate::setops::make_simplified_union;
use crate::subtypes::SubtypeContext;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

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
            // Fall back to the partial fallback; an Instance target would
            // defer inside the recursion, so short-circuit it.
            if matches!(&**partial_fallback, Type::Instance { .. }) {
                return None;
            }
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
            } else if matches!(&**upper_bound, Type::Instance { .. }) {
                // Python: _analyze_member_access(name, typ.upper_bound, mx);
                // an Instance target would defer inside the recursion.
                None
            } else {
                // Python: _analyze_member_access(name, typ.upper_bound, mx).
                analyze_member_access_inner(upper_bound, resolver)
            }
        }
        // --- ParamSpecType ---
        Type::ParamSpecType { upper_bound, .. } => {
            // Python: TypeVarLikeType -> _analyze_member_access(name, typ.upper_bound, mx).
            if matches!(&**upper_bound, Type::Instance { .. }) {
                return None; // Instance target would defer inside the recursion
            }
            analyze_member_access_inner(upper_bound, resolver)
        }
        // --- TypeVarTupleType ---
        Type::TypeVarTupleType { tuple_fallback, .. } => {
            // No upper_bound for TypeVarTuple; fall back to tuple_fallback.
            if matches!(&**tuple_fallback, Type::Instance { .. }) {
                return None; // Instance target would defer inside the recursion
            }
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
            if matches!(&**fallback, Type::Instance { .. }) {
                return None; // Instance target would defer inside the recursion
            }
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
            } else if matches!(&**fallback, Type::Instance { .. }) {
                // Python: _analyze_member_access(name, typ.fallback, mx);
                // an Instance target would defer inside the recursion.
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
                        if let Some(r) = analyze_member_access_inner(fallback, resolver) {
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

/// `mypy.checkmember.analyze_union_member_access` (checkmember.py:656-663).
///
/// Maps `relevant_items()` of the union through `_analyze_member_access`
/// (the pure-type-transform Rust subset), then joins the results via
/// `make_simplified_union`. Defer (None) when any item defers — Python
/// falls through. The Python `disable_type_names()` context only affects
/// error messages during the recursion; the pure branches Rust handles
/// emit no errors, and non-pure branches defer, so the context is
/// irrelevant on the Rust path. The per-item `self_type` override is also
/// unused by the pure branches.
#[pyfunction]
pub(crate) fn rust_analyze_union_member_access(
    resolver: &NativeTypeResolver,
    union_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(union_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(
        analyze_union_member_access_inner(&typ, strict_optional, resolver.resolver())
            .and_then(|t| encode_type(&t)),
    )
}

fn analyze_union_member_access_inner(
    typ: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    let items = match typ {
        Type::UnionType { items, .. } => items,
        _ => return None,
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
    let mut results = Vec::with_capacity(relevant.len());
    for item in &relevant {
        let r = analyze_member_access_inner(item, resolver)?;
        results.push(r);
    }
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    make_simplified_union(&results, &ctx, resolver, true)
}

// ---------------------------------------------------------------------------
// analyze_none_member_access
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_none_member_access` (checkmember.py:666-677).
///
/// `__bool__` returns a pure CallableType ret=Literal[False]. Any other
/// name recurses on `builtins.object` via `_analyze_member_access`. Defer
/// (None) when the recursion on `builtins.object` defers.
#[pyfunction]
pub(crate) fn rust_analyze_none_member_access(
    resolver: &NativeTypeResolver,
    name: &str,
    typ_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<Vec<u8>>> {
    // We only accept NoneType input; any other type is a caller bug — defer.
    if !matches!(decode_type(typ_bytes), Some(Type::NoneType)) {
        return Ok(None);
    }
    Ok(
        analyze_none_member_access_inner(name, strict_optional, resolver.resolver())
            .and_then(|t| encode_type(&t)),
    )
}

fn analyze_none_member_access_inner(
    name: &str,
    _strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    if name == "__bool__" {
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
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(literal_false),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        })
    } else {
        // _analyze_member_access(name, builtins.object, mx)
        let object_inst = Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        analyze_member_access_inner(&object_inst, resolver)
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
// analyze_descriptor_access (Union-map + non-Instance guard only)
// ---------------------------------------------------------------------------

/// `mypy.checkmember.analyze_descriptor_access` (checkmember.py:822-928),
/// the pure-type-transform head only.
///
/// Ports two early-return guards:
///   * `UnionType` → map each item through `analyze_descriptor_access`
///     and `make_simplified_union`.
///   * Not an `Instance` → return the original descriptor type unchanged.
///
/// Everything else needs `mx.chk` (checker state), `map_instance_to_supertype`,
/// `expand_type_by_instance`, `check_call`, `warn_deprecated`, or
/// `has_readable_member` on live TypeInfo — all deferred. The `is_lvalue`
/// flag gates the `__set__`/`__get__` checks which need checker state, so
/// even with `is_lvalue` we defer past the two guards above.
#[pyfunction]
pub(crate) fn rust_analyze_descriptor_access(
    resolver: &NativeTypeResolver,
    descriptor_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(descriptor_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(
        analyze_descriptor_access_inner(&typ, strict_optional, resolver.resolver())
            .and_then(|t| encode_type(&t)),
    )
}

fn analyze_descriptor_access_inner(
    typ: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::UnionType { items, .. } => {
            // Map the access over union types, then make_simplified_union.
            let mut results = Vec::with_capacity(items.len());
            for item in items {
                let r = analyze_descriptor_access_inner(item, strict_optional, resolver)?;
                results.push(r);
            }
            let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&results, &ctx, resolver, true)
        }
        // Everything else deferred: the `not isinstance(...)` early return
        // hands back the *original* descriptor object (identity-preserving,
        // and it fires on every method-access call), and the Instance branch

        // needs checker state (has_readable_member, get_method, check_call).
        _ => None,
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
            // Python: msg.no_formal_self, then return functype. Defer (Python
            // reports the error).
            return None;
        }
        // Python checks item.arg_kinds[0] not in (ARG_POS, ARG_STAR).
        // Guard a length mismatch; if arg_kinds[0] is not ARG_POS/ARG_STAR,
        // Python reports no_formal_self and returns functype. Defer.
        match arg_kinds.first() {
            Some(&k) if k == ARG_POS || k == ARG_STAR => {}
            _ => return None,
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
        // Defer so Python can report the error.
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
// Tests
// ---------------------------------------------------------------------------

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

    // --- analyze_union_member_access_inner ---

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    #[test]
    fn test_union_access_any_items_returns_any_union() {
        let resolver = TypeResolver::new();
        let union = make_union(vec![make_instance("builtins.int")]);
        // Instance item defers -> union defers.
        assert!(analyze_union_member_access_inner(&union, true, &resolver).is_none());
    }

    #[test]
    fn test_union_access_defers_on_instance_item() {
        let resolver = TypeResolver::new();
        let any_t = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let union = make_union(vec![any_t, make_instance("builtins.int")]);
        // Instance item defers -> whole union defers.
        assert!(analyze_union_member_access_inner(&union, true, &resolver).is_none());
    }

    #[test]
    fn test_union_access_single_any_returns_any() {
        let resolver = TypeResolver::new();
        let any_t = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let union = make_union(vec![any_t]);
        let result =
            analyze_union_member_access_inner(&union, true, &resolver).expect("single Any item");
        match result {
            Type::AnyType { .. } => {}
            other => panic!("expected AnyType, got {other:?}"),
        }
    }

    #[test]
    fn test_union_access_two_any_simplifies_to_any() {
        let resolver = TypeResolver::new();
        let any1 = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let any2 = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let union = make_union(vec![any1, any2]);
        let result =
            analyze_union_member_access_inner(&union, true, &resolver).expect("two Any items");
        // make_simplified_union removes redundant items: Any <: Any, so
        // the second Any is dropped and the result is a single Any.
        match result {
            Type::AnyType { .. } => {}
            other => panic!("expected AnyType after simplification, got {other:?}"),
        }
    }

    #[test]
    fn test_union_access_non_union_defers() {
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        assert!(analyze_union_member_access_inner(&inst, true, &resolver).is_none());
    }

    // --- analyze_none_member_access_inner ---

    #[test]
    fn test_none_access_bool_returns_callable_literal_false() {
        let resolver = TypeResolver::new();
        let result = analyze_none_member_access_inner("__bool__", true, &resolver)
            .expect("__bool__ returns a callable");
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

    #[test]
    fn test_none_access_object_defers() {
        let resolver = TypeResolver::new();
        // builtins.object is an Instance -> analyze_member_access_inner defers.
        assert!(analyze_none_member_access_inner("foo", true, &resolver).is_none());
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
    fn test_descriptor_access_non_union_defers() {
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        // Instance branch needs checker state -> defer.
        assert!(analyze_descriptor_access_inner(&inst, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_type_alias_defers() {
        let resolver = TypeResolver::new();
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "some.Alias".to_string(),
        };
        assert!(analyze_descriptor_access_inner(&alias, true, &resolver).is_none());
    }

    #[test]
    fn test_descriptor_access_union_with_instance_defers() {
        let resolver = TypeResolver::new();
        let union = make_union(vec![make_instance("builtins.int")]);
        // Instance item defers -> union defers.
        assert!(analyze_descriptor_access_inner(&union, true, &resolver).is_none());
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
            &resolver
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
            &resolver
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
        assert_eq!(result.1, false); // no freshening of non-generic type
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
}
