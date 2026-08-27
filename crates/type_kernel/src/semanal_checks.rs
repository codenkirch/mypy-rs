//! Native ports of `SemanticAnalyzer` check/arbitration functions.
//!
//! Ports semanal decision heads that read scalar facts and return a
//! branch tag; Python applies the side effects:
//! - `check_function_signature` (semanal.py:2072) count arbitration
//! - `check_decorated_function_is_method` (semanal.py:2256) predicate
//! - `check_fixed_args` (semanal.py:6962) arg-count + arg-kinds arbitration

use pyo3::prelude::*;
use pyo3::types::PyAny;

// ---------------------------------------------------------------------------
// check_function_signature count arbitration (issue #940)
// ---------------------------------------------------------------------------

/// Decision tags; must match `NATIVE_FUNC_SIG_*` in mypy/semanal.py.
pub(crate) const FUNC_SIG_OK: i64 = 0;
pub(crate) const FUNC_SIG_TOO_FEW: i64 = 1;
pub(crate) const FUNC_SIG_TOO_MANY: i64 = 2;

/// Pure decision core: compare the signature argument count against the
/// declared argument count. Kept separate from the PyO3 entry so the
/// decision table is unit-testable without a Python runtime.
fn classify_function_signature(sig_arg_types_len: usize, arguments_len: usize) -> i64 {
    if sig_arg_types_len < arguments_len {
        FUNC_SIG_TOO_FEW
    } else if sig_arg_types_len > arguments_len {
        FUNC_SIG_TOO_MANY
    } else {
        FUNC_SIG_OK
    }
}

/// `#[pyfunction]` entry for
/// `SemanticAnalyzer.check_function_signature` (semanal.py:2072).
///
/// Reads the length of the signature's `arg_types` and the length of the
/// function's `arguments`, returning a branch tag. Always decidable;
/// never returns `None`.
#[pyfunction]
#[pyo3(signature = (sig_arg_types_len, arguments_len))]
pub(crate) fn rust_classify_function_signature(
    sig_arg_types_len: usize,
    arguments_len: usize,
) -> PyResult<i64> {
    Ok(classify_function_signature(
        sig_arg_types_len,
        arguments_len,
    ))
}

// ---------------------------------------------------------------------------
// check_decorated_function_is_method predicate (issue #941)
// ---------------------------------------------------------------------------

/// The pure decision over resolved facts, kept separate from the PyO3
/// entry so the algebra is unit-testable without a Python runtime.
///
/// `self_type_is_none` mirrors `not self.type`; `is_func_scope` mirrors
/// `self.is_func_scope()`. Returns `true` when the function is a method
/// (inside a class body, not nested in a function scope) and `false`
/// when the decorator is used in a non-method context.
fn classify_decorated_function_is_method(self_type_is_none: bool, is_func_scope: bool) -> bool {
    // method iff self.type is not None and not in a function scope.
    !self_type_is_none && !is_func_scope
}

