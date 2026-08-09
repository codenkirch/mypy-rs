//! Attrs plugin transformation — ports `mypy/plugins/attrs.py`.
//!
//! The Rust side receives serialized field metadata, computes the
//! `__init__` method signature (arg names, kinds, types), and returns
//! `Option<Vec<u8>>` of wire-encoded method-info bytes. Python applies
//! the AST mutations (method injection, attribute additions) using the
//! deserialized result.
//!
//! Strangler-fig contract: unsupported cases return `None`; Python keeps
//! its full implementation for those cases.

use pyo3::prelude::*;

#[allow(unused_imports)]
use crate::wire::{read_type, write_type, Type};
use crate::wire::{ReadBuffer, WireError, WriteBuffer};

// ---------------------------------------------------------------------------
// Wire format tags for attrs metadata (non-colliding with wire.rs tags).
// ---------------------------------------------------------------------------

const ATTRS_FIELD: u8 = 200; // serialized AttrField
const ATTRS_INIT_SIG: u8 = 201; // serialized __init__ signature
const ATTRS_METHOD_INFO: u8 = 202; // full method info blob

// Input schema: field descriptors serialized to bytes.
// Each field: tag(ATTRS_FIELD) + count + name_len + name + type_bytes +
//   has_default(bool) + kw_only(bool) + init(bool).

/// A single attrs field descriptor, received from Python as a JSON-like
/// dict and serialized for the Rust computation.
#[derive(Debug, Clone)]
pub(crate) struct AttrField {
    pub name: String,
    #[allow(dead_code)]
    pub alias: Option<String>,
    pub init_type_bytes: Option<Vec<u8>>,
    pub has_default: bool,
    pub init: bool,
    pub kw_only: bool,
    #[allow(dead_code)]
    pub converter_init_type_bytes: Option<Vec<u8>>,
    #[allow(dead_code)]
    pub converter_ret_type_bytes: Option<Vec<u8>>,
}

/// Decoded field info for __init__ argument construction.
#[derive(Debug, Clone)]
pub(crate) struct InitArg {
    pub name: String,
    pub type_bytes: Option<Vec<u8>>,
    pub arg_kind: i64, // ARG_POS=0, ARG_OPT=1, ARG_NAMED=3, ARG_NAMED_OPT=5
    pub has_converter: bool,
}

#[allow(dead_code)]
/// Decoded method info that Python will use to inject methods.
#[derive(Debug, Clone)]
pub(crate) struct MethodInfo {
    pub method_name: String,
    pub arg_types: Vec<Option<Vec<u8>>>, // serialized Type
    pub arg_names: Vec<Option<String>>,
    pub arg_kinds: Vec<i64>,
    pub ret_type_bytes: Option<Vec<u8>>,
    pub self_type_bytes: Option<Vec<u8>>,
    pub tvd_name: Option<String>,
    pub tvd_fullname: Option<String>,
}

// ---------------------------------------------------------------------------
// Read / Write helpers for Attrs wire format
// ---------------------------------------------------------------------------

/// Read an AttrField from wire bytes.
fn read_attr_field(buf: &mut ReadBuffer<'_>) -> Result<AttrField, WireError> {
    // Wire format: tag + name_len + name + type_bytes_len + type_bytes +
    //   has_default + kw_only + init + conv_init_type + conv_ret_type.
    let name_len = read_short_int_from_buf(buf)?;
    if !(0..=10000).contains(&name_len) {
        return Err(WireError::invalid("attr field name too long"));
    }
    let name_bytes = buf.read_slice(name_len as usize)?;
    let name = std::str::from_utf8(name_bytes)
        .map_err(|_| WireError::invalid("invalid UTF-8 in attr field name"))?
        .to_string();

    let type_bytes_len = read_short_int_from_buf(buf)?;
    let init_type_bytes = if type_bytes_len > 0 {
        let data = buf.read_slice(type_bytes_len as usize)?;
        Some(data.to_vec())
    } else {
        None
    };

    let has_default = read_bool_from_buf(buf)?;
    let kw_only = read_bool_from_buf(buf)?;
    let init = read_bool_from_buf(buf)?;

    let converter_init_type_bytes_len = read_short_int_from_buf(buf)?;
    let converter_init_type_bytes = if converter_init_type_bytes_len > 0 {
        let data = buf.read_slice(converter_init_type_bytes_len as usize)?;
        Some(data.to_vec())
    } else {
        None
    };

    let converter_ret_type_bytes_len = read_short_int_from_buf(buf)?;
    let converter_ret_type_bytes = if converter_ret_type_bytes_len > 0 {
        let data = buf.read_slice(converter_ret_type_bytes_len as usize)?;
        Some(data.to_vec())
    } else {
        None
    };

    Ok(AttrField {
        name,
        alias: None,
        init_type_bytes,
        has_default,
        init,
        kw_only,
        converter_init_type_bytes,
        converter_ret_type_bytes,
    })
}

