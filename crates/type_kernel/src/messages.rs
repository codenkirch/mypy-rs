//! Stage 6 message formatting (messages.rs) for Issue #85.
//!
//! Ports pure formatting utility functions from `mypy/messages.py`:
//! - `format_key_list`
//! - `plural_s` / `quote_type_string`
//! - `rust_format_key_list` PyO3 binding

use pyo3::prelude::*;

/// Format a list of keys for TypedDict error messages.
#[pyfunction]
pub fn rust_format_key_list(keys: Vec<String>, short: bool) -> String {
    let formatted_keys: Vec<String> = keys.iter().map(|k| format!("\"{k}\"")).collect();
    let td = if short { "" } else { "TypedDict " };
    if keys.is_empty() {
        format!("no {td}keys")
    } else if keys.len() == 1 {
        format!("{td}key {}", formatted_keys[0])
    } else {
        format!("{td}keys ({})", formatted_keys.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key_list() {
        assert_eq!(rust_format_key_list(vec![], false), "no TypedDict keys");
        assert_eq!(
            rust_format_key_list(vec!["a".to_string()], false),
            "TypedDict key \"a\""
        );
        assert_eq!(
            rust_format_key_list(vec!["a".to_string(), "b".to_string()], true),
            "keys (\"a\", \"b\")"
        );
    }
}
