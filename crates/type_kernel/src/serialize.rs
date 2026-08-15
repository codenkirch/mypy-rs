//! Rust-side wire-format serializer for live `mypy.types.Type` objects.
//!
//! Mirrors the per-class `Type.write(WriteBuffer)` methods in `mypy/types.py`
//! and the byte primitives in `mypyc/lib-rt/internal/librt_internal.c`.
//! Walking the Python object graph via PyO3 and writing bytes into a Rust
//! `Vec<u8>` eliminates the per-tag Python→C FFI and method-dispatch overhead
//! that dominates kernel-on self-check time (issue #606).

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyList, PySet, PyTuple};

// Varint ranges (librt_internal.c:14-19).
const MIN_ONE_BYTE_INT: i64 = -10;
const MAX_ONE_BYTE_INT: i64 = 117;
const MIN_TWO_BYTES_INT: i64 = -100;
const MAX_TWO_BYTES_INT: i64 = 16283;
const MIN_FOUR_BYTES_INT: i64 = -10000;
const MAX_FOUR_BYTES_INT: i64 = 536860911;

const TWO_BYTES_INT_BIT: u8 = 1;
const FOUR_BYTES_INT_TRAILER: u8 = 3;
const LONG_INT_TRAILER: u8 = 15;

// Literal tags.
const LITERAL_NONE: u8 = 2;
const LITERAL_INT: u8 = 3;
const LITERAL_STR: u8 = 4;
const LITERAL_FLOAT: u8 = 6;

// Collection tags.
const LIST_GEN: u8 = 20;
const LIST_INT: u8 = 21;
const LIST_STR: u8 = 22;
const DICT_STR_GEN: u8 = 30;

// Type tags.
const INSTANCE: u8 = 80;
const INSTANCE_SIMPLE: u8 = 81;
const INSTANCE_GENERIC: u8 = 82;
const INSTANCE_STR: u8 = 83;
const INSTANCE_FUNCTION: u8 = 84;
const INSTANCE_INT: u8 = 85;
const INSTANCE_BOOL: u8 = 86;
const INSTANCE_OBJECT: u8 = 87;
const TYPE_ALIAS_TYPE: u8 = 100;
const TYPE_VAR_TYPE: u8 = 101;
const PARAM_SPEC_TYPE: u8 = 102;
const TYPE_VAR_TUPLE_TYPE: u8 = 103;
const UNBOUND_TYPE: u8 = 104;
const UNPACK_TYPE: u8 = 105;
const ANY_TYPE: u8 = 106;
const UNINHABITED_TYPE: u8 = 107;
const NONE_TYPE: u8 = 108;
const DELETED_TYPE: u8 = 109;
const CALLABLE_TYPE: u8 = 110;
const OVERLOADED: u8 = 111;
const TUPLE_TYPE: u8 = 112;
const TYPED_DICT_TYPE: u8 = 113;
const LITERAL_TYPE: u8 = 114;
const UNION_TYPE: u8 = 115;
const TYPE_TYPE: u8 = 116;
const PARAMETERS: u8 = 117;
const EXTRA_ATTRS: u8 = 150;
const END_TAG: u8 = 255;

/// A growable byte buffer mirroring librt's `WriteBuffer` C type.
pub(crate) struct WriteBuffer {
    pub data: Vec<u8>,
}

impl WriteBuffer {
    fn new() -> Self {
        WriteBuffer { data: Vec::new() }
    }

    fn push(&mut self, byte: u8) {
        self.data.push(byte);
    }