/// Short-int encoder (1-byte form, values -10..=117) for the attrs wire.
fn write_short_int_for_buf(buf: &mut WriteBuffer, value: i64) -> Result<(), WireError> {
    const MIN_ONE_BYTE_INT: i64 = -10;
    const MAX_ONE_BYTE_INT: i64 = 117;
    if (MIN_ONE_BYTE_INT..=MAX_ONE_BYTE_INT).contains(&value) {
        let payload = (value - MIN_ONE_BYTE_INT) << 1;
        buf.push(payload as u8);
        Ok(())
    } else {
        Err(WireError::invalid("short-int value out of 1-byte range"))
    }
}

fn read_short_int_from_buf(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
    const MIN_ONE_BYTE_INT: i64 = -10;
    const TWO_BYTES_INT_BIT: u8 = 1;
    const FOUR_BYTES_INT_BIT: u8 = 2;
    let first = buf.read_u8()?;
    if (first & TWO_BYTES_INT_BIT) == 0 {
        Ok(((first >> 1) as i64) + MIN_ONE_BYTE_INT)
    } else if (first & FOUR_BYTES_INT_BIT) == 0 {
        let second = buf.read_u8()?;
        Ok(((second as i64) << 6) + ((first >> 2) as i64) - 100)
    } else {
        let second = buf.read_u8()?;
        let two_more_bytes = buf.read_slice(2)?;
        let two_more = u16::from_le_bytes([two_more_bytes[0], two_more_bytes[1]]);
        let higher = ((two_more as i64) << 13) + ((second as i64) << 5);
        Ok(higher + ((first >> 3) as i64) - 10000)
    }
}

fn write_bool_for_buf(buf: &mut WriteBuffer, value: bool) {
    buf.push(if value { 1 } else { 0 });
}

fn read_bool_from_buf(buf: &mut ReadBuffer<'_>) -> Result<bool, WireError> {
    match buf.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(WireError::invalid(format!("invalid bool value {other}"))),
    }
}

#[allow(dead_code)]
/// Read a length-prefixed bytes value from the attrs wire.
fn read_bytes_prefixed(buf: &mut ReadBuffer<'_>) -> Result<Option<Vec<u8>>, WireError> {
    let len = read_short_int_from_buf(buf)?;
    if len < 0 {
        return Ok(None);
    }
    if len == 0 {
        return Ok(None);
    }
    let data = buf.read_slice(len as usize)?;
    Ok(Some(data.to_vec()))
}

// ---------------------------------------------------------------------------
// Core computation: compute __init__ signature from field metadata.
// ---------------------------------------------------------------------------

