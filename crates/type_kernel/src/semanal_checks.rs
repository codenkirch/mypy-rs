//! `SemanticAnalyzer.check_function_signature` count-arbitration port
//! (mypy.semanal).
//!
//! The Python method (semanal.py:2072) compares the length of a function's
//! declared type signature (`sig.arg_types`) against the number of declared
//! arguments (`fdef.arguments`) and picks one of three branches: too few,
//! too many, or ok. This module ports only the count comparison: it reads
//! two integers and returns a branch tag. The Python shim applies the side
//! effects (extending `sig.arg_types` with dummy `Any` arguments and calling
//! `self.fail`) and keeps the pure-Python body as the fallback.
//!
//! Also hosts the `check_decorated_function_is_method` predicate port
//! (semanal.py:2256-2258): a single bool conjunction
//! `not self.type or self.is_func_scope()`. Rust reads the live analyzer
//! state via PyO3 and returns the negation.

use pyo3::prelude::*;
use pyo3::types::PyAny;

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
/// function's `arguments`, returning a branch tag. Always decidable; never
/// returns `None`.
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
}
