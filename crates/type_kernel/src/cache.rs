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
//!
//! The writer half (`rust_write_cache_meta` / `rust_write_cache_meta_ex`,
//! issue #1503) mirrors `CacheMeta.write` / `CacheMetaEx.write`
//! byte-for-byte on live Python objects; a shape the format cannot encode
//! returns `None` so the Python body runs and raises the identical
//! exception.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyList, PyLong, PyString, PyTuple};

use crate::wire::{
    read_bool, read_int, read_int_bare, read_int_list, read_str, read_str_bare, read_str_opt,
    write_big_int, write_bool, write_bytes, write_bytes_list, write_float_bare, write_int_bare,
    write_str, write_str_bare, write_str_list, write_tag, BigInt, ReadBuffer, WireError,
    WriteBuffer, DICT_STR_GEN, LIST_BYTES, LIST_GEN, LIST_INT, LITERAL_BYTES, LITERAL_FALSE,
    LITERAL_FLOAT, LITERAL_INT, LITERAL_NONE, LITERAL_STR, LITERAL_TRUE, TUPLE_GEN,
};

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

// ---------------------------------------------------------------------------
// FF-format writers (mirror cache.py write_* helpers)
// ---------------------------------------------------------------------------

/// Write a tagged `str` field. `Ok(false)` defers on a non-str or an
/// encoding failure.
fn write_py_str(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    match value.downcast::<PyString>() {
        Ok(s) => match s.to_str() {
            Ok(text) => Ok(write_str(buf, text).is_ok()),
            Err(_) => Ok(false),
        },
        Err(_) => Ok(false),
    }
}

/// `write_str_opt`: `LITERAL_NONE` or a tagged str.
fn write_py_str_opt(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    if value.is_none() {
        write_tag(buf, LITERAL_NONE);
        return Ok(true);
    }
    write_py_str(buf, value)
}

/// Tagged `LITERAL_BYTES` + bare bytes.
fn write_py_bytes(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    match value.downcast::<PyBytes>() {
        Ok(b) => Ok(write_bytes(buf, b.as_bytes()).is_ok()),
        Err(_) => Ok(false),
    }
}

/// List/tuple items in Python iteration order; `Ok(None)` defers on any
/// other sequence kind.
fn sequence_items(value: &PyAny) -> PyResult<Option<Vec<&PyAny>>> {
    if let Ok(list) = value.downcast::<PyList>() {
        return Ok(Some(list.iter().collect()));
    }
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        return Ok(Some(tuple.iter().collect()));
    }
    Ok(None)
}

/// `write_str_list`: `LIST_STR` + bare size + N bare strs.
fn write_py_str_list(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    let mut strings = Vec::new();
    match sequence_items(value)? {
        Some(items) => {
            for item in items {
                match item.downcast::<PyString>() {
                    Ok(s) => match s.to_str() {
                        Ok(text) => strings.push(text.to_string()),
                        Err(_) => return Ok(false),
                    },
                    Err(_) => return Ok(false),
                }
            }
        }
        None => return Ok(false),
    }
    Ok(write_str_list(buf, &strings).is_ok())
}

