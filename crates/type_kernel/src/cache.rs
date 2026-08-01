//! Fixed-format cache metadata reading (M4 / docs milestone "Move Cache
//! Indexing and Validation Below Python Object Materialization").
//!
//! Ports `mypy/cache.py` `CacheMeta.read` and `CacheMetaEx.read` onto the
//! binary wire seam. Every parse is exact: any deviation from the Python
//! layout returns `None`, and the Python caller falls back to the
//! pure-Python reader — the strangler-fig per-call gate.
//!
//! The decoded payloads are returned as Python dicts built with PyO3, so
//! the caller consumes them without a new type format.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

use crate::wire::{
    read_bool, read_int, read_int_bare, read_int_list, read_str, read_str_bare, read_str_opt,
    ReadBuffer, WireError,
};

// Collection tags used by the cache format but not defined in wire.rs
// (cache.py:313-324).
const LIST_GEN: u8 = 20;
const LIST_BYTES: u8 = 23;
const TUPLE_GEN: u8 = 24;
const DICT_STR_GEN: u8 = 30;
const LITERAL_NONE: u8 = 2;
const LITERAL_FALSE: u8 = 0;
const LITERAL_TRUE: u8 = 1;
const LITERAL_INT: u8 = 3;
const LITERAL_STR: u8 = 4;
const LITERAL_BYTES: u8 = 5;
const LITERAL_FLOAT: u8 = 6;

// ---------------------------------------------------------------------------
// FF-format readers (mirror cache.py read_* helpers)
// ---------------------------------------------------------------------------

fn read_tag(buf: &mut ReadBuffer<'_>) -> Result<u8, WireError> {
    buf.read_u8()
}

/// `read_bytes`: `LITERAL_BYTES` tag, then bare bytes.
fn read_bytes(buf: &mut ReadBuffer<'_>) -> Result<Vec<u8>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LITERAL_BYTES {
        return Err(WireError::invalid(format!(
            "expected LITERAL_BYTES, got tag {tag}"
        )));
    }
    read_bytes_bare(buf)
}

/// Bare bytes: short-int length prefix + raw body (cache.py read_bytes_bare).
fn read_bytes_bare(buf: &mut ReadBuffer<'_>) -> Result<Vec<u8>, WireError> {
    let first = buf.read_u8()?;
    // Reject the long-int trailer as a length prefix (fail-fast like the C
    // reader); sizes are short ints only.
    if first == 15 {
        return Err(WireError::invalid("invalid bytes size"));
    }
    let size = read_short_int(buf, first)?;
    if size < 0 {
        return Err(WireError::invalid("invalid bytes size"));
    }
    Ok(buf.read_slice(size as usize)?.to_vec())
}

