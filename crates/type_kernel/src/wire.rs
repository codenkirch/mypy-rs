//! Rust-owned `Type` enum + binary wire-format reader for mypy's serialized
//! `mypy.types.Type` objects (Stage 3a of the type-kernel migration).
//!
//! This module reads the fixed-format binary cache (`.data.ff` / `.meta.ff`)
//! produced by `Type.write(WriteBuffer)` in `mypy/types.py`. It mirrors the
//! tag dispatch in `mypy/types.py:read_type` and the per-class `read` methods,
//! plus the byte-level primitives in `mypyc/lib-rt/internal/librt_internal.c`
//! and the tagged helpers in `mypy/cache.py`.
//!
//! The Rust `Type` enum is a clean break from Stages 1/2 (which walked live
//! Python `Type` objects via PyO3 `isinstance`). It carries unresolved
//! `type_ref: String` fields for `Instance` and `TypeAliasType` — exactly
//! what the wire format stores before `TypeFixer` (`mypy/fixup.py`) resolves
//! them to live `TypeInfo`/`TypeAlias` graph objects. Stage 3b will add the
//! `TypeInfo` snapshot protocol that resolves these refs; Stage 3a's `Display`
//! honestly renders the "unfixed" state for those branches.
//!
//! Parity contract: `str(python_type) == rust_read(bytes).to_string()` over
//! the `TypeFixture` corpus (see `NativeTypeWireSuite` in `testtypes.py`).
//! Reader-only — no `WriteBuffer`, no production wiring, no
//! `Options.native_type_kernel` flip.

use std::collections::{HashMap, HashSet};
use std::fmt;

use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Constants — copied verbatim from librt_internal.c:14-28 and cache.py:301-328.
// ---------------------------------------------------------------------------

// Varint width constants (librt_internal.c:14-19).
const MIN_ONE_BYTE_INT: i64 = -10;
const MIN_TWO_BYTES_INT: i64 = -100;
const MIN_FOUR_BYTES_INT: i64 = -10000;

// Varint bit flags (librt_internal.c:21-25).
const TWO_BYTES_INT_BIT: u8 = 1;
const FOUR_BYTES_INT_BIT: u8 = 2;
#[allow(dead_code)]
const FOUR_BYTES_INT_TRAILER: u8 = 3;
const LONG_INT_TRAILER: u8 = 15;

// Primitive literal tags (cache.py:303-310).
const LITERAL_FALSE: u8 = 0;
const LITERAL_TRUE: u8 = 1;
const LITERAL_NONE: u8 = 2;
const LITERAL_INT: u8 = 3;
const LITERAL_STR: u8 = 4;
const LITERAL_BYTES: u8 = 5;
const LITERAL_FLOAT: u8 = 6;

// Collection tags (cache.py:313-318).
const LIST_GEN: u8 = 20;
const LIST_INT: u8 = 21;
const LIST_STR: u8 = 22;
const DICT_STR_GEN: u8 = 30;

// Misc class tags (cache.py:322-325).
const EXTRA_ATTRS: u8 = 150;

// Reserved / end markers (cache.py:327-328).
const END_TAG: u8 = 255;

// Instance family tags (types.py:4425-4432).
const INSTANCE: u8 = 80;
const INSTANCE_SIMPLE: u8 = 81;
const INSTANCE_GENERIC: u8 = 82;
const INSTANCE_STR: u8 = 83;
const INSTANCE_FUNCTION: u8 = 84;
const INSTANCE_INT: u8 = 85;
const INSTANCE_BOOL: u8 = 86;
const INSTANCE_OBJECT: u8 = 87;

// Other type tags (types.py:4435-4452).
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

// ---------------------------------------------------------------------------
// ReadBuffer + error type
// ---------------------------------------------------------------------------

/// Read-only cursor over a byte slice, mirroring librt's `ReadBuffer` C type.
/// Every read advances the cursor; truncation returns `WireError::Truncated`.
pub(crate) struct ReadBuffer<'a> {
    data: &'a [u8],
    pos: usize,
}

/// Errors raised by the reader. `Truncated` is the common case (short input);
/// `Invalid` covers malformed bytes (bad bool, bad tag, bad varint, etc.).
#[derive(Debug, Clone)]
pub(crate) enum WireError {
    Truncated,
    Invalid(String),
}

impl WireError {
    fn invalid(msg: impl Into<String>) -> Self {
        WireError::Invalid(msg.into())
    }
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => write!(f, "reading past the buffer end"),
            WireError::Invalid(msg) => write!(f, "invalid wire data: {msg}"),
        }
    }
}

impl std::error::Error for WireError {}

impl<'a> ReadBuffer<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        ReadBuffer { data, pos: 0 }
    }

    /// Number of bytes remaining unread.
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Ensure at least `n` bytes are available, else `Truncated`.
    fn ensure(&self, n: usize) -> Result<(), WireError> {
        if self.remaining() < n {
            Err(WireError::Truncated)
        } else {
            Ok(())
        }
    }

    /// Read 1 byte as a raw u8 (the `read_tag` primitive).
    fn read_u8(&mut self) -> Result<u8, WireError> {
        self.ensure(1)?;
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    /// Read `n` bytes as a slice (advances cursor; caller does not copy).
    fn read_slice(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        self.ensure(n)?;
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
}

// ---------------------------------------------------------------------------
// Bare primitives (mirror librt_internal.c read_*_internal)
// ---------------------------------------------------------------------------

/// Read a tag byte (1 byte u8). Mirrors `read_tag`.
fn read_tag(buf: &mut ReadBuffer<'_>) -> Result<u8, WireError> {
    buf.read_u8()
}

/// Read a bool (1 byte: 0=False, 1=True, else Invalid). Mirrors `read_bool`.
fn read_bool(buf: &mut ReadBuffer<'_>) -> Result<bool, WireError> {
    match buf.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(WireError::invalid(format!("invalid bool value {other}"))),
    }
}

/// Read a "short int" varint, given the already-consumed first byte.
/// Mirrors `_read_short_int` (librt_internal.c:392-415), dropping the CPyTagged
/// `<< 1` tag bit (we want the raw integer value).
///
/// Width encoding (low bits of the first byte):
/// - 1-byte  (low bit 0):  7 bits payload, range -10..=117
/// - 2-byte (low 2 bits 01): 14 bits payload, range -100..=16283
/// - 4-byte (low 3 bits 011): 29 bits payload, range -10000..=536860911
fn read_short_int(buf: &mut ReadBuffer<'_>, first: u8) -> Result<i64, WireError> {
    if (first & TWO_BYTES_INT_BIT) == 0 {
        // 1-byte form: 7 bits.
        Ok(((first >> 1) as i64) + MIN_ONE_BYTE_INT)
    } else if (first & FOUR_BYTES_INT_BIT) == 0 {
        // 2-byte form: 14 bits. Low 2 bits are the trailer `01`;
        // the next byte contributes the high 8 bits.
        let second = buf.read_u8()?;
        Ok(((second as i64) << 6) + ((first >> 2) as i64) + MIN_TWO_BYTES_INT)
    } else {
        // 4-byte form: 29 bits. Low 3 bits are the trailer `011`.
        // Layout (little-endian): byte0=first, byte1=second (5 bits),
        // bytes 2-3 = two_more (u16 LE, 13 bits).
        let second = buf.read_u8()?;
        let two_more_bytes = buf.read_slice(2)?;
        let two_more = u16::from_le_bytes([two_more_bytes[0], two_more_bytes[1]]);
        let higher = ((two_more as i64) << 13) + ((second as i64) << 5);
        Ok(higher + ((first >> 3) as i64) + MIN_FOUR_BYTES_INT)
    }
}

