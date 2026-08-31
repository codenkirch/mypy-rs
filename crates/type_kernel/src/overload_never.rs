//! Overload-argument-prefix compatibility (mypy.checker).
//!
//! Ports three `checker.py` helpers that decide whether one overload
//! signature is "more general" than another, resting on the native
//! `is_callable_compatible` engine plus the `is_more_precise` /
//! `is_proper_subtype` / `is_same_type` predicates:
//!
//! * `overload_can_never_match` (checker.py:9974-9994): after erasing
//!   `signature`'s type vars to their bound/union, `is_callable_compatible
//!   (exp_sig, other, is_compat=is_more_precise, is_proper_subtype=True,
//!   ignore_return=True)`.
//! * `is_more_general_arg_prefix` (checker.py:9997-10012): the
//!   Callable-vs-Callable branch is `is_callable_compatible(t, s,
//!   is_compat=is_proper_subtype, is_proper_subtype=True, ignore_return=
//!   True)`; the Overloaded (FunctionLike) branch defers to Python.
//! * `is_same_arg_prefix` (checker.py:10015-10027): `is_callable_compatible
//!   (t, s, is_compat=is_same_type, is_proper_subtype=True, ignore_return=
//!   True, check_args_covariantly=True, ignore_pos_arg_names=True)`.
//!
//! Strangler-fig contract (same as `callable_compat::rust_callables_
//! compatible`): each entry decodes the wire blob, defers (`None`) whenever
//! Rust cannot reproduce the pure-Python answer, and the Python shim in
//! `mypy/checker.py` runs the pure-Python function. Two defer boundaries:
//!
//! * a non-`CallableType` operand (e.g. `Overloaded` / `Parameters`) — the
//!   wire cannot carry every such node and the Overloaded zip logic is
//!   kept in Python;
//! * a non-empty `variables` on either side — Python `is_callable_
//!   compatible` unifies a generic `left` via `unify_generic_callable`
//!   (constraint solving) before the parameter check, which this port does
//!   not replicate; for `overload_can_never_match` that is exactly the
//!   generic branch the erase+expand step exists for, so it defers too.
//!
//! When `signature.variables` is empty the erase+expand is a no-op, so the
//! non-generic fast path needs no expansion at all: the engine runs on the
//! decoded callables directly.

use pyo3::prelude::*;

use crate::callable_compat::is_callable_compatible;
use crate::subtypes::{is_more_precise, is_same_type, is_subtype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, ReadBuffer, Type};

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// The `CallableType` fields a caller must not trip over: the two operands
/// must both be plain non-generic `CallableType`s, else defer.
fn plain_callables(t_bytes: &[u8], s_bytes: &[u8]) -> Option<(Type, Type)> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    let tvar =
        |v: &Type| matches!(v, Type::CallableType { variables, .. } if !variables.is_empty());
    if tvar(&t) || tvar(&s) {
        return None;
    }
    let t_is_call = matches!(t, Type::CallableType { .. });
    let s_is_call = matches!(s, Type::CallableType { .. });
    if t_is_call && s_is_call {
        Some((t, s))
    } else {
        None
    }
}

/// `mypy.checker.overload_can_never_match` (checker.py:9974-9994):
/// `is_callable_compatible(exp_sig, other, is_compat=is_more_precise,
/// is_proper_subtype=True, ignore_return=True)`.
///
/// The engine runs on decoded wire types; `sig`/`other` must both be plain
/// non-generic `CallableType`s (the erase+expand Python does is a no-op when
/// there are no variables; generic operands defer). Shared by the
/// `#[pyfunction]` entry and the
/// `overload_override::rust_check_overlapping_overloads` driver loop.
pub(crate) fn overload_can_never_match_inner(
    sig: &Type,
    other: &Type,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let tvar =
        |v: &Type| matches!(v, Type::CallableType { variables, .. } if !variables.is_empty());
    if tvar(sig) || tvar(other) {
        return None;
    }
    let t_is_call = matches!(sig, Type::CallableType { .. });
    let s_is_call = matches!(other, Type::CallableType { .. });
    if !(t_is_call && s_is_call) {
        return None;
    }
    let res = resolver.resolver();
    let more_precise = |l: &Type, r: &Type| is_more_precise(l, r, false, strict_optional, res);
    is_callable_compatible(
        sig,
        other,
        &more_precise,
        true,  // is_proper_subtype
        false, // ignore_pos_arg_names
        false, // strict_concatenate
        true,  // ignore_return
        false, // check_args_covariantly
        false, // allow_partial_overlap
        res,
    )
}

/// `#[pyfunction]` entry: `mypy.checker.overload_can_never_match`
/// (checker.py:9974-9994). Wire blobs in, `Some(bool)` when Rust decided,
/// `None` (defer to the pure-Python path) for generic or non-callable
/// operands.
#[pyfunction]
pub(crate) fn rust_overload_can_never_match(
    signature_bytes: &[u8],
    other_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let (sig, other) = plain_callables(signature_bytes, other_bytes)?;
    overload_can_never_match_inner(&sig, &other, strict_optional, resolver)
}

/// `mypy.checker.is_more_general_arg_prefix` (checker.py:9997-10012), the
/// Callable-vs-Callable branch: `is_callable_compatible(t, s,
/// is_compat=is_proper_subtype, is_proper_subtype=True, ignore_return=True)`.
/// The Overloaded branch defers to Python.
#[pyfunction]
pub(crate) fn rust_is_more_general_arg_prefix(
    t_bytes: &[u8],
    s_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let (t, s) = plain_callables(t_bytes, s_bytes)?;
    let res = resolver.resolver();
    // is_proper_subtype == _is_subtype(proper_subtype=True).
    let proper = SubtypeContext::new(false, false, false, false, true, strict_optional);
    let is_proper_subtype = |l: &Type, r: &Type| is_subtype(l, r, &proper, res);
    is_callable_compatible(
        &t,
        &s,
        &is_proper_subtype,
        true,  // is_proper_subtype
        false, // ignore_pos_arg_names
        false, // strict_concatenate
        true,  // ignore_return
        false, // check_args_covariantly
        false, // allow_partial_overlap
        res,
    )
}

/// `mypy.checker.is_same_arg_prefix` (checker.py:10015-10027):
/// `is_callable_compatible(t, s, is_compat=is_same_type, is_proper_subtype=
/// True, ignore_return=True, check_args_covariantly=True,
/// ignore_pos_arg_names=True)`.
#[pyfunction]
pub(crate) fn rust_is_same_arg_prefix(
    t_bytes: &[u8],
    s_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let (t, s) = plain_callables(t_bytes, s_bytes)?;
    let res = resolver.resolver();
    let same = |l: &Type, r: &Type| is_same_type(l, r, false, strict_optional, res);
    is_callable_compatible(
        &t, &s, &same, true,  // is_proper_subtype
        true,  // ignore_pos_arg_names
        false, // strict_concatenate
        true,  // ignore_return
        true,  // check_args_covariantly
        false, // allow_partial_overlap
        res,
    )
}