/// `#[pyfunction]` entry for
/// `SemanticAnalyzer.check_decorated_function_is_method`
/// (mypy/semanal.py:2256-2258).
///
/// `semanal` is the live `SemanticAnalyzer` (`self`). Rust reads
/// `self.type` (None check) and calls `self.is_func_scope()` as a bound
/// method. Returns `Some(true)` when the function is a method (no-op),
/// `Some(false)` when it is a non-method context (Python emits the
/// fail), or `None` to defer when the live state cannot be read.
#[pyfunction]
pub(crate) fn rust_check_decorated_function_is_method(semanal: &PyAny) -> PyResult<Option<bool>> {
    let self_type = match semanal.getattr("type") {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let self_type_is_none = self_type.is_none();
    if self_type_is_none {
        // Outside a class body: not a method.
        return Ok(Some(false));
    }
    let is_func_scope = match semanal.call_method0("is_func_scope") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    Ok(Some(classify_decorated_function_is_method(
        self_type_is_none,
        is_func_scope,
    )))
}

// ---------------------------------------------------------------------------
// check_fixed_args arbitration (issue #935)
// ---------------------------------------------------------------------------

/// Decision tags for `check_fixed_args`; must match
/// `NATIVE_FIXED_ARGS_*` in mypy/semanal.py.
pub(crate) const FIXED_ARGS_OK: i64 = 0;
pub(crate) const FIXED_ARGS_WRONG_COUNT: i64 = 1;
pub(crate) const FIXED_ARGS_WRONG_KINDS: i64 = 2;

/// Pure decision core of `SemanticAnalyzer.check_fixed_args`
/// (semanal.py:6962-6976). Checks two gaps in order:
/// 1. `len(expr.args) != numargs` -> wrong count
/// 2. `expr.arg_kinds != [ARG_POS]*numargs` -> wrong kinds
///
/// `ARG_POS == 0` (mypy.nodes.ArgKind.ARG_POS).
fn classify_fixed_args(args_len: usize, arg_kinds: &[i64], numargs: usize) -> i64 {
    if args_len != numargs {
        return FIXED_ARGS_WRONG_COUNT;
    }
    if arg_kinds.len() != numargs || !arg_kinds.iter().all(|&k| k == 0) {
        return FIXED_ARGS_WRONG_KINDS;
    }
    FIXED_ARGS_OK
}

/// `#[pyfunction]` entry; the shim passes `len(expr.args)`, the integer
/// arg-kinds list, and `numargs`. Returns `Some(tag)` always (never
/// defers). Python applies the `self.fail` side effect per the tag.
#[pyfunction]
pub(crate) fn rust_classify_fixed_args(
    args_len: usize,
    arg_kinds: Vec<i64>,
    numargs: usize,
) -> PyResult<Option<i64>> {
    Ok(Some(classify_fixed_args(args_len, &arg_kinds, numargs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(sig_len: usize, args_len: usize) -> i64 {
        classify_function_signature(sig_len, args_len)
    }

    #[test]
    fn ok_when_equal() {
        assert_eq!(classify(0, 0), FUNC_SIG_OK);
        assert_eq!(classify(3, 3), FUNC_SIG_OK);
    }

    #[test]
    fn too_few_when_sig_shorter() {
        assert_eq!(classify(0, 1), FUNC_SIG_TOO_FEW);
        assert_eq!(classify(2, 5), FUNC_SIG_TOO_FEW);
    }

    #[test]
    fn too_many_when_sig_longer() {
        assert_eq!(classify(1, 0), FUNC_SIG_TOO_MANY);
        assert_eq!(classify(5, 2), FUNC_SIG_TOO_MANY);
    }

    fn classify_method(self_type_is_none: bool, is_func_scope: bool) -> bool {
        classify_decorated_function_is_method(self_type_is_none, is_func_scope)
    }

    #[test]
    fn test_method_in_class_body() {
        // self.type set, not in func scope: method.
        assert!(classify_method(false, false));
    }

    #[test]
    fn test_not_method_outside_class() {
        // self.type is None: not a method.
        assert!(!classify_method(true, false));
    }

    #[test]
    fn test_not_method_in_func_scope() {
        // Inside a function (nested): not a method even in a class.
        assert!(!classify_method(false, true));
    }

    #[test]
    fn test_not_method_outside_class_and_in_func() {
        assert!(!classify_method(true, true));
    }

    #[test]
    fn test_fixed_args_ok() {
        assert_eq!(classify_fixed_args(2, &[0, 0], 2), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_wrong_count() {
        assert_eq!(classify_fixed_args(1, &[0], 2), FIXED_ARGS_WRONG_COUNT);
        assert_eq!(
            classify_fixed_args(3, &[0, 0, 0], 2),
            FIXED_ARGS_WRONG_COUNT
        );
    }

    #[test]
    fn test_fixed_args_wrong_kinds() {
        assert_eq!(classify_fixed_args(2, &[0, 3], 2), FIXED_ARGS_WRONG_KINDS);
        assert_eq!(classify_fixed_args(2, &[3, 0], 2), FIXED_ARGS_WRONG_KINDS);
        assert_eq!(classify_fixed_args(2, &[3, 3], 2), FIXED_ARGS_WRONG_KINDS);
    }

    #[test]
    fn test_fixed_args_zero_args_ok() {
        assert_eq!(classify_fixed_args(0, &[], 0), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_one_arg_ok() {
        assert_eq!(classify_fixed_args(1, &[0], 1), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_arg_kinds_length_mismatch() {
        assert_eq!(classify_fixed_args(2, &[0], 2), FIXED_ARGS_WRONG_KINDS);
    }
}