/// `read_str_list`: `LIST_STR` tag, bare size, N bare strs.
fn read_str_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<String>, WireError> {
    let tag = read_tag(buf)?;
    if tag != 22 {
        return Err(WireError::invalid(format!(
            "expected LIST_STR, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        items.push(read_str_bare(buf)?);
    }
    Ok(items)
}

/// `read_bytes_list`: `LIST_BYTES` tag, bare size, N bare bytes.
fn read_bytes_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<Vec<u8>>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_BYTES {
        return Err(WireError::invalid(format!(
            "expected LIST_BYTES, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        items.push(read_bytes_bare(buf)?);
    }
    Ok(items)
}

/// `read_str_opt_list`: `LIST_GEN` tag, bare size, N `read_str_opt`s.
fn read_str_opt_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<Option<String>>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        items.push(read_str_opt(buf)?);
    }
    Ok(items)
}

/// `read_short_int`: the varint decoding inverse of `write_int_bare`. Mirrors
/// librt_internal.c `_read_short_int` and wire.rs `read_short_int`.
/// Delegates to the shared short-int varint reader in wire.rs (authoritative
/// mirror of librt_internal.c `_read_short_int`).
fn read_short_int(buf: &mut ReadBuffer<'_>, first: u8) -> Result<i64, WireError> {
    crate::wire::read_short_int(buf, first)
}

/// `read_json_value`: a single tagged JSON value (cache.py:490-516).
fn read_json_value(buf: &mut ReadBuffer<'_>) -> Result<JsonValue, WireError> {
    let tag = read_tag(buf)?;
    match tag {
        LITERAL_NONE => Ok(JsonValue::None),
        LITERAL_FALSE => Ok(JsonValue::Bool(false)),
        LITERAL_TRUE => Ok(JsonValue::Bool(true)),
        LITERAL_INT => read_int_bare(buf).map(JsonValue::Int),
        LITERAL_STR => read_str_bare(buf).map(JsonValue::Str),
        LITERAL_FLOAT => Ok(JsonValue::Float(read_float_bare(buf)?)),
        LIST_GEN => {
            let size = read_int_bare(buf)?;
            let mut items = Vec::with_capacity(size as usize);
            for _ in 0..size {
                items.push(read_json_value(buf)?);
            }
            Ok(JsonValue::Vec(items))
        }
        TUPLE_GEN => {
            let size = read_int_bare(buf)?;
            let mut items = Vec::with_capacity(size as usize);
            for _ in 0..size {
                items.push(read_json_value(buf)?);
            }
            Ok(JsonValue::Tuple(items))
        }
        DICT_STR_GEN => {
            let size = read_int_bare(buf)?;
            let mut items = Vec::with_capacity(size as usize);
            for _ in 0..size {
                let key = read_str_bare(buf)?;
                items.push((key, read_json_value(buf)?));
            }
            Ok(JsonValue::Dict(items))
        }
        other => Err(WireError::invalid(format!("invalid JSON tag {other}"))),
    }
}

/// `read_json_value` bare float (8 bytes, IEEE-754 little-endian).
fn read_float_bare(buf: &mut ReadBuffer<'_>) -> Result<f64, WireError> {
    let bytes = buf.read_slice(8)?;
    let le = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    Ok(f64::from_bits(le))
}

/// `read_json`: a string-keyed dict (cache.py:552-558). Returns an ordered
/// Vec of pairs so the caller can build a `PyDict`.
fn read_json(buf: &mut ReadBuffer<'_>) -> Result<Vec<(String, JsonValue)>, WireError> {
    let tag = read_tag(buf)?;
    if tag != DICT_STR_GEN {
        return Err(WireError::invalid(format!(
            "expected DICT_STR_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let key = read_str_bare(buf)?;
        items.push((key, read_json_value(buf)?));
    }
    Ok(items)
}

/// `read_errors`: a list of 8-tuples (cache.py:595-611).
fn read_errors(buf: &mut ReadBuffer<'_>) -> Result<Vec<ErrorTuple>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let tag = read_tag(buf)?;
        if tag != TUPLE_GEN {
            return Err(WireError::invalid(format!(
                "expected TUPLE_GEN, got tag {tag}"
            )));
        }
        items.push(ErrorTuple {
            path: read_str_opt(buf)?,
            line: read_int(buf)?,
            column: read_int(buf)?,
            end_line: read_int(buf)?,
            end_column: read_int(buf)?,
            severity: read_str(buf)?,
            message: read_str(buf)?,
            code: read_str_opt(buf)?,
        });
    }
    Ok(items)
}

/// A JSON value tree used while decoding `CacheMeta.options`.
#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Vec(Vec<JsonValue>),
    Tuple(Vec<JsonValue>),
    Dict(Vec<(String, JsonValue)>),
}

/// A decoded error tuple (mirrors `cache.py:ErrorTuple`).
#[derive(Debug, Clone, PartialEq)]
struct ErrorTuple {
    path: Option<String>,
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
    severity: String,
    message: String,
    code: Option<String>,
}

// ---------------------------------------------------------------------------
// CacheMeta reader
// ---------------------------------------------------------------------------

/// Decoded `CacheMeta` fields (see `cache.py:CacheMeta`). Field order must
/// match `CacheMeta.write` exactly.
#[derive(Debug, Clone, PartialEq)]
struct CacheMetaFields {
    id: String,
    path: String,
    mtime: i64,
    size: i64,
    hash: String,
    dependencies: Vec<String>,
    data_mtime: i64,
    suppressed: Vec<String>,
    imports_ignored: Vec<(i64, Vec<String>)>,
    options: Vec<(String, JsonValue)>,
    suppressed_deps_opts: Vec<u8>,
    dep_prios: Vec<i64>,
    dep_lines: Vec<i64>,
    dep_hashes: Vec<Vec<u8>>,
    interface_hash: Vec<u8>,
    trans_dep_hash: Vec<u8>,
    version_id: String,
    ignore_all: bool,
    plugin_data: JsonValue,
}

fn read_cache_meta_fields(buf: &mut ReadBuffer<'_>) -> Result<CacheMetaFields, WireError> {
    let id = read_str(buf)?;
    let path = read_str(buf)?;
    let mtime = read_int(buf)?;
    let size = read_int(buf)?;
    let hash = read_str(buf)?;
    let dependencies = read_str_list(buf)?;
    let data_mtime = read_int(buf)?;
    let suppressed = read_str_list(buf)?;
    let imports_ignored_count = read_int_bare(buf)?;
    let mut imports_ignored = Vec::with_capacity(imports_ignored_count as usize);
    for _ in 0..imports_ignored_count {
        imports_ignored.push((read_int(buf)?, read_str_list(buf)?));
    }
    Ok(CacheMetaFields {
        id,
        path,
        mtime,
        size,
        hash,
        dependencies,
        data_mtime,
        suppressed,
        imports_ignored,
        options: read_json(buf)?,
        suppressed_deps_opts: read_bytes(buf)?,
        dep_prios: read_int_list(buf)?,
        dep_lines: read_int_list(buf)?,
        dep_hashes: read_bytes_list(buf)?,
        interface_hash: read_bytes(buf)?,
        trans_dep_hash: read_bytes(buf)?,
        version_id: read_str(buf)?,
        ignore_all: read_bool(buf)?,
        plugin_data: read_json_value(buf)?,
    })
}

/// Decoded `CacheMetaEx` fields (see `cache.py:CacheMetaEx`).
#[derive(Debug, Clone, PartialEq)]
struct CacheMetaExFields {
    dependencies: Vec<String>,
    suppressed: Vec<String>,
    dep_hashes: Vec<Vec<u8>>,
    error_lines: Vec<ErrorTuple>,
}

fn read_cache_meta_ex_fields(buf: &mut ReadBuffer<'_>) -> Result<CacheMetaExFields, WireError> {
    Ok(CacheMetaExFields {
        dependencies: read_str_list(buf)?,
        suppressed: read_str_list(buf)?,
        dep_hashes: read_bytes_list(buf)?,
        error_lines: read_errors(buf)?,
    })
}

// ---------------------------------------------------------------------------
// PyO3 conversion helpers
// ---------------------------------------------------------------------------

fn json_value_to_py(py: Python<'_>, value: &JsonValue) -> PyResult<Py<PyAny>> {
    match value {
        JsonValue::None => Ok(py.None()),
        JsonValue::Bool(b) => Ok(b.into_py(py)),
        JsonValue::Int(i) => Ok(i.into_py(py)),
        JsonValue::Float(f) => Ok(f.into_py(py)),
        JsonValue::Str(s) => Ok(s.into_py(py)),
        JsonValue::Vec(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(json_value_to_py(py, item)?)?;
            }
            Ok(list.into_py(py))
        }
        JsonValue::Tuple(items) => {
            let mut objs = Vec::with_capacity(items.len());
            for item in items {
                objs.push(json_value_to_py(py, item)?);
            }
            let tuple: Py<PyAny> = PyTuple::new(py, objs).into_py(py);
            Ok(tuple)
        }
        JsonValue::Dict(entries) => {
            let dict = PyDict::new(py);
            for (key, value) in entries {
                dict.set_item(key, json_value_to_py(py, value)?)?;
            }
            Ok(dict.into_py(py))
        }
    }
}

fn json_dict_to_py(py: Python<'_>, entries: &[(String, JsonValue)]) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (key, value) in entries {
        dict.set_item(key, json_value_to_py(py, value)?)?;
    }
    Ok(dict.into_py(py))
}

fn bytes_list_to_py(py: Python<'_>, items: &[Vec<u8>]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for item in items {
        list.append(PyBytes::new(py, item))?;
    }
    Ok(list.into_py(py))
}

fn errors_to_py(py: Python<'_>, errors: &[ErrorTuple]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for err in errors {
        let items: Vec<Py<PyAny>> = vec![
            err.path.clone().into_py(py),
            err.line.into_py(py),
            err.column.into_py(py),
            err.end_line.into_py(py),
            err.end_column.into_py(py),
            err.severity.clone().into_py(py),
            err.message.clone().into_py(py),
            err.code.clone().into_py(py),
        ];
        let tuple: Py<PyAny> = PyTuple::new(py, items).into_py(py);
        list.append(tuple)?;
    }
    Ok(list.into_py(py))
}