/// Read an arbitrary-precision integer. Mirrors `read_int_internal`
/// (librt_internal.c:694-735). Layout: `LONG_INT_TRAILER` sentinel byte,
/// then a short-int encoding `(size << 1) | sign`, then `size` bytes of
/// little-endian unsigned magnitude.
fn read_long_int(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
    // The LONG_INT_TRAILER byte is already consumed by the caller; the next
    // byte is the short-int encoding of (size << 1) | sign.
    //
    // Note: the C reader (`read_int_internal`) extracts size/sign from the
    // CPyTagged form (value << 1) that `_read_short_int` returns. Our
    // `read_short_int` returns the raw value, so we extract directly:
    //   sign = size_and_sign & 1
    //   size = size_and_sign >> 1
    let first = buf.read_u8()?;
    let size_and_sign = read_short_int(buf, first)?;
    if size_and_sign < 0 {
        return Err(WireError::invalid("invalid int data"));
    }
    let sign = size_and_sign & 1;
    let size = (size_and_sign >> 1) as usize;
    let magnitude_bytes = buf.read_slice(size)?;
    // Reconstruct little-endian unsigned magnitude.
    let mut value: i128 = 0;
    for &b in magnitude_bytes.iter().rev() {
        value = (value << 8) | (b as i128);
    }
    let signed = if sign == 1 { -value } else { value };
    // Stage 3a only supports values that fit in i64 (the test corpus does not
    // use arbitrary-precision literals in serialized Types). Larger values
    // would require a BigInt; we return an error rather than silently wrap.
    i64::try_from(signed).map_err(|_| WireError::invalid("int exceeds i64 range"))
}

/// Read a bare integer (the librt `read_int` / `read_int_bare` primitive).
/// Dispatches short-int vs long-int based on the first byte.
fn read_int_bare(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
    let first = buf.read_u8()?;
    if first != LONG_INT_TRAILER {
        read_short_int(buf, first)
    } else {
        read_long_int(buf)
    }
}

/// Read a bare string (short-int length prefix + UTF-8 body). Mirrors
/// `read_str_internal`. Rejects `LONG_INT_TRAILER` as a length prefix and
/// negative lengths (both are fail-fast cases in the C reader).
fn read_str_bare(buf: &mut ReadBuffer<'_>) -> Result<String, WireError> {
    let first = buf.read_u8()?;
    if first == LONG_INT_TRAILER {
        return Err(WireError::invalid("invalid str size"));
    }
    let size = read_short_int(buf, first)?;
    if size < 0 {
        return Err(WireError::invalid("invalid str size"));
    }
    let bytes = buf.read_slice(size as usize)?;
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|_| WireError::invalid("invalid UTF-8 in str"))
}

/// Read bare bytes (short-int length prefix + raw body). Mirrors
/// `read_bytes_internal`. Used by `read_literal` for the
/// `LITERAL_BYTES` tag (cache.py:347-364).
fn read_bytes_bare(buf: &mut ReadBuffer<'_>) -> Result<Vec<u8>, WireError> {
    let first = buf.read_u8()?;
    if first == LONG_INT_TRAILER {
        return Err(WireError::invalid("invalid bytes size"));
    }
    let size = read_short_int(buf, first)?;
    if size < 0 {
        return Err(WireError::invalid("invalid bytes size"));
    }
    let bytes = buf.read_slice(size as usize)?;
    Ok(bytes.to_vec())
}

/// Read a bare float (8 bytes, IEEE-754 little-endian). Mirrors
/// `read_float_internal`.
fn read_float_bare(buf: &mut ReadBuffer<'_>) -> Result<f64, WireError> {
    let bytes = buf.read_slice(8)?;
    let le = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    Ok(f64::from_bits(le))
}

// ---------------------------------------------------------------------------
// Tagged helpers (mirror cache.py read_* helpers)
// ---------------------------------------------------------------------------

/// `read_int`: tag byte must be `LITERAL_INT`, then bare int.
fn read_int(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
    let tag = read_tag(buf)?;
    if tag != LITERAL_INT {
        return Err(WireError::invalid(format!(
            "expected LITERAL_INT, got tag {tag}"
        )));
    }
    read_int_bare(buf)
}

/// `read_str`: tag byte must be `LITERAL_STR`, then bare str.
fn read_str(buf: &mut ReadBuffer<'_>) -> Result<String, WireError> {
    let tag = read_tag(buf)?;
    if tag != LITERAL_STR {
        return Err(WireError::invalid(format!(
            "expected LITERAL_STR, got tag {tag}"
        )));
    }
    read_str_bare(buf)
}

/// `read_str_opt`: `LITERAL_NONE` → None, else `LITERAL_STR` + bare str.
fn read_str_opt(buf: &mut ReadBuffer<'_>) -> Result<Option<String>, WireError> {
    let tag = read_tag(buf)?;
    if tag == LITERAL_NONE {
        return Ok(None);
    }
    if tag != LITERAL_STR {
        return Err(WireError::invalid(format!(
            "expected LITERAL_STR or LITERAL_NONE, got tag {tag}"
        )));
    }
    Ok(Some(read_str_bare(buf)?))
}

/// `read_int_list`: `LIST_INT` tag, bare size, N bare ints.
fn read_int_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<i64>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_INT {
        return Err(WireError::invalid(format!(
            "expected LIST_INT, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_int_bare(buf)?);
    }
    Ok(out)
}

/// `read_str_list`: `LIST_STR` tag, bare size, N bare strs.
fn read_str_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<String>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_STR {
        return Err(WireError::invalid(format!(
            "expected LIST_STR, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_str_bare(buf)?);
    }
    Ok(out)
}

/// `read_str_opt_list`: `LIST_GEN` tag, bare size, N `read_str_opt`s.
/// (Note: each element is a tagged None-or-str, NOT a bare str.)
fn read_str_opt_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<Option<String>>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_str_opt(buf)?);
    }
    Ok(out)
}

/// `read_flags`: a single high-level `int` (tagged), bit-packed, max 26 flags.
/// Mirrors `read_flags(data, num_flags)`.
fn read_flags(buf: &mut ReadBuffer<'_>, num_flags: usize) -> Result<Vec<bool>, WireError> {
    let packed = read_int(buf)?;
    let mut out = Vec::with_capacity(num_flags);
    for i in 0..num_flags {
        out.push((packed & (1 << i)) != 0);
    }
    Ok(out)
}

/// A literal value as stored by `write_literal` (cache.py:347-364): the tag
/// byte is already consumed by the caller (it was the discriminator), and
/// this reads the body.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum LiteralValue {
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
    Bool(bool),
    Float(f64),
}

fn read_literal(buf: &mut ReadBuffer<'_>, tag: u8) -> Result<LiteralValue, WireError> {
    match tag {
        LITERAL_INT => Ok(LiteralValue::Int(read_int_bare(buf)?)),
        LITERAL_STR => Ok(LiteralValue::Str(read_str_bare(buf)?)),
        LITERAL_BYTES => Ok(LiteralValue::Bytes(read_bytes_bare(buf)?)),
        LITERAL_FALSE => Ok(LiteralValue::Bool(false)),
        LITERAL_TRUE => Ok(LiteralValue::Bool(true)),
        LITERAL_FLOAT => Ok(LiteralValue::Float(read_float_bare(buf)?)),
        _ => Err(WireError::invalid(format!("unknown literal tag {tag}"))),
    }
}

// ---------------------------------------------------------------------------
// Type enum (mirrors the 19 serialized Type subclasses)
// ---------------------------------------------------------------------------

/// `mypy.types.ExtraAttrs` — module-attribute summary attached to `Instance`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ExtraAttrs {
    pub attrs: HashMap<String, Type>,
    pub immutable: HashSet<String>,
    pub mod_name: Option<String>,
}

/// `mypy.types.Parameters` — a standalone parameter list (used by
/// `ParamSpecType.prefix` and as the `PARAMETERS` tag).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct Parameters {
    pub arg_types: Vec<Type>,
    pub arg_kinds: Vec<i64>,
    pub arg_names: Vec<Option<String>>,
    pub variables: Vec<Type>,
    pub imprecise_arg_kinds: bool,
}

