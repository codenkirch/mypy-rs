//! Native ports of pure utility functions from `mypy/util.py`.
//!
//! Each function mirrors a pure Python helper that takes/returns plain
//! Python objects (str, int, bytes, list). No live `Type` graph dependency.
//! Functions that need OS/path interaction delegate to Python's `os.path`
//! via PyO3 calls, preserving exact semantics.

#![allow(non_local_definitions)]

use std::collections::HashMap;
use std::collections::HashSet;

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyTuple;

create_exception!(type_kernel, DecodeError, PyValueError);

// ---------------------------------------------------------------------------
// Constants (mirror mypy/util.py)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
const DEFAULT_SOURCE_OFFSET: i64 = 4;
#[allow(dead_code)]
const MINIMUM_WIDTH: i64 = 20;

const SPECIAL_DUNDERS: &[&str] = &[
    "__init__",
    "__new__",
    "__call__",
    "__init_subclass__",
    "__class_getitem__",
];

// ---------------------------------------------------------------------------
// is_dunder / is_sunder
// ---------------------------------------------------------------------------

/// `mypy/util.py:is_dunder` — whether name is a dunder name.
#[pyfunction(signature = (name, exclude_special=false))]
pub fn rust_is_dunder(name: &str, exclude_special: bool) -> bool {
    if exclude_special && SPECIAL_DUNDERS.contains(&name) {
        return false;
    }
    name.starts_with("__") && name.ends_with("__")
}

/// `mypy/util.py:is_sunder` — whether name is a sunder name.
#[pyfunction]
pub fn rust_is_sunder(name: &str) -> bool {
    !rust_is_dunder(name, false) && name.starts_with('_') && name.ends_with('_') && name != "_"
}

// ---------------------------------------------------------------------------
// split_module_names / module_prefix / split_target
// ---------------------------------------------------------------------------

/// `mypy/util.py:split_module_names` — module + all parent module names.
#[pyfunction]
pub fn rust_split_module_names(mod_name: &str) -> Vec<String> {
    let mut out = vec![mod_name.to_string()];
    let mut current = mod_name;
    while let Some(pos) = current.rfind('.') {
        current = &current[..pos];
        out.push(current.to_string());
    }
    out
}

/// `mypy/util.py:module_prefix` — prefix of target in modules, or None.
#[pyfunction]
pub fn rust_module_prefix(modules: Vec<String>, target: &str) -> Option<String> {
    rust_split_target_inner(&modules, target).map(|(prefix, _)| prefix)
}

/// `mypy/util.py:split_target` — split target at module boundary.
#[pyfunction]
pub fn rust_split_target(modules: Vec<String>, target: &str) -> Option<(String, String)> {
    rust_split_target_inner(&modules, target)
}

fn rust_split_target_inner(modules: &[String], target: &str) -> Option<(String, String)> {
    let module_set: HashSet<&str> = modules.iter().map(|s| s.as_str()).collect();
    let mut remaining: Vec<&str> = Vec::new();
    let mut target = target;
    loop {
        if module_set.contains(target) {
            return Some((target.to_string(), remaining.join(".")));
        }
        let pos = target.rfind('.')?;
        remaining.insert(0, &target[pos + 1..]);
        target = &target[..pos];
    }
}

// ---------------------------------------------------------------------------
// short_type
// ---------------------------------------------------------------------------

