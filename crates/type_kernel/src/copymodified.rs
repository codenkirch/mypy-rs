//! Native `copy_modified` port — field-level type-swap on the wire-format
//! `Type` enum (issue #475).
//!
//! Mirrors `mypy.types.Type.copy_modified(**changes)` as a single Rust
//! function:
//!
//! ```text
//! rust_copy_modified(type_bytes, field, value_bytes) -> Option<Vec<u8>>
//! ```
//!
//! `type_bytes` is the wire-format blob of the original type, `field` the
//! name of the field to replace, and `value_bytes` the wire-format blob of
//! the replacement value (booleans encode as a single 0/1 byte). The result
//! is the wire-format blob of the modified type, or `None` to defer to the
//! pure-Python `copy_modified` (strangler-fig per-call gate).
//!
//! Supported swaps:
//!   * Instance:     `args` (list<Type>), `last_known_value` (Type|None)
//!   * CallableType: `arg_types`, `arg_kinds`, `arg_names`, `ret_type`,
//!     `fallback`, `type_guard`, `type_is`
//!   * TupleType:    `fallback`, `items`, `implicit`
//!   * TypedDictType:`fallback`, `items`, `required_keys`, `readonly_keys`,
//!     `is_closed`
//!   * UnionType:    `items`
//!   * TypeType:     `item`
//!   * LiteralType:  `fallback`
//!
//! Value blobs use the same wire layout as the field inside the enclosing
//! type: a type is a full tagged `Type` record, a list<Type> is
//! `LIST_GEN + size + N types`, arg_names is `LIST_GEN + size + N
//! (LITERAL_NONE | LITERAL_STR + bare str)`, arg_kinds /
//! required_keys / readonly_keys are `LIST_INT / LIST_STR + size + N bare
//! values`, bool is a single byte.
//!
//! Every failure (unknown class, unknown field, malformed blob, or a type
//! the writer cannot re-encode, e.g. TypeAliasType) yields `None` so the
//! Python caller falls back to Python.

use pyo3::prelude::*;

use crate::wire::{
    read_bool, read_int_bare, read_str_bare, read_tag, read_type, read_type_list, write_type, Type,
    WriteBuffer, DICT_STR_GEN, LIST_GEN, LIST_INT, LIST_STR, LITERAL_NONE, LITERAL_STR,
};

#[cfg(test)]
use crate::wire::{write_int_bare, write_str_bare, write_str_opt, write_tag};

// ---------------------------------------------------------------------------
// Wire-read helpers for replacement values
// ---------------------------------------------------------------------------

/// Read one full tagged type from a value blob.
fn read_type_blob(bytes: &[u8]) -> Option<Type> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Read `LITERAL_NONE` | tagged type from a value blob (write_type_opt).
fn read_type_opt_blob(bytes: &[u8]) -> Option<Option<Type>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag == LITERAL_NONE {
        return Some(None);
    }
    read_type(&mut buf, Some(tag)).ok().map(Some)
}

/// Read `LIST_GEN + size + N read_str_opt` (arg_names).
fn read_str_opt_list_blob(bytes: &[u8]) -> Option<Vec<Option<String>>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag != LIST_GEN {
        return None;
    }
    let size = read_int_bare(&mut buf).ok()?;
    if size < 0 {
        return None;
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let inner = read_tag(&mut buf).ok()?;
        if inner == LITERAL_NONE {
            out.push(None);
        } else if inner == LITERAL_STR {
            out.push(Some(read_str_bare(&mut buf).ok()?));
        } else {
            return None;
        }
    }
    Some(out)
}

/// Read `LIST_INT + size + N bare ints` (arg_kinds) or
/// `LIST_STR + size + N bare strs` (required_keys / readonly_keys).
fn read_scalar_list_blob(bytes: &[u8], list_tag: u8) -> Option<Vec<String>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag != list_tag {
        return None;
    }
    let size = read_int_bare(&mut buf).ok()?;
    if size < 0 {
        return None;
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_str_bare(&mut buf).ok()?);
    }
    Some(out)
}