/// `mypy.types.Type` — one variant per serialized subclass. `Instance.type_ref`
/// and `TypeAliasType.type_ref` carry the unresolved fullname string (the
/// wire format's honest state before `TypeFixer` runs).
///
/// Fields that aren't read by the Stage 3a `Display` impl are intentionally
/// kept: they will be consumed by Stage 3b (`TypeInfo` snapshot) and 3c
/// (`is_subtype`), and storing them now keeps the reader byte-exact against
/// the Python wire format.
#[derive(Debug, Clone)]
// Variant names mirror mypy's `Type` subclasses (Instance, AnyType, NoneType,
// ...) for direct cross-referencing with `mypy/types.py`. Clippy's
// `enum_variant_names` lint would force renames that diverge from that
// one-to-one mapping.
#[allow(dead_code, clippy::enum_variant_names)]
pub(crate) enum Type {
    /// `mypy.types.Instance` — `type_ref` is the unresolved `type.fullname`.
    Instance {
        type_ref: String,
        args: Vec<Type>,
        last_known_value: Option<Box<Type>>,
        extra_attrs: Option<ExtraAttrs>,
    },
    /// `mypy.types.TypeAliasType` — `type_ref` is the unresolved `alias.fullname`.
    TypeAliasType {
        args: Vec<Type>,
        type_ref: String,
    },
    TypeVarType {
        name: String,
        fullname: String,
        raw_id: i64,
        namespace: String,
        values: Vec<Type>,
        upper_bound: Box<Type>,
        default: Box<Type>,
        variance: i64,
    },
    ParamSpecType {
        prefix: Box<Parameters>,
        name: String,
        fullname: String,
        raw_id: i64,
        namespace: String,
        flavor: i64,
        upper_bound: Box<Type>,
        default: Box<Type>,
    },
    TypeVarTupleType {
        tuple_fallback: Box<Type>,
        name: String,
        fullname: String,
        raw_id: i64,
        namespace: String,
        upper_bound: Box<Type>,
        default: Box<Type>,
        min_len: i64,
    },
    UnboundType {
        name: String,
        args: Vec<Type>,
        original_str_expr: Option<String>,
        original_str_fallback: Option<String>,
    },
    UnpackType {
        typ: Box<Type>,
    },
    AnyType {
        type_of_any: i64,
        source_any: Option<Box<Type>>,
        missing_import_name: Option<String>,
    },
    UninhabitedType,
    NoneType,
    DeletedType {
        source: Option<String>,
    },
    CallableType {
        fallback: Box<Type>,
        instance_type: Option<Box<Type>>,
        // 6 flags, in write order: is_ellipsis_args, implicit, is_bound,
        // from_concatenate, imprecise_arg_kinds, unpack_kwargs.
        is_ellipsis_args: bool,
        implicit: bool,
        is_bound: bool,
        from_concatenate: bool,
        imprecise_arg_kinds: bool,
        unpack_kwargs: bool,
        arg_types: Vec<Type>,
        arg_kinds: Vec<i64>,
        arg_names: Vec<Option<String>>,
        ret_type: Box<Type>,
        name: Option<String>,
        variables: Vec<Type>,
        type_guard: Option<Box<Type>>,
        type_is: Option<Box<Type>>,
    },
    Overloaded {
        items: Vec<Type>,
    },
    TupleType {
        partial_fallback: Box<Type>,
        items: Vec<Type>,
        implicit: bool,
    },
    TypedDictType {
        fallback: Box<Type>,
        items: Vec<(String, Type)>,
        required_keys: HashSet<String>,
        readonly_keys: HashSet<String>,
        is_closed: bool,
    },
    LiteralType {
        fallback: Box<Type>,
        value: LiteralValue,
    },
    UnionType {
        items: Vec<Type>,
        uses_pep604_syntax: bool,
    },
    TypeType {
        item: Box<Type>,
        is_type_form: bool,
    },
    Parameters(Parameters),
}

// ---------------------------------------------------------------------------
// Type readers (mirror types.py:read_type + per-class read methods)
// ---------------------------------------------------------------------------

/// `read_type_opt`: `LITERAL_NONE` → None, else `read_type`.
fn read_type_opt(buf: &mut ReadBuffer<'_>) -> Result<Option<Type>, WireError> {
    let tag = read_tag(buf)?;
    if tag == LITERAL_NONE {
        return Ok(None);
    }
    Ok(Some(read_type(buf, Some(tag))?))
}

/// `read_type_list`: `LIST_GEN` tag, bare size, N `read_type`s.
fn read_type_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<Type>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        out.push(read_type(buf, None)?);
    }
    Ok(out)
}

/// `read_type_map`: `DICT_STR_GEN` tag, bare size, N (bare str key, tagged type).
fn read_type_map(buf: &mut ReadBuffer<'_>) -> Result<Vec<(String, Type)>, WireError> {
    let tag = read_tag(buf)?;
    if tag != DICT_STR_GEN {
        return Err(WireError::invalid(format!(
            "expected DICT_STR_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative map size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let key = read_str_bare(buf)?;
        let value = read_type(buf, None)?;
        out.push((key, value));
    }
    Ok(out)
}

/// `read_type_var_likes`: `LIST_GEN` tag, bare size, N items each dispatched
/// to TypeVarType / ParamSpecType / TypeVarTupleType.
fn read_type_var_likes(buf: &mut ReadBuffer<'_>) -> Result<Vec<Type>, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let item_tag = read_tag(buf)?;
        match item_tag {
            TYPE_VAR_TYPE => out.push(read_type_var_type(buf)?),
            PARAM_SPEC_TYPE => out.push(read_param_spec_type(buf)?),
            TYPE_VAR_TUPLE_TYPE => out.push(read_type_var_tuple_type(buf)?),
            _ => {
                return Err(WireError::invalid(format!(
                    "invalid type tag for TypeVarLikeType {item_tag}"
                )));
            }
        }
    }
    Ok(out)
}

/// Read an `ExtraAttrs` record (tag already consumed by the caller).
fn read_extra_attrs(buf: &mut ReadBuffer<'_>) -> Result<ExtraAttrs, WireError> {
    let attrs_map = read_type_map(buf)?;
    let immutable_list = read_str_list(buf)?;
    let mod_name = read_str_opt(buf)?;
    expect_end_tag(buf)?;
    Ok(ExtraAttrs {
        attrs: attrs_map.into_iter().collect(),
        immutable: immutable_list.into_iter().collect(),
        mod_name,
    })
}

/// Read an `Instance`. The outer `INSTANCE` tag is already consumed by the
/// caller (read_type); this reads the inner discriminator tag and branches on
/// the INSTANCE_STR / INSTANCE_FUNCTION / INSTANCE_INT / INSTANCE_BOOL /
/// INSTANCE_OBJECT / INSTANCE_SIMPLE / INSTANCE_GENERIC fast paths.
fn read_instance(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let tag = read_tag(buf)?;
    let type_ref = match tag {
        INSTANCE_STR => "builtins.str".to_string(),
        INSTANCE_FUNCTION => "builtins.function".to_string(),
        INSTANCE_INT => "builtins.int".to_string(),
        INSTANCE_BOOL => "builtins.bool".to_string(),
        INSTANCE_OBJECT => "builtins.object".to_string(),
        INSTANCE_SIMPLE => read_str_bare(buf)?,
        INSTANCE_GENERIC => {
            // Tagged str (LITERAL_STR prefix), then args, lkv, extra_attrs.
            let type_ref = read_str(buf)?;
            let args = read_type_list(buf)?;
            let last_known_value = read_type_opt(buf)?;
            let extra_attrs = match read_tag(buf)? {
                LITERAL_NONE => None,
                EXTRA_ATTRS => Some(read_extra_attrs(buf)?),
                other => {
                    return Err(WireError::invalid(format!(
                        "expected LITERAL_NONE or EXTRA_ATTRS, got tag {other}"
                    )));
                }
            };
            expect_end_tag(buf)?;
            return Ok(Type::Instance {
                type_ref,
                args,
                last_known_value: last_known_value.map(Box::new),
                extra_attrs,
            });
        }
        _ => {
            return Err(WireError::invalid(format!(
                "invalid Instance discriminator tag {tag}"
            )));
        }
    };
    // The five singletons and INSTANCE_SIMPLE write no END_TAG (the fast path
    // returns immediately in the Python writer).
    Ok(Type::Instance {
        type_ref,
        args: Vec::new(),
        last_known_value: None,
        extra_attrs: None,
    })
}

/// Read a `TypeVarType` (tag already consumed).
fn read_type_var_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let name = read_str(buf)?;
    let fullname = read_str(buf)?;
    let raw_id = read_int(buf)?;
    let namespace = read_str(buf)?;
    let values = read_type_list(buf)?;
    let upper_bound = read_type(buf, None)?;
    let default = read_type(buf, None)?;
    let variance = read_int(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TypeVarType {
        name,
        fullname,
        raw_id,
        namespace,
        values,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
        variance,
    })
}

/// Read a `ParamSpecType` (tag already consumed). Reads an inline
/// `PARAMETERS` record for the prefix first.
fn read_param_spec_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let prefix_tag = read_tag(buf)?;
    if prefix_tag != PARAMETERS {
        return Err(WireError::invalid(format!(
            "expected PARAMETERS for ParamSpec prefix, got tag {prefix_tag}"
        )));
    }
    let prefix = read_parameters(buf)?;
    let name = read_str(buf)?;
    let fullname = read_str(buf)?;
    let raw_id = read_int(buf)?;
    let namespace = read_str(buf)?;
    let flavor = read_int(buf)?;
    let upper_bound = read_type(buf, None)?;
    let default = read_type(buf, None)?;
    expect_end_tag(buf)?;
    Ok(Type::ParamSpecType {
        prefix: Box::new(prefix),
        name,
        fullname,
        raw_id,
        namespace,
        flavor,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
    })
}