/// Bare Python int, including arbitrary-precision values via the C writer's
/// long-int path.
fn write_py_int_bare(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    if !value.is_instance_of::<PyLong>() {
        return Ok(false);
    }
    if let Ok(v) = value.extract::<i64>() {
        return Ok(write_int_bare(buf, v).is_ok());
    }
    // |value| > i64::MAX: the wire form is (size << 1 | sign) followed by a
    // minimal little-endian magnitude, so read the magnitude from Python.
    let abs = value.call_method0("__abs__")?;
    let bits: i64 = match abs.call_method0("bit_length")?.extract() {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    let nbytes = ((bits + 7) / 8).max(1);
    let raw = abs.call_method1("to_bytes", (nbytes, "little"))?;
    let magnitude: Vec<u8> = match raw.extract() {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    let neg = value.call_method1("__lt__", (0i64,))?.is_true()?;
    let big = BigInt::from_le_bytes(&magnitude, neg);
    Ok(write_big_int(buf, &big).is_ok())
}

/// `write_int`: tagged `LITERAL_INT` + bare int.
fn write_py_int(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    write_tag(buf, LITERAL_INT);
    write_py_int_bare(buf, value)
}

/// `write_int_list`: `LIST_INT` + bare size + N bare ints.
fn write_py_int_list(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    let items = match sequence_items(value)? {
        Some(items) => items,
        None => return Ok(false),
    };
    write_tag(buf, LIST_INT);
    if write_int_bare(buf, items.len() as i64).is_err() {
        return Ok(false);
    }
    for item in items {
        if !write_py_int_bare(buf, item)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `write_bytes_list`: `LIST_BYTES` + bare size + N bare bytes.
fn write_py_bytes_list(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    let items = match sequence_items(value)? {
        Some(items) => items,
        None => return Ok(false),
    };
    let mut parts = Vec::with_capacity(items.len());
    for item in items {
        match item.downcast::<PyBytes>() {
            Ok(b) => parts.push(b.as_bytes().to_vec()),
            Err(_) => return Ok(false),
        }
    }
    Ok(write_bytes_list(buf, &parts).is_ok())
}

/// `write_json_value`: a single tagged JSON value (cache.py:604-635).
/// Unsupported types defer (`Ok(false)`).
fn write_json_value(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    if value.is_none() {
        write_tag(buf, LITERAL_NONE);
        return Ok(true);
    }
    if value.is_instance_of::<PyBool>() {
        write_bool(buf, value.is_true()?);
        return Ok(true);
    }
    if value.is_instance_of::<PyLong>() {
        return write_py_int(buf, value);
    }
    if let Ok(s) = value.downcast::<PyString>() {
        return match s.to_str() {
            Ok(text) => Ok(write_str(buf, text).is_ok()),
            Err(_) => Ok(false),
        };
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        return write_json_dict(buf, dict);
    }
    if let Ok(list) = value.downcast::<PyList>() {
        return write_json_sequence(buf, list.iter(), LIST_GEN);
    }
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        return write_json_sequence(buf, tuple.iter(), TUPLE_GEN);
    }
    if value.is_instance_of::<PyFloat>() {
        return match value.extract::<f64>() {
            Ok(f) => {
                write_tag(buf, LITERAL_FLOAT);
                Ok(write_float_bare(buf, f).is_ok())
            }
            Err(_) => Ok(false),
        };
    }
    Ok(false)
}

/// `write_json_value` list/tuple arm: tag + bare size + N values.
fn write_json_sequence<'py, I>(buf: &mut WriteBuffer, items: I, tag: u8) -> PyResult<bool>
where
    I: Iterator<Item = &'py PyAny>,
{
    let items: Vec<&PyAny> = items.collect();
    write_tag(buf, tag);
    if write_int_bare(buf, items.len() as i64).is_err() {
        return Ok(false);
    }
    for item in items {
        if !write_json_value(buf, item)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `write_json_value` dict arm / `write_json` (cache.py:625-651): bare
/// size, then keys in Python `sorted()` order (Rust `str` order equals
/// Python code-point order for UTF-8).
fn write_json_dict(buf: &mut WriteBuffer, dict: &PyDict) -> PyResult<bool> {
    let mut entries: Vec<(String, &PyAny)> = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        match key.downcast::<PyString>() {
            Ok(s) => match s.to_str() {
                Ok(text) => entries.push((text.to_string(), value)),
                Err(_) => return Ok(false),
            },
            Err(_) => return Ok(false),
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    write_tag(buf, DICT_STR_GEN);
    if write_int_bare(buf, entries.len() as i64).is_err() {
        return Ok(false);
    }
    for (key, value) in entries {
        if write_str_bare(buf, &key).is_err() {
            return Ok(false);
        }
        if !write_json_value(buf, value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `write_json`: a `dict[str, Any]` at the top level.
fn write_json(buf: &mut WriteBuffer, value: &PyAny) -> PyResult<bool> {
    match value.downcast::<PyDict>() {
        Ok(dict) => write_json_dict(buf, dict),
        Err(_) => Ok(false),
    }
}

/// `write_errors`: `LIST_GEN` of 8-element error tuples (cache.py:668-680).
fn write_errors_live(buf: &mut WriteBuffer, errs: &PyAny) -> PyResult<bool> {
    let items = match sequence_items(errs)? {
        Some(items) => items,
        None => return Ok(false),
    };
    write_tag(buf, LIST_GEN);
    if write_int_bare(buf, items.len() as i64).is_err() {
        return Ok(false);
    }
    for err in items {
        let fields = match sequence_items(err)? {
            Some(fields) if fields.len() == 8 => fields,
            _ => return Ok(false),
        };
        write_tag(buf, TUPLE_GEN);
        let written = write_py_str_opt(buf, fields[0])?
            && write_py_int(buf, fields[1])?
            && write_py_int(buf, fields[2])?
            && write_py_int(buf, fields[3])?
            && write_py_int(buf, fields[4])?
            && write_py_str(buf, fields[5])?
            && write_py_str(buf, fields[6])?
            && write_py_str_opt(buf, fields[7])?;
        if !written {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Serialize a live `CacheMeta`; `Ok(None)` defers to the Python body.
/// Field order must match `CacheMeta.write` (cache.py:274-298).
fn cache_meta_bytes(meta: &PyAny) -> PyResult<Option<Vec<u8>>> {
    let mut buf = WriteBuffer::new();
    if !write_py_str(&mut buf, meta.getattr("id")?)? {
        return Ok(None);
    }
    if !write_py_str(&mut buf, meta.getattr("path")?)? {
        return Ok(None);
    }
    if !write_py_int(&mut buf, meta.getattr("mtime")?)? {
        return Ok(None);
    }
    if !write_py_int(&mut buf, meta.getattr("size")?)? {
        return Ok(None);
    }
    if !write_py_str(&mut buf, meta.getattr("hash")?)? {
        return Ok(None);
    }
    if !write_py_str_list(&mut buf, meta.getattr("dependencies")?)? {
        return Ok(None);
    }
    if !write_py_int(&mut buf, meta.getattr("data_mtime")?)? {
        return Ok(None);
    }
    if !write_py_str_list(&mut buf, meta.getattr("suppressed")?)? {
        return Ok(None);
    }
    let imports_ignored = match meta.getattr("imports_ignored")?.downcast::<PyDict>() {
        Ok(dict) => dict,
        Err(_) => return Ok(None),
    };
    if write_int_bare(&mut buf, imports_ignored.len() as i64).is_err() {
        return Ok(None);
    }
    for (line, codes) in imports_ignored.iter() {
        if !write_py_int(&mut buf, line)? || !write_py_str_list(&mut buf, codes)? {
            return Ok(None);
        }
    }
    if !write_json(&mut buf, meta.getattr("options")?)? {
        return Ok(None);
    }
    if !write_py_bytes(&mut buf, meta.getattr("suppressed_deps_opts")?)? {
        return Ok(None);
    }
    if !write_py_int_list(&mut buf, meta.getattr("dep_prios")?)? {
        return Ok(None);
    }
    if !write_py_int_list(&mut buf, meta.getattr("dep_lines")?)? {
        return Ok(None);
    }
    if !write_py_bytes_list(&mut buf, meta.getattr("dep_hashes")?)? {
        return Ok(None);
    }
    if !write_py_bytes(&mut buf, meta.getattr("interface_hash")?)? {
        return Ok(None);
    }
    if !write_py_bytes(&mut buf, meta.getattr("trans_dep_hash")?)? {
        return Ok(None);
    }
    if !write_py_str(&mut buf, meta.getattr("version_id")?)? {
        return Ok(None);
    }
    write_bool(&mut buf, meta.getattr("ignore_all")?.is_true()?);
    if !write_json_value(&mut buf, meta.getattr("plugin_data")?)? {
        return Ok(None);
    }
    Ok(Some(buf.into_bytes()))
}

/// Serialize a live `CacheMetaEx`; `Ok(None)` defers to the Python body.
fn cache_meta_ex_bytes(meta_ex: &PyAny) -> PyResult<Option<Vec<u8>>> {
    let mut buf = WriteBuffer::new();
    if !write_py_str_list(&mut buf, meta_ex.getattr("dependencies")?)? {
        return Ok(None);
    }
    if !write_py_str_list(&mut buf, meta_ex.getattr("suppressed")?)? {
        return Ok(None);
    }
    if !write_py_bytes_list(&mut buf, meta_ex.getattr("dep_hashes")?)? {
        return Ok(None);
    }
    if !write_errors_live(&mut buf, meta_ex.getattr("error_lines")?)? {
        return Ok(None);
    }
    Ok(Some(buf.into_bytes()))
}

/// `#[pyfunction]` entry for `CacheMeta.write` (cache.py:274). Returns the
/// exact bytes the Python `write` method would produce, or `None` to
/// defer. Per the #1466/#1468 contract only `PyAttributeError` maps to a
/// defer; other PyErrs propagate so kernel bugs stay visible.
#[pyfunction]
pub(crate) fn rust_write_cache_meta(py: Python<'_>, meta: &PyAny) -> PyResult<Option<Py<PyBytes>>> {
    match cache_meta_bytes(meta) {
        Ok(Some(bytes)) => Ok(Some(PyBytes::new(py, &bytes).into())),
        Ok(None) => Ok(None),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
    }
}

/// `#[pyfunction]` entry for `CacheMetaEx.write` (cache.py:366). Mirrors
/// `rust_write_cache_meta`'s defer contract.
#[pyfunction]
pub(crate) fn rust_write_cache_meta_ex(
    py: Python<'_>,
    meta_ex: &PyAny,
) -> PyResult<Option<Py<PyBytes>>> {
    match cache_meta_ex_bytes(meta_ex) {
        Ok(Some(bytes)) => Ok(Some(PyBytes::new(py, &bytes).into())),
        Ok(None) => Ok(None),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
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

    #[test]
    fn write_bytes_and_list_roundtrip() {
        let mut wbuf = WriteBuffer::new();
        write_bytes(&mut wbuf, b"hello").unwrap();
        let blob = wbuf.into_bytes();
        let mut rbuf = ReadBuffer::new(&blob);
        assert_eq!(read_bytes(&mut rbuf).unwrap(), b"hello");

        let mut wbuf = WriteBuffer::new();
        write_bytes_list(&mut wbuf, &[b"a".to_vec(), b"bc".to_vec(), Vec::new()]).unwrap();
        let blob = wbuf.into_bytes();
        let mut rbuf = ReadBuffer::new(&blob);
        assert_eq!(
            read_bytes_list(&mut rbuf).unwrap(),
            vec![b"a".to_vec(), b"bc".to_vec(), Vec::new()]
        );
    }

    #[test]
    fn write_str_list_roundtrip() {
        let mut wbuf = WriteBuffer::new();
        write_str_list(
            &mut wbuf,
            &["x".to_string(), String::new(), "yz".to_string()],
        )
        .unwrap();
        let blob = wbuf.into_bytes();
        let mut rbuf = ReadBuffer::new(&blob);
        assert_eq!(read_str_list(&mut rbuf).unwrap(), vec!["x", "", "yz"]);
    }

    #[test]
    fn write_big_int_roundtrip() {
        // 2**88 (> i64) encoded through the long-int path; read back with
        // the shared arbitrary-precision reader.
        let mut magnitude = [0u8; 12];
        magnitude[11] = 1;
        let big = BigInt::from_le_bytes(&magnitude, false);
        let mut wbuf = WriteBuffer::new();
        write_big_int(&mut wbuf, &big).unwrap();
        let blob = wbuf.into_bytes();
        let mut rbuf = ReadBuffer::new(&blob);
        assert_eq!(rbuf.read_u8().unwrap(), crate::wire::LONG_INT_TRAILER);
        assert_eq!(crate::wire::read_long_int_big(&mut rbuf).unwrap(), big);
    }
}
