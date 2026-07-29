//! Stage 8c pattern matching (checkpattern.rs) for Issue #91.
//!
//! Ports pattern matching constants and pure helpers:
//! - `SELF_MATCH_TYPE_NAMES`, `NON_SEQUENCE_MATCH_TYPE_NAMES`
//! - `rust_is_self_match_type` PyO3 binding

use pyo3::prelude::*;

const SELF_MATCH_TYPE_NAMES: &[&str] = &[
    "builtins.bool",
    "builtins.bytearray",
    "builtins.bytes",
    "builtins.dict",
    "builtins.float",
    "builtins.frozenset",
    "builtins.int",
    "builtins.list",
    "builtins.set",
    "builtins.str",
    "builtins.tuple",
];

const NON_SEQUENCE_MATCH_TYPE_NAMES: &[&str] =
    &["builtins.str", "builtins.bytes", "builtins.bytearray"];

/// Check if a builtin class fullname is self-matching in pattern matching.
#[pyfunction]
pub fn rust_is_self_match_type(fullname: &str) -> bool {
    SELF_MATCH_TYPE_NAMES.contains(&fullname)
}

/// Check if a builtin class fullname is non-sequence matching.
#[pyfunction]
pub fn rust_is_non_sequence_match_type(fullname: &str) -> bool {
    NON_SEQUENCE_MATCH_TYPE_NAMES.contains(&fullname)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_match_type() {
        assert!(rust_is_self_match_type("builtins.int"));
        assert!(rust_is_self_match_type("builtins.str"));
        assert!(!rust_is_self_match_type("builtins.object"));
    }

    #[test]
    fn test_non_sequence_match_type() {
        assert!(rust_is_non_sequence_match_type("builtins.str"));
        assert!(!rust_is_non_sequence_match_type("builtins.list"));
    }
}