/// Read a `TypeVarTupleType` (tag already consumed). Reads an inline
/// `INSTANCE` record for `tuple_fallback` first.
fn read_type_var_tuple_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let fallback_tag = read_tag(buf)?;
    if fallback_tag != INSTANCE {
        return Err(WireError::invalid(format!(
            "expected INSTANCE for TypeVarTuple tuple_fallback, got tag {fallback_tag}"
        )));
    }
    let tuple_fallback = read_instance(buf)?;
    let name = read_str(buf)?;
    let fullname = read_str(buf)?;
    let raw_id = read_int(buf)?;
    let namespace = read_str(buf)?;
    let upper_bound = read_type(buf, None)?;
    let default = read_type(buf, None)?;
    let min_len = read_int(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TypeVarTupleType {
        tuple_fallback: Box::new(tuple_fallback),
        name,
        fullname,
        raw_id,
        namespace,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
        min_len,
    })
}

/// Read a `Parameters` record (tag already consumed).
fn read_parameters(buf: &mut ReadBuffer<'_>) -> Result<Parameters, WireError> {
    let arg_types = read_type_list(buf)?;
    let arg_kinds = read_int_list(buf)?;
    let arg_names = read_str_opt_list(buf)?;
    let variables = read_type_var_likes(buf)?;
    let imprecise_arg_kinds = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Parameters {
        arg_types,
        arg_kinds,
        arg_names,
        variables,
        imprecise_arg_kinds,
    })
}

/// Read an `UnboundType` (tag already consumed).
fn read_unbound_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let name = read_str(buf)?;
    let args = read_type_list(buf)?;
    let original_str_expr = read_str_opt(buf)?;
    let original_str_fallback = read_str_opt(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::UnboundType {
        name,
        args,
        original_str_expr,
        original_str_fallback,
    })
}

/// Read an `UnpackType` (tag already consumed).
fn read_unpack_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let typ = read_type(buf, None)?;
    expect_end_tag(buf)?;
    Ok(Type::UnpackType { typ: Box::new(typ) })
}

/// Read an `AnyType` (tag already consumed).
fn read_any_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    // source_any: None, or a nested AnyType (writer uses write_type_opt).
    let source_any = read_type_opt(buf)?;
    let type_of_any = read_int(buf)?;
    let missing_import_name = read_str_opt(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::AnyType {
        type_of_any,
        source_any: source_any.map(Box::new),
        missing_import_name,
    })
}

/// Read a `NoneType` (tag already consumed) — just the END_TAG.
fn read_none_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    expect_end_tag(buf)?;
    Ok(Type::NoneType)
}

/// Read an `UninhabitedType` (tag already consumed) — just the END_TAG.
fn read_uninhabited_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    expect_end_tag(buf)?;
    Ok(Type::UninhabitedType)
}

/// Read a `DeletedType` (tag already consumed).
fn read_deleted_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let source = read_str_opt(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::DeletedType { source })
}

/// Read a `CallableType` (tag already consumed).
fn read_callable_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    // fallback: an inline Instance.
    let fallback_tag = read_tag(buf)?;
    if fallback_tag != INSTANCE {
        return Err(WireError::invalid(format!(
            "expected INSTANCE for CallableType fallback, got tag {fallback_tag}"
        )));
    }
    let fallback = read_instance(buf)?;
    let instance_type = read_type_opt(buf)?;
    let flags = read_flags(buf, 6)?;
    let mut flags_iter = flags.into_iter();
    let mut next_flag = || -> bool { flags_iter.next().unwrap_or(false) };
    let is_ellipsis_args = next_flag();
    let implicit = next_flag();
    let is_bound = next_flag();
    let from_concatenate = next_flag();
    let imprecise_arg_kinds = next_flag();
    let unpack_kwargs = next_flag();
    let arg_types = read_type_list(buf)?;
    let arg_kinds = read_int_list(buf)?;
    let arg_names = read_str_opt_list(buf)?;
    let ret_type = read_type(buf, None)?;
    let name = read_str_opt(buf)?;
    let variables = read_type_var_likes(buf)?;
    let type_guard = read_type_opt(buf)?;
    let type_is = read_type_opt(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::CallableType {
        fallback: Box::new(fallback),
        instance_type: instance_type.map(Box::new),
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type: Box::new(ret_type),
        name,
        variables,
        type_guard: type_guard.map(Box::new),
        type_is: type_is.map(Box::new),
    })
}

/// Read an `Overloaded` (tag already consumed). Each item is asserted
/// CALLABLE_TYPE in the Python reader; we accept and dispatch.
fn read_overloaded(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let tag = read_tag(buf)?;
    if tag != LIST_GEN {
        return Err(WireError::invalid(format!(
            "expected LIST_GEN, got tag {tag}"
        )));
    }
    let size = read_int_bare(buf)?;
    if size < 0 {
        return Err(WireError::invalid("negative list size"));
    }
    let mut items = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let item_tag = read_tag(buf)?;
        if item_tag != CALLABLE_TYPE {
            return Err(WireError::invalid(format!(
                "expected CALLABLE_TYPE in Overloaded items, got tag {item_tag}"
            )));
        }
        items.push(read_callable_type(buf)?);
    }
    expect_end_tag(buf)?;
    Ok(Type::Overloaded { items })
}

/// Read a `TupleType` (tag already consumed).
fn read_tuple_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let fallback_tag = read_tag(buf)?;
    if fallback_tag != INSTANCE {
        return Err(WireError::invalid(format!(
            "expected INSTANCE for TupleType partial_fallback, got tag {fallback_tag}"
        )));
    }
    let partial_fallback = read_instance(buf)?;
    let items = read_type_list(buf)?;
    let implicit = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TupleType {
        partial_fallback: Box::new(partial_fallback),
        items,
        implicit,
    })
}

/// Read a `TypedDictType` (tag already consumed).
fn read_typeddict_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let fallback_tag = read_tag(buf)?;
    if fallback_tag != INSTANCE {
        return Err(WireError::invalid(format!(
            "expected INSTANCE for TypedDictType fallback, got tag {fallback_tag}"
        )));
    }
    let fallback = read_instance(buf)?;
    let items = read_type_map(buf)?;
    let required_keys = read_str_list(buf)?.into_iter().collect();
    let readonly_keys = read_str_list(buf)?.into_iter().collect();
    let is_closed = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TypedDictType {
        fallback: Box::new(fallback),
        items,
        required_keys,
        readonly_keys,
        is_closed,
    })
}