/// `mypy/util.py:short_type` — last component of type name of obj.
///
/// Python: `t = str(type(obj)); return t.split(".")[-1].rstrip("'>")`.
/// `str(type(obj))` yields `<class 'module.TypeName'>`. We replicate
/// by calling Python's `type()` and `str()` for exact fidelity.
#[pyfunction(signature = (obj=None))]
pub fn rust_short_type(py: Python<'_>, obj: Option<&PyAny>) -> PyResult<String> {
    match obj {
        None => Ok("nil".to_string()),
        Some(o) => {
            let builtins = py.import("builtins")?;
            let typ = builtins.getattr("type")?.call1((o,))?;
            let type_str: String = typ.repr().map(|r| r.to_string()).unwrap_or_default();
            let last = type_str.rsplit('.').next().unwrap_or("");
            Ok(last.trim_end_matches("'>").to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// find_python_encoding
// ---------------------------------------------------------------------------

/// `mypy/util.py:find_python_encoding` — PEP-263 encoding detection.
#[pyfunction]
pub fn rust_find_python_encoding(text: &[u8]) -> (String, i64) {
    match find_encoding_re(text) {
        Some((encoding, has_group1)) => {
            let mut enc = encoding;
            if enc.starts_with("iso-latin-1-")
                || enc.starts_with("latin-1-")
                || enc == "iso-latin-1"
            {
                enc = "latin-1".to_string();
            }
            let line = if has_group1 { 2 } else { 1 };
            (enc, line)
        }
        None => ("utf8".to_string(), -1),
    }
}

/// Mirrors the Python regex:
/// `([ \t\v]*#.*(\r\n?|\n))??[ \t\v]*#.*coding[:=][ \t]*([-\w.]+)`
fn find_encoding_re(text: &[u8]) -> Option<(String, bool)> {
    // Find the encoding declaration (PEP 263). Matches an optional
    // first comment line, then a line with `coding[:=]` directive.
    let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return None;
    }

    // Check first line for coding declaration.
    if let Some((enc, _)) = check_line_for_coding(lines[0]) {
        return Some((enc, false));
    }
    // Check second line if first line is a comment.
    if lines.len() > 1 {
        // The first line must be a comment line for the second line
        // to be checked (the regex's optional group captures a full
        // first comment line including its newline).
        if is_comment_line(lines[0]) {
            if let Some((enc, _)) = check_line_for_coding(lines[1]) {
                return Some((enc, true));
            }
        }
    }
    None
}

fn is_comment_line(line: &[u8]) -> bool {
    let mut i = 0;
    while i < line.len() {
        match line[i] {
            b' ' | b'\t' | b'\x0b' => i += 1,
            b'#' => return true,
            _ => return false,
        }
    }
    false
}

fn check_line_for_coding(line: &[u8]) -> Option<(String, ())> {
    // Find "coding" followed by ":" or "=" then the encoding name.
    // The regex: [ \t\v]*#.*coding[:=][ \t]*([-\w.]+)
    // First find the '#' that starts the comment.
    let mut i = 0;
    while i < line.len() {
        match line[i] {
            b' ' | b'\t' | b'\x0b' => i += 1,
            b'#' => break,
            _ => return None,
        }
    }
    if i >= line.len() {
        return None;
    }
    // i is at '#'. Search for "coding" in the rest of the line.
    let rest = &line[i..];
    let s = String::from_utf8_lossy(rest);
    let lower = s.to_ascii_lowercase();
    if let Some(pos) = lower.find("coding") {
        let after = &s[pos + 6..];
        // Must be followed by ':' or '='.
        let after_trimmed = after.trim_start();
        let first_char = after_trimmed.chars().next()?;
        if first_char != ':' && first_char != '=' {
            return None;
        }
        // Skip the : or = and whitespace.
        let enc_start = after_trimmed[1..].trim_start();
        // Collect [-\w.]+ characters.
        let enc: String = enc_start
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect();
        if enc.is_empty() {
            return None;
        }
        return Some((enc, ()));
    }
    None
}

// ---------------------------------------------------------------------------
// bytes_to_human_readable_repr
// ---------------------------------------------------------------------------

/// `mypy/util.py:bytes_to_human_readable_repr` — repr(b)[2:-1].
#[pyfunction]
pub fn rust_bytes_to_human_readable_repr(b: &[u8]) -> String {
    // Python: repr(b)[2:-1]. repr(bytes) is b'...', so [2:-1]
    // strips the b' prefix and trailing ' — content only, no quotes.
    let mut out = String::new();
    for &byte in b {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{:02x}", byte)),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// decode_python_encoding
// ---------------------------------------------------------------------------

/// `mypy/util.py:decode_python_encoding` — decode bytes per PEP-263.
///
/// Strips BOM if present, detects encoding via `find_python_encoding`,
/// then calls `bytes.decode(encoding)`. A `LookupError` (unknown
/// encoding) is wrapped as `DecodeError`, matching the Python wrapper.
#[pyfunction]
pub fn rust_decode_python_encoding(py: Python<'_>, source: &[u8]) -> PyResult<String> {
    let (body, encoding) = if source.starts_with(b"\xef\xbb\xbf") {
        (&source[3..], "utf8".to_string())
    } else {
        let (enc, _) = rust_find_python_encoding(source);
        (source, enc)
    };

    // Python: source.decode(encoding) — bytes.decode raises LookupError
    // for unknown encodings. We replicate by calling codecs.lookup to
    // validate, then bytes.decode.
    let codecs = py.import("codecs")?;
    let lookup = codecs.getattr("lookup")?;
    match lookup.call((encoding.clone(),), None) {
        Ok(_) => {}
        Err(_) => {
            return Err(DecodeError::new_err(format!(
                "unknown encoding: {}",
                encoding
            )));
        }
    }

    let py_bytes = PyBytes::new(py, body);
    let decode_result = py_bytes.call_method("decode", (encoding.clone(),), None);
    match decode_result {
        Ok(r) => r.extract::<String>(),
        Err(_) => Err(DecodeError::new_err(format!(
            "unknown encoding: {}",
            encoding
        ))),
    }
}

// ---------------------------------------------------------------------------
// trim_source_line
// ---------------------------------------------------------------------------

/// `mypy/util.py:trim_source_line` — trim a source line to fit max_len.
#[pyfunction]
pub fn rust_trim_source_line(line: &str, max_len: i64, col: i64, min_width: i64) -> (String, i64) {
    let mut max_len = max_len;
    if max_len < 2 * min_width + 1 {
        max_len = 2 * min_width + 1;
    }

    let line_len = line.chars().count() as i64;
    if line_len <= max_len {
        return (line.to_string(), 0);
    }

    if col + min_width < max_len {
        let truncated: String = line.chars().take(max_len as usize).collect();
        return (format!("{}...", truncated), 0);
    }

    if col < line_len - min_width - 1 {
        let offset = col - max_len + min_width + 1;
        let chars: Vec<char> = line.chars().collect();
        let start = offset as usize;
        let end = (col + min_width + 1) as usize;
        let middle: String = chars[start..end].iter().collect();
        return (format!("...{}...", middle), offset - 3);
    }

    // Column near end: trim start.
    let chars: Vec<char> = line.chars().collect();
    let start = (line_len - max_len) as usize;
    let tail: String = chars[start..].iter().collect();
    (format!("...{}", tail), line_len - max_len - 3)
}

// ---------------------------------------------------------------------------
// get_mypy_comments
// ---------------------------------------------------------------------------

/// `mypy/util.py:get_mypy_comments` — find `# mypy:` comments.
#[pyfunction]
pub fn rust_get_mypy_comments(source: &str) -> Vec<(i64, String)> {
    let prefix = "# mypy: ";
    if !source.contains(prefix) {
        return vec![];
    }
    let mut results = Vec::new();
    for (i, line) in source.split('\n').enumerate() {
        if let Some(rest) = line.strip_prefix(prefix) {
            results.push((i as i64 + 1, rest.to_string()));
        }
    }
    results
}

// ---------------------------------------------------------------------------
// get_prefix
// ---------------------------------------------------------------------------

/// `mypy/util.py:get_prefix` — drop final component of qualified name.
#[pyfunction]
pub fn rust_get_prefix(fullname: &str) -> String {
    match fullname.rfind('.') {
        Some(pos) => fullname[..pos].to_string(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// correct_relative_import
// ---------------------------------------------------------------------------

/// `mypy/util.py:correct_relative_import` — resolve a relative import.
#[pyfunction]
pub fn rust_correct_relative_import(
    cur_mod_id: &str,
    relative: i64,
    target: &str,
    is_cur_package_init_file: bool,
) -> (String, bool) {
    if relative == 0 {
        return (target.to_string(), true);
    }
    let parts: Vec<&str> = cur_mod_id.split('.').collect();
    let mut rel = relative;
    if is_cur_package_init_file {
        rel -= 1;
    }
    let ok = parts.len() as i64 >= rel;
    let new_mod = if rel != 0 && parts.len() as i64 >= rel {
        let cut = parts.len() - rel as usize;
        parts[..cut].join(".")
    } else if rel != 0 {
        // rel > parts.len(): can't go up that far
        String::new()
    } else {
        cur_mod_id.to_string()
    };
    let result = if target.is_empty() {
        new_mod
    } else {
        format!("{}.{}", new_mod, target)
    };
    (result, ok)
}

// ---------------------------------------------------------------------------
// unmangle
// ---------------------------------------------------------------------------

/// `mypy/util.py:unmangle` — remove internal suffixes from a short name.
#[pyfunction]
pub fn rust_unmangle(name: &str) -> String {
    name.trim_end_matches('\'').to_string()
}

// ---------------------------------------------------------------------------
// get_unique_redefinition_name
// ---------------------------------------------------------------------------

/// `mypy/util.py:get_unique_redefinition_name` — unique redefinition name.
#[pyfunction]
pub fn rust_get_unique_redefinition_name(name: &str, existing: Vec<String>) -> String {
    let existing_set: HashSet<&str> = existing.iter().map(|s| s.as_str()).collect();
    let r_name = format!("{}-redefinition", name);
    if !existing_set.contains(r_name.as_str()) {
        return r_name;
    }
    let mut i = 2;
    loop {
        let candidate = format!("{}{}", r_name, i);
        if !existing_set.contains(candidate.as_str()) {
            return candidate;
        }
        i += 1;
    }
}

// ---------------------------------------------------------------------------
// count_stats
// ---------------------------------------------------------------------------

/// `mypy/util.py:count_stats` — count errors, notes, error_files.
#[pyfunction]
pub fn rust_count_stats(messages: Vec<String>) -> (i64, i64, i64) {
    let errors: i64 = messages.iter().filter(|e| e.contains(": error:")).count() as i64;
    let notes: i64 = messages.iter().filter(|e| e.contains(": note:")).count() as i64;
    let error_files: i64 = messages
        .iter()
        .filter(|e| e.contains(": error:"))
        .map(|e| e.split(':').next().unwrap_or("").to_string())
        .collect::<HashSet<String>>()
        .len() as i64;
    (errors, notes, error_files)
}

// ---------------------------------------------------------------------------
// split_words
// ---------------------------------------------------------------------------

/// `mypy/util.py:split_words` — split text into words (not within quotes).
#[pyfunction]
pub fn rust_split_words(msg: &str) -> Vec<String> {
    let mut next_word = String::new();
    let mut res: Vec<String> = Vec::new();
    let mut allow_break = true;
    for c in msg.chars() {
        if c == ' ' && allow_break {
            res.push(std::mem::take(&mut next_word));
            continue;
        }
        if c == '"' {
            allow_break = !allow_break;
        }
        next_word.push(c);
    }
    res.push(next_word);
    res
}

// ---------------------------------------------------------------------------
// soft_wrap
// ---------------------------------------------------------------------------

/// `mypy/util.py:soft_wrap` — wrap a long error message into few lines.
#[pyfunction(signature = (msg, max_len, first_offset, num_indent=0))]
pub fn rust_soft_wrap(msg: &str, max_len: i64, first_offset: i64, num_indent: i64) -> String {
    let mut words = rust_split_words(msg);
    if words.is_empty() {
        return String::new();
    }
    let mut next_line = words.remove(0);
    let mut lines: Vec<String> = Vec::new();

    for next_word in words {
        let max_line_len = if !lines.is_empty() {
            max_len - num_indent
        } else {
            max_len - first_offset
        };
        if (next_line.len() as i64) + (next_word.len() as i64) < max_line_len {
            next_line.push(' ');
            next_line.push_str(&next_word);
        } else {
            lines.push(std::mem::take(&mut next_line));
            next_line = next_word;
        }
    }
    lines.push(next_line);

    let padding = format!("\n{}", " ".repeat(num_indent as usize));
    lines.join(&padding)
}

// ---------------------------------------------------------------------------
// hash_digest / hash_digest_bytes
// ---------------------------------------------------------------------------

/// `mypy/util.py:hash_digest` — SHA-1 hex digest.
#[pyfunction]
pub fn rust_hash_digest(data: &[u8]) -> String {
    use std::fmt::Write;
    let digest = sha1(data);
    let mut hex = String::with_capacity(40);
    for b in digest.iter() {
        write!(&mut hex, "{:02x}", b).unwrap();
    }
    hex
}

/// `mypy/util.py:hash_digest_bytes` — SHA-1 raw digest.
#[pyfunction]
pub fn rust_hash_digest_bytes(py: Python<'_>, data: &[u8]) -> PyObject {
    PyBytes::new(py, &sha1(data)).into()
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let len = data.len();
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    let bit_len = (len as u64) * 8;
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word_bytes) in chunk.chunks(4).take(16).enumerate() {
            w[i] = u32::from_be_bytes([word_bytes[0], word_bytes[1], word_bytes[2], word_bytes[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut result = [0u8; 20];
    result[0..4].copy_from_slice(&h0.to_be_bytes());
    result[4..8].copy_from_slice(&h1.to_be_bytes());
    result[8..12].copy_from_slice(&h2.to_be_bytes());
    result[12..16].copy_from_slice(&h3.to_be_bytes());
    result[16..20].copy_from_slice(&h4.to_be_bytes());
    result
}

// ---------------------------------------------------------------------------
// hash_path_stem
// ---------------------------------------------------------------------------

/// `mypy/util.py:hash_path_stem` — hash stem of a cache file path.
#[pyfunction]
pub fn rust_hash_path_stem(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let len = bytes.len() as i64;

    // Find end of stem (scanning backwards, stop at first separator
    // after last separator, or first dot in basename).
    let mut i = len - 1;
    let mut end = i;
    while i >= 0 {
        let c = bytes[i as usize] as i64;
        if c == b'/' as i64 || c == b'\\' as i64 {
            break;
        }
        if c == b'.' as i64 {
            end = i;
        }
        i -= 1;
    }

    // Calculate hash (DJB2-ish: hv * 33 ^ c).
    let mut hv: u64 = 123;
    i = end;
    while i >= 0 {
        let c = bytes[i as usize] as u64;
        hv = (hv.wrapping_mul(33)) ^ c;
        i -= 1;
    }

    // Murmur3 finalizer for better bit avalanche.
    hv = hv ^ (hv >> 32);
    hv ^= hv >> 16;
    hv = hv.wrapping_mul(0x85EBCA6B);
    hv ^= hv >> 13;
    hv = hv.wrapping_mul(0xC2B2AE35);
    hv ^= hv >> 16;
    hv as i64
}

// ---------------------------------------------------------------------------
// is_sub_path_normabs
// ---------------------------------------------------------------------------

/// `mypy/util.py:is_sub_path_normabs` — is path a sub-path of dir?
#[pyfunction]
pub fn rust_is_sub_path_normabs(py: Python<'_>, path: &str, dir: &str) -> PyResult<bool> {
    let os_mod = py.import("os")?;
    let sep: String = os_mod.getattr("sep")?.extract()?;
    let mut dir = dir.to_string();
    if !dir.ends_with(&sep) {
        dir.push_str(&sep);
    }
    Ok(path.starts_with(&dir))
}

// ---------------------------------------------------------------------------
// is_typeshed_file / is_stdlib_file
// ---------------------------------------------------------------------------

/// `mypy/util.py:is_typeshed_file` — is file under typeshed dir?
#[pyfunction(signature = (typeshed_dir=None, *, file))]
pub fn rust_is_typeshed_file(
    py: Python<'_>,
    typeshed_dir: Option<String>,
    file: &str,
) -> PyResult<bool> {
    let typeshed_dir = match typeshed_dir {
        Some(d) => d,
        None => {
            let importlib_resources = py.import("importlib.resources")?;
            let mypy_files = importlib_resources
                .getattr("files")?
                .call(("mypy",), None)?;
            let joined = mypy_files.getattr("__truediv__")?.call1(("typeshed",))?;
            let str_val = joined.getattr("__str__")?.call0()?.extract::<String>()?;
            str_val
        }
    };
    is_common_path_prefix(py, &typeshed_dir, file)
}

/// `mypy/util.py:is_stdlib_file` — is file under typeshed/stdlib?
#[pyfunction(signature = (typeshed_dir=None, *, file))]
pub fn rust_is_stdlib_file(
    py: Python<'_>,
    typeshed_dir: Option<String>,
    file: &str,
) -> PyResult<bool> {
    if !file.contains("stdlib") {
        return Ok(false);
    }
    let typeshed_dir = match typeshed_dir {
        Some(d) => d,
        None => {
            let importlib_resources = py.import("importlib.resources")?;
            let mypy_files = importlib_resources
                .getattr("files")?
                .call(("mypy",), None)?;
            let joined = mypy_files.getattr("__truediv__")?.call1(("typeshed",))?;
            let str_val = joined.getattr("__str__")?.call0()?.extract::<String>()?;
            str_val
        }
    };
    let os_mod = py.import("os")?;
    let os_path = os_mod.getattr("path")?;
    let stdlib_dir = os_path
        .getattr("join")?
        .call1((&typeshed_dir, "stdlib"))?
        .extract::<String>()?;
    is_common_path_prefix(py, &stdlib_dir, file)
}

fn is_common_path_prefix(py: Python<'_>, dir: &str, file: &str) -> PyResult<bool> {
    let os_mod = py.import("os")?;
    let os_path = os_mod.getattr("path")?;
    let abs_file = os_path
        .getattr("abspath")?
        .call1((file,))?
        .extract::<String>()?;
    // Python: os.path.commonpath((dir, abspath(file))) == dir.
    // Raises ValueError on different drives (Windows) → return False.
    let tuple = PyTuple::new(py, [dir, abs_file.as_str()]);
    match os_path.getattr("commonpath")?.call1((tuple,)) {
        Ok(result) => {
            let common: String = result.extract()?;
            Ok(common == dir)
        }
        Err(_) => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// is_stub_package_file
// ---------------------------------------------------------------------------

/// `mypy/util.py:is_stub_package_file` — PEP 561 stub package heuristic.
#[pyfunction]
pub fn rust_is_stub_package_file(py: Python<'_>, file: &str) -> PyResult<bool> {
    if !file.ends_with(".pyi") {
        return Ok(false);
    }
    let os_mod = py.import("os")?;
    let os_path = os_mod.getattr("path")?;
    let abs = os_path
        .getattr("abspath")?
        .call1((file,))?
        .extract::<String>()?;
    // Python: os.path.split returns (head, tail) — a 2-tuple.
    let (head, tail): (String, String) = os_path.getattr("split")?.call1((&abs,))?.extract()?;
    Ok(head.ends_with("-stubs") || tail.ends_with("-stubs"))
}

// ---------------------------------------------------------------------------
// unnamed_function
// ---------------------------------------------------------------------------

/// `mypy/util.py:unnamed_function` — is name the unnamed sentinel "_"?
#[pyfunction]
pub fn rust_unnamed_function(name: Option<String>) -> bool {
    name.is_some() && name.unwrap() == "_"
}

// ---------------------------------------------------------------------------
// time_spent_us
// ---------------------------------------------------------------------------

/// `mypy/util.py:time_spent_us` — microseconds since t0 (perf_counter_ns).
#[pyfunction]
pub fn rust_time_spent_us(py: Python<'_>, t0: i64) -> PyResult<i64> {
    let time_mod = py.import("time")?;
    let now: i64 = time_mod.getattr("perf_counter_ns")?.call0()?.extract()?;
    Ok((now - t0) / 1000)
}

// ---------------------------------------------------------------------------
// plural_s
// ---------------------------------------------------------------------------

/// `mypy/util.py:plural_s` — "s" if count != 1 else "".
#[pyfunction]
pub fn rust_plural_s(_py: Python<'_>, s: &PyAny) -> PyResult<String> {
    let count: i64 = if let Ok(n) = s.extract::<i64>() {
        n
    } else if let Ok(seq) = s.extract::<&PyList>() {
        seq.len() as i64
    } else {
        let len_method = s.getattr("__len__")?;
        len_method.call0()?.extract::<i64>()?
    };
    Ok(if count != 1 {
        "s".to_string()
    } else {
        "".to_string()
    })
}

// ---------------------------------------------------------------------------
// json_dumps
// ---------------------------------------------------------------------------

/// `mypy/util.py:json_dumps` — serialize obj to JSON bytes.
///
/// Delegates to Python's `json.dumps` to match exact serialization
/// semantics (sort_keys, separators, indent). Avoids adding a
/// serde_json dependency for fidelity.
#[pyfunction(signature = (obj, debug=false))]
pub fn rust_json_dumps(py: Python<'_>, obj: &PyAny, debug: bool) -> PyResult<PyObject> {
    let json_mod = py.import("json")?;
    let dumps = json_mod.getattr("dumps")?;
    let kwargs = PyDict::new(py);
    if debug {
        kwargs.set_item("indent", 2)?;
    }
    kwargs.set_item("sort_keys", true)?;
    if !debug {
        let sep: (&str, &str) = (",", ":");
        kwargs.set_item("separators", sep)?;
    }
    let result = dumps.call((obj,), Some(kwargs))?;
    let s: String = result.extract()?;
    Ok(PyBytes::new(py, s.as_bytes()).into())
}

// ---------------------------------------------------------------------------
// IdMapper
// ---------------------------------------------------------------------------

/// `mypy/util.py:IdMapper` — generate integer ids for objects.
///
/// Unlike `id()`, these start from 0 and increment by 1, and ids
/// won't get reused across the lifetime of the IdMapper. Uses the
/// raw Python object pointer as the key (identity-based, matching
/// Python's default `__hash__`/`__eq__` for arbitrary objects).
#[pyclass]
pub struct IdMapper {
    id_map: HashMap<usize, i64>,
    next_id: i64,
}

#[pymethods]
impl IdMapper {
    #[new]
    fn new() -> Self {
        IdMapper {
            id_map: HashMap::new(),
            next_id: 0,
        }
    }

    /// Assign (or reuse) an integer id for object `o`.
    fn id(&mut self, _py: Python<'_>, o: &PyAny) -> i64 {
        let key = o.as_ptr() as usize;
        if let Some(&existing) = self.id_map.get(&key) {
            return existing;
        }
        let id = self.next_id;
        self.id_map.insert(key, id);
        self.next_id += 1;
        id
    }

    fn __len__(&self) -> usize {
        self.id_map.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dunder() {
        assert!(rust_is_dunder("__init__", false));
        assert!(!rust_is_dunder("__init__", true));
        assert!(rust_is_dunder("__custom__", false));
        assert!(rust_is_dunder("__custom__", true));
        assert!(!rust_is_dunder("init", false));
    }

    #[test]
    fn test_is_sunder() {
        assert!(rust_is_sunder("_private_"));
        assert!(!rust_is_sunder("__init__"));
        assert!(!rust_is_sunder("_"));
        assert!(!rust_is_sunder("foo"));
    }

    #[test]
    fn test_split_module_names() {
        assert_eq!(rust_split_module_names("a.b.c"), vec!["a.b.c", "a.b", "a"]);
        assert_eq!(rust_split_module_names("a"), vec!["a"]);
    }

    #[test]
    fn test_get_prefix() {
        assert_eq!(rust_get_prefix("x.y"), "x");
        assert_eq!(rust_get_prefix("x"), "");
    }

    #[test]
    fn test_unmangle() {
        assert_eq!(rust_unmangle("foo''"), "foo");
        assert_eq!(rust_unmangle("foo"), "foo");
    }

    #[test]
    fn test_split_words() {
        assert_eq!(rust_split_words("hello world"), vec!["hello", "world"]);
        assert_eq!(rust_split_words(r#"a "b c" d"#), vec!["a", r#""b c""#, "d"]);
    }

    #[test]
    fn test_hash_digest() {
        let d = rust_hash_digest(b"hello");
        assert_eq!(d.len(), 40);
        assert_eq!(d, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_hash_digest_bytes() {
        let d = sha1(b"hello");
        assert_eq!(d.len(), 20);
        assert_eq!(
            d,
            [
                0xaa, 0xf4, 0xc6, 0x1d, 0xdc, 0xc5, 0xe8, 0xa2, 0xda, 0xbe, 0xde, 0x0f, 0x3b,
                0x48, 0x2c, 0xd9, 0xae, 0xa9, 0x43, 0x4d,
            ]
        );
    }

    #[test]
    fn test_hash_path_stem() {
        // Just ensure it doesn't panic and returns a value.
        let h = rust_hash_path_stem("/tmp/foo/bar.py.123");
        assert!(h >= 0);
    }

    #[test]
    fn test_get_mypy_comments() {
        let src = "# mypy: disallow-untyped-defs\nfoo = 1\n# mypy: strict\n";
        let comments = rust_get_mypy_comments(src);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].0, 1);
        assert_eq!(comments[0].1, "disallow-untyped-defs");
        assert_eq!(comments[1].0, 3);
        assert_eq!(comments[1].1, "strict");
    }

    #[test]
    fn test_get_unique_redefinition_name() {
        assert_eq!(
            rust_get_unique_redefinition_name("foo", vec![]),
            "foo-redefinition"
        );
        assert_eq!(
            rust_get_unique_redefinition_name("foo", vec!["foo-redefinition".to_string()]),
            "foo-redefinition2"
        );
        assert_eq!(
            rust_get_unique_redefinition_name(
                "foo",
                vec![
                    "foo-redefinition".to_string(),
                    "foo-redefinition2".to_string(),
                ]
            ),
            "foo-redefinition3"
        );
    }

    #[test]
    fn test_count_stats() {
        let msgs = vec![
            "a.py:1: error: bad".to_string(),
            "a.py:2: note: hint".to_string(),
            "b.py:3: error: worse".to_string(),
        ];
        assert_eq!(rust_count_stats(msgs), (2, 1, 2));
    }

    #[test]
    fn test_find_python_encoding() {
        assert_eq!(
            rust_find_python_encoding(b"# coding: utf-8\n"),
            ("utf-8".to_string(), 1)
        );
        assert_eq!(
            rust_find_python_encoding(b"pass\n"),
            ("utf8".to_string(), -1)
        );
        let src = b"# comment\n# coding: latin-1\n";
        assert_eq!(rust_find_python_encoding(src), ("latin-1".to_string(), 2));
    }

    #[test]
    fn test_bytes_to_human_readable_repr() {
        let b = vec![102, 111, 111, 10, 0];
        assert_eq!(rust_bytes_to_human_readable_repr(&b), "foo\\n\\x00");
    }

    #[test]
    fn test_trim_source_line() {
        let line = "short";
        assert_eq!(
            rust_trim_source_line(line, 80, 0, 20),
            ("short".to_string(), 0)
        );

        let line = "a very long line that exceeds the max length";
        let (out, off) = rust_trim_source_line(line, 10, 5, 20);
        assert!(out.starts_with("...") || out.ends_with("..."));
        let _ = off;
    }

    #[test]
    fn test_correct_relative_import() {
        assert_eq!(
            rust_correct_relative_import("a.b.c", 0, "target", false),
            ("target".to_string(), true)
        );
        assert_eq!(
            rust_correct_relative_import("a.b.c", 2, "d", false),
            ("a.d".to_string(), true)
        );
        assert_eq!(
            rust_correct_relative_import("a.b.c", 5, "d", false),
            (".d".to_string(), false)
        );
    }

    #[test]
    fn test_unnamed_function() {
        assert!(!rust_unnamed_function(None));
        assert!(rust_unnamed_function(Some("_".to_string())));
        assert!(!rust_unnamed_function(Some("foo".to_string())));
    }

    #[test]
    fn test_soft_wrap() {
        let result = rust_soft_wrap("hello world", 80, 0, 0);
        assert_eq!(result, "hello world");

        let result = rust_soft_wrap("a b c d", 5, 0, 0);
        assert!(result.contains("\n"));
    }
}