/// Compute the `__init__` signature from a list of field descriptors.
///
/// Returns `Some(Vec<u8>)` of wire-encoded method info, or `None` if
/// the fields can't be handled by Rust (e.g. unsupported type variants).
pub(crate) fn compute_init_signature(fields: Vec<AttrField>) -> Option<Vec<u8>> {
    // Separate positional and keyword-only args.
    let mut pos_args: Vec<InitArg> = Vec::new();
    let mut kw_only_args: Vec<InitArg> = Vec::new();

    for field in &fields {
        if !field.init {
            continue;
        }

        // Determine init type: prefer converter init_type, fall back to
        // field's own init_type.
        let type_bytes = field.init_type_bytes.clone();
        let has_converter = field.converter_init_type_bytes.is_some();

        // Build the init argument name (alias overrides, strip leading _).
        let name = match &field.alias {
            Some(a) => a.clone(),
            None => field
                .name
                .strip_prefix('_')
                .unwrap_or(&field.name)
                .to_string(),
        };

        let arg_kind = if field.kw_only {
            if field.has_default {
                5 // ARG_NAMED_OPT
            } else {
                3 // ARG_NAMED
            }
        } else if field.has_default {
            1 // ARG_OPT
        } else {
            0 // ARG_POS
        };

        let arg = InitArg {
            name: name.clone(),
            type_bytes: type_bytes.clone(),
            arg_kind,
            has_converter,
        };

        if field.kw_only {
            kw_only_args.push(arg);
        } else {
            pos_args.push(arg);
        }
    }

    // Build serialized method info.
    let mut buf = WriteBuffer::new();
    write_tag(&mut buf, ATTRS_METHOD_INFO);

    // Count valid arg slots.
    let total_args = pos_args.len() + kw_only_args.len();
    write_short_int_for_buf(&mut buf, total_args as i64).ok()?;

    // Write each arg: type_bytes_len + type_bytes or 0
    for arg in &pos_args {
        match &arg.type_bytes {
            Some(bytes) => {
                write_short_int_for_buf(&mut buf, bytes.len() as i64).ok()?;
                buf.extend(bytes);
            }
            None => {
                write_short_int_for_buf(&mut buf, 0).ok()?;
            }
        }
    }
    for arg in &kw_only_args {
        match &arg.type_bytes {
            Some(bytes) => {
                write_short_int_for_buf(&mut buf, bytes.len() as i64).ok()?;
                buf.extend(bytes);
            }
            None => {
                write_short_int_for_buf(&mut buf, 0).ok()?;
            }
        }
    }

    // Write arg kinds.
    for arg in &pos_args {
        write_short_int_for_buf(&mut buf, arg.arg_kind).ok()?;
    }
    for arg in &kw_only_args {
        write_short_int_for_buf(&mut buf, arg.arg_kind).ok()?;
    }

    // Write arg names.
    for arg in &pos_args {
        let name_bytes = arg.name.as_bytes();
        write_short_int_for_buf(&mut buf, name_bytes.len() as i64).ok()?;
        buf.extend(name_bytes);
    }
    for arg in &kw_only_args {
        let name_bytes = arg.name.as_bytes();
        write_short_int_for_buf(&mut buf, name_bytes.len() as i64).ok()?;
        buf.extend(name_bytes);
    }

    // Write has_converter flags.
    for arg in &pos_args {
        write_bool_for_buf(&mut buf, arg.has_converter);
    }
    for arg in &kw_only_args {
        write_bool_for_buf(&mut buf, arg.has_converter);
    }

    // Write kw_only flags.
    for arg in &pos_args {
        write_bool_for_buf(&mut buf, arg.arg_kind >= 3);
    }
    for arg in &kw_only_args {
        write_bool_for_buf(&mut buf, arg.arg_kind >= 3);
    }

    // Write ret_type: NoneType (NONE_TYPE=108, END_TAG=255).
    buf.push(108); // NONE_TYPE
    buf.push(255); // END_TAG

    let bytes = buf.into_bytes();
    Some(bytes)
}

/// Compute the ordering method signatures (__lt__, __le__, __gt__, __ge__).
/// Each returns `bool`. Signature: `def method(self: AT, other: AT) -> bool`.
pub(crate) fn compute_order_method_info(
    _class_fullname: &str,
    _method_name: &str,
) -> Option<Vec<u8>> {
    // Wire: ATTRS_METHOD_INFO + "self" + type_of_self (empty = Any)
    //       + "other" + type_of_other (same as self, resolved later)
    //       + arg_kinds [0, 0] + arg_names + ret_type (bool instance)

    let mut buf = WriteBuffer::new();
    write_tag(&mut buf, ATTRS_METHOD_INFO);
    write_short_int_for_buf(&mut buf, 2).ok()?; // 2 args

    // "self" arg type: empty (uses fill_typevars from Python side).
    write_short_int_for_buf(&mut buf, 0).ok()?;
    // "other" arg type: empty (same as self from Python side).
    write_short_int_for_buf(&mut buf, 0).ok()?;

    // Arg kinds: both ARG_POS (0).
    write_short_int_for_buf(&mut buf, 0).ok()?;
    write_short_int_for_buf(&mut buf, 0).ok()?;

    // Arg names.
    let self_bytes = b"self";
    write_short_int_for_buf(&mut buf, self_bytes.len() as i64).ok()?;
    buf.extend(self_bytes);
    let other_bytes = b"other";
    write_short_int_for_buf(&mut buf, other_bytes.len() as i64).ok()?;
    buf.extend(other_bytes);

    // Ret type: Instance(builtins.bool) — wire: INSTANCE(80) + INSTANCE_BOOL(86).
    buf.push(80); // INSTANCE
    buf.push(86); // INSTANCE_BOOL

    let bytes = buf.into_bytes();
    Some(bytes)
}

// ---------------------------------------------------------------------------
// PyO3 entry point: transform_attr_class
// ---------------------------------------------------------------------------

