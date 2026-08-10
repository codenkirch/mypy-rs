//! Dataclasses plugin `__init__`/`__post_init__` transform seams
//! (Issue #356, completed by #393).
//!
//! Receives serialized `@dataclass` field metadata, computes the `__init__`
//! argument list (names and kinds) — and the `__post_init__` argument list
//! (the `dataclasses.InitVar` fields only) — and returns wire-encoded result
//! bytes. Python applies the AST mutation (`add_method_to_class`) using the
//! deserialized result. The Python caller fully validates the result against
//! its own computation before applying it, so a Rust bug cannot silently
//! change semantics.
//!
//! Strangler-fig contract: unsupported inputs return `None`; Python keeps
//! its full implementation for those cases. This mirrors the attrs seam
//! (`crates/type_kernel/src/attrs.rs`, Issue #357); the dataclass seam is
//! narrower: it only computes argument names and kinds, no types.

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::wire::{read_bool, read_str_bare, ReadBuffer, WireError, WriteBuffer};

// Tag bytes for the dataclass wire format. Values in the 200s avoid any
// collision with wire.rs type tags (which stay below 200).
const DC_FIELD: u8 = 210; // serialized field descriptor
const DC_INIT_SIG: u8 = 211; // serialized __init__ signature result
const NONE_TYPE: u8 = 108; // lib.rs-wire NONE_TYPE
const END_TAG: u8 = 255; // lib.rs-wire END_TAG

// Argument kinds as mypy.nodes.ArgumentKind ordinals; see
// `mypy/plugins/dataclasses.py` `to_argument` and `Argument.__init__`.
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_NAMED: i64 = 3;
const ARG_NAMED_OPT: i64 = 5;

/// A single `@dataclass` field, received from Python serialized as bytes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Field {
    pub name: String,
    pub alias: String,
    pub has_default: bool,
    pub kw_only: bool,
    pub is_in_init: bool,
    pub is_init_var: bool,
}

/// Write a single byte to the wire buffer (tag-like values use this instead
/// of `write_int_bare`, which would encode a varint).
fn write_u8(buf: &mut WriteBuffer, byte: u8) {
    buf.push(byte);
}

/// Read one field descriptor. Returns `None` on any malformed byte; the
/// caller treats that as "unsupported, fall back to Python".
fn read_field(buf: &mut ReadBuffer<'_>) -> Option<Field> {
    let name = read_str_bare(buf).ok()?;
    let alias = read_str_bare(buf).ok()?;
    let has_default = read_bool(buf).ok()?;
    let kw_only = read_bool(buf).ok()?;
    let is_in_init = read_bool(buf).ok()?;
    let is_init_var = read_bool(buf).ok()?;
    Some(Field {
        name,
        alias,
        has_default,
        kw_only,
        is_in_init,
        is_init_var,
    })
}

/// Deserialize the full field list (count + tagged fields). Returns `None`
/// if the count looks unreasonable or a field tag is wrong. A first-byte
/// `LONG_INT_TRAILER` on the count means Python could not encode it; that
/// also falls back to Python.
fn deserialize_fields(buf: &mut ReadBuffer<'_>) -> Option<Vec<Field>> {
    let first = buf.read_u8().ok()?;
    const LONG_INT_TRAILER: u8 = 15; // wire.rs long-int sentinel
    if first == LONG_INT_TRAILER {
        return None;
    }
    let count = crate::wire::read_short_int(buf, first).ok()?;
    if !(0..=1000).contains(&count) {
        return None;
    }
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let tag = buf.read_u8().ok()?;
        if tag != DC_FIELD {
            return None;
        }
        fields.push(read_field(buf)?);
    }
    Some(fields)
}

