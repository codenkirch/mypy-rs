//! Pure helper functions from mypy/errors.py (Issue #534).
//!
//! Ports string/list operations that have no Type dependency:
//! - `remove_path_prefix`
//! - `create_errors`
//! - `report_internal_error` (formatting part only)
//! - `format_messages_default` (full pretty-rendering path)
//! - `sort_within_context`
//! - `yield_nonoverlapping_types`

use pyo3::prelude::*;
use pyo3::types::PyList;

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

/// If path starts with prefix, return copy of path with the prefix removed.
/// Otherwise, return path. If prefix is None, return path unchanged.
#[pyfunction]
pub fn rust_remove_path_prefix(path: &str, prefix: Option<String>) -> String {
    match prefix {
        Some(ref p) if path.starts_with(p.as_str()) => path[p.len()..].to_string(),
        _ => path.to_string(),
    }
}

/// Build the "INTERNAL ERROR" banner. Returns the lines that would be
/// written to stderr/stdout. Pure string formatting; no sys.exit or pdb.
#[pyfunction]
#[pyo3(signature = (file, line, show_traceback, mypy_version))]
pub fn rust_report_internal_error(
    file: Option<String>,
    line: i64,
    show_traceback: bool,
    mypy_version: &str,
) -> Vec<String> {
    let prefix = if let Some(ref f) = file {
        if line > 0 {
            format!("{}:{}: ", f, line)
        } else {
            format!("{}: ", f)
        }
    } else {
        String::new()
    };

    let mut out = Vec::new();
    out.push(format!(
        "{}error: INTERNAL ERROR -- Please try using mypy master on GitHub:\n\
         https://mypy.readthedocs.io/en/stable/common_issues.html\
         #using-a-development-mypy-build",
        prefix
    ));
    if show_traceback {
        out.push("Please report a bug at https://github.com/python/mypy/issues".to_string());
    } else {
        out.push(
            "If this issue continues with mypy master, \
             please report a bug at https://github.com/python/mypy/issues"
                .to_string(),
        );
    }
    out.push(format!("version: {}", mypy_version));
    if !show_traceback {
        out.push(format!(
            "{}note: please use --show-traceback to print a traceback \
             when reporting a bug",
            prefix
        ));
    } else {
        out.push(format!("{}note: use --pdb to drop into pdb", prefix));
    }
    out
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

/// Sort error indices within the same context by priority.
/// Mirrors Errors.sort_within_context. Takes flat tuples
/// (line, column, end_line, end_column, code, priority, original_index)
/// and returns the reordered indices.
#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn rust_sort_within_context(
    errors: Vec<(i64, i64, i64, i64, Option<String>, i64, i64)>,
) -> Vec<i64> {
    let n = errors.len();
    let mut result: Vec<i64> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let i0 = i;
        while i + 1 < n
            && errors[i + 1].0 == errors[i].0
            && errors[i + 1].1 == errors[i].1
            && errors[i + 1].2 == errors[i].2
            && errors[i + 1].3 == errors[i].3
            && errors[i + 1].4 == errors[i].4
        {
            i += 1;
        }
        i += 1;
        let mut slice: Vec<usize> = (i0..i).collect();
        slice.sort_by_key(|&idx| errors[idx].5);
        for idx in slice {
            result.push(errors[idx].6);
        }
    }
    result
}

/// Build MypyError-like PyObjects from error tuples.
/// Mirrors mypy.errors.create_errors. Each tuple is
/// (file_path, line, column, end_line, end_column, severity, message, errorcode).
/// Returns a list of MypyError-style dicts with hints accumulated.
#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn rust_create_errors(
    py: Python<'_>,
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
) -> PyResult<Py<PyList>> {
    let out = PyList::empty(py);
    type Loc = (String, i64, i64);
    let mut latest: std::collections::HashMap<Loc, usize> = std::collections::HashMap::new();

    for (file_path, line, column, end_line, end_column, severity, message, errorcode) in
        error_tuples
    {
        let Some(ref fp) = file_path else {
            continue;
        };
        if severity == "note" {
            let loc = (fp.clone(), line, column);
            if let Some(&idx) = latest.get(&loc) {
                let obj = out.get_item(idx)?;
                let hints: &PyList = obj.getattr("hints")?.downcast()?;
                hints.append(message)?;
            } else {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("file_path", fp.clone())?;
                dict.set_item("line", line)?;
                dict.set_item("column", column)?;
                dict.set_item("end_line", end_line)?;
                dict.set_item("end_column", end_column)?;
                dict.set_item("message", message)?;
                dict.set_item("errorcode", errorcode)?;
                dict.set_item("severity", "note")?;
                dict.set_item("hints", PyList::empty(py))?;
                out.append(dict)?;
            }
        } else {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("file_path", fp.clone())?;
            dict.set_item("line", line)?;
            dict.set_item("column", column)?;
            dict.set_item("end_line", end_line)?;
            dict.set_item("end_column", end_column)?;
            dict.set_item("message", message)?;
            dict.set_item("errorcode", errorcode)?;
            dict.set_item("severity", "error")?;
            dict.set_item("hints", PyList::empty(py))?;
            out.append(dict)?;
            let loc = (fp.clone(), line, column);
            latest.insert(loc, out.len() - 1);
        }
    }
    Ok(out.into())
}

