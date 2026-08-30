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
// Mirrors FORMAT_RE. All groups after % are optional and greedy;
// re.finditer matches every '%', extending as far as the groups allow.

/// A parsed printf-style specifier returned to Python.
///
/// Fields: (whole_seq, start_pos, key, conv_type, flags, width, precision).
type PrintfSpecTuple = (
    String,
    usize,
    Option<String>,
    String,
    String,
    String,
    String,
);

/// Parse one printf specifier starting at `pos` (bytes[pos] == '%').
/// Returns (whole_seq, start_pos, key, conv_type, flags, width, precision, end).
/// `start_pos` is a char offset (Python parity); `end` is a byte offset.
fn parse_one_printf_spec(
    format_str: &str,
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
        for (i, &c) in bytes.iter().enumerate().take(n).skip(key_start) {
            if c == b')' {
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

    // Optional type: .? (any single char, possibly multi-byte)
    let conv_type = if idx < n {
        let c = format_str[idx..].chars().next().unwrap();
        idx += c.len_utf8();
        c.to_string()
    } else {
        String::new()
    };

    let whole_seq = format_str[start..idx].to_string();
    let start_char = format_str[..start].chars().count();
    (
        whole_seq, start_char, key, conv_type, flags, width, precision, idx,
    )
}

/// Parse a printf-style format string into conversion specifiers.
///
/// Returns a list of tuples:
/// (whole_seq, start_pos, key, conv_type, flags, width, precision)
///
/// `key` is `None` when no mapping key was present.
#[pyfunction]
pub fn rust_parse_conversion_specifiers(format_str: &str) -> Vec<PrintfSpecTuple> {
    let bytes = format_str.as_bytes();
    let n = bytes.len();
    let mut result = Vec::new();
    let mut pos = 0;
    while pos < n {
        if bytes[pos] == b'%' {
            let (whole, sp, key, ct, fl, w, pr, end) =
                parse_one_printf_spec(format_str, bytes, pos);
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
/// Returns `(error_code, targets)`: `(0, [(target, start_pos), ...])` on
/// success, `(code, [])` on error; positions are char offsets (Python parity).
#[pyfunction]
pub fn rust_find_non_escaped_targets(format_value: &str) -> (i32, Vec<(String, usize)>) {
    // Walks characters, never bytes: the Python regex `.` matches any char
    // (including multi-byte fill chars), and consumers index char offsets.
    let chars: Vec<char> = format_value.chars().collect();
    let n = chars.len();
    let mut result: Vec<(String, usize)> = Vec::new();
    let mut next_spec = String::new();
    let mut pos = 0;
    let mut nesting = 0i32;

    while pos < n {
        let c = chars[pos];
        if nesting == 0 {
            if c == '{' {
                if pos < n - 1 && chars[pos + 1] == '{' {
                    pos += 1;
                } else {
                    nesting = 1;
                }
            }
            if c == '}' {
                if pos < n - 1 && chars[pos + 1] == '}' {
                    pos += 1;
                } else {
                    return (ERR_UNEXPECTED_CLOSE, Vec::new());
                }
            }
        } else {
            if c == '{' {
                nesting += 1;
            }
            if c == '}' {
                nesting -= 1;
            }
            if nesting > 0 {
                next_spec.push(c);
            } else {
                let start = pos - next_spec.chars().count();
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
// Mirrors FORMAT_RE_NEW (built-in types) and FORMAT_RE_NEW_CUSTOM, both
// fullmatch. If FORMAT_RE_NEW doesn't match, try CUSTOM.

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
/// The conversion char may be multi-byte; idx advances by its full length.
fn parse_conversion(
    target: &str,
    bytes: &[u8],
    mut idx: usize,
    n: usize,
) -> (Option<String>, usize) {
    if idx < n && bytes[idx] == b'!' {
        if idx + 1 < n && bytes[idx + 1] != b':' {
            let c = target[idx + 1..].chars().next().unwrap();
            idx += 1 + c.len_utf8();
            (Some(format!("!{}", c)), idx)
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

    let (conversion, after_conv) = parse_conversion(target, bytes, idx, n);
    idx = after_conv;

    if idx < n && bytes[idx] == b':' {
        let spec_start = idx;
        idx += 1;

        // fill_align: .?[<>=^]? (optional, greedy: tries 2 chars then 1).
        // The regex `.` matches any char, so the fill may be multi-byte;
        // advance by whole chars (utf-8 parity with the Python regex).
        {
            let ci = idx;
            let first = target[ci..].chars().next();
            if let Some(f) = first {
                let flen = f.len_utf8();
                let second = target[ci + flen..].chars().next();
                if second.is_some_and(|c| matches!(c, '<' | '>' | '=' | '^')) {
                    idx = ci + flen + second.unwrap().len_utf8();
                } else if matches!(f, '<' | '>' | '=' | '^') {
                    idx = ci + flen;
                }
            }
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

        // type: .? (optional, single char, possibly multi-byte)
        let conv_type = if idx < n {
            let c = target[idx..].chars().next().unwrap();
            idx += c.len_utf8();
            c.to_string()
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

    let (conversion, after_conv) = parse_conversion(target, bytes, idx, n);
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

// ===== Placeholder format spec taxonomy =====
// Parses the format_spec (the part after ':' in a str.format() placeholder)
// into its individual components, matching Python's FORMAT_RE_NEW structure.

// Returns Option<(fill, align, sign, alt, zero_pad, width, grouping, precision,
// conv_type)>.

/// Parsed placeholder format spec tuple returned to Python.
/// Fields: (fill, align, sign, alternate, zero_pad, width, grouping, precision,
/// conv_type)
type PlaceholderSpecTuple = (
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    bool,
    String,
    Option<String>,
    String,
    String,
);

/// Parse a format placeholder spec string (the part after `:`) into its
/// individual components. This mirrors the num_spec portion of FORMAT_RE_NEW.
///
/// Returns None if the spec does not match the built-in format grammar.
/// On success, returns the decomposed fields:
/// - fill: optional single char before align
/// - align: one of < > = ^ or None
/// - sign: one of + - space or None
/// - alternate: # present
/// - zero_pad: 0 present after # (if any)
/// - width: digit string or ""
/// - grouping: _ or , or None
/// - precision: ".NNN" or ""
/// - conv_type: single char or ""
#[pyfunction]
pub fn rust_parse_placeholder_format(format_spec: &str) -> Option<PlaceholderSpecTuple> {
    parse_placeholder_format_inner(format_spec)
}

fn parse_placeholder_format_inner(spec: &str) -> Option<PlaceholderSpecTuple> {
    // The input is the format_spec WITHOUT the leading colon, i.e. the part
    // after ':' in the placeholder. This is what FORMAT_RE_NEW's format_spec
    // group captures (minus the leading ':').

    // The Python regex matches *characters* (`.` matches any char, including
    // multi-byte fill chars), so this walks chars, never raw bytes (utf-8
    // parity invariant).
    let chars: Vec<char> = spec.chars().collect();
    let n = chars.len();
    let is_align = |c: char| matches!(c, '<' | '>' | '=' | '^');
    let mut idx = 0;

    // fill_align: .?[<>=^]? (greedy: tries 2 chars then 1)
    let (fill, align) = if idx + 1 < n && is_align(chars[idx + 1]) {
        let f = Some(chars[idx].to_string());
        let a = Some(chars[idx + 1].to_string());
        idx += 2;
        (f, a)
    } else if idx < n && is_align(chars[idx]) {
        let a = Some(chars[idx].to_string());
        idx += 1;
        (None, a)
    } else {
        (None, None)
    };

    // sign: [+\- ]?
    let sign = if idx < n && matches!(chars[idx], '+' | '-' | ' ') {
        let s = Some(chars[idx].to_string());
        idx += 1;
        s
    } else {
        None
    };

    // alternate: #?
    let alternate = if idx < n && chars[idx] == '#' {
        idx += 1;
        true
    } else {
        false
    };

    // zero_pad: 0?
    let zero_pad = if idx < n && chars[idx] == '0' {
        idx += 1;
        true
    } else {
        false
    };

    // width: \d+? (greedy, optional)
    let width_start = idx;
    while idx < n && chars[idx].is_ascii_digit() {
        idx += 1;
    }
    let width: String = chars[width_start..idx].iter().collect();

    // grouping: [_,]? (optional)
    let grouping = if idx < n && matches!(chars[idx], '_' | ',') {
        let g = Some(chars[idx].to_string());
        idx += 1;
        g
    } else {
        None
    };

    // precision: \.\d+? (optional, includes the dot)
    let precision = if idx < n && chars[idx] == '.' {
        let p_start = idx;
        idx += 1;
        while idx < n && chars[idx].is_ascii_digit() {
            idx += 1;
        }
        chars[p_start..idx].iter().collect()
    } else {
        String::new()
    };

    // conv_type: .? (optional, single char, possibly multi-byte)
    let conv_type = if idx < n {
        let c = chars[idx];
        idx += 1;
        c.to_string()
    } else {
        String::new()
    };

    // Must consume all input for a full match.
    if idx != n {
        return None;
    }

    Some((
        fill, align, sign, alternate, zero_pad, width, grouping, precision, conv_type,
    ))
}

// ===== Conversion specifier analysis =====
// Mirrors analyze_conversion_specifiers: classifies a list of specifiers into
// whether they have mapping keys, star widths/precisions, etc. Pure analysis,

// no mutation or error reporting — Python handles those.

/// Spec info needed for analysis: (key_present, conv_type, width, precision).
/// key_present is True if key is Some and not empty.
type SpecInfo = (bool, String, String, String);

/// Analysis result: (has_star, has_key, all_have_keys)
/// Returns None on error (has_key and has_star, or has_key and not
/// all_have_keys). Python maps None to the appropriate error message.
#[pyfunction]
pub fn rust_analyze_conversion_specifiers(specs: Vec<SpecInfo>) -> Option<(bool, bool, bool)> {
    let has_star = specs.iter().any(|(_, _, w, p)| w == "*" || p == "*");
    let has_key = specs.iter().any(|(kp, _, _, _)| *kp);
    let all_have_keys = specs.iter().all(|(kp, ct, _, _)| *kp || ct == "%");

    if has_key && has_star {
        return None;
    }
    if has_key && !all_have_keys {
        return None;
    }
    Some((has_star, has_key, all_have_keys))
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

    // multibyte format strings: positions must be char offsets and target
    // contents must survive intact (mypy-rs issue #1248).
    #[test]
    fn test_find_targets_multibyte_char_offsets() {
        let (err, targets) = rust_find_non_escaped_targets("ā{}");
        assert_eq!(err, 0);
        assert_eq!(targets, vec![("".to_string(), 2)]);
    }

    #[test]
    fn test_find_targets_multibyte_target_content() {
        let (err, targets) = rust_find_non_escaped_targets("x{ā}");
        assert_eq!(err, 0);
        assert_eq!(targets, vec![("ā".to_string(), 2)]);
    }

    #[test]
    fn test_parse_format_value_multibyte() {
        let (err, specs) = rust_parse_format_value("ā{}, {b}😀");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].1, 2);
        assert_eq!(specs[1].1, 6);
    }

    #[test]
    fn test_parse_format_value_multibyte_key() {
        let (err, specs) = rust_parse_format_value("ā{名:x}");
        assert_eq!(err, 0);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].0, "名:x");
        assert_eq!(specs[0].2, Some("名".to_string()));
        assert_eq!(specs[0].1, 2);
    }

    #[test]
    fn test_parse_printf_multibyte_prefix() {
        let specs = rust_parse_conversion_specifiers("ā{}%s");
        assert_eq!(specs[0].1, 3); // char offset of '%'
    }

    #[test]
    fn test_parse_printf_multibyte_conv_type() {
        let specs = rust_parse_conversion_specifiers("%ā");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].3, "ā");
        assert_eq!(specs[0].0, "%ā");
    }

    #[test]
    fn test_placeholder_multibyte_fill() {
        let spec = rust_parse_placeholder_format("ā<10d");
        assert!(spec.is_some());
        let (fill, align, _, _, _, width, _, _, conv_type) = spec.unwrap();
        assert_eq!(fill, Some("ā".to_string()));
        assert_eq!(align, Some("<".to_string()));
        assert_eq!(width, "10");
        assert_eq!(conv_type, "d");
    }

    #[test]
    fn test_placeholder_multibyte_conv_type() {
        let spec = rust_parse_placeholder_format("10.3ā");
        assert!(spec.is_some());
        let (_, _, _, _, _, _, _, _, conv_type) = spec.unwrap();
        assert_eq!(conv_type, "ā");
    }

    #[test]
    fn test_conversion_multibyte() {
        let (err, specs) = rust_parse_format_value("{!名}");
        assert_eq!(err, 0);
        assert_eq!(specs[0].9, Some("!名".to_string()));
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

    #[test]
    fn test_parse_placeholder_simple() {
        let spec = rust_parse_placeholder_format("");
        assert!(spec.is_some());
        let (_, _, _, _, _, width, _, _, conv_type) = spec.unwrap();
        assert_eq!(width, "");
        assert_eq!(conv_type, "");
    }

    #[test]
    fn test_parse_placeholder_type_only() {
        let spec = rust_parse_placeholder_format("d");
        assert!(spec.is_some());
        let (_, _, _, _, _, _, _, _, conv_type) = spec.unwrap();
        assert_eq!(conv_type, "d");
    }

    #[test]
    fn test_parse_placeholder_width_and_type() {
        let spec = rust_parse_placeholder_format("10d");
        assert!(spec.is_some());
        let (_, _, _, _, _, width, _, _, conv_type) = spec.unwrap();
        assert_eq!(width, "10");
        assert_eq!(conv_type, "d");
    }

    #[test]
    fn test_parse_placeholder_align() {
        let spec = rust_parse_placeholder_format("<10s");
        assert!(spec.is_some());
        let (fill, align, _, _, _, width, _, _, conv_type) = spec.unwrap();
        assert_eq!(fill, None);
        assert_eq!(align, Some("<".to_string()));
        assert_eq!(width, "10");
        assert_eq!(conv_type, "s");
    }

    #[test]
    fn test_parse_placeholder_fill_and_align() {
        let spec = rust_parse_placeholder_format("x<10s");
        assert!(spec.is_some());
        let (fill, align, _, _, _, _, _, _, _) = spec.unwrap();
        assert_eq!(fill, Some("x".to_string()));
        assert_eq!(align, Some("<".to_string()));
    }

    #[test]
    fn test_parse_placeholder_sign_and_alt() {
        let spec = rust_parse_placeholder_format("+#010x");
        assert!(spec.is_some());
        let (_, _, sign, alt, zero, width, _, _, conv_type) = spec.unwrap();
        assert_eq!(sign, Some("+".to_string()));
        assert!(alt);
        assert!(zero);
        assert_eq!(width, "10");
        assert_eq!(conv_type, "x");
    }

    #[test]
    fn test_parse_placeholder_precision() {
        let spec = rust_parse_placeholder_format(".2f");
        assert!(spec.is_some());
        let (_, _, _, _, _, _, _, precision, conv_type) = spec.unwrap();
        assert_eq!(precision, ".2");
        assert_eq!(conv_type, "f");
    }

    #[test]
    fn test_parse_placeholder_grouping() {
        let spec = rust_parse_placeholder_format(",d");
        assert!(spec.is_some());
        let (_, _, _, _, _, _, grouping, _, conv_type) = spec.unwrap();
        assert_eq!(grouping, Some(",".to_string()));
        assert_eq!(conv_type, "d");
    }

    #[test]
    fn test_parse_placeholder_full() {
        // "x<+#012,.3f" — fill=x, align=<, sign=+, alt=#, zero=0, width=12,
        // grouping=,, precision=.3, type=f
        let spec = rust_parse_placeholder_format("x<+#012,.3f");
        assert!(spec.is_some());
        let (fill, align, sign, alt, zero, width, grouping, precision, conv_type) = spec.unwrap();
        assert_eq!(fill, Some("x".to_string()));
        assert_eq!(align, Some("<".to_string()));
        assert_eq!(sign, Some("+".to_string()));
        assert!(alt);
        assert!(zero);
        assert_eq!(width, "12");
        assert_eq!(grouping, Some(",".to_string()));
        assert_eq!(precision, ".3");
        assert_eq!(conv_type, "f");
    }

    #[test]
    fn test_parse_placeholder_invalid() {
        // After consuming the type char, extra characters = no fullmatch
        let spec = rust_parse_placeholder_format("d extra");
        assert!(spec.is_none());
    }

    #[test]
    fn test_analyze_specs_simple() {
        let specs = vec![(false, "s".to_string(), "".to_string(), "".to_string())];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, Some((false, false, false)));
    }

    #[test]
    fn test_analyze_specs_with_keys() {
        let specs = vec![
            (true, "s".to_string(), "".to_string(), "".to_string()),
            (true, "d".to_string(), "".to_string(), "".to_string()),
        ];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, Some((false, true, true)));
    }

    #[test]
    fn test_analyze_specs_mixed_keys_error() {
        let specs = vec![
            (true, "s".to_string(), "".to_string(), "".to_string()),
            (false, "d".to_string(), "".to_string(), "".to_string()),
        ];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_analyze_specs_key_and_star_error() {
        let specs = vec![(true, "s".to_string(), "*".to_string(), "".to_string())];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_analyze_specs_star_only() {
        let specs = vec![(false, "d".to_string(), "*".to_string(), "".to_string())];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, Some((true, false, false)));
    }

    #[test]
    fn test_analyze_specs_percent_excluded_from_key_check() {
        // %% specifier has conv_type "%" and no key, but should not fail
        // the all_have_keys check when other specs have keys.
        let specs = vec![
            (true, "s".to_string(), "".to_string(), "".to_string()),
            (false, "%".to_string(), "".to_string(), "".to_string()),
        ];
        let result = rust_analyze_conversion_specifiers(specs);
        assert_eq!(result, Some((false, true, true)));
    }
}
