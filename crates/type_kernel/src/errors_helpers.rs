//! Pure helper functions from mypy/errors.py (Issue #534).
//!
//! Ports `format_messages_default` (full pretty-rendering path), a
//! string/list operation with no Type dependency.
//!
//! Five further port stubs (`remove_path_prefix`, `create_errors`,
//! `report_internal_error`, `sort_within_context`, and the surplus
//! `yield_nonoverlapping_types`) landed with zero callers and were
//! deleted in #1459; re-create the four B6-relevant ones inside a real
//! render-bundle port, where the parity burden is paid once.

use pyo3::prelude::*;

/// Default source offset for pretty-rendered source snippets.
const DEFAULT_SOURCE_OFFSET: usize = 4;

/// Error codes that should be shown even on notes.
const SHOW_NOTE_CODES: &[&str] = &["annotation-unchecked", "deprecated"];

/// Expand tabs to spaces (tabsize=8, matching Python str.expandtabs).
fn expandtabs(s: &str) -> String {
    let tabsize: usize = 8;
    let mut out = String::with_capacity(s.len());
    let mut col: usize = 0;
    for ch in s.chars() {
        if ch == '\t' {
            let n = tabsize - (col % tabsize);
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

/// Return number of leading whitespace chars (matching Python str.lstrip len diff).
fn leading_whitespace_len(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

/// Format error tuples into default string representation.
/// Mirrors Errors.format_messages_default (non-pretty path).
#[allow(clippy::type_complexity)]
#[pyfunction]
#[pyo3(signature = (error_tuples, source_lines, show_column_numbers, show_error_end, hide_error_codes, pretty))]
pub fn rust_format_messages_default_pretty(
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
    source_lines: Option<Vec<String>>,
    show_column_numbers: bool,
    show_error_end: bool,
    hide_error_codes: bool,
    pretty: bool,
) -> Vec<String> {
    let mut a: Vec<String> = Vec::with_capacity(error_tuples.len());
    for (file, line, mut column, end_line, end_column, severity, message, code) in error_tuples {
        let s = if let Some(ref f) = file {
            let srcloc = if show_column_numbers && line >= 0 && column >= 0 {
                let mut loc = format!("{}:{}:{}", f, line, 1 + column);
                if show_error_end && end_line >= 0 && end_column >= 0 {
                    loc.push_str(&format!(":{}:{}", end_line, end_column));
                }
                loc
            } else if line >= 0 {
                format!("{}:{}", f, line)
            } else {
                f.clone()
            };
            format!("{}: {}: {}", srcloc, severity, message)
        } else {
            message
        };

        let mut s = s;
        if !hide_error_codes {
            if let Some(ref c) = code {
                if severity != "note" || SHOW_NOTE_CODES.contains(&c.as_str()) {
                    s.push_str(&format!("  [{}]", c));
                }
            }
        }
        a.push(s);

        if pretty && severity == "error" {
            if let Some(ref lines) = source_lines {
                if line > 0 && (line as usize) <= lines.len() {
                    let source_line = &lines[(line - 1) as usize];
                    let source_line_expanded = expandtabs(source_line);
                    let leading = leading_whitespace_len(source_line);
                    if (column as usize) < leading {
                        column = leading as i64;
                    }
                    let col_expanded =
                        expandtabs(&source_line[..(column as usize).min(source_line.len())]).len();
                    let end_expanded =
                        expandtabs(&source_line[..(end_column as usize).min(source_line.len())])
                            .len();
                    a.push(format!(
                        "{}{}",
                        " ".repeat(DEFAULT_SOURCE_OFFSET),
                        source_line_expanded
                    ));
                    let marker = if end_line == line && end_column > column {
                        format!(
                            "^{}",
                            "~".repeat(end_expanded.saturating_sub(col_expanded + 1))
                        )
                    } else if end_line != line {
                        format!(
                            "^{}",
                            "~".repeat(source_line_expanded.len().saturating_sub(col_expanded + 1))
                        )
                    } else {
                        "^".to_string()
                    };
                    a.push(format!(
                        "{}{}",
                        " ".repeat(DEFAULT_SOURCE_OFFSET + col_expanded),
                        marker
                    ));
                }
            }
        }
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_messages_default_basic() {
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
        let res = rust_format_messages_default_pretty(tuples, None, true, true, false, false);
        assert_eq!(
            res,
            vec!["foo.py:10:3:10:5: error: Undefined name  [name-defined]"]
        );
    }

    #[test]
    fn test_format_messages_note_with_code() {
        let tuples = vec![(
            Some("f.py".to_string()),
            1,
            0,
            1,
            0,
            "note".to_string(),
            "msg".to_string(),
            Some("annotation-unchecked".to_string()),
        )];
        let res = rust_format_messages_default_pretty(tuples, None, false, false, false, false);
        assert_eq!(res, vec!["f.py:1: note: msg  [annotation-unchecked]"]);
    }

    #[test]
    fn test_format_messages_note_hidden_code() {
        let tuples = vec![(
            Some("f.py".to_string()),
            1,
            0,
            1,
            0,
            "note".to_string(),
            "msg".to_string(),
            Some("misc".to_string()),
        )];
        let res = rust_format_messages_default_pretty(tuples, None, false, false, false, false);
        assert_eq!(res, vec!["f.py:1: note: msg"]);
    }
}