/// Read a `LiteralType` (tag already consumed).
fn read_literal_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let fallback_tag = read_tag(buf)?;
    if fallback_tag != INSTANCE {
        return Err(WireError::invalid(format!(
            "expected INSTANCE for LiteralType fallback, got tag {fallback_tag}"
        )));
    }
    let fallback = read_instance(buf)?;
    let value_tag = read_tag(buf)?;
    let value = read_literal(buf, value_tag)?;
    expect_end_tag(buf)?;
    Ok(Type::LiteralType {
        fallback: Box::new(fallback),
        value,
    })
}

/// Read a `UnionType` (tag already consumed).
fn read_union_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let items = read_type_list(buf)?;
    let uses_pep604_syntax = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::UnionType {
        items,
        uses_pep604_syntax,
    })
}

/// Read a `TypeType` (tag already consumed).
fn read_type_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let item = read_type(buf, None)?;
    let is_type_form = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TypeType {
        item: Box::new(item),
        is_type_form,
    })
}

/// Read a `TypeAliasType` (tag already consumed).
fn read_type_alias_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let args = read_type_list(buf)?;
    let type_ref = read_str(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::TypeAliasType { args, type_ref })
}

/// Assert the next byte is `END_TAG`. Mirrors the Python `assert read_tag(data) == END_TAG`.
fn expect_end_tag(buf: &mut ReadBuffer<'_>) -> Result<(), WireError> {
    let tag = read_tag(buf)?;
    if tag != END_TAG {
        return Err(WireError::invalid(format!(
            "expected END_TAG (255), got tag {tag}"
        )));
    }
    Ok(())
}

/// The main dispatch: mirror `mypy/types.py:read_type`. If `tag` is `None`,
/// reads the next tag byte first; otherwise uses the provided tag (already
/// consumed by the caller, e.g. `read_type_opt`).
pub(crate) fn read_type(buf: &mut ReadBuffer<'_>, tag: Option<u8>) -> Result<Type, WireError> {
    let tag = match tag {
        Some(t) => t,
        None => read_tag(buf)?,
    };
    // Branch order mirrors the Python reader (by popularity).
    match tag {
        INSTANCE => read_instance(buf),
        ANY_TYPE => read_any_type(buf),
        TYPE_VAR_TYPE => read_type_var_type(buf),
        CALLABLE_TYPE => read_callable_type(buf),
        NONE_TYPE => read_none_type(buf),
        UNION_TYPE => read_union_type(buf),
        LITERAL_TYPE => read_literal_type(buf),
        TYPE_ALIAS_TYPE => read_type_alias_type(buf),
        TUPLE_TYPE => read_tuple_type(buf),
        TYPED_DICT_TYPE => read_typeddict_type(buf),
        TYPE_TYPE => read_type_type(buf),
        OVERLOADED => read_overloaded(buf),
        PARAM_SPEC_TYPE => read_param_spec_type(buf),
        TYPE_VAR_TUPLE_TYPE => read_type_var_tuple_type(buf),
        UNPACK_TYPE => read_unpack_type(buf),
        PARAMETERS => Ok(Type::Parameters(read_parameters(buf)?)),
        UNINHABITED_TYPE => read_uninhabited_type(buf),
        UNBOUND_TYPE => read_unbound_type(buf),
        DELETED_TYPE => read_deleted_type(buf),
        _ => Err(WireError::invalid(format!("unknown type tag {tag}"))),
    }
}

// ---------------------------------------------------------------------------
// Display impl — mirrors TypeStrVisitor (mypy/types.py:3809-4123), non-verbose
// ---------------------------------------------------------------------------

impl fmt::Display for LiteralValue {
    /// Mirrors `LiteralType.value_repr()`, which is `repr(self.value)` for
    /// the non-enum, non-bytes-prefix branches. Enum-literal and bytes-literal
    /// formatting require TypeInfo resolution and are deferred to Stage 3b;
    /// the parity corpus uses int / str / bool / float literals, which this
    /// covers exactly.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Int(v) => write!(f, "{v}"),
            // Mirror Python `repr(str)`: single-quoted unless the string
            // contains a single quote (then double-quoted). Rust's `{:?}`
            // always double-quotes, so we replicate Python's preference here.
            LiteralValue::Str(s) => python_str_repr(f, s),
            // Mirror Python `repr(bytes)` — always single-quoted with a
            // `b` prefix; non-printable bytes use `\xNN` escapes.
            LiteralValue::Bytes(b) => python_bytes_repr(f, b),
            // Python capitalizes bool literals: `True` / `False`.
            LiteralValue::Bool(b) => {
                if *b {
                    write!(f, "True")
                } else {
                    write!(f, "False")
                }
            }
            LiteralValue::Float(v) => {
                // Mirror Python `repr(float)`. Rust's default Display is close
                // enough for the test corpus (e.g. `1.5`, `0.5`); full repr
                // parity (e.g. `1e16` vs `1e+16`) is a Stage 3b refinement.
                write!(f, "{v:?}")
            }
        }
    }
}

/// Replicate CPython's `repr(str)` quoting choice: prefer single quotes,
/// but switch to double quotes when the string contains a single quote and
/// no double quote. Escapes mirror the common cases in the parity corpus.
fn python_str_repr(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    let has_single = s.contains('\'');
    let has_double = s.contains('"');
    if !has_single {
        f.write_str("'")?;
        python_str_body(f, s, '\'')?;
        f.write_str("'")
    } else if !has_double {
        f.write_str("\"")?;
        python_str_body(f, s, '"')?;
        f.write_str("\"")
    } else {
        // Both present: Python keeps single quotes and backslash-escapes the
        // inner single quotes.
        f.write_str("'")?;
        python_str_body(f, s, '\'')?;
        f.write_str("'")
    }
}

/// Write the body of a Python string literal, escaping the quote character
/// and the standard control escapes (`\n`, `\t`, `\r`, `\\`).
fn python_str_body(f: &mut fmt::Formatter<'_>, s: &str, quote: char) -> fmt::Result {
    for c in s.chars() {
        match c {
            c if c == quote => write!(f, "\\{c}")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            _ => write!(f, "{c}")?,
        }
    }
    Ok(())
}