/// Read `DICT_STR_GEN + size + N (bare str, tagged type)` (TypedDict items).
fn read_type_map_blob(bytes: &[u8]) -> Option<Vec<(String, Type)>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag != DICT_STR_GEN {
        return None;
    }
    let size = read_int_bare(&mut buf).ok()?;
    if size < 0 {
        return None;
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let key = read_str_bare(&mut buf).ok()?;
        let value = read_type(&mut buf, None).ok()?;
        out.push((key, value));
    }
    Some(out)
}

/// Read `LIST_GEN + size + N types` (arg_types, TupleType items, etc.).
fn read_type_list_blob(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    read_type_list(&mut buf).ok()
}

/// Read a single 0/1 byte (a bool value).
fn read_bool_blob(bytes: &[u8]) -> Option<bool> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    read_bool(&mut buf).ok()
}

/// Read `LIST_INT + size + N bare ints` (arg_kinds, as i64).
fn read_int_list_blob(bytes: &[u8]) -> Option<Vec<i64>> {
    let mut buf = crate::wire::ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag != LIST_INT {
        return None;
    }
    let size = read_int_bare(&mut buf).ok()?;
    if size < 0 {
        return None;
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_int_bare(&mut buf).ok()?);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Re-encoders for replacement values (mirror the Python write helpers).
// Only used by unit tests: in production the value blob is built by Python.

// ---------------------------------------------------------------------------

#[cfg(test)]
fn write_str_opt_list(
    buf: &mut WriteBuffer,
    items: &[Option<String>],
) -> Result<(), crate::wire::WireError> {
    write_tag(buf, LIST_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_str_opt(buf, item.as_deref())?;
    }
    Ok(())
}

#[cfg(test)]
fn write_int_list(buf: &mut WriteBuffer, items: &[i64]) -> Result<(), crate::wire::WireError> {
    write_tag(buf, LIST_INT);
    write_int_bare(buf, items.len() as i64)?;
    for &item in items {
        write_int_bare(buf, item)?;
    }
    Ok(())
}

#[cfg(test)]
fn write_str_list(buf: &mut WriteBuffer, items: &[String]) -> Result<(), crate::wire::WireError> {
    write_tag(buf, LIST_STR);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_str_bare(buf, item)?;
    }
    Ok(())
}

