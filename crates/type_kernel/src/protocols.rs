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
//! This port decides the pure member-compat loop:
//!   * both member lookups run through `get_protocol_member_inner`
//!     (checker_helpers.rs), which mirrors `find_member`'s
//!     plain-method / plain-Var path and defers on anything needing
//!     checker state, plugins, descriptors, or error emission;
//!   * the recursive `is_subtype` on the member pair runs natively
//!     (subtypes.rs:146) with a FRESH default context plus per-member
//!     `ignore_pos_arg_names` (Python: `is_subtype(subtype, supertype,
//!     ignore_pos_arg_names=ignore_names, options=options)`) and the
//!     caller's `proper_subtype`;
//!   * the member-flag loop (subtypes.py:2025-2055, IS_SETTABLE /
//!     IS_CLASSVAR / IS_CLASS_OR_STATIC rejections) is mirrored from the
//!     two flag sets and the already-extracted member types; members
//!     carrying IS_EXPLICIT_SETTER need lvalue re-resolution and defer;
//!   * the `assuming` recursion guard (subtypes.py:1972-1976) is mirrored
//!     by the caller (`subtypes.rs:protocol_right_decision`) with a
//!     thread-local stack keyed by the proper-subtype dimension.
//!
//! Deferral is the safe default: a protocol left now runs the full
//! ported path (#1344: the subtypes.py:1906-1911 trivial member-set
//! check, a cut on a pair held in flight in a Python frame, then the
//! member loop), and any member the lookups cannot decide still defers
//! (descriptors, extra_attrs/module instances, base-class-defined
//! members behind the same-class guard).

