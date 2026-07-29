//! Stage 7b TypeInfo enrichment + node serialization (nodes.rs) for Issue #89.
//!
//! Exposes helper utilities and PyO3 functions operating on `TypeInfo` and AST nodes.

use pyo3::prelude::*;

#[pyfunction]
pub fn rust_is_builtin_type(fullname: &str) -> bool {
    fullname.starts_with("builtins.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_builtin_type() {
        assert!(rust_is_builtin_type("builtins.int"));
        assert!(rust_is_builtin_type("builtins.str"));
        assert!(!rust_is_builtin_type("typing.List"));
    }
}