fn cache_meta_to_py(py: Python<'_>, meta: &CacheMetaFields) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("id", &meta.id)?;
    dict.set_item("path", &meta.path)?;
    dict.set_item("mtime", meta.mtime)?;
    dict.set_item("size", meta.size)?;
    dict.set_item("hash", &meta.hash)?;
    dict.set_item("dependencies", &meta.dependencies)?;
    dict.set_item("data_mtime", meta.data_mtime)?;
    dict.set_item("suppressed", &meta.suppressed)?;
    let imports_ignored = PyDict::new(py);
    for (line, codes) in &meta.imports_ignored {
        imports_ignored.set_item(line, codes)?;
    }
    dict.set_item("imports_ignored", imports_ignored)?;
    dict.set_item("options", json_dict_to_py(py, &meta.options)?)?;
    dict.set_item(
        "suppressed_deps_opts",
        PyBytes::new(py, &meta.suppressed_deps_opts),
    )?;
    dict.set_item("dep_prios", &meta.dep_prios)?;
    dict.set_item("dep_lines", &meta.dep_lines)?;
    dict.set_item("dep_hashes", bytes_list_to_py(py, &meta.dep_hashes)?)?;
    dict.set_item("interface_hash", PyBytes::new(py, &meta.interface_hash))?;
    dict.set_item("trans_dep_hash", PyBytes::new(py, &meta.trans_dep_hash))?;
    dict.set_item("version_id", &meta.version_id)?;
    dict.set_item("ignore_all", meta.ignore_all)?;
    dict.set_item("plugin_data", json_value_to_py(py, &meta.plugin_data)?)?;
    Ok(dict.into_py(py))
}