use crate::checker_helpers::{get_protocol_member_inner, GetProtocolMemberResult};
use crate::checkmember::encode_type;
use crate::member_flags::{
    get_member_flags_inner_pub, IS_CLASSVAR, IS_CLASS_OR_STATIC, IS_EXPLICIT_SETTER, IS_SETTABLE,
};
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, ReadBuffer, Type};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Always-skipped members (subtypes.py:1801-1803).
fn member_is_skipped(member: &str, skip: &[String]) -> bool {
    member == "__init__" || member == "__new__" || skip.iter().any(|m| m == member)
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
    resolver: &TypeResolver,
) -> Option<bool> {
    let right_snap = resolver.get(right_ref(right)?)?;
    if !right_snap.is_protocol {
        // Non-protocol right must defer to SubtypeVisitor.
        return None;
    }
    let left_snap = left_ref(left).and_then(|r| resolver.get(r));
    // Protocol-left mirrors two Python facts (#1344): the trivial
    // member-set fast path below, and the Python-frame assuming matrix
    // this thread cannot see (registry consult, see helper below).
    let left_is_protocol = left_snap.is_some_and(|s| s.is_protocol);
    if left_is_protocol {
        let left_members = &left_snap.unwrap().protocol_members;
        if !right_snap
            .protocol_members
            .iter()
            .filter(|m| !member_is_skipped(m, skip))
            .all(|m| left_members.iter().any(|l| l == m))
        {
            return Some(false);
        }
        if protocol_pair_in_flight(py, left, right, ctx.proper_subtype) {
            return Some(true);
        }
    }
    // Recursion guard (subtypes.py:1972-1976): the member loop always runs
    // under `pop_on_exit(assuming, left, right)`, so a re-entered pair is
    // assumed True; the entry path needs this once the sup fetch re-enters.
    if crate::subtypes::assuming_contains(left, right, ctx.proper_subtype) {
        return Some(true);
    }
    let _assuming =
        crate::subtypes::AssumingPush::new(left.clone(), right.clone(), ctx.proper_subtype);
    let members: Vec<&String> = right_snap
        .protocol_members
        .iter()
        .filter(|m| !member_is_skipped(m, skip))
        .collect();
    for member in members {
        // Python (subtypes.py:1906-1909): subtype = get_protocol_member(left,
        // original_left, ...); supertype = find_member(member, right,
        // original_left). self_type is original_left (== left) on both fetches.
        let sub = match get_protocol_member_inner(py, left, left, member, false, false, resolver) {
            Some(GetProtocolMemberResult::Found(t)) => Some(t),
            Some(GetProtocolMemberResult::NoneVal) => None,
            Some(GetProtocolMemberResult::Defer) => {
                return None;
            }
            None => {
                return None;
            }
        };
        let sup = match get_protocol_member_inner(py, right, left, member, false, false, resolver) {
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
        match is_subtype(&sub, &sup, &member_ctx, resolver) {
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
        // Member-flag loop (subtypes.py:2025-2055): reads both flag sets
        // off the live Var nodes; any flag set the kernel cannot compute
        // defers to Python, which recomputes it exactly.
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
        // Member-flag loop (subtypes.py:2025-2055), decided whenever the
        // arbitration hangs only off the two flag sets and the already
        // extracted member types. Explicit-setter members need lvalue
        // re-resolution (find_member / get_protocol_member with
        // is_lvalue=True) and defer.
        let settable_sub = subflags.contains(&IS_SETTABLE);
        let settable_super = superflags.contains(&IS_SETTABLE);
        if settable_super {
            if superflags.contains(&IS_EXPLICIT_SETTER) || subflags.contains(&IS_EXPLICIT_SETTER) {
                return None;
            }
            // Reversed subtype check (subtypes.py:2035-2037): plain
            // is_subtype(supertype, subtype) — fresh visitor, so
            // proper_subtype=False, ignore_pos_arg_names=False; the
            // strict flags derive from the same options, hence ctx's.
            let rev_ctx = SubtypeContext::with_callable_flags(
                false,
                false,
                false,
                false,
                false, // proper_subtype
                ctx.strict_optional,
                false, // ignore_pos_arg_names
                ctx.strict_concatenate,
            );
            match is_subtype(&sup, &sub, &rev_ctx, resolver) {
                Some(true) => {}
                Some(false) => return Some(false),
                None => return None,
            }
        }
        if settable_super && !settable_sub {
            return Some(false);
        }
        // class_obj is always false here (the inner defers on class_obj).
        if !settable_super {
            if superflags.contains(&IS_CLASSVAR) && !subflags.contains(&IS_CLASSVAR) {
                return Some(false);
            }
        } else if subflags.contains(&IS_CLASSVAR) != superflags.contains(&IS_CLASSVAR) {
            return Some(false);
        }
        if superflags.contains(&IS_CLASS_OR_STATIC) && !subflags.contains(&IS_CLASS_OR_STATIC) {
            return Some(false);
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

/// Consult the Python-side in-flight registry (`mypy.subtypes`.
/// `protocol_pair_in_flight`). Python holds recursive-protocol check
/// pairs on `right.type.assuming` while a member loop runs; when that
/// loop defers a sub-check into Rust, the fresh Rust derivation must cut
/// the re-derived pair exactly where Python's matrix scan would. Keyed
/// on the same wire bytes Python registered with (the builtin-instance
/// short-circuit is shared through `write_type`), plus the proper flag.
/// Any failure (undecodable, import, call) is a silent miss: a missing
/// cut only costs re-derivation, never a wrong verdict.
fn protocol_pair_in_flight(py: Python<'_>, left: &Type, right: &Type, proper: bool) -> bool {
    let (Some(left_bytes), Some(right_bytes)) = (encode_type(left), encode_type(right)) else {
        return false;
    };
    let py_left = PyBytes::new(py, &left_bytes);
    let py_right = PyBytes::new(py, &right_bytes);
    py.import("mypy.subtypes")
        .ok()
        .and_then(|m| m.getattr("protocol_pair_in_flight").ok())
        .and_then(|f| f.call1((py_left, py_right, proper)).ok())
        .and_then(|o| o.extract::<bool>().ok())
        .unwrap_or(false)
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
    is_protocol_implementation_inner(py, &left, &right, &skip, &ctx, resolver.resolver())
}