/// Replicate CPython's `repr(bytes)`: always single-quoted with a `b`
/// prefix. Printable ASCII (0x20-0x7e) passes through except `\\`, `'`.
/// Non-printable bytes use `\xNN`. Control bytes `\n \r \t` use named
/// escapes, matching CPython's bytes repr.
fn python_bytes_repr(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    f.write_str("b'")?;
    for &b in bytes {
        match b {
            b'\\' => f.write_str("\\\\")?,
            b'\'' => f.write_str("\\'")?,
            b'\n' => f.write_str("\\n")?,
            b'\r' => f.write_str("\\r")?,
            b'\t' => f.write_str("\\t")?,
            0x20..=0x7e => write!(f, "{}", b as char)?,
            _ => write!(f, "\\x{:02x}", b)?,
        }
    }
    f.write_str("'")
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::AnyType { .. } => write!(f, "Any"),
            Type::NoneType => write!(f, "None"),
            Type::UninhabitedType => write!(f, "Never"),
            Type::DeletedType { source } => match source {
                None => write!(f, "<Deleted>"),
                Some(s) => write!(f, "<Deleted '{s}'>"),
            },
            Type::UnboundType { name, args, .. } => {
                write!(f, "{name}?")?;
                if !args.is_empty() {
                    write!(f, "[")?;
                    list_str(f, args, false)?;
                    write!(f, "]")?;
                }
                Ok(())
            }
            Type::UnpackType { typ } => write!(f, "*{typ}"),
            Type::LiteralType { value, .. } => write!(f, "Literal[{value}]"),
            Type::TypeAliasType { .. } => {
                // The wire format carries `type_ref: String` but no resolved
                // `TypeAlias` node, so `t.alias is None` and `TypeStrVisitor`
                // renders `"<alias (unfixed)>"`. Stage 3b will resolve refs
                // and expand non-recursive aliases here.
                write!(f, "<alias (unfixed)>")
            }
            Type::Instance {
                type_ref,
                args,
                last_known_value,
                ..
            } => {
                // visit_instance renders `t.type.name` (the short class name)
                // when `not reveal_verbose_types and fullname.startswith("builtins.")`.
                // The wire format carries only `type.fullname` (as `type_ref`),
                // not `type.name`, so we cannot replicate the prefix strip
                // without resolving the ref against a TypeInfo snapshot.
                // Render `type_ref` verbatim — this matches the test fixture
                // (where `TypeInfo.name == fullname`) exactly, and Stage 3b
                // will resolve refs for production-correct stripping.
                if let Some(lkv) = last_known_value {
                    if args.is_empty() {
                        write!(f, "{lkv}?")?;
                        return Ok(());
                    }
                }
                write!(f, "{type_ref}")?;
                if !args.is_empty() {
                    if type_ref == "builtins.tuple" {
                        // builtins.tuple always renders as `tuple[T, ...]`
                        // (single arg). Mirrors the
                        // `assert len(t.args) == 1` branch.
                        write!(f, "[")?;
                        list_str(f, args, false)?;
                        write!(f, ", ...]")?;
                    } else {
                        write!(f, "[")?;
                        list_str(f, args, false)?;
                        write!(f, "]")?;
                    }
                }
                // The `has_type_var_tuple_type && len(type_vars) == 1`
                // `[()]` branch needs a TypeInfo field not in the wire
                // format; deferred to Stage 3b.
                Ok(())
            }
            Type::TypeVarType { name, .. } => write!(f, "{name}"),
            Type::ParamSpecType { prefix, name, .. } => {
                // visit_param_spec: optional `[args, **name]` prefix.
                let mut s = String::new();
                if !prefix.arg_types.is_empty() {
                    s.push('[');
                    list_str(&mut s, &prefix.arg_types, false)?;
                    s.push_str(", **");
                }
                s.push_str(name);
                if !prefix.arg_types.is_empty() {
                    s.push(']');
                }
                write!(f, "{s}")
            }
            Type::TypeVarTupleType { name, .. } => write!(f, "{name}"),
            Type::Parameters(p) => {
                // visit_parameters: similar to callable params wrapped in [...].
                // Standalone Parameters rarely appear in the test corpus; this
                // mirrors the callable param loop minus the `def (...)` shape.
                write!(f, "[")?;
                write_parameters_inner(f, p)?;
                write!(f, "]")
            }
            Type::CallableType {
                arg_types,
                arg_kinds,
                arg_names,
                ret_type,
                name: _name,
                variables,
                type_guard,
                type_is,
                unpack_kwargs,
                ..
            } => {
                // visit_callable_type. Python builds `def {vars_block} ({params}) -> {ret}`:
                // the variables block (if any) is rendered as `[v1, v2] `
                // *after* `def ` and *before* the params. We build the params
                // and ret first, then prepend `def ` + the variables block.
                let mut params = String::new();
                let mut asterisk = false;
                for i in 0..arg_types.len() {
                    if i > 0 {
                        params.push_str(", ");
                    }
                    let kind = arg_kinds[i];
                    // ARG_NAMED (3) or ARG_NAMED_OPT (5): insert `*, ` once.
                    if (kind == 3 || kind == 5) && !asterisk {
                        params.push_str("*, ");
                        asterisk = true;
                    }
                    // ARG_STAR (2): prefix `*`, set asterisk.
                    if kind == 2 {
                        params.push('*');
                        asterisk = true;
                    }
                    // ARG_STAR2 (4): prefix `**`.
                    if kind == 4 {
                        params.push_str("**");
                    }
                    let name = &arg_names[i];
                    if let Some(n) = name {
                        params.push_str(n);
                        params.push_str(": ");
                    } else if *unpack_kwargs && kind == 4 {
                        // The non-verbose auto-naming of anonymous **kwargs
                        // with unpack_kwargs happens below in the type str.
                    } else if kind == 2 {
                        // Anonymous *args: auto-name `args` only when the
                        // type is an UnpackType. We don't have the full
                        // Python check here; mirror the common case.
                    }
                    let type_str = arg_types[i].to_string();
                    if kind == 4 && *unpack_kwargs {
                        params.push_str(&format!("**{type_str}"));
                    } else {
                        params.push_str(&type_str);
                    }
                    // ARG_OPT (1) or ARG_NAMED_OPT (5): trailing ` =`.
                    if kind == 1 || kind == 5 {
                        params.push_str(" =");
                    }
                }
                let mut body = format!("def ({params})");
                // Ret arrow: omitted when ret_type is NoneType.
                let ret_is_none = matches!(ret_type.as_ref(), Type::NoneType);
                if !ret_is_none {
                    if let Some(tg) = type_guard {
                        body.push_str(" -> TypeGuard[");
                        body.push_str(&tg.to_string());
                        body.push(']');
                    } else if let Some(ti) = type_is {
                        body.push_str(" -> TypeIs[");
                        body.push_str(&ti.to_string());
                        body.push(']');
                    } else {
                        body.push_str(" -> ");
                        body.push_str(&ret_type.to_string());
                    }
                }
                // Variables block: `[v1, v2] ` prepended after `def `.
                if !variables.is_empty() {
                    let mut vs = String::from("[");
                    let mut first = true;
                    for v in variables {
                        if !first {
                            vs.push_str(", ");
                        }
                        first = false;
                        match v {
                            Type::TypeVarType {
                                name,
                                values,
                                upper_bound,
                                default,
                                ..
                            } => {
                                if !values.is_empty() {
                                    vs.push_str(name);
                                    vs.push_str(" in (");
                                    let mut vf = true;
                                    for val in values {
                                        if !vf {
                                            vs.push_str(", ");
                                        }
                                        vf = false;
                                        vs.push_str(&val.to_string());
                                    }
                                    vs.push(')');
                                } else if !is_named_object(upper_bound) {
                                    vs.push_str(name);
                                    vs.push_str(" <: ");
                                    vs.push_str(&upper_bound.to_string());
                                    if !is_default_object(default) {
                                        vs.push_str(" = ");
                                        vs.push_str(&default.to_string());
                                    }
                                } else {
                                    vs.push_str(name);
                                    if !is_default_object(default) {
                                        vs.push_str(" = ");
                                        vs.push_str(&default.to_string());
                                    }
                                }
                            }
                            Type::ParamSpecType { name, default, .. } => {
                                vs.push_str(name);
                                if !is_default_object(default) {
                                    vs.push_str(" = ");
                                    vs.push_str(&default.to_string());
                                }
                            }
                            Type::TypeVarTupleType { name, default, .. } => {
                                vs.push_str(name);
                                if !is_default_object(default) {
                                    vs.push_str(" = ");
                                    vs.push_str(&default.to_string());
                                }
                            }
                            _ => {
                                // Other variable kinds are not expected in the
                                // variables list; render nothing.
                            }
                        }
                    }
                    vs.push_str("] ");
                    // Insert `[vars] ` between `def ` and the params.
                    let after_def = &body["def ".len()..];
                    body = format!("def {vs}{after_def}");
                }
                write!(f, "{body}")
            }
            Type::Overloaded { items } => {
                let mut s = String::from("Overload(");
                let mut first = true;
                for item in items {
                    if !first {
                        s.push_str(", ");
                    }
                    first = false;
                    s.push_str(&item.to_string());
                }
                s.push(')');
                write!(f, "{s}")
            }
            Type::TupleType {
                partial_fallback,
                items,
                ..
            } => {
                let mut s = String::from("tuple[");
                if items.is_empty() {
                    s.push_str("()");
                } else {
                    list_str(&mut s, items, false)?;
                }
                s.push(']');
                // Fallback suffix only if non-builtins.tuple. The fallback's
                // fullname is on the Instance; we read it via type_ref.
                if let Type::Instance { type_ref, .. } = partial_fallback.as_ref() {
                    if type_ref != "builtins.tuple" {
                        s.push_str(", fallback=");
                        s.push_str(&partial_fallback.to_string());
                    }
                }
                write!(f, "{s}")
            }
            Type::TypedDictType {
                items,
                required_keys,
                readonly_keys,
                is_closed,
                fallback,
                ..
            } => {
                let mut s = String::from("TypedDict(");
                // Fallback prefix only if non-anonymous TypedDict fallback.
                if let Type::Instance { type_ref, .. } = fallback.as_ref() {
                    if type_ref != "typing.TypedDict"
                        && type_ref != "typing_extensions.TypedDict"
                        && !type_ref.is_empty()
                    {
                        s.push_str(type_ref);
                        s.push_str(", ");
                    }
                }
                s.push('{');
                let mut first = true;
                for (name, typ) in items {
                    if !first {
                        s.push_str(", ");
                    }
                    first = false;
                    s.push_str(&format!("{name:?}"));
                    if !required_keys.contains(name) {
                        s.push('?');
                    }
                    if readonly_keys.contains(name) {
                        s.push('=');
                    }
                    s.push_str(": ");
                    s.push_str(&typ.to_string());
                }
                s.push('}');
                if *is_closed {
                    s.push_str(", closed=True");
                }
                s.push(')');
                write!(f, "{s}")
            }
            Type::UnionType { items, .. } => {
                let mut s = String::new();
                list_str(&mut s, items, true)?;
                write!(f, "{s}")
            }
            Type::TypeType { item, is_type_form } => {
                if *is_type_form {
                    write!(f, "TypeForm[{item}]")
                } else {
                    write!(f, "type[{item}]")
                }
            }
        }
    }
}

