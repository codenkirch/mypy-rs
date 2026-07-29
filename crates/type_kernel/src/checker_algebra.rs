//! Stage 10 checker algebra subset (checker_algebra.rs) for Issue #94.
//!
//! Pure Type-algebra functions from checker:
//! - `rust_is_type_overlap` PyO3 entry point

use pyo3::prelude::*;

#[pyfunction]
pub fn rust_is_type_overlap(type_a_name: &str, type_b_name: &str) -> bool {
    type_a_name == type_b_name
        || type_a_name == "builtins.object"
        || type_b_name == "builtins.object"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_type_overlap() {
        assert!(rust_is_type_overlap("builtins.int", "builtins.int"));
        assert!(rust_is_type_overlap("builtins.int", "builtins.object"));
        assert!(!rust_is_type_overlap("builtins.int", "builtins.str"));
    }
}
