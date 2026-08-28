//! `mypy.subtypes.is_protocol_implementation` (subtypes.py:1766-1895),
//! Rust common-path port.
//!
//! Python checks whether `left` structurally implements the protocol
//! `right` by looping `right.protocol_members` (minus the
//! always-skipped `__init__`/`__new__` and any caller-supplied `skip`):
//!
//! ```text
//! subtype     = get_protocol_member(left, original_left, member, class_obj)
//! supertype   = find_member(member, right, original_left)
//! is_compat   = is_subtype(subtype, supertype, ignore_pos_arg_names=...)
//! flags       = (sub/super member-flags checks for settable/classvar)
//! ```
//!
//! This port decides the pure member-compat loop only:
//!   * both member lookups run through `get_protocol_member_inner`
//!     (checker_helpers.rs), which mirrors `find_member`'s
//!     plain-method / plain-Var path and defers on anything needing
//!     checker state, plugins, descriptors, or error emission;
//!   * the recursive `is_subtype` on the member pair runs natively
//!     (subtypes.rs:146) with a FRESH default context plus per-member
//!     `ignore_pos_arg_names` (Python: `is_subtype(subtype, supertype,
//!     ignore_pos_arg_names=ignore_names, options=options)`) and the
//!     caller's `proper_subtype`;
//!   * the member-flag loop (IS_SETTABLE / IS_CLASSVAR /
//!     IS_CLASS_OR_STATIC rejections) is mirrored only in the trivial
//!     direction: when both flag sets are subsets of {IS_VAR} the flags
//!     cannot reject, so Rust decides; any other flag combination
//!     defers to the pure-Python body, which runs the full check.
//!
//! Deferral is the safe default: recursion through `assuming` is not
//! mirrored (the snapshot omits the recursive-protocol matrices), so a
//! protocol left (the recursion-prone case) defers wholesale.

use crate::checker_helpers::{get_protocol_member_inner, GetProtocolMemberResult};
use crate::member_flags::get_member_flags_inner_pub;
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, ReadBuffer, Type};
use pyo3::prelude::*;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Always-skipped members (subtypes.py:1801-1803).
fn member_is_skipped(member: &str, skip: &[String]) -> bool {
    member == "__init__" || member == "__new__" || skip.iter().any(|m| m == member)
}

/// The member-flag loop (subtypes.py:1877-1907) can only reject on
/// IS_SETTABLE / IS_CLASSVAR / IS_CLASS_OR_STATIC; when both sides
/// carry only IS_VAR (or nothing) the loop cannot change the
/// member-compat verdict, so Rust decides without it. `get_member_flags`
/// returns `[]` for plain/decorated methods and `[IS_VAR]` for vars
/// (member_flags.rs), so both are trivial.
const IS_VAR: i64 = 4;

fn flags_trivial(flags: &[i64]) -> bool {
    flags.iter().all(|f| *f == IS_VAR)
}