    fn push_slice(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn push_u16_le(&mut self, val: u16) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    fn push_u32_le(&mut self, val: u32) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    fn push_f64_le(&mut self, val: f64) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    fn write_tag(&mut self, tag: u8) {
        self.push(tag);
    }

    fn write_bool(&mut self, val: bool) {
        self.push(if val { 1 } else { 0 });
    }

    fn write_short_int(&mut self, val: i64) {
        if (MIN_ONE_BYTE_INT..=MAX_ONE_BYTE_INT).contains(&val) {
            let encoded = ((val - MIN_ONE_BYTE_INT) << 1) as u8;
            self.push(encoded);
        } else if (MIN_TWO_BYTES_INT..=MAX_TWO_BYTES_INT).contains(&val) {
            let encoded = ((val - MIN_TWO_BYTES_INT) << 2) as u16 | TWO_BYTES_INT_BIT as u16;
            self.push_u16_le(encoded);
        } else {
            let encoded = ((val - MIN_FOUR_BYTES_INT) << 3) as u32 | FOUR_BYTES_INT_TRAILER as u32;
            self.push_u32_le(encoded);
        }
    }

    fn write_int_bare(&mut self, val: i64) {
        if (MIN_FOUR_BYTES_INT..=MAX_FOUR_BYTES_INT).contains(&val) {
            self.write_short_int(val);
        } else {
            self.write_long_int(val);
        }
    }

    fn write_long_int(&mut self, val: i64) {
        self.push(LONG_INT_TRAILER);
        let neg = val < 0;
        let mag = val.unsigned_abs();
        let bytes = mag.to_le_bytes();
        // Strip trailing zero bytes (little-endian → trailing high zeros).
        let mut size = 8;
        while size > 0 && bytes[size - 1] == 0 {
            size -= 1;
        }
        let encoded = ((size as i64) << 1) | (neg as i64);
        self.write_short_int(encoded);
        self.push_slice(&bytes[..size]);
    }

    fn write_str_bare(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_short_int(bytes.len() as i64);
        self.push_slice(bytes);
    }

    fn write_int(&mut self, val: i64) {
        self.write_tag(LITERAL_INT);
        self.write_int_bare(val);
    }

    fn write_str(&mut self, s: &str) {
        self.write_tag(LITERAL_STR);
        self.write_str_bare(s);
    }

    fn write_str_opt(&mut self, val: Option<&str>) {
        match val {
            Some(s) => self.write_str(s),
            None => self.write_tag(LITERAL_NONE),
        }
    }

    fn write_flags(&mut self, flags: &[bool]) {
        let mut packed: i64 = 0;
        for (i, &flag) in flags.iter().enumerate() {
            if flag {
                packed |= 1 << i;
            }
        }
        self.write_int(packed);
    }

    fn write_int_list(&mut self, list: &[i64]) {
        self.write_tag(LIST_INT);
        self.write_int_bare(list.len() as i64);
        for &item in list {
            self.write_int_bare(item);
        }
    }

    fn write_str_list(&mut self, list: &[String]) {
        self.write_tag(LIST_STR);
        self.write_int_bare(list.len() as i64);
        for item in list {
            self.write_str_bare(item);
        }
    }

    fn write_str_opt_list(&mut self, list: &[Option<String>]) {
        self.write_tag(LIST_GEN);
        self.write_int_bare(list.len() as i64);
        for item in list {
            self.write_str_opt(item.as_deref());
        }
    }
}

/// Extract a string attribute from a Python object.
fn getattr_str(_py: Python<'_>, obj: &PyAny, name: &str) -> PyResult<String> {
    obj.getattr(name)?.extract::<String>()
}

/// Extract an optional string attribute (None → None).
fn getattr_str_opt(_py: Python<'_>, obj: &PyAny, name: &str) -> PyResult<Option<String>> {
    let attr = obj.getattr(name)?;
    if attr.is_none() {
        Ok(None)
    } else {
        Ok(Some(attr.extract::<String>()?))
    }
}

/// Extract an i64 attribute from a Python object.
fn getattr_i64(_py: Python<'_>, obj: &PyAny, name: &str) -> PyResult<i64> {
    obj.getattr(name)?.extract::<i64>()
}

/// Extract a bool attribute from a Python object.
fn getattr_bool(_py: Python<'_>, obj: &PyAny, name: &str) -> PyResult<bool> {
    obj.getattr(name)?.extract::<bool>()
}

/// Get the `fullname` string from `obj.type` (Instance) or `obj.alias`
/// (TypeAliasType). Falls back to `obj.type_ref` if the live attribute
/// is not resolvable (FakeInfo / NOT_READY).
fn get_type_ref(_py: Python<'_>, obj: &PyAny, parent: &str) -> PyResult<String> {
    let parent_obj = obj.getattr(parent)?;
    match parent_obj.getattr("fullname") {
        Ok(fullname) => fullname.extract::<String>(),
        Err(_) => {
            // FakeInfo raises on .fullname access; use type_ref fallback.
            let type_ref = obj.getattr("type_ref")?;
            type_ref.extract::<String>()
        }
    }
}

/// Serialize a `Type` object into `buf`. Dispatches on `__class__.__name__`.
fn serialize_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    let class_name = obj.get_type().name().unwrap_or("");

