//! `ExpressionChecker.check_callable_call` typeobj-fail gate port
//! (mypy.checkexpr, issue #1464 C2).
//!
//! The Python gate (checkexpr.py:2941) is an `if` / `elif` over a
//! `CallableType` callee: when `is_type_obj()` holds, a protocol
//! type-object fires `CANNOT_INSTANTIATE_PROTOCOL` and an abstract
//! type-object (not exempt by `fallback_to_any`) fires
//! `cannot_instantiate_abstract_class`, both suppressed by
//! `from_type_type` (the `Type[...]` exemption). The original double-
//! evaluates `is_type_obj()` (once in the `if`, once in the `elif`) and
//! re-runs `type_object()` per arm; this port collapses that to one
//! `is_type_obj()` and one `type_object()` and returns an arm tag. The
//! Python shim applies the two `fail`s and the `can_return_none`
//! abstract-attribute fold, and keeps the pure-Python `if`/`elif` as the
//! fallback.
//!
//! Strangler-fig contract: `None` defers to Python. The only deferrals are
//! an unreadable `from_type_type` attribute or `type_object()` raising
//! (its `assert isinstance(..., Instance)`), which mirror the shim's
//! `try/except`. Every reachable arm is classified, including the
//! no-fail tail.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

/// Decision tags for the typeobj-fail gate; must match
/// `NATIVE_TYPEOBJ_GATE_*` in mypy/checkexpr.py.
pub const TYPEOBJ_GATE_NONE: i64 = 0;
pub const TYPEOBJ_GATE_PROTOCOL: i64 = 1;
pub const TYPEOBJ_GATE_ABSTRACT: i64 = 2;

/// Pure arm decision mirroring the checkexpr.py:2941-2969 `if` / `elif`
/// short-circuit order. The `from_type_type` exemption is checked before
/// the arm flag is *applied* but the flag is still read (matching the
/// original, which evaluates `type_object().is_protocol` then
/// `not from_type_type`).
pub fn classify_typeobj_gate(
    is_type_obj: bool,
    is_protocol: bool,
    is_abstract: bool,
    from_type_type: bool,
    fallback_to_any: bool,
) -> i64 {
    if !is_type_obj {
        return TYPEOBJ_GATE_NONE;
    }
    if is_protocol && !from_type_type {
        return TYPEOBJ_GATE_PROTOCOL;
    }
    if is_abstract && !from_type_type && !fallback_to_any {
        return TYPEOBJ_GATE_ABSTRACT;
    }
    TYPEOBJ_GATE_NONE
}

/// `#[pyfunction]` entry. Reads the live `CallableType` via PyO3: one
/// `is_type_obj()` (short-circuiting the `type_object()` call entirely for
/// the 88% non-type-object callees), then one `type_object()` for the
/// flags. Defers (`None`) on an unreadable attribute or a `type_object()`
/// assertion so the shim re-runs the pure-Python gate.
#[pyfunction]
pub(crate) fn rust_classify_typeobj_gate(py: Python<'_>, callee: &PyAny) -> PyResult<Option<i64>> {
    let classify: PyResult<Option<i64>> = (|| {
        let is_to = callee.call_method0("is_type_obj")?.extract::<bool>()?;
        if !is_to {
            return Ok(Some(classify_typeobj_gate(
                false, false, false, false, false,
            )));
        }
        let to = callee.call_method0("type_object")?;
        let is_protocol = to.getattr("is_protocol")?.extract::<bool>()?;
        let is_abstract = to.getattr("is_abstract")?.extract::<bool>()?;
        let fallback_to_any = to.getattr("fallback_to_any")?.extract::<bool>()?;
        let from_type_type = callee.getattr("from_type_type")?.extract::<bool>()?;
        Ok(Some(classify_typeobj_gate(
            is_to,
            is_protocol,
            is_abstract,
            from_type_type,
            fallback_to_any,
        )))
    })();
    // Contract (#1466): an unreadable attribute defers (None), other PyErrs
    // stay visible so genuine kernel bugs surface in the parity tests.
    match classify {
        Ok(v) => Ok(v),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod checkcall_typeobj_tests {
    use super::*;

    fn g(is_to: bool, is_proto: bool, is_abs: bool, from_tt: bool, fb_any: bool) -> i64 {
        classify_typeobj_gate(is_to, is_proto, is_abs, from_tt, fb_any)
    }

    #[test]
    fn test_not_typeobj_is_none() {
        assert_eq!(g(false, true, true, false, false), TYPEOBJ_GATE_NONE);
    }

    #[test]
    fn test_protocol_fires() {
        assert_eq!(g(true, true, false, false, false), TYPEOBJ_GATE_PROTOCOL);
    }

    #[test]
    fn test_protocol_exempt_by_from_type_type() {
        assert_eq!(g(true, true, false, true, false), TYPEOBJ_GATE_NONE);
    }

    #[test]
    fn test_abstract_fires() {
        assert_eq!(g(true, false, true, false, false), TYPEOBJ_GATE_ABSTRACT);
    }

    #[test]
    fn test_abstract_exempt_by_fallback_to_any() {
        assert_eq!(g(true, false, true, false, true), TYPEOBJ_GATE_NONE);
    }

    #[test]
    fn test_abstract_exempt_by_from_type_type() {
        assert_eq!(g(true, false, true, true, false), TYPEOBJ_GATE_NONE);
    }

    #[test]
    fn test_protocol_beats_abstract() {
        // A type-object that is both a protocol and abstract: the original
        // fires the protocol arm first and never reaches the elif.
        assert_eq!(g(true, true, true, false, false), TYPEOBJ_GATE_PROTOCOL);
    }

    #[test]
    fn test_typeobj_no_arm_is_none() {
        assert_eq!(g(true, false, false, false, false), TYPEOBJ_GATE_NONE);
    }

    #[test]
    fn test_from_type_type_protects_both_arms() {
        assert_eq!(g(true, true, true, true, true), TYPEOBJ_GATE_NONE);
    }
}