/// Compute the `__init__` argument list for the given fields.
///
/// Mirrors `mypy/plugins/dataclasses.py` `to_argument` (of="__init__"),
/// driven through the dataclasses `__init__` list-comprehension order:
/// each attribute appears exactly once, in attribute order, using
/// `attr.alias or attr.name` as the argument name. Kinds only, no types.
fn compute_init_signature(fields: &[Field]) -> Result<Vec<(String, i64)>, WireError> {
    let mut args: Vec<(String, i64)> = Vec::with_capacity(fields.len());
    for f in fields {
        let arg_name = if f.alias.is_empty() {
            f.name.clone()
        } else {
            f.alias.clone()
        };
        let kind = if f.kw_only {
            if f.has_default {
                ARG_NAMED_OPT
            } else {
                ARG_NAMED
            }
        } else if f.has_default {
            ARG_OPT
        } else {
            ARG_POS
        };
        args.push((arg_name, kind));
    }
    Ok(args)
}

// ---------------------------------------------------------------------------
// PyO3 entry point
// ---------------------------------------------------------------------------

/// `rust_dataclass_transform` — the dataclass `__init__` signature seam.
///
/// Python serializes the filtered `DataclassAttribute` list (those with
/// `is_in_init` and without a `dataclasses.KW_ONLY` type) into `fields_bytes`
/// and receives back wire-encoded result bytes. The result is only a list of
/// argument names and kinds; Python validates it against its own
/// computation, so the Rust path is a compute-offload, not an authority.
///
/// Returns `Some(Py<PyBytes>)` of serialized signature, or `None` for
/// unsupported cases (falls back to Python). `class_fullname` and the
/// decorator flags are accepted for ABI symmetry with the fields Python
/// already uses for the full transform; they do not affect the `__init__`
/// args.
#[pyfunction]
#[pyo3(name = "rust_dataclass_transform")]
pub fn rust_dataclass_transform(
    py: Python<'_>,
    fields_bytes: &[u8],
    _class_fullname: &str,
    _decorator_init: bool,
    _decorator_eq: bool,
    _decorator_order: bool,
    _decorator_frozen: bool,
) -> PyResult<Option<Py<PyBytes>>> {
    let mut buf = ReadBuffer::new(fields_bytes);
    let fields = match deserialize_fields(&mut buf) {
        Some(f) => f,
        None => return Ok(None),
    };
    let args = match compute_init_signature(&fields) {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let bytes = encode_signature(&args);
    Ok(Some(PyBytes::new(py, &bytes).into()))
}

/// Compute the `__post_init__` argument list for the given fields.
///
/// Mirrors `mypy/plugins/dataclasses.py` `to_argument (of="__post_init__")`
/// as used by `_add_internal_post_init_method`: only the `InitVar` fields,
/// in attribute order, all as `ARG_POS` without defaults. The runtime
/// `__post_init__` protocol resolves each value from the field default, so a
/// parameter default would over-constrain direct calls.
fn compute_post_init_signature(fields: &[Field]) -> Vec<(String, i64)> {
    fields
        .iter()
        .filter(|f| f.is_init_var)
        .map(|f| {
            let arg_name = if f.alias.is_empty() {
                f.name.clone()
            } else {
                f.alias.clone()
            };
            (arg_name, ARG_POS)
        })
        .collect()
}

/// `rust_dataclass_post_init_transform` — the dataclass `__post_init__`
/// signature seam.
///
/// Same wire contract as `rust_dataclass_transform` (this is a sibling of
/// that function, not a wire-format extension). Python serializes the full
/// `DataclassAttribute` list; Rust selects the `InitVar` fields and emits
/// their argument names and kinds, all `ARG_POS`. Python validates the
/// result against its own computation and applies the AST mutation, so a
/// Rust bug cannot silently change semantics.
///
/// Returns `Some(Py<PyBytes>)` of serialized signature, or `None` for
/// unsupported cases (falls back to Python). `class_fullname` is accepted
/// for ABI symmetry and does not affect the result.
#[pyfunction]
#[pyo3(name = "rust_dataclass_post_init_transform")]
pub fn rust_dataclass_post_init_transform(
    py: Python<'_>,
    fields_bytes: &[u8],
    _class_fullname: &str,
) -> PyResult<Option<Py<PyBytes>>> {
    let mut buf = ReadBuffer::new(fields_bytes);
    let fields = match deserialize_fields(&mut buf) {
        Some(f) => f,
        None => return Ok(None),
    };
    let args = compute_post_init_signature(&fields);
    let bytes = encode_signature(&args);
    Ok(Some(PyBytes::new(py, &bytes).into()))
}

/// Encode a computed argument list in the dataclass signature wire format.
///
/// Output: DC_INIT_SIG + len + [DC_INIT_SIG + n_args + per-arg
/// (name, kind)] + returns-flag + NONE_TYPE + END_TAG. Shared by the
/// `__init__` and `__post_init__` seams so both emit byte-identical
/// envelopes; only the arg computation differs.
fn encode_signature(args: &[(String, i64)]) -> Vec<u8> {
    let mut body = WriteBuffer::new();
    write_u8(&mut body, DC_INIT_SIG);
    crate::wire::write_int_bare(&mut body, args.len() as i64).expect("arg count fits short-int");
    for (name, kind) in args {
        let name_bytes = name.as_bytes();
        crate::wire::write_int_bare(&mut body, name_bytes.len() as i64)
            .expect("name length fits short-int");
        body.extend(name_bytes);
        crate::wire::write_int_bare(&mut body, *kind).expect("kind fits short-int");
    }
    write_u8(&mut body, 0); // returns_flag: no annotation
    write_u8(&mut body, NONE_TYPE);
    write_u8(&mut body, END_TAG);

    let body_bytes = body.into_bytes();
    let mut result = WriteBuffer::new();
    write_u8(&mut result, DC_INIT_SIG);
    crate::wire::write_int_bare(&mut result, body_bytes.len() as i64)
        .expect("body length fits short-int");
    result.extend(&body_bytes);
    result.into_bytes()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::read_int_bare;

    /// Serialize a field list in the exact layout Python produces
    /// (see `mypy/plugins/dataclasses.py` `_serialize_dataclass_fields`).
    fn serialize(fields: &[Field]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut buf, fields.len() as i64).expect("count fits");
        for f in fields {
            write_u8(&mut buf, DC_FIELD);
            crate::wire::write_int_bare(&mut buf, f.name.len() as i64).expect("name fits");
            buf.extend(f.name.as_bytes());
            crate::wire::write_int_bare(&mut buf, f.alias.len() as i64).expect("alias fits");
            buf.extend(f.alias.as_bytes());
            crate::wire::write_bool(&mut buf, f.has_default);
            crate::wire::write_bool(&mut buf, f.kw_only);
            crate::wire::write_bool(&mut buf, f.is_in_init);
            crate::wire::write_bool(&mut buf, f.is_init_var);
        }
        buf.into_bytes()
    }

    /// Encode `args` via `encode_signature` and parse it back with a
    /// synthetic Python-side reader (names/kinds only). Exercises the same
    /// encode path the seam entry points run, without needing the librt
    /// Python environment.
    fn parse_signature(args: &[(String, i64)]) -> (Vec<String>, Vec<i64>) {
        let result = encode_signature(args);

        assert_eq!(result[0], DC_INIT_SIG);
        let mut buf = ReadBuffer::new(&result);
        assert_eq!(buf.read_u8().ok(), Some(DC_INIT_SIG));
        let _body_len = read_int_bare(&mut buf).unwrap();
        assert_eq!(buf.read_u8().ok(), Some(DC_INIT_SIG));
        let n_args = read_int_bare(&mut buf).unwrap();
        let mut names = Vec::new();
        let mut kinds = Vec::new();
        for _ in 0..n_args {
            let name = read_str_bare(&mut buf).unwrap();
            let kind = read_int_bare(&mut buf).unwrap();
            names.push(name);
            kinds.push(kind);
        }
        assert_eq!(buf.read_u8().ok(), Some(0)); // returns_flag
        assert_eq!(buf.read_u8().ok(), Some(NONE_TYPE));
        assert_eq!(buf.read_u8().ok(), Some(END_TAG));
        assert_eq!(buf.read_u8().ok(), None); // fully consumed
        (names, kinds)
    }

    /// Round-trip through the `__init__` chain: deserialize -> compute ->
    /// encode (via `encode_signature`) -> parse back.
    fn roundtrip(fields: &[Field]) -> Option<(Vec<String>, Vec<i64>)> {
        let bytes = serialize(fields);
        let mut buf = ReadBuffer::new(&bytes);
        let parsed = deserialize_fields(&mut buf)?;
        let args = compute_init_signature(&parsed).ok()?;
        Some(parse_signature(&args))
    }

    /// Round-trip through the `__post_init__` chain (only `InitVar` fields,
    /// all `ARG_POS`).
    fn roundtrip_post_init(fields: &[Field]) -> Option<(Vec<String>, Vec<i64>)> {
        let bytes = serialize(fields);
        let mut buf = ReadBuffer::new(&bytes);
        let parsed = deserialize_fields(&mut buf)?;
        let args = compute_post_init_signature(&parsed);
        Some(parse_signature(&args))
    }

    fn field(name: &str, alias: &str, has_default: bool, kw_only: bool) -> Field {
        Field {
            name: name.to_string(),
            alias: alias.to_string(),
            has_default,
            kw_only,
            is_in_init: true,
            is_init_var: false,
        }
    }

    #[test]
    fn empty_fields_roundtrip() {
        let names_kinds = roundtrip(&[]).expect("empty input handles");
        assert_eq!(names_kinds.0, Vec::<String>::new());
        assert_eq!(names_kinds.1, Vec::<i64>::new());
    }

    #[test]
    fn positions_kinds_match_python() {
        // Non-kw-only, no default -> ARG_POS (0); default -> ARG_OPT (1).
        let fields = vec![
            field("a", "", false, false),
            field("b", "", true, false),
            field("c", "", true, true),
            field("d", "", false, true),
        ];
        let names_kinds = roundtrip(&fields).expect("roundtrip handles");
        assert_eq!(names_kinds.0, vec!["a", "b", "c", "d"]);
        assert_eq!(names_kinds.1, vec![0, 1, 5, 3]);
    }

    #[test]
    fn fields_order_preserved() {
        // The transform must yield arguments in attribute order (Python's
        // list comprehension keeps the order, it does not split positionals
        // first).
        let fields = vec![
            field("kw_first", "", false, true),
            field("pos_first", "", false, false),
        ];
        let names_kinds = roundtrip(&fields).expect("roundtrip handles");
        assert_eq!(names_kinds.0, vec!["kw_first", "pos_first"]);
        assert_eq!(names_kinds.1, vec![3, 0]);
    }

    #[test]
    fn alias_overrides_field_name() {
        // Python uses `attr.alias or attr.name` as the argument name.
        let fields = vec![field("y", "yy", false, false)];
        let names_kinds = roundtrip(&fields).expect("roundtrip handles");
        assert_eq!(names_kinds.0, vec!["yy"]);
        assert_eq!(names_kinds.1, vec![0]);
    }

    #[test]
    fn non_init_fields_skipped_by_caller() {
        // The Python side filters `is_in_init` before serializing; make sure
        // an is_in_init=False field still parses and yields no kind issue.
        let fields = vec![Field {
            name: "skipped".to_string(),
            alias: String::new(),
            has_default: false,
            kw_only: false,
            is_in_init: false,
            is_init_var: false,
        }];
        let names_kinds = roundtrip(&fields).expect("roundtrip handles");
        assert_eq!(names_kinds.0, vec!["skipped"]);
        assert_eq!(names_kinds.1, vec![0]);
    }

    #[test]
    fn bad_tag_falls_back() {
        // A wrong field tag must yield no parsed fields (Python then uses
        // its own path). deserialize_fields returns None for a bad tag.
        let mut buf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut buf, 1).expect("count");
        write_u8(&mut buf, 99); // wrong tag
        let bytes = buf.into_bytes();
        let mut read = ReadBuffer::new(&bytes);
        assert!(deserialize_fields(&mut read).is_none());
    }

    #[test]
    fn long_int_count_falls_back() {
        // Python can never emit the long-int sentinel for the field count
        // (`_write_short_int` always encodes a short int), but the wire
        // reader must still defer rather than misread it.
        let mut buf = WriteBuffer::new();
        write_u8(&mut buf, 15); // LONG_INT_TRAILER first byte
        let bytes = buf.into_bytes();
        let mut read = ReadBuffer::new(&bytes);
        assert!(deserialize_fields(&mut read).is_none());
    }

    #[test]
    fn huge_count_falls_back() {
        // count > 1000 is rejected outright; reading would be unreasonable.
        let mut buf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut buf, 1001).expect("count");
        let bytes = buf.into_bytes();
        let mut read = ReadBuffer::new(&bytes);
        assert!(deserialize_fields(&mut read).is_none());
    }

    #[test]
    fn large_field_count_uses_two_byte_varint() {
        // 200 fields force a 2-byte short-int arg-count on output; the
        // synthetic Python reader must decode it back exactly.
        let fields: Vec<Field> = (0..200)
            .map(|i| field(&format!("f{i}"), "", false, false))
            .collect();
        let names_kinds = roundtrip(&fields).expect("200 fields handle");
        assert_eq!(names_kinds.0.len(), 200);
        assert_eq!(names_kinds.1, vec![0; 200]);
        assert_eq!(names_kinds.0[0], "f0");
        assert_eq!(names_kinds.0[199], "f199");
    }

    #[test]
    fn post_init_only_keeps_init_var_fields() {
        // __post_init__ receives only the InitVar fields, in attribute
        // order, all positional without defaults.
        let fields = vec![
            Field {
                name: "a".to_string(),
                alias: String::new(),
                has_default: false,
                kw_only: false,
                is_in_init: true,
                is_init_var: false,
            },
            Field {
                name: "b".to_string(),
                alias: String::new(),
                has_default: true,
                kw_only: true,
                is_in_init: true,
                is_init_var: true,
            },
            Field {
                name: "c".to_string(),
                alias: String::new(),
                has_default: false,
                kw_only: false,
                is_in_init: true,
                is_init_var: true,
            },
        ];
        let names_kinds = roundtrip_post_init(&fields).expect("post-init handles");
        assert_eq!(names_kinds.0, vec!["b", "c"]);
        assert_eq!(names_kinds.1, vec![0, 0]);
    }

    #[test]
    fn post_init_kind_is_pos_even_with_default_or_kw_only() {
        // Even a kw-only field with a default is a plain positional arg in
        // __post_init__ (the runtime protocol resolves the value from the
        // field default).
        let fields = vec![Field {
            name: "x".to_string(),
            alias: String::new(),
            has_default: true,
            kw_only: true,
            is_in_init: true,
            is_init_var: true,
        }];
        let names_kinds = roundtrip_post_init(&fields).expect("post-init handles");
        assert_eq!(names_kinds.0, vec!["x"]);
        assert_eq!(names_kinds.1, vec![0]);
    }

    #[test]
    fn post_init_uses_alias() {
        // The argument name is `alias or name`, same as __init__.
        let fields = vec![Field {
            name: "y".to_string(),
            alias: "provided".to_string(),
            has_default: false,
            kw_only: false,
            is_in_init: true,
            is_init_var: true,
        }];
        let names_kinds = roundtrip_post_init(&fields).expect("post-init handles");
        assert_eq!(names_kinds.0, vec!["provided"]);
        assert_eq!(names_kinds.1, vec![0]);
    }
}
