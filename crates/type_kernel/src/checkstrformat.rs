//! Stage 6b string format checking (checkstrformat.rs) for Issue #86/#297.
//!
//! Ports the pure format-string parsing functions from mypy/checkstrformat.py:
//! - `rust_is_numeric_format_type` — numeric format type check
//! - `rust_parse_conversion_specifiers` — printf-style `%` format parsing
//! - `rust_find_non_escaped_targets` — brace-matching for str.format()
//! - `rust_parse_format_value` — full str.format() specifier parsing
//!
//! These are the self-contained parsing functions that don't need the type
//! checker. The type-checking logic (StringFormatterChecker methods) stays in
//! Python because it requires the checker, message builder, and named_type.
//!
//! Rust returns parsed specifier data as tuples; Python constructs
//! `ConversionSpecifier` objects via `from_fields`. Error codes are returned
//! as integers so Python can call `msg.fail()` with the right message.

use pyo3::prelude::*;

const NUMERIC_TYPES_OLD: &[&str] = &["d", "i", "o", "u", "x", "X", "e", "E", "f", "F", "g", "G"];
const NUMERIC_TYPES_NEW: &[&str] = &[
    "b", "d", "o", "e", "E", "f", "F", "g", "G", "n", "x", "X", "%",
];

/// Check if a conversion specifier type character is numeric for printf
/// or str.format.
#[pyfunction]
pub fn rust_is_numeric_format_type(conv_type: &str, is_new_style: bool) -> bool {
    if is_new_style {
        NUMERIC_TYPES_NEW.contains(&conv_type)
    } else {
        NUMERIC_TYPES_OLD.contains(&conv_type)
    }
}

// ===== Printf-style (% interpolation) parsing =====
//
// Mirrors FORMAT_RE:
//   %(\((?P<key>[^)]*)\))?(?P<flags>[#0\-+ ]*)?
//   (?P<width>[1-9][0-9]*|\*)?(?:\.(?P<precision>\*|[0-9]+)?)?
//   [hlL]?(?P<type>.)?
//
// All groups after % are optional and greedy. re.finditer matches every
// '%' in the string, each match extending as far as the groups allow.