fn cache_meta_ex_to_py(py: Python<'_>, meta: &CacheMetaExFields) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("dependencies", &meta.dependencies)?;
    dict.set_item("suppressed", &meta.suppressed)?;
    dict.set_item("dep_hashes", bytes_list_to_py(py, &meta.dep_hashes)?)?;
    dict.set_item("error_lines", errors_to_py(py, &meta.error_lines)?)?;
    Ok(dict.into_py(py))
}

// ---------------------------------------------------------------------------
// pyfunction entries
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for `CacheMeta.read` (cache.py:214). Consumes the
/// fixed-format meta bytes (`bytes[2:]` — the 2-byte version header is
/// stripped by the caller) and returns a decoded dict of the fields;
/// `None` if the record doesn't match the Python layout.
#[pyfunction]
pub(crate) fn rust_read_cache_meta(py: Python<'_>, blob: &[u8]) -> PyResult<Option<Py<PyAny>>> {
    let mut buf = ReadBuffer::new(blob);
    match read_cache_meta_fields(&mut buf) {
        Ok(fields) => Ok(Some(cache_meta_to_py(py, &fields)?)),
        Err(_) => Ok(None),
    }
}

/// `#[pyfunction]` entry for `CacheMetaEx.read` (cache.py:286). Consumes the
/// fixed-format meta_ex bytes and returns a decoded dict of the fields;
/// `None` if the record doesn't match the Python layout.
#[pyfunction]
pub(crate) fn rust_read_cache_meta_ex(py: Python<'_>, blob: &[u8]) -> PyResult<Option<Py<PyAny>>> {
    let mut buf = ReadBuffer::new(blob);
    match read_cache_meta_ex_fields(&mut buf) {
        Ok(fields) => Ok(Some(cache_meta_ex_to_py(py, &fields)?)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::WriteBuffer;

    #[test]
    fn read_bytes_roundtrip() {
        let mut wbuf = WriteBuffer::new();
        // Emulate the low-level byte layout: LITERAL_BYTES tag, then the
        // short-int length prefix for 5 bytes ((5 + 10) << 1 = 30), then body.
        wbuf.push(LITERAL_BYTES);
        wbuf.push(30);
        wbuf.extend(b"hello");
        let blob = wbuf.into_bytes();
        let mut rbuf = ReadBuffer::new(&blob);
        assert_eq!(read_bytes(&mut rbuf).unwrap(), b"hello");
    }
}