    match class_name {
        "Instance" => serialize_instance(py, obj, buf),
        "TypeAliasType" => serialize_type_alias_type(py, obj, buf),
        "TypeVarType" => serialize_type_var_type(py, obj, buf),
        "ParamSpecType" => serialize_param_spec_type(py, obj, buf),
        "TypeVarTupleType" => serialize_type_var_tuple_type(py, obj, buf),
        "UnboundType" => serialize_unbound_type(py, obj, buf),
        "UnpackType" => serialize_unpack_type(py, obj, buf),
        "AnyType" => serialize_any_type(py, obj, buf),
        "UninhabitedType" => serialize_uninhabited_type(py, obj, buf),
        "NoneType" => serialize_none_type(py, obj, buf),
        "DeletedType" => serialize_deleted_type(py, obj, buf),
        "CallableType" => serialize_callable_type(py, obj, buf),
        "Overloaded" => serialize_overloaded(py, obj, buf),
        "TupleType" => serialize_tuple_type(py, obj, buf),
        "TypedDictType" => serialize_typed_dict_type(py, obj, buf),
        "LiteralType" => serialize_literal_type(py, obj, buf),
        "UnionType" => serialize_union_type(py, obj, buf),
        "TypeType" => serialize_type_type(py, obj, buf),
        "Parameters" => serialize_parameters(py, obj, buf),
        _ => {
            // Unknown / TypeGuardedType — raise so the Python caller
            // falls back to the pure-Python serializer.
            Err(pyo3::exceptions::PyNotImplementedError::new_err(format!(
                "rust_serialize_type: unhandled type class {class_name}"
            )))
        }
    }
}

/// Collect items from a Python list or tuple into a Vec.
fn collect_seq(obj: &PyAny) -> PyResult<Vec<&PyAny>> {
    if let Ok(list) = obj.downcast::<PyList>() {
        Ok(list.iter().collect())
    } else if let Ok(tuple) = obj.downcast::<PyTuple>() {
        Ok(tuple.iter().collect())
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected list or tuple",
        ))
    }
}

/// Serialize a type list/tuple (LIST_GEN + count + items).
fn serialize_type_list(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(LIST_GEN);
    let items = collect_seq(obj)?;
    buf.write_int_bare(items.len() as i64);
    for item in items {
        serialize_type(py, item, buf)?;
    }
    Ok(())
}

/// Serialize an optional type (None → LITERAL_NONE, else recurse).
fn serialize_type_opt(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    if obj.is_none() {
        buf.write_tag(LITERAL_NONE);
    } else {
        serialize_type(py, obj, buf)?;
    }
    Ok(())
}

/// Serialize a dict[str, Type] (DICT_STR_GEN + count + key/val pairs).
fn serialize_type_map(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(DICT_STR_GEN);
    let dict: &PyDict = obj.downcast::<PyDict>()?;
    buf.write_int_bare(dict.len() as i64);
    for (key, value) in dict.iter() {
        let key_str = key.extract::<String>()?;
        buf.write_str_bare(&key_str);
        serialize_type(py, value, buf)?;
    }
    Ok(())
}

