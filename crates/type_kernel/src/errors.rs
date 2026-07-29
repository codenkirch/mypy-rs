//! Error collection and formatting logic for type-kernel (Issue #88).
//!
//! Ports pure data operations from `mypy.errors`:
//! - `ErrorInfo` data representation, serialization, and deserialization
//! - Path simplification (`remove_path_prefix`, `simplify_path`)
//! - Error sorting (`sort_messages`, `sort_within_context`)
//! - Duplicate removal (`remove_duplicates`)
//! - Simple message formatting (`format_messages_default`)

use pyo3::prelude::*;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErrorInfoData {
    pub import_ctx: Vec<(String, i64)>,
    pub local_ctx: (Option<String>, Option<String>),
    pub line: i64,
    pub column: i64,
    pub end_line: i64,
    pub end_column: i64,
    pub severity: String,
    pub message: String,
    pub code: Option<String>,
    pub blocker: bool,
    pub only_once: bool,
    pub module: Option<String>,
    pub target: Option<String>,
    pub origin_span: Vec<i64>,
    pub priority: i64,
}

/// Remove `prefix` from `path` if `path` starts with `prefix`.
#[allow(dead_code)]
pub(crate) fn remove_path_prefix(path: &str, prefix: Option<&str>) -> String {
    match prefix {
        Some(p) if path.starts_with(p) => path[p.len()..].to_string(),
        _ => path.to_string(),
    }
}

/// Format error tuples into default string representation (simplifying path, line, col, code).
#[allow(clippy::type_complexity)]
#[pyfunction]
pub fn rust_format_messages_default(
    error_tuples: Vec<(
        Option<String>,
        i64,
        i64,
        i64,
        i64,
        String,
        String,
        Option<String>,
    )>,
    show_column_numbers: bool,
    show_error_end: bool,
    hide_error_codes: bool,
) -> Vec<String> {
    let mut out = Vec::with_capacity(error_tuples.len());
    for (file, line, column, end_line, end_column, severity, message, code) in error_tuples {
        let mut s = if let Some(f) = file {
            let srcloc = if show_column_numbers && line >= 0 && column >= 0 {
                let mut loc = format!("{}:{}:{}", f, line, 1 + column);
                if show_error_end && end_line >= 0 && end_column >= 0 {
                    loc.push_str(&format!(":{}:{}", end_line, end_column));
                }
                loc
            } else if line >= 0 {
                format!("{}:{}", f, line)
            } else {
                f
            };
            format!("{}: {}: {}", srcloc, severity, message)
        } else {
            message
        };

        if !hide_error_codes {
            if let Some(c) = code {
                if severity != "note" {
                    s.push_str(&format!("  [{}]", c));
                }
            }
        }
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_path_prefix() {
        assert_eq!(
            remove_path_prefix("foo/bar/baz.py", Some("foo/")),
            "bar/baz.py"
        );
        assert_eq!(
            remove_path_prefix("foo/bar/baz.py", Some("other/")),
            "foo/bar/baz.py"
        );
        assert_eq!(remove_path_prefix("foo/bar/baz.py", None), "foo/bar/baz.py");
    }

    #[test]
    fn test_format_messages_default() {
        let tuples = vec![(
            Some("foo.py".to_string()),
            10,
            2,
            10,
            5,
            "error".to_string(),
            "Undefined name".to_string(),
            Some("name-defined".to_string()),
        )];
        let res = rust_format_messages_default(tuples, true, true, false);
        assert_eq!(
            res,
            vec!["foo.py:10:3:10:5: error: Undefined name  [name-defined]"]
        );
    }
}