/// `rust_transform_attrs` — the seam function.
///
/// Python serializes the field metadata from the attrs plugin's `Attribute`
/// objects into a bytes blob, passes it here, and receives back serialized
/// `MethodInfo` data it can use to inject `__init__` and ordering methods.
///
/// Arguments:
///   - `fields_bytes`: wire-encoded list of field descriptors.
///   - `class_fullname`: the decorated class's fullname.
///   - `init_name`: "__init__" or "__attrs_init__".
///   - `add_order`: whether to also compute ordering methods.
///
/// Returns `Some(Vec<u8>)` of serialized method info, or `None` for
/// unsupported cases (falls back to Python).
#[pyfunction]
#[pyo3(name = "rust_transform_attrs")]
pub fn rust_transform_attrs(
    _py: Python<'_>,
    fields_bytes: &[u8],
    class_fullname: &str,
    init_name: &str,
    add_order: bool,
) -> PyResult<Option<Vec<u8>>> {
    // Deserialize the fields.
    let mut buf = ReadBuffer::new(fields_bytes);
    let fields = match deserialize_fields(&mut buf) {
        Some(f) => f,
        None => return Ok(None), // unsupported field format
    };

    // Compute __init__ signature.
    let init_info = compute_init_signature(fields).ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err("failed to compute __init__ signature")
    })?;

    let mut result = WriteBuffer::new();
    write_tag(&mut result, ATTRS_INIT_SIG);
    // Write init_name length + bytes.
    let name_bytes = init_name.as_bytes();
    write_short_int_for_buf(&mut result, name_bytes.len() as i64)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    result.extend(name_bytes);
    // Write init_info blob.
    write_short_int_for_buf(&mut result, init_info.len() as i64)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    result.extend(&init_info);

    // Compute ordering methods if requested.
    if add_order {
        let methods = ["__lt__", "__le__", "__gt__", "__ge__"];
        for method in &methods {
            let method_info = compute_order_method_info(class_fullname, method);
            if let Some(mi) = method_info {
                write_short_int_for_buf(&mut result, mi.len() as i64)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                result.extend(&mi);
            }
        }
    }

    Ok(Some(result.into_bytes()))
}

/// Deserialize field descriptors from the attrs wire format.
fn deserialize_fields(buf: &mut ReadBuffer<'_>) -> Option<Vec<AttrField>> {
    let count = read_short_int_from_buf(buf).ok()?;
    if !(0..=1000).contains(&count) {
        return None;
    }
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // Each field starts with the ATTRS_FIELD tag (consumed by the caller
        // or we read it here).
        let tag = buf.read_u8().ok()?;
        if tag != ATTRS_FIELD {
            return None;
        }
        let field = read_attr_field(buf).ok()?;
        fields.push(field);
    }
    Some(fields)
}

// ---------------------------------------------------------------------------
// PyO3: serialize_fields — helper for Python to serialize fields into bytes.
// ---------------------------------------------------------------------------

#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn rust_serialize_fields(
    _py: Python<'_>,
    fields: Vec<(String, Option<String>, Option<String>, bool, bool, bool)>,
) -> PyResult<Vec<u8>> {
    // fields: Vec of (name, alias, init_type_fullname, has_default, kw_only, init).
    // We don't have full Type bytes from here; Python should pre-serialize
    // the types. This helper handles the struct encoding for the wire.
    let mut buf = WriteBuffer::new();
    write_short_int_for_buf(&mut buf, fields.len() as i64)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    for (name, _alias, _init_type_fullname, has_default, kw_only, init_flag) in fields {
        // Tag.
        write_tag(&mut buf, ATTRS_FIELD);

        // Name.
        let name_bytes = name.as_bytes();
        write_short_int_for_buf(&mut buf, name_bytes.len() as i64)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        buf.extend(name_bytes);

        // Alias (we skip it for now; Python handles alias separately).
        // Placeholder: 0-length alias.
        write_short_int_for_buf(&mut buf, 0).ok();

        // init_type_bytes: empty (Python provides actual type bytes via another path).
        write_short_int_for_buf(&mut buf, 0).ok();

        // has_default, kw_only, init.
        write_bool_for_buf(&mut buf, has_default);
        write_bool_for_buf(&mut buf, kw_only);
        write_bool_for_buf(&mut buf, init_flag);

        // converter fields: none.
        write_short_int_for_buf(&mut buf, 0).ok();
        write_short_int_for_buf(&mut buf, 0).ok();
    }

    Ok(buf.into_bytes())
}

/// Write tag helper for attrs wire format.
fn write_tag(buf: &mut WriteBuffer, tag: u8) {
    buf.push(tag);
}
