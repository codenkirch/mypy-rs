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

use pyo3::prelude::*;

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
}