#[cfg(test)]
fn write_type_map(
    buf: &mut WriteBuffer,
    items: &[(String, Type)],
) -> Result<(), crate::wire::WireError> {
    write_tag(buf, DICT_STR_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for (key, value) in items {
        write_str_bare(buf, key)?;
        write_type(buf, value)?;
    }
    Ok(())
}

#[cfg(test)]
fn write_type_list(buf: &mut WriteBuffer, items: &[Type]) -> Result<(), crate::wire::WireError> {
    write_tag(buf, LIST_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_type(buf, item)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Re-encode a modified `Type` (discarding truthiness flags, which the
/// caller re-applies in Python). `None` when the wire writer cannot encode
/// the result.
fn re_encode(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// Apply one field swap to a decoded type.
fn swap_field(t: &mut Type, field: &str, value_bytes: &[u8]) -> Option<()> {
    match t {
        Type::Instance {
            args,
            last_known_value,
            ..
        } => match field {
            "args" => {
                *args = read_type_list_blob(value_bytes)?;
                Some(())
            }
            "last_known_value" => {
                *last_known_value = read_type_opt_blob(value_bytes)?.map(Box::new);
                Some(())
            }
            _ => None,
        },
        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            fallback,
            type_guard,
            type_is,
            ..
        } => match field {
            "arg_types" => {
                *arg_types = read_type_list_blob(value_bytes)?;
                Some(())
            }
            "arg_kinds" => {
                *arg_kinds = read_int_list_blob(value_bytes)?;
                Some(())
            }
            "arg_names" => {
                *arg_names = read_str_opt_list_blob(value_bytes)?;
                Some(())
            }
            "ret_type" => {
                **ret_type = read_type_blob(value_bytes)?;
                Some(())
            }
            "fallback" => {
                **fallback = read_type_blob(value_bytes)?;
                Some(())
            }
            "type_guard" => {
                *type_guard = read_type_opt_blob(value_bytes)?.map(Box::new);
                Some(())
            }
            "type_is" => {
                *type_is = read_type_opt_blob(value_bytes)?.map(Box::new);
                Some(())
            }
            _ => None,
        },
        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => match field {
            "fallback" => {
                **partial_fallback = read_type_blob(value_bytes)?;
                Some(())
            }
            "items" => {
                *items = read_type_list_blob(value_bytes)?;
                Some(())
            }
            "implicit" => {
                *implicit = read_bool_blob(value_bytes)?;
                Some(())
            }
            _ => None,
        },
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => match field {
            "fallback" => {
                **fallback = read_type_blob(value_bytes)?;
                Some(())
            }
            "items" => {
                *items = read_type_map_blob(value_bytes)?;
                Some(())
            }
            "required_keys" => {
                *required_keys = read_scalar_list_blob(value_bytes, LIST_STR)?
                    .into_iter()
                    .collect();
                Some(())
            }
            "readonly_keys" => {
                *readonly_keys = read_scalar_list_blob(value_bytes, LIST_STR)?
                    .into_iter()
                    .collect();
                Some(())
            }
            "is_closed" => {
                *is_closed = read_bool_blob(value_bytes)?;
                Some(())
            }
            _ => None,
        },
        Type::UnionType { items, .. } => match field {
            "items" => {
                *items = read_type_list_blob(value_bytes)?;
                Some(())
            }
            _ => None,
        },
        Type::TypeType { item, .. } => match field {
            "item" => {
                **item = read_type_blob(value_bytes)?;
                Some(())
            }
            _ => None,
        },
        Type::LiteralType { fallback, .. } => match field {
            "fallback" => {
                **fallback = read_type_blob(value_bytes)?;
                Some(())
            }
            _ => None,
        },
        _ => None,
    }
}

/// `rust_copy_modified` — swap one field of a wire-format type (issue #475).
///
/// Returns the modified wire-format blob, or `None` for any unsupported
/// class/field or malformed input so the caller falls back to Python.
#[pyfunction]
pub(crate) fn rust_copy_modified(
    type_bytes: &[u8],
    field: &str,
    value_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let mut typ = match read_type_blob(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    if swap_field(&mut typ, field, value_bytes).is_none() {
        return Ok(None);
    }
    Ok(re_encode(&typ))
}
// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::read_type;
    use std::collections::HashSet;

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn int_instance() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_callable() -> Type {
        Type::CallableType {
            fallback: Box::new(int_instance()),
            instance_type: None,
            is_ellipsis_args: true,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![any_type()],
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(any_type()),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn make_tuple() -> Type {
        Type::TupleType {
            partial_fallback: Box::new(int_instance()),
            items: vec![any_type()],
            implicit: false,
        }
    }

    fn make_typeddict() -> Type {
        Type::TypedDictType {
            fallback: Box::new(int_instance()),
            items: vec![("a".to_string(), any_type())],
            required_keys: ["a".to_string()].into(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        }
    }

    fn encode_type(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    fn encode_bool(v: bool) -> Vec<u8> {
        vec![if v { 1 } else { 0 }]
    }

    fn encode_int_list(values: &[i64]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_int_list(&mut buf, values).unwrap();
        buf.into_bytes()
    }

    fn encode_str_opt_list(values: &[Option<String>]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_str_opt_list(&mut buf, values).unwrap();
        buf.into_bytes()
    }

    fn encode_str_list(values: &[String]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_str_list(&mut buf, values).unwrap();
        buf.into_bytes()
    }

    fn encode_type_list(values: &[Type]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type_list(&mut buf, values).unwrap();
        buf.into_bytes()
    }

    fn encode_type_map(values: &[(String, Type)]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type_map(&mut buf, values).unwrap();
        buf.into_bytes()
    }

    fn decode(bytes: &[u8]) -> Type {
        let mut buf = crate::wire::ReadBuffer::new(bytes);
        read_type(&mut buf, None).unwrap()
    }

    #[test]
    fn instance_args() {
        let t = encode_type(&int_instance());
        let replacement = vec![any_type(), any_type()];
        let value = encode_type_list(&replacement);
        let out = rust_copy_modified(&t, "args", &value).unwrap().unwrap();
        match decode(&out) {
            Type::Instance { args, .. } => assert_eq!(args.len(), 2),
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn instance_last_known_value() {
        let t = encode_type(&int_instance());
        let value = encode_type(&any_type());
        let out = rust_copy_modified(&t, "last_known_value", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::Instance {
                last_known_value, ..
            } => assert!(last_known_value.is_some()),
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn instance_last_known_value_none() {
        let t = encode_type(&int_instance());
        let value = vec![LITERAL_NONE];
        let out = rust_copy_modified(&t, "last_known_value", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::Instance {
                last_known_value, ..
            } => assert!(last_known_value.is_none()),
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn callable_arg_types() {
        let t = encode_type(&make_callable());
        let value = encode_type_list(&[any_type(), any_type(), any_type()]);
        let out = rust_copy_modified(&t, "arg_types", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::CallableType { arg_types, .. } => assert_eq!(arg_types.len(), 3),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_ret_type() {
        let t = encode_type(&make_callable());
        let value = encode_type(&int_instance());
        let out = rust_copy_modified(&t, "ret_type", &value).unwrap().unwrap();
        match decode(&out) {
            Type::CallableType { ret_type, .. } => {
                assert!(matches!(ret_type.as_ref(), Type::Instance { .. }))
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_arg_kinds() {
        let t = encode_type(&make_callable());
        let value = encode_int_list(&[0, 1, 2]);
        let out = rust_copy_modified(&t, "arg_kinds", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::CallableType { arg_kinds, .. } => assert_eq!(arg_kinds, vec![0, 1, 2]),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_arg_names() {
        let t = encode_type(&make_callable());
        let value = encode_str_opt_list(&[Some("x".to_string()), None]);
        let out = rust_copy_modified(&t, "arg_names", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::CallableType { arg_names, .. } => {
                assert_eq!(arg_names, vec![Some("x".to_string()), None])
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_type_guard() {
        let t = encode_type(&make_callable());
        let value = encode_type(&any_type());
        let out = rust_copy_modified(&t, "type_guard", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::CallableType { type_guard, .. } => assert!(type_guard.is_some()),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_type_is() {
        let t = encode_type(&make_callable());
        let value = encode_type(&any_type());
        let out = rust_copy_modified(&t, "type_is", &value).unwrap().unwrap();
        match decode(&out) {
            Type::CallableType { type_is, .. } => assert!(type_is.is_some()),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn callable_fallback() {
        let t = encode_type(&make_callable());
        let value = encode_type(&int_instance());
        let out = rust_copy_modified(&t, "fallback", &value).unwrap().unwrap();
        match decode(&out) {
            Type::CallableType { fallback, .. } => {
                assert!(matches!(fallback.as_ref(), Type::Instance { .. }))
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn tuple_items() {
        let t = encode_type(&make_tuple());
        let value = encode_type_list(&[any_type(), any_type()]);
        let out = rust_copy_modified(&t, "items", &value).unwrap().unwrap();
        match decode(&out) {
            Type::TupleType { items, .. } => assert_eq!(items.len(), 2),
            _ => panic!("expected TupleType"),
        }
    }

    #[test]
    fn tuple_implicit() {
        let t = encode_type(&make_tuple());
        let value = encode_bool(true);
        let out = rust_copy_modified(&t, "implicit", &value).unwrap().unwrap();
        match decode(&out) {
            Type::TupleType { implicit, .. } => assert!(implicit),
            _ => panic!("expected TupleType"),
        }
    }

    #[test]
    fn typeddict_items() {
        let t = encode_type(&make_typeddict());
        let value =
            encode_type_map(&[("a".to_string(), any_type()), ("b".to_string(), any_type())]);
        let out = rust_copy_modified(&t, "items", &value).unwrap().unwrap();
        match decode(&out) {
            Type::TypedDictType { items, .. } => assert_eq!(items.len(), 2),
            _ => panic!("expected TypedDictType"),
        }
    }

    #[test]
    fn typeddict_required_keys() {
        let t = encode_type(&make_typeddict());
        let value = encode_str_list(&["a".to_string(), "b".to_string()]);
        let out = rust_copy_modified(&t, "required_keys", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::TypedDictType { required_keys, .. } => {
                assert_eq!(required_keys.len(), 2);
                assert!(required_keys.contains("b"))
            }
            _ => panic!("expected TypedDictType"),
        }
    }

    #[test]
    fn typeddict_is_closed() {
        let t = encode_type(&make_typeddict());
        let value = encode_bool(true);
        let out = rust_copy_modified(&t, "is_closed", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::TypedDictType { is_closed, .. } => assert!(is_closed),
            _ => panic!("expected TypedDictType"),
        }
    }

    #[test]
    fn union_items() {
        let t = Type::UnionType {
            items: vec![any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let bytes = encode_type(&t);
        let value = encode_type_list(&[any_type(), any_type()]);
        let out = rust_copy_modified(&bytes, "items", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::UnionType { items, .. } => assert_eq!(items.len(), 2),
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn type_type_item() {
        let t = Type::TypeType {
            item: Box::new(any_type()),
            is_type_form: false,
        };
        let bytes = encode_type(&t);
        let value = encode_type(&int_instance());
        let out = rust_copy_modified(&bytes, "item", &value).unwrap().unwrap();
        match decode(&out) {
            Type::TypeType { item, .. } => {
                assert!(matches!(item.as_ref(), Type::Instance { .. }))
            }
            _ => panic!("expected TypeType"),
        }
    }

    #[test]
    fn literal_fallback() {
        let t = Type::LiteralType {
            fallback: Box::new(int_instance()),
            value: crate::wire::LiteralValue::Int(5),
        };
        let bytes = encode_type(&t);
        let value = encode_type(&int_instance());
        let out = rust_copy_modified(&bytes, "fallback", &value)
            .unwrap()
            .unwrap();
        match decode(&out) {
            Type::LiteralType { fallback, .. } => {
                assert!(matches!(fallback.as_ref(), Type::Instance { .. }))
            }
            _ => panic!("expected LiteralType"),
        }
    }

    #[test]
    fn unsupported_class_returns_none() {
        let t = encode_type(&Type::NoneType);
        let value = encode_type(&any_type());
        assert!(rust_copy_modified(&t, "items", &value).unwrap().is_none());
    }

    #[test]
    fn unsupported_field_returns_none() {
        let t = encode_type(&int_instance());
        let value = encode_type(&any_type());
        assert!(rust_copy_modified(&t, "bogus", &value).unwrap().is_none());
    }

    #[test]
    fn malformed_input_returns_none() {
        let t = encode_type(&int_instance());
        assert!(rust_copy_modified(&t, "args", &[0xFF, 0x01])
            .unwrap()
            .is_none());
        assert!(rust_copy_modified(&[0xFF], "args", &[]).unwrap().is_none());
    }

    #[test]
    fn type_alias_type_returns_none() {
        // TypeAliasType cannot be written to the wire format, so the caller
        // serializes it first; the decode fails and we defer to Python.
        let value = encode_type(&any_type());
        assert!(rust_copy_modified(&[0xFF], "args", &value)
            .unwrap()
            .is_none());
    }
}