/// `is_protocol_implementation` member loop, Rust common path.
///
/// Returns `Some(bool)` when Rust decided; `None` defers to the
/// pure-Python body. `class_obj` / `is_lvalue` are not supported
/// (get_protocol_member_inner defers on them anyway, so the loop
/// defers before deciding anything).
pub(crate) fn is_protocol_implementation_inner(
    py: Python<'_>,
    left: &Type,
    right: &Type,
    skip: &[String],
    ctx: &SubtypeContext,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let right_snap = resolver.resolver().get(right_ref(right)?)?;
    if !right_snap.is_protocol {
        // Non-protocol right must defer to SubtypeVisitor.
        return None;
    }
    // Protocol-left: recursion-prone (`assuming` guard not mirrored).
    // Defer to Python's guarded loop.
    let left_snap = resolver.resolver().get(left_ref(left)?);
    if left_snap.is_some_and(|s| s.is_protocol) {
        return None;
    }
    let members: Vec<&String> = right_snap
        .protocol_members
        .iter()
        .filter(|m| !member_is_skipped(m, skip))
        .collect();
    for member in members {
        // Missing-member pre-check: if the member is absent from the
        // impl's `names` MRO, Python returns False. Decide it here.
        let left_ref_str = left_ref(left)?;
        if let Some(live_info) = resolver.live_typeinfo(py, left_ref_str) {
            if !mro_has(live_info, member) {
                return Some(false);
            }
        }
        let sub = match get_protocol_member_inner(py, left, member, false, false, resolver) {
            Some(GetProtocolMemberResult::Found(t)) => Some(t),
            Some(GetProtocolMemberResult::NoneVal) => None,
            Some(GetProtocolMemberResult::Defer) => {
                return None;
            }
            None => {
                return None;
            }
        };
        let sup = match get_protocol_member_inner(py, right, member, false, false, resolver) {
            Some(GetProtocolMemberResult::Found(t)) => Some(t),
            Some(GetProtocolMemberResult::NoneVal) => None,
            Some(GetProtocolMemberResult::Defer) => {
                return None;
            }
            None => {
                return None;
            }
        };
        // Missing member on either side: not an implementation.
        let (sub, sup) = match (sub, sup) {
            (Some(s), Some(p)) => (s, p),
            _ => {
                return Some(false);
            }
        };
        // __call__ compares arg names; everything else ignores them.
        // Python builds a FRESH default SubtypeContext per member call.
        let member_ctx = SubtypeContext::with_callable_flags(
            false,
            false,
            false,
            false,
            ctx.proper_subtype,
            ctx.strict_optional,
            member != "__call__",
            ctx.strict_concatenate,
        );
        match is_subtype(&sub, &sup, &member_ctx, resolver.resolver()) {
            Some(true) => {}
            Some(false) => {
                return Some(false);
            }
            None => {
                return None;
            }
        }
        // __hash__ = None idiom (subtypes.py:1872-1876): None-typed
        // member + Callable supertype is not an implementation.
        if matches!(sub, Type::NoneType) && matches!(sup, Type::CallableType { .. }) {
            return Some(false);
        }
        // Member-flag loop: only decided in the trivial direction. Both
        // sides must be plain vars (flags == {IS_VAR}); any other flag
        // combination can reject and defers to Python.
        let left_ref = left_ref(left)?;
        let right_ref = right_ref(right)?;
        let left_info = resolver.live_typeinfo(py, left_ref)?;
        let right_info = resolver.live_typeinfo(py, right_ref)?;
        let left_extra = extra_attrs(left);
        let right_extra = extra_attrs(right);
        let subflags = match get_member_flags_inner_pub(
            py,
            left_info,
            member,
            false,
            left_extra,
            ctx.strict_optional,
            resolver,
        ) {
            Some(f) => f,
            None => {
                return None;
            }
        };
        let superflags = match get_member_flags_inner_pub(
            py,
            right_info,
            member,
            false,
            right_extra,
            ctx.strict_optional,
            resolver,
        ) {
            Some(f) => f,
            None => {
                return None;
            }
        };
        if !flags_trivial(&subflags) || !flags_trivial(&superflags) {
            return None;
        }
    }
    Some(true)
}

fn left_ref(t: &Type) -> Option<&str> {
    match t {
        Type::Instance { type_ref, .. } => Some(type_ref),
        _ => None,
    }
}

fn right_ref(t: &Type) -> Option<&str> {
    left_ref(t)
}

fn extra_attrs(_t: &Type) -> Option<&pyo3::PyAny> {
    // `extra_attrs` is not wire-decoded to a live PyAny; a present
    // extra_attrs already defers in `get_protocol_member_inner`, and the
    // flag loop here only needs the common no-extra_attrs case (None).
    None
}

/// True iff `name` resolves via a `names` walk of `info`'s MRO (the
/// same walk `get_protocol_member_inner` uses; a missing name there is
/// determinate "member absent" for the implementation loop).
///
/// Looks up with `names.get(name)` (mirroring `TypeInfo.get`): a missing
/// key on one base must continue the walk, not abort it — a dict
/// subscript raises `KeyError` on the first base that lacks the name.
fn mro_has(info: &PyAny, name: &str) -> bool {
    let mro = match info.getattr("mro") {
        Ok(m) => m,
        Err(_) => return false,
    };
    let mro_list = match mro.downcast::<pyo3::types::PyList>() {
        Ok(l) => l,
        Err(_) => return false,
    };
    for base in mro_list.iter() {
        let names = match base.getattr("names") {
            Ok(n) => n,
            Err(_) => return false,
        };
        let get = match names.getattr("get") {
            Ok(g) => g,
            Err(_) => return false,
        };
        match get.call1((name,)) {
            Ok(v) => {
                if !v.is_none() {
                    return true;
                }
            }
            Err(_) => return false,
        }
    }
    false
}

/// `#[pyfunction]` entry for `is_protocol_implementation`.
///
/// Serialized `left`/`right` wire bytes plus the skip list and the
/// SubtypeContext flags; returns `Some(bool)` when decided, `None` to
/// defer to the pure-Python body.
#[pyfunction]
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
pub(crate) fn rust_is_protocol_implementation(
    py: Python<'_>,
    left_bytes: &[u8],
    right_bytes: &[u8],
    skip: Vec<String>,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let ctx = SubtypeContext::with_callable_flags(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate,
    );
    is_protocol_implementation_inner(py, &left, &right, &skip, &ctx, resolver)
}
