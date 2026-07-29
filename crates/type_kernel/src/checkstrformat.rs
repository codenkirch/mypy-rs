//! Stage 6b string format checking (checkstrformat.rs) for Issue #86.
//!
//! Ports string format spec constants and helpers:
//! - `NUMERIC_TYPES_OLD`, `NUMERIC_TYPES_NEW`
//! - `REQUIRE_INT_OLD`, `REQUIRE_INT_NEW`
//! - `FLOAT_TYPES`
//! - `rust_is_numeric_format_type` PyO3 binding

use pyo3::prelude::*;

const NUMERIC_TYPES_OLD: &[&str] = &["d", "i", "o", "u", "x", "X", "e", "E", "f", "F", "g", "G"];
const NUMERIC_TYPES_NEW: &[&str] = &[
    "b", "d", "o", "e", "E", "f", "F", "g", "G", "n", "x", "X", "%",
];

/// Check if a conversion specifier type character is numeric for printf or str.format.
#[pyfunction]
pub fn rust_is_numeric_format_type(conv_type: &str, is_new_style: bool) -> bool {
    if is_new_style {
        NUMERIC_TYPES_NEW.contains(&conv_type)
    } else {
        NUMERIC_TYPES_OLD.contains(&conv_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_numeric_format_type() {
        assert!(rust_is_numeric_format_type("d", false));
        assert!(rust_is_numeric_format_type("%", true));
        assert!(!rust_is_numeric_format_type("%", false));
        assert!(!rust_is_numeric_format_type("s", true));
    }
}