/// Compute which NonOverlapErrorInfo candidates are persistent across
/// all iterations. Mirrors Errors.yield_nonoverlapping_types.
///
/// Each candidate is (line, column, end_line, end_column, kind).
/// nonoverlapping_types is a list of lists of (candidate, left, right).
/// unreachable_lines is a list of sets of line numbers.
/// Returns the selected candidates (those present in every iteration).
#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn rust_yield_nonoverlapping_types(
    nonoverlapping_types: Vec<Vec<((i64, i64, i64, i64, String), Py<PyAny>)>>,
    unreachable_lines: Vec<Vec<i64>>,
) -> Vec<(i64, i64, i64, i64, String)> {
    type Cand = (i64, i64, i64, i64, String);
    let mut selected: std::collections::HashSet<Cand> = std::collections::HashSet::new();
    let all_keys: Vec<Cand> = {
        let mut seen = std::collections::HashSet::new();
        let mut keys = Vec::new();
        for m in &nonoverlapping_types {
            for (c, _) in m {
                if seen.insert(c.clone()) {
                    keys.push(c.clone());
                }
            }
        }
        keys
    };
    for cand in all_keys {
        let in_all = nonoverlapping_types
            .iter()
            .zip(unreachable_lines.iter())
            .all(|(m, lines)| m.iter().any(|(c, _)| c == &cand) || lines.contains(&cand.0));
        if in_all {
            selected.insert(cand);
        }
    }
    let mut out: Vec<Cand> = selected.into_iter().collect();
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
            .then(a.4.cmp(&b.4))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_path_prefix() {
        assert_eq!(
            rust_remove_path_prefix("foo/bar/baz.py", Some("foo/".to_string())),
            "bar/baz.py"
        );
        assert_eq!(
            rust_remove_path_prefix("foo/bar/baz.py", Some("other/".to_string())),
            "foo/bar/baz.py"
        );
        assert_eq!(
            rust_remove_path_prefix("foo/bar/baz.py", None),
            "foo/bar/baz.py"
        );
    }

    #[test]
    fn test_report_internal_error_with_file() {
        let lines = rust_report_internal_error(Some("foo.py".to_string()), 10, false, "1.0");
        assert!(lines[0].starts_with("foo.py:10: error: INTERNAL ERROR"));
        assert!(lines[2].starts_with("version: 1.0"));
        assert!(lines[3].contains("show-traceback"));
    }

    #[test]
    fn test_report_internal_error_no_file() {
        let lines = rust_report_internal_error(None, 0, false, "1.0");
        assert!(lines[0].starts_with("error: INTERNAL ERROR"));
    }

    #[test]
    fn test_report_internal_error_show_traceback() {
        let lines = rust_report_internal_error(Some("f.py".to_string()), 1, true, "2.0");
        assert!(lines[1].contains("report a bug"));
        assert!(lines[3].contains("use --pdb"));
    }

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

    #[test]
    fn test_sort_within_context() {
        let errors = vec![
            (1, 0, 1, 5, Some("c1".to_string()), 2, 0),
            (1, 0, 1, 5, Some("c1".to_string()), 1, 1),
            (2, 0, 2, 3, Some("c2".to_string()), 0, 2),
        ];
        let res = rust_sort_within_context(errors);
        assert_eq!(res, vec![1, 0, 2]);
    }

    #[test]
    fn test_sort_within_context_diff_code() {
        let errors = vec![
            (1, 0, 1, 5, Some("c1".to_string()), 5, 0),
            (1, 0, 1, 5, Some("c2".to_string()), 1, 1),
        ];
        let res = rust_sort_within_context(errors);
        assert_eq!(res, vec![0, 1]);
    }
}