/// Serialize an ExtraAttrs object.
fn serialize_extra_attrs(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(EXTRA_ATTRS);
    let attrs = obj.getattr("attrs")?;
    serialize_type_map(py, attrs, buf)?;
    let immutable = obj.getattr("immutable")?;
    let immutable_set: &PySet = immutable.downcast::<PySet>()?;
    let mut immutable_sorted: Vec<String> = immutable_set
        .iter()
        .map(|v| v.extract::<String>())
        .collect::<PyResult<_>>()?;
    immutable_sorted.sort();
    buf.write_str_list(&immutable_sorted);
    let mod_name = getattr_str_opt(py, obj, "mod_name")?;
    buf.write_str_opt(mod_name.as_deref());
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_instance(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(INSTANCE);
    let args = obj.getattr("args")?;
    let args_seq = collect_seq(args)?;
    let lkv = obj.getattr("last_known_value")?;
    let extra_attrs = obj.getattr("extra_attrs")?;

    if args_seq.is_empty() && lkv.is_none() && extra_attrs.is_none() {
        let type_ref = get_type_ref(py, obj, "type")?;
        match type_ref.as_str() {
            "builtins.str" => buf.write_tag(INSTANCE_STR),
            "builtins.function" => buf.write_tag(INSTANCE_FUNCTION),
            "builtins.int" => buf.write_tag(INSTANCE_INT),
            "builtins.bool" => buf.write_tag(INSTANCE_BOOL),
            "builtins.object" => buf.write_tag(INSTANCE_OBJECT),
            _ => {
                buf.write_tag(INSTANCE_SIMPLE);
                buf.write_str_bare(&type_ref);
            }
        }
        return Ok(());
    }

    buf.write_tag(INSTANCE_GENERIC);
    let type_ref = get_type_ref(py, obj, "type")?;
    buf.write_str(&type_ref);
    buf.write_tag(LIST_GEN);
    buf.write_int_bare(args_seq.len() as i64);
    for item in args_seq {
        serialize_type(py, item, buf)?;
    }
    serialize_type_opt(py, lkv, buf)?;
    if extra_attrs.is_none() {
        buf.write_tag(LITERAL_NONE);
    } else {
        serialize_extra_attrs(py, extra_attrs, buf)?;
    }
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_type_alias_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(TYPE_ALIAS_TYPE);
    let args = obj.getattr("args")?;
    serialize_type_list(py, args, buf)?;
    let type_ref = get_type_ref(py, obj, "alias")?;
    buf.write_str(&type_ref);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_type_var_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(TYPE_VAR_TYPE);
    buf.write_str(&getattr_str(py, obj, "name")?);
    buf.write_str(&getattr_str(py, obj, "fullname")?);
    let id = obj.getattr("id")?;
    buf.write_int(getattr_i64(py, id, "raw_id")?);
    buf.write_str(&getattr_str(py, id, "namespace")?);
    let values = obj.getattr("values")?;
    serialize_type_list(py, values, buf)?;
    let upper_bound = obj.getattr("upper_bound")?;
    serialize_type(py, upper_bound, buf)?;
    let default = obj.getattr("default")?;
    serialize_type(py, default, buf)?;
    buf.write_int(getattr_i64(py, obj, "variance")?);
    let meta_level = getattr_i64(py, id, "meta_level")?;
    if meta_level != 0 {
        buf.write_int(meta_level);
    }
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_param_spec_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(PARAM_SPEC_TYPE);
    let prefix = obj.getattr("prefix")?;
    serialize_parameters(py, prefix, buf)?;
    buf.write_str(&getattr_str(py, obj, "name")?);
    buf.write_str(&getattr_str(py, obj, "fullname")?);
    let id = obj.getattr("id")?;
    buf.write_int(getattr_i64(py, id, "raw_id")?);
    buf.write_str(&getattr_str(py, id, "namespace")?);
    buf.write_int(getattr_i64(py, obj, "flavor")?);
    let upper_bound = obj.getattr("upper_bound")?;
    serialize_type(py, upper_bound, buf)?;
    let default = obj.getattr("default")?;
    serialize_type(py, default, buf)?;
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_type_var_tuple_type(
    py: Python<'_>,
    obj: &PyAny,
    buf: &mut WriteBuffer,
) -> PyResult<()> {
    buf.write_tag(TYPE_VAR_TUPLE_TYPE);
    let tuple_fallback = obj.getattr("tuple_fallback")?;
    serialize_type(py, tuple_fallback, buf)?;
    buf.write_str(&getattr_str(py, obj, "name")?);
    buf.write_str(&getattr_str(py, obj, "fullname")?);
    let id = obj.getattr("id")?;
    buf.write_int(getattr_i64(py, id, "raw_id")?);
    buf.write_str(&getattr_str(py, id, "namespace")?);
    let upper_bound = obj.getattr("upper_bound")?;
    serialize_type(py, upper_bound, buf)?;
    let default = obj.getattr("default")?;
    serialize_type(py, default, buf)?;
    buf.write_int(getattr_i64(py, obj, "min_len")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_unbound_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(UNBOUND_TYPE);
    buf.write_str(&getattr_str(py, obj, "name")?);
    let args = obj.getattr("args")?;
    serialize_type_list(py, args, buf)?;
    let orig_expr = getattr_str_opt(py, obj, "original_str_expr")?;
    buf.write_str_opt(orig_expr.as_deref());
    let orig_fb = getattr_str_opt(py, obj, "original_str_fallback")?;
    buf.write_str_opt(orig_fb.as_deref());
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_unpack_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(UNPACK_TYPE);
    let typ = obj.getattr("type")?;
    serialize_type(py, typ, buf)?;
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_any_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(ANY_TYPE);
    let source_any = obj.getattr("source_any")?;
    serialize_type_opt(py, source_any, buf)?;
    buf.write_int(getattr_i64(py, obj, "type_of_any")?);
    let missing = getattr_str_opt(py, obj, "missing_import_name")?;
    buf.write_str_opt(missing.as_deref());
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_uninhabited_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(UNINHABITED_TYPE);
    buf.write_bool(getattr_bool(py, obj, "ambiguous")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_none_type(_py: Python<'_>, _obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(NONE_TYPE);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_deleted_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(DELETED_TYPE);
    let source = getattr_str_opt(py, obj, "source")?;
    buf.write_str_opt(source.as_deref());
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_callable_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(CALLABLE_TYPE);
    let fallback = obj.getattr("fallback")?;
    serialize_type(py, fallback, buf)?;
    let instance_type = obj.getattr("instance_type")?;
    serialize_type_opt(py, instance_type, buf)?;
    buf.write_flags(&[
        getattr_bool(py, obj, "is_ellipsis_args")?,
        getattr_bool(py, obj, "implicit")?,
        getattr_bool(py, obj, "is_bound")?,
        getattr_bool(py, obj, "from_concatenate")?,
        getattr_bool(py, obj, "imprecise_arg_kinds")?,
        getattr_bool(py, obj, "unpack_kwargs")?,
    ]);
    let arg_types = obj.getattr("arg_types")?;
    serialize_type_list(py, arg_types, buf)?;
    let arg_kinds = obj.getattr("arg_kinds")?;
    let kinds_seq = collect_seq(arg_kinds)?;
    let mut kind_vals = Vec::with_capacity(kinds_seq.len());
    for kind in kinds_seq {
        let val = kind.getattr("value")?.extract::<i64>()?;
        kind_vals.push(val);
    }
    buf.write_int_list(&kind_vals);
    let arg_names = obj.getattr("arg_names")?;
    let names_seq = collect_seq(arg_names)?;
    let mut name_vals = Vec::with_capacity(names_seq.len());
    for name in names_seq {
        if name.is_none() {
            name_vals.push(None);
        } else {
            name_vals.push(Some(name.extract::<String>()?));
        }
    }
    buf.write_str_opt_list(&name_vals);
    let ret_type = obj.getattr("ret_type")?;
    serialize_type(py, ret_type, buf)?;
    let name = getattr_str_opt(py, obj, "name")?;
    buf.write_str_opt(name.as_deref());
    let variables = obj.getattr("variables")?;
    serialize_type_list(py, variables, buf)?;
    let type_guard = obj.getattr("type_guard")?;
    serialize_type_opt(py, type_guard, buf)?;
    let type_is = obj.getattr("type_is")?;
    serialize_type_opt(py, type_is, buf)?;
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_overloaded(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(OVERLOADED);
    let items = obj.getattr("items")?;
    serialize_type_list(py, items, buf)?;
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_tuple_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(TUPLE_TYPE);
    let partial_fallback = obj.getattr("partial_fallback")?;
    serialize_type(py, partial_fallback, buf)?;
    let items = obj.getattr("items")?;
    serialize_type_list(py, items, buf)?;
    buf.write_bool(getattr_bool(py, obj, "implicit")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_typed_dict_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(TYPED_DICT_TYPE);
    let fallback = obj.getattr("fallback")?;
    serialize_type(py, fallback, buf)?;
    let items = obj.getattr("items")?;
    serialize_type_map(py, items, buf)?;
    let required_keys = obj.getattr("required_keys")?;
    let req_set: &PySet = required_keys.downcast::<PySet>()?;
    let mut req_sorted: Vec<String> = req_set
        .iter()
        .map(|v| v.extract::<String>())
        .collect::<PyResult<_>>()?;
    req_sorted.sort();
    buf.write_str_list(&req_sorted);
    let readonly_keys = obj.getattr("readonly_keys")?;
    let ro_set: &PySet = readonly_keys.downcast::<PySet>()?;
    let mut ro_sorted: Vec<String> = ro_set
        .iter()
        .map(|v| v.extract::<String>())
        .collect::<PyResult<_>>()?;
    ro_sorted.sort();
    buf.write_str_list(&ro_sorted);
    buf.write_bool(getattr_bool(py, obj, "is_closed")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_literal_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(LITERAL_TYPE);
    let fallback = obj.getattr("fallback")?;
    serialize_type(py, fallback, buf)?;
    let value = obj.getattr("value")?;
    write_literal_value(py, value, buf)?;
    buf.write_tag(END_TAG);
    Ok(())
}

fn write_literal_value(_py: Python<'_>, value: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    if value.is_none() {
        buf.write_tag(LITERAL_NONE);
    } else if let Ok(b) = value.downcast::<PyBool>() {
        buf.write_bool(b.is_true());
    } else if let Ok(n) = value.extract::<i64>() {
        buf.write_tag(LITERAL_INT);
        buf.write_int_bare(n);
    } else if let Ok(s) = value.extract::<String>() {
        buf.write_tag(LITERAL_STR);
        buf.write_str_bare(&s);
    } else if let Ok(f) = value.downcast::<PyFloat>() {
        buf.write_tag(LITERAL_FLOAT);
        buf.push_f64_le(f.value());
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "rust_serialize_type: unhandled literal value type",
        ));
    }
    Ok(())
}

fn serialize_union_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(UNION_TYPE);
    let items = obj.getattr("items")?;
    serialize_type_list(py, items, buf)?;
    buf.write_bool(getattr_bool(py, obj, "uses_pep604_syntax")?);
    buf.write_bool(getattr_bool(py, obj, "can_be_true")?);
    buf.write_bool(getattr_bool(py, obj, "can_be_false")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_type_type(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(TYPE_TYPE);
    let item = obj.getattr("item")?;
    serialize_type(py, item, buf)?;
    buf.write_bool(getattr_bool(py, obj, "is_type_form")?);
    buf.write_tag(END_TAG);
    Ok(())
}

fn serialize_parameters(py: Python<'_>, obj: &PyAny, buf: &mut WriteBuffer) -> PyResult<()> {
    buf.write_tag(PARAMETERS);
    let arg_types = obj.getattr("arg_types")?;
    serialize_type_list(py, arg_types, buf)?;
    let arg_kinds = obj.getattr("arg_kinds")?;
    let kinds_seq = collect_seq(arg_kinds)?;
    let mut kind_vals = Vec::with_capacity(kinds_seq.len());
    for kind in kinds_seq {
        let val = kind.getattr("value")?.extract::<i64>()?;
        kind_vals.push(val);
    }
    buf.write_int_list(&kind_vals);
    let arg_names = obj.getattr("arg_names")?;
    let names_seq = collect_seq(arg_names)?;
    let mut name_vals = Vec::with_capacity(names_seq.len());
    for name in names_seq {
        if name.is_none() {
            name_vals.push(None);
        } else {
            name_vals.push(Some(name.extract::<String>()?));
        }
    }
    buf.write_str_opt_list(&name_vals);
    let variables = obj.getattr("variables")?;
    serialize_type_list(py, variables, buf)?;
    buf.write_bool(getattr_bool(py, obj, "imprecise_arg_kinds")?);
    buf.write_tag(END_TAG);
    Ok(())
}

/// PyO3 entry point: serialize a live `mypy.types.Type` to wire bytes.
#[pyfunction]
pub(crate) fn rust_serialize_type(py: Python<'_>, obj: &PyAny) -> PyResult<PyObject> {
    let mut buf = WriteBuffer::new();
    serialize_type(py, obj, &mut buf)?;
    Ok(pyo3::types::PyBytes::new(py, &buf.data).into())
}