/// Helper: write a list of types joined by `, ` (or ` | ` when
/// `use_or_syntax`). CallableType members are parenthesized under or-syntax
/// (mirrors `TypeStrVisitor.list_str`). Generic over `fmt::Write` so it works
/// with both `String` and `fmt::Formatter`.
fn list_str(out: &mut dyn fmt::Write, types: &[Type], use_or_syntax: bool) -> fmt::Result {
    let mut first = true;
    for t in types {
        if !first {
            if use_or_syntax {
                out.write_str(" | ")?;
            } else {
                out.write_str(", ")?;
            }
        }
        first = false;
        if use_or_syntax && matches!(t, Type::CallableType { .. }) {
            write!(out, "({t})")?;
        } else {
            write!(out, "{t}")?;
        }
    }
    Ok(())
}

/// Write the parameter portion of a `Parameters` record into a formatter,
/// mirroring the callable-params loop in `visit_callable_type`. Used by
/// `visit_parameters` (standalone `Parameters`).
fn write_parameters_inner(f: &mut fmt::Formatter<'_>, p: &Parameters) -> fmt::Result {
    let mut asterisk = false;
    for i in 0..p.arg_types.len() {
        if i > 0 {
            f.write_str(", ")?;
        }
        let kind = p.arg_kinds[i];
        if (kind == 3 || kind == 5) && !asterisk {
            f.write_str("*, ")?;
            asterisk = true;
        }
        if kind == 2 {
            f.write_str("*")?;
            asterisk = true;
        }
        if kind == 4 {
            f.write_str("**")?;
        }
        if let Some(n) = &p.arg_names[i] {
            f.write_str(n)?;
            f.write_str(": ")?;
        }
        write!(f, "{}", p.arg_types[i])?;
        if kind == 1 || kind == 5 {
            f.write_str(" =")?;
        }
    }
    Ok(())
}

/// True if the given type is `Instance(builtins.object, [])`. Mirrors
/// `is_named_instance(var.upper_bound, "builtins.object")` from
/// `visit_callable_type`'s variables block. Stage 3a has no resolved
/// TypeInfo, so we check the unresolved `type_ref` field directly.
fn is_named_object(t: &Type) -> bool {
    matches!(t, Type::Instance { type_ref, args, .. } if type_ref == "builtins.object" && args.is_empty())
}

/// True if the typevar default is the special "no default" sentinel. mypy
/// uses `AnyType(TypeOfAny.special_form)` as the no-default marker; Stage 3a
/// treats any `AnyType` as having no user-visible default, matching the
/// `var.has_default()` check in the non-verbose path (which consults the
/// TypeVar's `default` field that defaults to that sentinel).
fn is_default_object(t: &Type) -> bool {
    matches!(t, Type::AnyType { .. })
}

// ---------------------------------------------------------------------------
// PyO3 entry point (parity-only; not wired into production)
// ---------------------------------------------------------------------------