/// Parse one printf specifier starting at `pos` (bytes[pos] == '%').
/// Returns (whole_seq, start_pos, key, conv_type, flags, width, precision, end).
fn parse_one_printf_spec(
    bytes: &[u8],
    pos: usize,
) -> (
    String,
    usize,
    Option<String>,
    String,
    String,
    String,
    String,
    usize,
) {
    let start = pos;
    let n = bytes.len();
    let mut idx = pos + 1;

    // Optional key: (\(([^)]*)\))?
    // If '(' found, scan for ')'. If found, extract key. If not, skip group.
    let key = if idx < n && bytes[idx] == b'(' {
        let key_start = idx + 1;
        let mut key_end = None;
        for i in key_start..n {
            if bytes[i] == b')' {
                key_end = Some(i);
                break;
            }
        }
        if let Some(end) = key_end {
            let k = String::from_utf8_lossy(&bytes[key_start..end]).into_owned();
            idx = end + 1;
            Some(k)
        } else {
            None
        }
    } else {
        None
    };

    // Optional flags: [#0\-+ ]* (greedy)
    let flags_start = idx;
    while idx < n && matches!(bytes[idx], b'#' | b'0' | b'-' | b'+' | b' ') {
        idx += 1;
    }
    let flags = String::from_utf8_lossy(&bytes[flags_start..idx]).into_owned();

    // Optional width: [1-9][0-9]* | *
    let width = if idx < n && bytes[idx] == b'*' {
        idx += 1;
        "*".to_string()
    } else if idx < n && (b'1'..=b'9').contains(&bytes[idx]) {
        let w_start = idx;
        idx += 1;
        while idx < n && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        String::from_utf8_lossy(&bytes[w_start..idx]).into_owned()
    } else {
        String::new()
    };

    // Optional precision: (\.(\*|[0-9]+))?
    // Python precision captures only `\*|[0-9]+`, so a bare `.` yields "".
    let precision = if idx < n && bytes[idx] == b'.' {
        idx += 1;
        if idx < n && bytes[idx] == b'*' {
            idx += 1;
            "*".to_string()
        } else if idx < n && bytes[idx].is_ascii_digit() {
            let d_start = idx;
            while idx < n && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            String::from_utf8_lossy(&bytes[d_start..idx]).into_owned()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Optional length modifier: [hlL]?
    if idx < n && matches!(bytes[idx], b'h' | b'l' | b'L') {
        idx += 1;
    }

    // Optional type: .? (any single char)
    let conv_type = if idx < n {
        let ch = char::from(bytes[idx]);
        idx += 1;
        ch.to_string()
    } else {
        String::new()
    };

    let whole_seq = String::from_utf8_lossy(&bytes[start..idx]).into_owned();
    (
        whole_seq, start, key, conv_type, flags, width, precision, idx,
    )
}

/// Parse a printf-style format string into conversion specifiers.
///
/// Returns a list of tuples:
/// (whole_seq, start_pos, key, conv_type, flags, width, precision)
///
/// `key` is `None` when no mapping key was present.
#[pyfunction]
pub fn rust_parse_conversion_specifiers(
    format_str: &str,
) -> Vec<(
    String,
    usize,
    Option<String>,
    String,
    String,
    String,
    String,
)> {
    let bytes = format_str.as_bytes();
    let n = bytes.len();
    let mut result = Vec::new();
    let mut pos = 0;
    while pos < n {
        if bytes[pos] == b'%' {
            let (whole, sp, key, ct, fl, w, pr, end) = parse_one_printf_spec(bytes, pos);
            result.push((whole, sp, key, ct, fl, w, pr));
            pos = end;
        } else {
            pos += 1;
        }
    }
    result
}

// ===== str.format() brace-matching =====

// Error codes matching the Python error messages.
const ERR_UNEXPECTED_CLOSE: i32 = 1;
const ERR_UNMATCHED_OPEN: i32 = 2;
const ERR_INVALID_SPECIFIER: i32 = 3;
const ERR_KEY_HAS_BRACE: i32 = 4;
const ERR_NESTING_TOO_DEEP: i32 = 5;

/// Find non-escaped format targets in a str.format() format string.
///
/// Returns `(error_code, targets)` where error_code is 0 on success.
/// `targets` is a list of `(target_string, start_pos)` tuples.
/// On error, `targets` is empty.
#[pyfunction]
pub fn rust_find_non_escaped_targets(format_value: &str) -> (i32, Vec<(String, usize)>) {
    let bytes = format_value.as_bytes();
    let n = bytes.len();
    let mut result: Vec<(String, usize)> = Vec::new();
    let mut next_spec = String::new();
    let mut pos = 0;
    let mut nesting = 0i32;

    while pos < n {
        let c = bytes[pos];
        if nesting == 0 {
            if c == b'{' {
                if pos < n - 1 && bytes[pos + 1] == b'{' {
                    pos += 1;
                } else {
                    nesting = 1;
                }
            }
            if c == b'}' {
                if pos < n - 1 && bytes[pos + 1] == b'}' {
                    pos += 1;
                } else {
                    return (ERR_UNEXPECTED_CLOSE, Vec::new());
                }
            }
        } else {
            if c == b'{' {
                nesting += 1;
            }
            if c == b'}' {
                nesting -= 1;
            }
            if nesting > 0 {
                next_spec.push(c as char);
            } else {
                let start = pos - next_spec.len();
                result.push((next_spec.clone(), start));
                next_spec.clear();
            }
        }
        pos += 1;
    }
    if nesting > 0 {
        return (ERR_UNMATCHED_OPEN, Vec::new());
    }
    (0, result)
}

// ===== str.format() specifier parsing =====
//
// Mirrors FORMAT_RE_NEW (built-in types) and FORMAT_RE_NEW_CUSTOM.
// Both use fullmatch. If FORMAT_RE_NEW doesn't match, try CUSTOM.

struct NewFormatSpec {
    key: Option<String>,
    conv_type: String,
    flags: String,
    width: String,
    precision: String,
    format_spec: Option<String>,
    non_standard_format_spec: bool,
    conversion: Option<String>,
    field: Option<String>,
}

/// Parse the field portion: key = [^.[!:]*, rest = [^:!]+?
/// Returns (key_string, field_string, new_idx).
fn parse_field(bytes: &[u8], mut idx: usize, n: usize) -> (String, Option<String>, usize) {
    let key_start = idx;
    while idx < n && !matches!(bytes[idx], b'.' | b'[' | b'!' | b':') {
        idx += 1;
    }
    let key_str = String::from_utf8_lossy(&bytes[key_start..idx]).into_owned();

    let rest_start = idx;
    while idx < n && !matches!(bytes[idx], b':' | b'!') {
        idx += 1;
    }
    let field = if idx > rest_start {
        Some(format!(
            "{}{}",
            key_str,
            String::from_utf8_lossy(&bytes[rest_start..idx])
        ))
    } else {
        Some(key_str.clone())
    };
    (key_str, field, idx)
}

/// Parse conversion: ![^:]? (optional '!' + one non-':' char).
fn parse_conversion(bytes: &[u8], mut idx: usize, n: usize) -> (Option<String>, usize) {
    if idx < n && bytes[idx] == b'!' {
        if idx + 1 < n && bytes[idx + 1] != b':' {
            let ch = char::from(bytes[idx + 1]);
            idx += 2;
            (Some(format!("!{}", ch)), idx)
        } else {
            (None, idx)
        }
    } else {
        (None, idx)
    }
}

/// Try FORMAT_RE_NEW (built-in format spec) via fullmatch.
fn match_new_format_builtin(target: &str) -> Option<NewFormatSpec> {
    let bytes = target.as_bytes();
    let n = bytes.len();
    let mut idx = 0;

    let (key_str, field, after_field) = parse_field(bytes, idx, n);
    idx = after_field;

    let (conversion, after_conv) = parse_conversion(bytes, idx, n);
    idx = after_conv;

    if idx < n && bytes[idx] == b':' {
        let spec_start = idx;
        idx += 1;

        // fill_align: .?[<>=^]? (optional, greedy: tries 2 chars then 1)
        if idx + 1 < n && matches!(bytes[idx + 1], b'<' | b'>' | b'=' | b'^') {
            idx += 2;
        } else if idx < n && matches!(bytes[idx], b'<' | b'>' | b'=' | b'^') {
            idx += 1;
        }

        // flags: [+\- ]?#?0? (optional)
        let flags_start = idx;
        if idx < n && matches!(bytes[idx], b'+' | b'-' | b' ') {
            idx += 1;
        }
        if idx < n && bytes[idx] == b'#' {
            idx += 1;
        }
        if idx < n && bytes[idx] == b'0' {
            idx += 1;
        }
        let flags = String::from_utf8_lossy(&bytes[flags_start..idx]).into_owned();

        // width: \d+? (optional)
        let width_start = idx;
        while idx < n && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        let width = String::from_utf8_lossy(&bytes[width_start..idx]).into_owned();

        // [_,]? (optional grouping)
        if idx < n && matches!(bytes[idx], b'_' | b',') {
            idx += 1;
        }

        // precision: \.\d+? (optional)
        let precision = if idx < n && bytes[idx] == b'.' {
            let p_start = idx;
            idx += 1;
            while idx < n && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            String::from_utf8_lossy(&bytes[p_start..idx]).into_owned()
        } else {
            String::new()
        };

        // type: .? (optional, any single char)
        let conv_type = if idx < n {
            let ch = char::from(bytes[idx]);
            idx += 1;
            ch.to_string()
        } else {
            String::new()
        };

        if idx != n {
            return None;
        }

        let format_spec = String::from_utf8_lossy(&bytes[spec_start..n]).into_owned();
        Some(NewFormatSpec {
            key: Some(key_str),
            conv_type,
            flags,
            width,
            precision,
            format_spec: Some(format_spec),
            non_standard_format_spec: false,
            conversion,
            field,
        })
    } else {
        if idx != n {
            return None;
        }
        Some(NewFormatSpec {
            key: Some(key_str),
            conv_type: String::new(),
            flags: String::new(),
            width: String::new(),
            precision: String::new(),
            format_spec: None,
            non_standard_format_spec: false,
            conversion,
            field,
        })
    }
}

/// Try FORMAT_RE_NEW_CUSTOM via fullmatch: field + conversion + :.*
fn match_new_format_custom(target: &str) -> Option<NewFormatSpec> {
    let bytes = target.as_bytes();
    let n = bytes.len();
    let mut idx = 0;

    let (key_str, field, after_field) = parse_field(bytes, idx, n);
    idx = after_field;

    let (conversion, after_conv) = parse_conversion(bytes, idx, n);
    idx = after_conv;

    let format_spec = if idx < n && bytes[idx] == b':' {
        let fs = String::from_utf8_lossy(&bytes[idx..n]).into_owned();
        idx = n;
        Some(fs)
    } else {
        None
    };

    if idx != n {
        return None;
    }

    Some(NewFormatSpec {
        key: Some(key_str),
        conv_type: String::new(),
        flags: String::new(),
        width: String::new(),
        precision: String::new(),
        format_spec,
        non_standard_format_spec: true,
        conversion,
        field,
    })
}

/// Full specifier tuple type returned to Python.
type SpecTuple = (
    String,
    usize,
    Option<String>,
    String,
    String,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
);

/// Convert a NewFormatSpec + target + start_pos into a SpecTuple.
fn spec_to_tuple(spec: &NewFormatSpec, target: &str, start_pos: usize) -> SpecTuple {
    (
        target.to_string(),
        start_pos,
        spec.key.clone(),
        spec.conv_type.clone(),
        spec.flags.clone(),
        spec.width.clone(),
        spec.precision.clone(),
        spec.format_spec.clone(),
        spec.non_standard_format_spec,
        spec.conversion.clone(),
        spec.field.clone(),
    )
}

/// Parse a str.format() format string into conversion specifiers (top level).
///
/// Returns `(error_code, specs)`. Error codes:
/// 0=ok, 1=unexpected }, 2=unmatched {, 3=invalid specifier,
/// 4=key has brace, 5=nesting too deep.
#[pyfunction]
pub fn rust_parse_format_value(format_value: &str) -> (i32, Vec<SpecTuple>) {
    parse_format_value_inner(format_value, 0)
}

fn parse_format_value_inner(format_value: &str, depth: u32) -> (i32, Vec<SpecTuple>) {
    let (err, targets) = rust_find_non_escaped_targets(format_value);
    if err != 0 {
        return (err, Vec::new());
    }

    let mut result: Vec<SpecTuple> = Vec::new();
    for (target, start_pos) in targets {
        let spec = match_new_format_builtin(&target).or_else(|| match_new_format_custom(&target));

        let spec = match spec {
            Some(s) => s,
            None => return (ERR_INVALID_SPECIFIER, Vec::new()),
        };

        if let Some(ref k) = spec.key {
            if k.contains('{') || k.contains('}') {
                return (ERR_KEY_HAS_BRACE, Vec::new());
            }
        }

        result.push(spec_to_tuple(&spec, &target, start_pos));

        if let Some(ref fs) = spec.format_spec {
            if spec.non_standard_format_spec && (fs.contains('{') || fs.contains('}')) {
                if depth >= 1 {
                    return (ERR_NESTING_TOO_DEEP, Vec::new());
                }
                let (sub_err, sub_specs) = parse_format_value_inner(fs, depth + 1);
                if sub_err != 0 {
                    return (sub_err, Vec::new());
                }
                result.extend(sub_specs);
            }
        }
    }
    (0, result)
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

    #[test]
    fn test_parse_simple_printf() {
        let specs = rust_parse_conversion_specifiers("%s");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].3, "s");
        assert_eq!(specs[0].2, None);
    }

    #[test]
    fn test_parse_printf_with_key() {
        let specs = rust_parse_conversion_specifiers("%(name)s");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].2, Some("name".to_string()));
        assert_eq!(specs[0].3, "s");
    }

    #[test]
    fn test_parse_printf_multiple() {
        let specs = rust_parse_conversion_specifiers("%d %s %f");
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].3, "d");
        assert_eq!(specs[1].3, "s");
        assert_eq!(specs[2].3, "f");
    }

    #[test]
    fn test_parse_printf_width_star() {
        let specs = rust_parse_conversion_specifiers("%*d");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].5, "*");
    }

    #[test]
    fn test_parse_printf_percent_literal() {
        let specs = rust_parse_conversion_specifiers("100%%");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].3, "%");
    }

    #[test]
    fn test_find_targets_simple() {
        let (err, targets) = rust_find_non_escaped_targets("{}");
        assert_eq!(err, 0);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "");
    }

    #[test]
    fn test_find_targets_named() {
        let (err, targets) = rust_find_non_escaped_targets("{name}");
        assert_eq!(err, 0);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "name");
    }

    #[test]
    fn test_find_targets_escaped() {
        let (err, targets) = rust_find_non_escaped_targets("{{}}");
        assert_eq!(err, 0);
        assert_eq!(targets.len(), 0);
    }

    #[test]
    fn test_find_targets_unexpected_close() {
        let (err, _) = rust_find_non_escaped_targets("}");
        assert_eq!(err, ERR_UNEXPECTED_CLOSE);
    }

    #[test]
    fn test_find_targets_unmatched_open() {
        let (err, _) = rust_find_non_escaped_targets("{");
        assert_eq!(err, ERR_UNMATCHED_OPEN);
    }

    #[test]
    fn test_parse_format_value_simple() {
        let (err, specs) = rust_parse_format_value("{}");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 1);
    }

    #[test]
    fn test_parse_format_value_with_spec() {
        let (err, specs) = rust_parse_format_value("{:s}");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].3, "s");
    }

    #[test]
    fn test_parse_format_value_named() {
        let (err, specs) = rust_parse_format_value("{name:d}");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].2, Some("name".to_string()));
        assert_eq!(specs[0].3, "d");
    }

    #[test]
    fn test_parse_format_value_conversion() {
        let (err, specs) = rust_parse_format_value("{!r}");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].9, Some("!r".to_string()));
    }
}