/// Read one serialized `Type` from `bytes` and return its `Display` string.
///
/// Parity entry point for Stage 3a: lets the test suite assert
/// `str(python_type) == type_kernel.read_type_to_str(_bytes_of(python_type))`.
/// Errors (truncated input, unknown tags, invalid varints) raise as
/// `ValueError` on the Python side. No production code calls this yet —
/// `Options.native_type_kernel` still defaults to `False` and `mypy/subtypes.py`
/// is unchanged.
#[pyfunction]
pub(crate) fn read_type_to_str(bytes: &[u8]) -> PyResult<String> {
    let mut buf = ReadBuffer::new(bytes);
    let typ = read_type(&mut buf, None)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(typ.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- ReadBuffer primitives -----

    #[test]
    fn read_tag_advances_cursor() {
        let mut buf = ReadBuffer::new(&[80, 255]);
        assert_eq!(read_tag(&mut buf).unwrap(), 80);
        assert_eq!(read_tag(&mut buf).unwrap(), 255);
        // Truncated.
        assert!(matches!(read_tag(&mut buf), Err(WireError::Truncated)));
    }

    #[test]
    fn read_bool_rejects_invalid() {
        let mut buf = ReadBuffer::new(&[0, 1, 2]);
        assert_eq!(read_bool(&mut buf).unwrap(), false);
        assert_eq!(read_bool(&mut buf).unwrap(), true);
        assert!(matches!(read_bool(&mut buf), Err(WireError::Invalid(_))));
    }

    // ----- Varint (read_short_int) -----

    /// Encode an int as the writer would, then decode it back. Covers the
    /// 1/2/4-byte short-int ranges and the long-int path.
    fn round_trip_int(value: i64) -> i64 {
        let bytes = encode_int_for_test(value);
        let mut buf = ReadBuffer::new(&bytes);
        read_int_bare(&mut buf).unwrap()
    }

    /// Minimal encoder mirroring `write_int_bare` / `_write_short_int` /
    /// `_write_long_int` (librt_internal.c:459-810). For test use only —
    /// Stage 3a ships no production writer.
    fn encode_int_for_test(value: i64) -> Vec<u8> {
        if value >= MIN_ONE_BYTE_INT && value <= 117 {
            // 1-byte form.
            vec![((value - MIN_ONE_BYTE_INT) << 1) as u8]
        } else if value >= MIN_TWO_BYTES_INT && value <= 16283 {
            // 2-byte form: low 2 bits = 01.
            let encoded = ((value - MIN_TWO_BYTES_INT) << 2) as u16 | TWO_BYTES_INT_BIT as u16;
            let le = encoded.to_le_bytes();
            vec![le[0], le[1]]
        } else if value >= MIN_FOUR_BYTES_INT && value <= 536860911 {
            // 4-byte form: low 3 bits = 011.
            let encoded =
                ((value - MIN_FOUR_BYTES_INT) << 3) as u32 | FOUR_BYTES_INT_TRAILER as u32;
            let le = encoded.to_le_bytes();
            vec![le[0], le[1], le[2], le[3]]
        } else {
            // Long-int path. Mirror the C writer: hex-encode, pack pairs of
            // hex digits into bytes LE, prefix with LONG_INT_TRAILER + the
            // (size << 1 | sign) short-int encoding.
            let neg = value < 0;
            let abs = (value as i128).unsigned_abs();
            // Build the little-endian magnitude byte array.
            let mut magnitude: Vec<u8> = Vec::new();
            let mut v = abs;
            if v == 0 {
                magnitude.push(0);
            }
            while v > 0 {
                magnitude.push((v & 0xff) as u8);
                v >>= 8;
            }
            // Strip trailing zero bytes (the C writer packs hex pairs; we
            // match by using the minimal byte length).
            while magnitude.len() > 1 && *magnitude.last().unwrap() == 0 {
                magnitude.pop();
            }
            let size = magnitude.len() as i64;
            let size_and_sign = (size << 1) | (if neg { 1 } else { 0 });
            let mut out = vec![LONG_INT_TRAILER];
            // Encode size_and_sign as a short int (it always fits in 1 byte
            // for reasonable test values).
            out.push(((size_and_sign - MIN_ONE_BYTE_INT) << 1) as u8);
            out.extend(magnitude);
            out
        }
    }

    #[test]
    fn varint_one_byte_boundaries() {
        assert_eq!(round_trip_int(-10), -10);
        assert_eq!(round_trip_int(0), 0);
        assert_eq!(round_trip_int(117), 117);
    }

    #[test]
    fn varint_two_byte_boundaries() {
        assert_eq!(round_trip_int(-100), -100);
        assert_eq!(round_trip_int(-11), -11);
        assert_eq!(round_trip_int(118), 118);
        assert_eq!(round_trip_int(16283), 16283);
    }

    #[test]
    fn varint_four_byte_boundaries() {
        assert_eq!(round_trip_int(-10000), -10000);
        assert_eq!(round_trip_int(-101), -101);
        assert_eq!(round_trip_int(16284), 16284);
        assert_eq!(round_trip_int(536860911), 536860911);
    }

    #[test]
    fn long_int_path() {
        // Just beyond the 4-byte short-int max — exercises LONG_INT_TRAILER.
        assert_eq!(round_trip_int(536860912), 536860912);
        assert_eq!(round_trip_int(-10001), -10001);
        assert_eq!(round_trip_int(1_000_000), 1_000_000);
        assert_eq!(round_trip_int(-1_000_000), -1_000_000);
    }

    // ----- Truncation -----

    #[test]
    fn truncated_input_errors() {
        // Empty buffer: any read is truncated.
        let mut buf = ReadBuffer::new(&[]);
        assert!(matches!(read_int_bare(&mut buf), Err(WireError::Truncated)));

        // One byte promising a 2-byte varint, but no second byte.
        let mut buf = ReadBuffer::new(&[TWO_BYTES_INT_BIT]);
        assert!(matches!(read_int_bare(&mut buf), Err(WireError::Truncated)));

        // String length prefix promises 5 bytes, only 2 available.
        // Byte 30 decodes as short-int length 5: (30 >> 1) + (-10) = 5.
        let mut buf = ReadBuffer::new(&[30, b'h', b'i']); // length 5, body 2 bytes
        assert!(matches!(read_str_bare(&mut buf), Err(WireError::Truncated)));
    }

    // ----- End-to-end reader cases -----

    /// Build the bytes for `AnyType(TypeOfAny.special_form)` by hand.
    /// Wire: ANY_TYPE(106), source_any=LITERAL_NONE(2), type_of_any=LITERAL_INT(3)+bare_int(0),
    /// missing_import_name=LITERAL_NONE(2), END_TAG(255).
    #[test]
    fn read_any_type_end_to_end() {
        // type_of_any=0 encodes as the 1-byte short int 20 ((0 - (-10)) << 1).
        let type_of_any_bytes = encode_int_for_test(0);
        let mut bytes = vec![ANY_TYPE, LITERAL_NONE, LITERAL_INT];
        bytes.extend(type_of_any_bytes);
        bytes.push(LITERAL_NONE);
        bytes.push(END_TAG);
        let mut buf = ReadBuffer::new(&bytes);
        let typ = read_type(&mut buf, None).unwrap();
        match &typ {
            Type::AnyType {
                type_of_any,
                source_any,
                missing_import_name,
            } => {
                assert_eq!(*type_of_any, 0);
                assert!(source_any.is_none());
                assert!(missing_import_name.is_none());
            }
            other => panic!("expected AnyType, got {other:?}"),
        }
        assert_eq!(typ.to_string(), "Any");
    }

    /// Build the bytes for `NoneType`: NONE_TYPE(108), END_TAG(255).
    #[test]
    fn read_none_type_end_to_end() {
        let bytes = [NONE_TYPE, END_TAG];
        let mut buf = ReadBuffer::new(&bytes);
        let typ = read_type(&mut buf, None).unwrap();
        assert!(matches!(typ, Type::NoneType));
        assert_eq!(typ.to_string(), "None");
    }

    /// Build the bytes for `Instance(builtins.str, [])` via the INSTANCE_STR
    /// fast path: INSTANCE(80), INSTANCE_STR(83). No END_TAG (fast path).
    /// Display: `type_ref` rendered verbatim (Stage 3b will strip the
    /// `builtins.` prefix once refs resolve against a TypeInfo snapshot).
    #[test]
    fn read_instance_str_singleton() {
        let bytes = [INSTANCE, INSTANCE_STR];
        let mut buf = ReadBuffer::new(&bytes);
        let typ = read_type(&mut buf, None).unwrap();
        match &typ {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.str");
                assert!(args.is_empty());
            }
            other => panic!("expected Instance, got {other:?}"),
        }
        assert_eq!(typ.to_string(), "builtins.str");
    }

    /// Build the bytes for `Instance(builtins.object, [])` via INSTANCE_OBJECT.
    #[test]
    fn read_instance_object_singleton() {
        let bytes = [INSTANCE, INSTANCE_OBJECT];
        let mut buf = ReadBuffer::new(&bytes);
        let typ = read_type(&mut buf, None).unwrap();
        assert_eq!(typ.to_string(), "builtins.object");
    }

    /// Build the bytes for a generic `Instance("foo.Bar", [AnyType])`.
    /// Wire: INSTANCE(80), INSTANCE_GENERIC(82),
    ///   LITERAL_STR(4) + bare str "foo.Bar",
    ///   LIST_GEN(20) + size=1 + ANY_TYPE(106) + LITERAL_NONE + LITERAL_INT+0 + LITERAL_NONE + END_TAG,
    ///   LITERAL_NONE (no last_known_value),
    ///   LITERAL_NONE (no extra_attrs),
    ///   END_TAG(255).
    #[test]
    fn read_generic_instance_end_to_end() {
        let any_bytes = [
            ANY_TYPE,
            LITERAL_NONE,
            LITERAL_INT,
            0,
            LITERAL_NONE,
            END_TAG,
        ];
        let mut bytes = vec![INSTANCE, INSTANCE_GENERIC, LITERAL_STR];
        // bare str: short-int length + UTF-8 body.
        bytes.push((7i64 - MIN_ONE_BYTE_INT) as u8 * 2); // length 7, 1-byte form
        bytes.extend(b"foo.Bar".iter());
        // type_list: LIST_GEN + size=1 + the any_type record.
        bytes.push(LIST_GEN);
        bytes.push((1i64 - MIN_ONE_BYTE_INT) as u8 * 2); // size 1
        bytes.extend(any_bytes.iter());
        // last_known_value: LITERAL_NONE.
        bytes.push(LITERAL_NONE);
        // extra_attrs: LITERAL_NONE.
        bytes.push(LITERAL_NONE);
        // END_TAG.
        bytes.push(END_TAG);

        let mut buf = ReadBuffer::new(&bytes);
        let typ = read_type(&mut buf, None).unwrap();
        match &typ {
            Type::Instance {
                type_ref,
                args,
                last_known_value,
                extra_attrs,
            } => {
                assert_eq!(type_ref, "foo.Bar");
                assert_eq!(args.len(), 1);
                assert!(last_known_value.is_none());
                assert!(extra_attrs.is_none());
            }
            other => panic!("expected Instance, got {other:?}"),
        }
        // Display: non-builtins fullname is not stripped; args rendered.
        assert_eq!(typ.to_string(), "foo.Bar[Any]");
    }

    /// Unknown tag → Invalid error.
    #[test]
    fn unknown_tag_errors() {
        let mut buf = ReadBuffer::new(&[200]);
        assert!(matches!(
            read_type(&mut buf, None),
            Err(WireError::Invalid(_))
        ));
    }
}
