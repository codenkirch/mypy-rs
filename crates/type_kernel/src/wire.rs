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
//! `write_type` mirrors `Type.write` for the round-trip seams (`rust_type_analyze`,
//! solve_one, etc.); `TypeAliasType` serializes its tagged args + `type_ref`
//! string because the Python `write` insists on a live alias node but the
//! kernel only ever sees the wire form (alias=None, type_ref set).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;

use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Constants — copied verbatim from librt_internal.c:14-28 and cache.py:301-328.
// ---------------------------------------------------------------------------

// Varint width constants (librt_internal.c:14-19).
const MIN_ONE_BYTE_INT: i64 = -10;
const MAX_ONE_BYTE_INT: i64 = 117;
const MIN_TWO_BYTES_INT: i64 = -100;
const MAX_TWO_BYTES_INT: i64 = 16283;
const MIN_FOUR_BYTES_INT: i64 = -10000;
const MAX_FOUR_BYTES_INT: i64 = 536860911;

// Varint bit flags (librt_internal.c:21-25).
const TWO_BYTES_INT_BIT: u8 = 1;
const FOUR_BYTES_INT_BIT: u8 = 2;
#[allow(dead_code)]
const FOUR_BYTES_INT_TRAILER: u8 = 3;
const LONG_INT_TRAILER: u8 = 15;

// Primitive literal tags (cache.py:303-310).
const LITERAL_FALSE: u8 = 0;
const LITERAL_TRUE: u8 = 1;
pub(crate) const LITERAL_NONE: u8 = 2;
const LITERAL_INT: u8 = 3;
pub(crate) const LITERAL_STR: u8 = 4;
const LITERAL_BYTES: u8 = 5;
const LITERAL_FLOAT: u8 = 6;

// Collection tags (cache.py:313-318).
pub(crate) const LIST_GEN: u8 = 20;
pub(crate) const LIST_INT: u8 = 21;
pub(crate) const LIST_STR: u8 = 22;
pub(crate) const DICT_STR_GEN: u8 = 30;

// Misc class tags (cache.py:322-325).
const EXTRA_ATTRS: u8 = 150;

// Reserved / end markers (cache.py:327-328).
pub(crate) const END_TAG: u8 = 255;

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
pub(crate) const TYPE_ALIAS_TYPE: u8 = 100;
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
pub(crate) const ERASED_TYPE: u8 = 122;

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
    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
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
    pub(crate) fn read_u8(&mut self) -> Result<u8, WireError> {
        self.ensure(1)?;
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    /// Read `n` bytes as a slice (advances cursor; caller does not copy).
    pub(crate) fn read_slice(&mut self, n: usize) -> Result<&'a [u8], WireError> {
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
pub(crate) fn read_tag(buf: &mut ReadBuffer<'_>) -> Result<u8, WireError> {
    buf.read_u8()
}

/// Peek the next tag without advancing (None on truncated input).
pub(crate) fn peek_tag(buf: &ReadBuffer<'_>) -> Option<u8> {
    buf.data.get(buf.pos).copied()
}

/// Read a bool (1 byte: 0=False, 1=True, else Invalid). Mirrors `read_bool`.
pub(crate) fn read_bool(buf: &mut ReadBuffer<'_>) -> Result<bool, WireError> {
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
pub(crate) fn read_short_int(buf: &mut ReadBuffer<'_>, first: u8) -> Result<i64, WireError> {
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
    let big = read_long_int_big(buf)?;
    // Non-literal int fields (lengths, ids, kinds) are bounded far below
    // i64 in every real serialized tree, so an overflow here is corrupt
    // input: fail fast instead of silently wrapping.
    i64::try_from(big).map_err(|_| WireError::invalid("int exceeds i64 range"))
}

/// Read an arbitrary-precision long-int into a `BigInt`. The encoded
/// magnitude is unbounded, so this is the primitive shared by
/// `read_long_int` (i64 fail-fast for non-literal fields) and the
/// literal reader, which carries the full value (issue #1329).
fn read_long_int_big(buf: &mut ReadBuffer<'_>) -> Result<BigInt, WireError> {
    // Short-int encoding: (size << 1) | sign.
    // read_short_int returns raw value, so we extract directly:
    let first = buf.read_u8()?;
    let size_and_sign = read_short_int(buf, first)?;
    if size_and_sign < 0 {
        return Err(WireError::invalid("invalid int data"));
    }
    let sign = size_and_sign & 1;
    let size = (size_and_sign >> 1) as usize;
    let magnitude_bytes = buf.read_slice(size)?;
    Ok(BigInt::from_le_bytes(magnitude_bytes, sign == 1))
}

/// Read a bare integer (the librt `read_int` / `read_int_bare` primitive).
/// Dispatches short-int vs long-int based on the first byte.
pub(crate) fn read_int_bare(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
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
pub(crate) fn read_str_bare(buf: &mut ReadBuffer<'_>) -> Result<String, WireError> {
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
pub(crate) fn read_int(buf: &mut ReadBuffer<'_>) -> Result<i64, WireError> {
    let tag = read_tag(buf)?;
    if tag != LITERAL_INT {
        return Err(WireError::invalid(format!(
            "expected LITERAL_INT, got tag {tag}"
        )));
    }
    read_int_bare(buf)
}

/// `read_str`: tag byte must be `LITERAL_STR`, then bare str.
pub(crate) fn read_str(buf: &mut ReadBuffer<'_>) -> Result<String, WireError> {
    let tag = read_tag(buf)?;
    if tag != LITERAL_STR {
        return Err(WireError::invalid(format!(
            "expected LITERAL_STR, got tag {tag}"
        )));
    }
    read_str_bare(buf)
}

/// `read_str_opt`: `LITERAL_NONE` → None, else `LITERAL_STR` + bare str.
pub(crate) fn read_str_opt(buf: &mut ReadBuffer<'_>) -> Result<Option<String>, WireError> {
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
pub(crate) fn read_int_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<i64>, WireError> {
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

/// An arbitrary-precision signed integer, carried by `LiteralValue` for
/// literal ints whose magnitude exceeds i64 (issue #1329). Canonical form:
/// little-endian magnitude with no leading (trailing) zero bytes; the value
/// 0 is an empty magnitude with `neg == false`. `PartialEq`/`Eq`/`Hash` are
/// therefore value-equality on the canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BigInt {
    neg: bool,
    magnitude: Vec<u8>,
}

impl BigInt {
    /// Build from raw little-endian unsigned magnitude bytes plus a sign,
    /// normalizing zero and any non-canonical leading zero bytes (the
    /// C writer never emits them, but defensive parity is cheap).
    fn from_le_bytes(bytes: &[u8], neg: bool) -> BigInt {
        let mut magnitude = bytes.to_vec();
        while let Some(&0) = magnitude.last() {
            magnitude.pop();
        }
        BigInt {
            neg: neg && !magnitude.is_empty(),
            magnitude,
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.magnitude.is_empty()
    }

    /// Narrow to i64 when the value fits; `None` otherwise.
    fn to_i64(&self) -> Option<i64> {
        if self.magnitude.len() > 8 {
            return None;
        }
        let mut value: u128 = 0;
        for &b in self.magnitude.iter().rev() {
            value = (value << 8) | (b as u128);
        }
        if self.neg {
            if value <= (i64::MAX as u128) + 1 {
                Some(-(value as i128) as i64)
            } else {
                None
            }
        } else if value <= i64::MAX as u128 {
            Some(value as i64)
        } else {
            None
        }
    }

    /// Magnitude for the wire writer: at least one byte, matching the C
    /// writer's `[0]` byte for value 0 and canonical minimal LE otherwise.
    fn wire_magnitude(&self) -> Vec<u8> {
        if self.magnitude.is_empty() {
            vec![0]
        } else {
            self.magnitude.clone()
        }
    }

    /// Decimal digits, most significant first. Mirrors `str(int)` in
    /// Python (`LiteralValue::Display` renders int literals as decimal).
    fn decimal_digits(&self) -> Vec<u8> {
        const CHUNK: u128 = 10_000_000_000_000_000_000; // 10^19
        let mut work = self.magnitude.clone();
        let mut chunks: Vec<u128> = Vec::new();
        while !work.is_empty() {
            chunks.push(divmod_in_place(&mut work, CHUNK));
        }
        if chunks.is_empty() {
            return vec![b'0'];
        }
        let mut digits = Vec::new();
        let mut last = chunks.len() - 1;
        digits.extend_from_slice(chunks[last].to_string().as_bytes());
        while last > 0 {
            last -= 1;
            // Zero-pad each lower chunk to the full 19 digits.
            let text = chunks[last].to_string();
            digits.extend(std::iter::repeat_n(b'0', 19 - text.len()));
            digits.extend_from_slice(text.as_bytes());
        }
        digits
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.neg && !self.is_zero() {
            write!(f, "-")?;
        }
        for d in self.decimal_digits() {
            write!(f, "{}", d as char)?;
        }
        Ok(())
    }
}

impl TryFrom<BigInt> for i64 {
    type Error = ();

    fn try_from(big: BigInt) -> Result<i64, ()> {
        big.to_i64().ok_or(())
    }
}

/// Divide a little-endian magnitude by `divisor` in place, returning the
/// remainder. Schoolbook short division, most significant byte first.
fn divmod_in_place(magnitude: &mut Vec<u8>, divisor: u128) -> u128 {
    let mut carry: u128 = 0;
    for byte in magnitude.iter_mut().rev() {
        carry = (carry << 8) | (*byte as u128);
        *byte = (carry / divisor) as u8;
        carry %= divisor;
    }
    while let Some(&0) = magnitude.last() {
        magnitude.pop();
    }
    carry
}

/// A literal value as stored by `write_literal` (cache.py:347-364): the tag
/// byte is already consumed by the caller (it was the discriminator), and
/// this reads the body.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) enum LiteralValue {
    Int(i64),
    /// Int literal whose magnitude exceeds i64 (issue #1329). Small ints
    /// stay in `Int(i64)`; the variant a value takes is a pure function of
    /// the value, so same-value equality never crosses variants.
    BigInt(BigInt),
    Str(String),
    Bytes(Vec<u8>),
    Bool(bool),
    Float(f64),
}

fn read_literal(buf: &mut ReadBuffer<'_>, tag: u8) -> Result<LiteralValue, WireError> {
    match tag {
        LITERAL_INT => Ok(read_int_literal(buf)?),
        LITERAL_STR => Ok(LiteralValue::Str(read_str_bare(buf)?)),
        LITERAL_BYTES => Ok(LiteralValue::Bytes(read_bytes_bare(buf)?)),
        LITERAL_FALSE => Ok(LiteralValue::Bool(false)),
        LITERAL_TRUE => Ok(LiteralValue::Bool(true)),
        LITERAL_FLOAT => Ok(LiteralValue::Float(read_float_bare(buf)?)),
        _ => Err(WireError::invalid(format!("unknown literal tag {tag}"))),
    }
}

/// Read an `LITERAL_INT` body: a short int when the value fits i64 (the
/// overwhelmingly common case, kept on the zero-alloc `Int(i64)` path) and
/// a `BigInt` long int otherwise (issue #1329).
fn read_int_literal(buf: &mut ReadBuffer<'_>) -> Result<LiteralValue, WireError> {
    let first = buf.read_u8()?;
    if first != LONG_INT_TRAILER {
        return Ok(LiteralValue::Int(read_short_int(buf, first)?));
    }
    let big = read_long_int_big(buf)?;
    match i64::try_from(big.clone()) {
        Ok(v) => Ok(LiteralValue::Int(v)),
        Err(_) => Ok(LiteralValue::BigInt(big)),
    }
}

// ---------------------------------------------------------------------------
// Type enum (mirrors the 19 serialized Type subclasses)
// ---------------------------------------------------------------------------

/// `mypy.types.ExtraAttrs` — module-attribute summary attached to `Instance`.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct ExtraAttrs {
    pub attrs: HashMap<String, Type>,
    pub immutable: HashSet<String>,
    pub mod_name: Option<String>,
}

/// `mypy.types.Parameters` — a standalone parameter list (used by
/// `ParamSpecType.prefix` and as the `PARAMETERS` tag).
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct Parameters {
    pub arg_types: Vec<Type>,
    pub arg_kinds: Vec<i64>,
    pub arg_names: Vec<Option<String>>,
    pub variables: Vec<Type>,
    pub imprecise_arg_kinds: bool,
    pub is_ellipsis_args: bool,
}

/// `mypy.types.Type` — one variant per serialized subclass. `Instance.type_ref`
/// and `TypeAliasType.type_ref` carry the unresolved fullname string (the
/// wire format's honest state before `TypeFixer` runs).
///
/// Fields that aren't read by the Stage 3a `Display` impl are intentionally
/// kept: they will be consumed by Stage 3b (`TypeInfo` snapshot) and 3c
/// (`is_subtype`), and storing them now keeps the reader byte-exact against
/// the Python wire format.
#[derive(Debug, Clone, PartialEq)]
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
    /// `is_recursive` rides the wire as a tagged conditional int emitted only
    /// when True (wave31, #1361); here it is a plain field, defaulting to the
    /// `false` shape kernel-constructed aliases carry.
    TypeAliasType {
        args: Vec<Type>,
        type_ref: String,
        is_recursive: bool,
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
        meta_level: i64,
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
        meta_level: i64,
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
        meta_level: i64,
    },
    UnboundType {
        name: String,
        args: Vec<Type>,
        original_str_expr: Option<String>,
        original_str_fallback: Option<String>,
        // Plain data mypy.types carries but the wire never serializes
        // (Phase F0, #1349): the reader fills Python defaults, the
        // writer never emits them. See doc/f0_coverage.md.
        optional: bool,
        empty_tuple_index: bool,
    },
    UnpackType {
        typ: Box<Type>,
        /// `UnboundType.from_star_syntax` on the unpacked target side; see
        /// the plain-data note on `UnboundType`.
        from_star_syntax: bool,
    },
    AnyType {
        type_of_any: i64,
        source_any: Option<Box<Type>>,
        missing_import_name: Option<String>,
    },
    UninhabitedType {
        ambiguous: bool,
    },
    NoneType,
    ErasedType,
    DeletedType {
        source: Option<String>,
    },
    CallableType {
        fallback: Box<Type>,
        instance_type: Option<Box<Type>>,
        // 7 flags, in write order: is_ellipsis_args, implicit, is_bound,
        // from_concatenate, imprecise_arg_kinds, unpack_kwargs, from_type_type.
        is_ellipsis_args: bool,
        implicit: bool,
        is_bound: bool,
        from_concatenate: bool,
        imprecise_arg_kinds: bool,
        unpack_kwargs: bool,
        from_type_type: bool,
        arg_types: Vec<Type>,
        arg_kinds: Vec<i64>,
        arg_names: Vec<Option<String>>,
        ret_type: Box<Type>,
        name: Option<String>,
        variables: Vec<Type>,
        type_guard: Option<Box<Type>>,
        type_is: Option<Box<Type>>,
        /// `CallableType.special_sig` ("partial" or None) — round-trips on
        /// the wire, serialized after `name`.
        special_sig: Option<String>,
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
        can_be_true: bool,
        can_be_false: bool,
        // Plain-data fields `mypy.types.UnionType` carries but the wire
        // format does not serialize (Phase F0, #1349). Rust-resident only.
        is_evaluated: bool,
        original_str_expr: Option<String>,
        original_str_fallback: Option<String>,
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
pub(crate) fn read_type_list(buf: &mut ReadBuffer<'_>) -> Result<Vec<Type>, WireError> {
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

/// The optional trailing `meta_level` append shared by the TypeVar /
/// ParamSpec / TypeVarTuple readers: written only when non-zero, so the
/// next tag is either `LITERAL_INT` (value present, itself followed by
/// END_TAG) or END_TAG (absent, defaults to 0). `what` labels error
/// messages so parity failures stay diagnosable per reader.
fn read_optional_trailing_meta_level(
    buf: &mut ReadBuffer<'_>,
    what: &str,
) -> Result<i64, WireError> {
    match read_tag(buf)? {
        LITERAL_INT => {
            let ml = read_int_bare(buf)?;
            // The writer always appends END_TAG after an optional
            // meta_level: with LITERAL_INT the END_TAG is still in the
            // stream; consume it so back-to-back records stay aligned.
            let end = read_tag(buf)?;
            if end != END_TAG {
                return Err(WireError::invalid(format!(
                    "expected END_TAG (255) after {what}, got tag {end}"
                )));
            }
            Ok(ml)
        }
        END_TAG => Ok(0),
        other => Err(WireError::invalid(format!(
            "expected END_TAG (255) or LITERAL_INT ({what}), got tag {other}"
        ))),
    }
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
    // Backward-compatible meta_level append: written only when non-zero,
    // so the next tag is either LITERAL_INT (meta_level present) or
    // END_TAG (absent, defaults to 0).
    let meta_level = read_optional_trailing_meta_level(buf, "meta_level")?;
    Ok(Type::TypeVarType {
        name,
        fullname,
        raw_id,
        namespace,
        values,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
        variance,
        meta_level,
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
    // Backward-compatible meta_level append (same as read_type_var_type):
    // written only when non-zero, so the next tag is either LITERAL_INT
    // (meta_level present, itself followed by END_TAG) or END_TAG (absent).
    let meta_level = read_optional_trailing_meta_level(buf, "ParamSpec meta_level")?;
    Ok(Type::ParamSpecType {
        prefix: Box::new(prefix),
        name,
        fullname,
        raw_id,
        namespace,
        flavor,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
        meta_level,
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
    // Backward-compatible meta_level append (same as read_type_var_type):
    // written only when non-zero, so the next tag is either LITERAL_INT
    // (meta_level present, itself followed by END_TAG) or END_TAG (absent).
    let meta_level = read_optional_trailing_meta_level(buf, "TypeVarTuple meta_level")?;
    Ok(Type::TypeVarTupleType {
        tuple_fallback: Box::new(tuple_fallback),
        name,
        fullname,
        raw_id,
        namespace,
        upper_bound: Box::new(upper_bound),
        default: Box::new(default),
        min_len,
        meta_level,
    })
}

/// Read a `Parameters` record (tag already consumed).
fn read_parameters(buf: &mut ReadBuffer<'_>) -> Result<Parameters, WireError> {
    let arg_types = read_type_list(buf)?;
    let arg_kinds = read_int_list(buf)?;
    let arg_names = read_str_opt_list(buf)?;
    let variables = read_type_var_likes(buf)?;
    let imprecise_arg_kinds = read_bool(buf)?;
    let is_ellipsis_args = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Parameters {
        arg_types,
        arg_kinds,
        arg_names,
        variables,
        imprecise_arg_kinds,
        is_ellipsis_args,
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
        // Plain-data fields the wire format does not carry (Phase F0,
        // #1349): fill from defaults so Rust-side trees still model them.
        optional: false,
        empty_tuple_index: false,
    })
}

/// Read an `UnpackType` (tag already consumed).
fn read_unpack_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let typ = read_type(buf, None)?;
    expect_end_tag(buf)?;
    Ok(Type::UnpackType {
        typ: Box::new(typ),
        // Wire format does not carry `from_star_syntax` (Phase F0, #1349).
        from_star_syntax: false,
    })
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

/// Read an `ErasedType` (tag already consumed) — just the END_TAG.
fn read_erased_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    expect_end_tag(buf)?;
    Ok(Type::ErasedType)
}

/// Read an `UninhabitedType` (tag already consumed): the ambiguous flag
/// bool, then END_TAG. Older writers omit the bool (END_TAG follows the
/// tag directly); that reads back as ambiguous=false.
fn read_uninhabited_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let first = buf.read_u8()?;
    let ambiguous = match first {
        u8::MAX => false, // old format: END_TAG, no bool written
        0 => false,
        1 => true,
        other => {
            return Err(WireError::invalid(format!(
                "invalid ambiguous value {other}"
            )))
        }
    };
    if first != u8::MAX {
        expect_end_tag(buf)?;
    }
    Ok(Type::UninhabitedType { ambiguous })
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
    let flags = read_flags(buf, 7)?;
    let mut flags_iter = flags.into_iter();
    let mut next_flag = || -> bool { flags_iter.next().unwrap_or(false) };
    let is_ellipsis_args = next_flag();
    let implicit = next_flag();
    let is_bound = next_flag();
    let from_concatenate = next_flag();
    let imprecise_arg_kinds = next_flag();
    let unpack_kwargs = next_flag();
    let from_type_type = next_flag();
    let arg_types = read_type_list(buf)?;
    let arg_kinds = read_int_list(buf)?;
    let arg_names = read_str_opt_list(buf)?;
    let ret_type = read_type(buf, None)?;
    let name = read_str_opt(buf)?;
    let special_sig = read_str_opt(buf)?;
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
        from_type_type,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type: Box::new(ret_type),
        name,
        variables,
        type_guard: type_guard.map(Box::new),
        type_is: type_is.map(Box::new),
        special_sig,
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
    // Truthiness flags (cache wire layout >= 11): written by
    // `UnionType.write` after the uses_pep604_syntax bool. The Python
    // `read` consumes them in the same order.
    let can_be_true = read_bool(buf)?;
    let can_be_false = read_bool(buf)?;
    expect_end_tag(buf)?;
    Ok(Type::UnionType {
        items,
        uses_pep604_syntax,
        can_be_true,
        can_be_false,
        // Plain-data fields the wire format does not carry (Phase F0,
        // #1349): fill from defaults so Rust-side trees still model them.
        is_evaluated: true,
        original_str_expr: None,
        original_str_fallback: None,
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
///
/// Mirrors `TypeAliasType.write` in types.py: the recursion flag is a
/// tagged conditional int appended only when True (same pattern as
/// `TypeVarType.meta_level`). The flag lands in the variant's
/// `is_recursive` field, which consumers like the `is_recursive_pair`
/// seam read directly.
fn read_type_alias_type(buf: &mut ReadBuffer<'_>) -> Result<Type, WireError> {
    let (t, _is_rec) = read_type_alias_type_flagged(buf)?;
    Ok(t)
}

/// Read a `TypeAliasType` and report the trailing recursion flag
/// (`true` iff the writer emitted the conditional int).
fn read_type_alias_type_flagged(buf: &mut ReadBuffer<'_>) -> Result<(Type, bool), WireError> {
    let args = read_type_list(buf)?;
    let type_ref = read_str(buf)?;
    let is_rec = match read_tag(buf)? {
        LITERAL_INT => {
            let flag = read_int_bare(buf)?;
            // The writer ends with a conditional int then END_TAG; the
            // LITERAL_INT read consumed the int but not END_TAG, so
            // consume it to keep back-to-back records aligned.
            let end = read_tag(buf)?;
            if end != END_TAG {
                return Err(WireError::invalid(format!(
                    "expected END_TAG (255) after TypeAliasType recursion flag, got tag {end}"
                )));
            }
            flag != 0
        }
        END_TAG => false,
        other => {
            return Err(WireError::invalid(format!(
                "expected END_TAG (255) or LITERAL_INT (TypeAliasType recursion flag), got tag \
                 {other}"
            )));
        }
    };
    // Mirror the Python reader: the LITERAL_INT flag is consumed before
    // END_TAG, so it is never seen again after this helper returns.
    Ok((
        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive: is_rec,
        },
        is_rec,
    ))
}

/// Read ONLY the trailing recursion flag of a serialized
/// `TypeAliasType` without decoding into a `Type`. `None` when the
/// bytes do not carry a `TypeAliasType` at the top level or do not
/// parse (the caller defers to Python).
#[pyfunction]
pub(crate) fn read_alias_recursion_flag(bytes: &[u8]) -> Option<bool> {
    let mut buf = ReadBuffer::new(bytes);
    let tag = read_tag(&mut buf).ok()?;
    if tag != TYPE_ALIAS_TYPE {
        return None;
    }
    read_type_alias_type_flagged(&mut buf).ok().map(|(_, r)| r)
}

/// Assert the next byte is `END_TAG`.
/// Mirrors Python `assert read_tag(data) == END_TAG`.
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
        ERASED_TYPE => read_erased_type(buf),
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
            LiteralValue::BigInt(v) => write!(f, "{v}"),
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
            c if c.is_control() => write!(f, "\\x{:02x}", c as u32)?,
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
            Type::ErasedType => write!(f, "<Erased>"),
            Type::UninhabitedType { .. } => write!(f, "Never"),
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
            Type::UnpackType { typ, .. } => write!(f, "*{typ}"),
            Type::LiteralType { value, .. } => write!(f, "Literal[{value}]"),
            Type::TypeAliasType { args, .. } => {
                // Wire `type_ref` has no resolved alias node: renders "<alias (unfixed)>".
                write!(f, "<alias (unfixed)>")?;
                if !args.is_empty() {
                    write!(f, "[")?;
                    list_str(f, args, false)?;
                    write!(f, "]")?;
                }
                Ok(())
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
/// uses `AnyType(TypeOfAny.from_omitted_generics)` (enum value 4) as the
/// no-default marker; any other AnyType (e.g. special_form) is a real,
/// user-visible default. Mirrors the `has_default()` check in
/// `visit_callable_type`'s variables block.
fn is_default_object(t: &Type) -> bool {
    matches!(t, Type::AnyType { type_of_any: 4, .. })
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

// ---------------------------------------------------------------------------
// WriteBuffer + write_type (Stage 3c M8s)
// ---------------------------------------------------------------------------

// The reader above is the source of truth for the byte layout: every
// `write_*` here is the exact inverse of the corresponding `read_*`, so
// `read_type(&write_type(t)) == t` over the supported variants. Mirrors

// `Type.write(WriteBuffer)` in mypy/types.py and the bare primitives in
// librt_internal.c. Only the variants the set-ops visitors can produce
// (leaf types + args-less Instance + TypeType) are implemented; other

// variants return `Err(WireError::Invalid(...))` so callers fail loudly
// rather than emitting malformed bytes.

/// Append-only byte buffer mirroring librt's `WriteBuffer` C type.
pub(crate) struct WriteBuffer {
    out: Vec<u8>,
}

impl WriteBuffer {
    pub(crate) fn new() -> Self {
        WriteBuffer { out: Vec::new() }
    }

    /// The encoded bytes (consumes the buffer).
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.out
    }

    pub(crate) fn push(&mut self, byte: u8) {
        self.out.push(byte);
    }

    pub(crate) fn extend(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }
}

/// `write_tag`: a single byte.
pub(crate) fn write_tag(buf: &mut WriteBuffer, tag: u8) {
    buf.push(tag);
}

/// `write_bool`: 0 or 1.
pub(crate) fn write_bool(buf: &mut WriteBuffer, value: bool) {
    buf.push(if value { 1 } else { 0 });
}

/// `write_int_bare`: the tagged-int encoding inverse of `read_int_bare`.
///
/// Layout mirrors librt_internal.c `write_int_internal` / `_write_short_int`
/// (wire.rs:185-212). Three width tiers, chosen by value range:
/// - 1-byte:  value in [-10, 117], payload = (value + 10) << 1 (low bit 0)
/// - 2-byte: value in [-100, 16283], payload = (value + 100) << 2 | 0b01
///   (byte0 low 2 bits = 0b01, byte1 = high 8 bits)
/// - 4-byte: value in [-10000, 536860911], payload = (value + 10000) << 3
///   | 0b011 (byte0 low 3 bits = 0b011, byte1 = bits 3..8, bytes 2-3 =
///   bits 8..21 little-endian)
///
/// Values outside the 4-byte short-int range use the `LONG_INT_TRAILER` form,
/// matching `write_int_internal` (librt_internal.c:833-839). This covers all
/// values mypy's wire format serializes (TypeVarId raw_ids and enum values
/// fit the short-int range); the long-int path is implemented for
/// completeness and never silently truncates.
pub(crate) fn write_int_bare(buf: &mut WriteBuffer, value: i64) -> Result<(), WireError> {
    if (MIN_ONE_BYTE_INT..=MAX_ONE_BYTE_INT).contains(&value) {
        let payload = (value - MIN_ONE_BYTE_INT) << 1;
        buf.push(payload as u8);
    } else if (MIN_TWO_BYTES_INT..=MAX_TWO_BYTES_INT).contains(&value) {
        let payload = (value - MIN_TWO_BYTES_INT) << 2 | (TWO_BYTES_INT_BIT as i64);
        buf.push((payload & 0xFF) as u8);
        buf.push(((payload >> 8) & 0xFF) as u8);
    } else if (MIN_FOUR_BYTES_INT..=MAX_FOUR_BYTES_INT).contains(&value) {
        let payload = (value - MIN_FOUR_BYTES_INT) << 3
            | (TWO_BYTES_INT_BIT as i64)
            | (FOUR_BYTES_INT_BIT as i64);
        buf.push((payload & 0xFF) as u8);
        buf.push(((payload >> 8) & 0xFF) as u8);
        buf.push(((payload >> 16) & 0xFF) as u8);
        buf.push(((payload >> 24) & 0xFF) as u8);
    } else {
        // Long-int form (LONG_INT_TRAILER): sentinel, then a short-int
        // size_and_sign header (size << 1 | sign), then LE magnitude bytes.
        // Mirrors `_write_long_int` (librt_internal.c:764-827).
        let neg = value < 0;
        let mut magnitude = (value as i128).unsigned_abs();
        let mut bytes = Vec::new();
        if magnitude == 0 {
            bytes.push(0);
        }
        while magnitude > 0 {
            bytes.push((magnitude & 0xFF) as u8);
            magnitude >>= 8;
        }
        return write_long_int_bytes(buf, &bytes, neg);
    }
    Ok(())
}

/// Emit the long-int encoding (sentinel, header, magnitude) for a
/// little-endian unsigned magnitude. The header is written as a short int,
/// exactly like `_write_long_int`; a magnitude whose size header would
/// exceed `MAX_FOUR_BYTES_INT` raises the same "int too long to
/// serialize" error as the C writer (librt_internal.c:813-816).
fn write_long_int_bytes(
    buf: &mut WriteBuffer,
    magnitude: &[u8],
    neg: bool,
) -> Result<(), WireError> {
    let size_and_sign = ((magnitude.len() as i64) << 1) | if neg { 1 } else { 0 };
    if size_and_sign > MAX_FOUR_BYTES_INT {
        return Err(WireError::invalid("int too long to serialize"));
    }
    buf.push(LONG_INT_TRAILER);
    write_int_bare(buf, size_and_sign)?;
    buf.extend(magnitude);
    Ok(())
}

/// `write_str_bare`: short-int length prefix + UTF-8 body. Inverse of
/// `read_str_bare` (wire.rs:261-274).
pub(crate) fn write_str_bare(buf: &mut WriteBuffer, s: &str) -> Result<(), WireError> {
    write_int_bare(buf, s.len() as i64)?;
    buf.extend(s.as_bytes());
    Ok(())
}

/// `write_str`: tagged `LITERAL_STR` + bare str. Inverse of `read_str`.
pub(crate) fn write_str(buf: &mut WriteBuffer, s: &str) -> Result<(), WireError> {
    write_tag(buf, LITERAL_STR);
    write_str_bare(buf, s)
}

/// `write_str_set`: `LIST_STR` + bare size + N bare strs, iterating a
/// `HashSet`. Inverse of `read_str_list` (wire.rs:362-378). Used for
/// `TypedDictType.required_keys` / `readonly_keys`. Python's writer also
/// iterates a set (via `list(...)`), so order parity is not guaranteed.
fn write_str_set(buf: &mut WriteBuffer, items: &HashSet<String>) -> Result<(), WireError> {
    let v: Vec<&String> = items.iter().collect();
    write_tag(buf, LIST_STR);
    write_int_bare(buf, v.len() as i64)?;
    for item in v {
        write_str_bare(buf, item)?;
    }
    Ok(())
}

/// `write_type_map`: `DICT_STR_GEN` + bare size + N (bare str, type). Inverse
/// of `read_type_map` (wire.rs:620-638). Used for `TypedDictType.items`.
fn write_type_map(buf: &mut WriteBuffer, items: &[(String, Type)]) -> Result<(), WireError> {
    write_tag(buf, DICT_STR_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for (key, value) in items {
        write_str_bare(buf, key)?;
        write_type(buf, value)?;
    }
    Ok(())
}

/// `write_str_opt`: `LITERAL_NONE` for None, else `write_str`. Inverse of
/// `read_str_opt` (wire.rs:328-340).
pub(crate) fn write_str_opt(buf: &mut WriteBuffer, value: Option<&str>) -> Result<(), WireError> {
    match value {
        Some(s) => write_str(buf, s),
        None => {
            write_tag(buf, LITERAL_NONE);
            Ok(())
        }
    }
}

/// `write_int`: tagged `LITERAL_INT` + bare int. Inverse of `read_int`.
pub(crate) fn write_int(buf: &mut WriteBuffer, value: i64) -> Result<(), WireError> {
    write_tag(buf, LITERAL_INT);
    write_int_bare(buf, value)
}

/// `write_bytes_bare`: short-int length prefix + body. Inverse of
/// `read_bytes_bare` (wire.rs:279-290).
fn write_bytes_bare(buf: &mut WriteBuffer, bytes: &[u8]) -> Result<(), WireError> {
    write_int_bare(buf, bytes.len() as i64)?;
    buf.extend(bytes);
    Ok(())
}

/// `write_float_bare`: 8 bytes IEEE-754 little-endian. Inverse of
/// `read_float_bare` (wire.rs:294-300).
fn write_float_bare(buf: &mut WriteBuffer, value: f64) -> Result<(), WireError> {
    buf.extend(&value.to_le_bytes());
    Ok(())
}

/// `write_literal_value`: bare literal value, tag chosen by variant.
/// Inverse of `read_literal` (wire.rs:424-434).
fn write_literal_value(buf: &mut WriteBuffer, value: &LiteralValue) -> Result<(), WireError> {
    match value {
        LiteralValue::Int(i) => {
            write_tag(buf, LITERAL_INT);
            write_int_bare(buf, *i)
        }
        LiteralValue::BigInt(big) => {
            write_tag(buf, LITERAL_INT);
            write_long_int_bytes(buf, &big.wire_magnitude(), big.neg)
        }
        LiteralValue::Str(s) => {
            write_tag(buf, LITERAL_STR);
            write_str_bare(buf, s)
        }
        LiteralValue::Bytes(b) => {
            write_tag(buf, LITERAL_BYTES);
            write_bytes_bare(buf, b)
        }
        LiteralValue::Float(f) => {
            write_tag(buf, LITERAL_FLOAT);
            write_float_bare(buf, *f)
        }
        LiteralValue::Bool(true) => {
            write_tag(buf, LITERAL_TRUE);
            Ok(())
        }
        LiteralValue::Bool(false) => {
            write_tag(buf, LITERAL_FALSE);
            Ok(())
        }
    }
}

/// `write_type_opt`: `LITERAL_NONE` for None, else `write_type`. Inverse
/// of `read_type_opt` (wire.rs:591-600).
fn write_type_opt(buf: &mut WriteBuffer, value: Option<&Type>) -> Result<(), WireError> {
    match value {
        Some(t) => write_type(buf, t),
        None => {
            write_tag(buf, LITERAL_NONE);
            Ok(())
        }
    }
}

/// `write_type_list`: `LIST_GEN` + bare size + N types. Inverse of
/// `read_type_list` (wire.rs:4543-4553).
pub(crate) fn write_type_list(buf: &mut WriteBuffer, items: &[Type]) -> Result<(), WireError> {
    write_tag(buf, LIST_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_type(buf, item)?;
    }
    Ok(())
}

/// `write_int_list`: `LIST_INT` + bare size + N bare ints. Inverse of
/// `read_int_list` (wire.rs:343-359). Used for `CallableType.arg_kinds`.
pub(crate) fn write_int_list(buf: &mut WriteBuffer, items: &[i64]) -> Result<(), WireError> {
    write_tag(buf, LIST_INT);
    write_int_bare(buf, items.len() as i64)?;
    for &item in items {
        write_int_bare(buf, item)?;
    }
    Ok(())
}

/// `write_str_opt_list`: `LIST_GEN` + bare size + N `write_str_opt`s.
/// Inverse of `read_str_opt_list` (wire.rs:382-398). Used for
/// `CallableType.arg_names` (None means positional/unnamed).
fn write_str_opt_list(buf: &mut WriteBuffer, items: &[Option<String>]) -> Result<(), WireError> {
    write_tag(buf, LIST_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_str_opt(buf, item.as_deref())?;
    }
    Ok(())
}

/// `write_flags`: bit-pack up to 26 bools into one tagged int. Inverse
/// of `read_flags` (wire.rs:402-409). Mirrors `cache.write_flags`.
fn write_flags(buf: &mut WriteBuffer, flags: &[bool]) -> Result<(), WireError> {
    if flags.len() > 26 {
        return Err(WireError::invalid(format!(
            "write_flags: {} flags exceed the 26-flag limit",
            flags.len()
        )));
    }
    let mut packed: i64 = 0;
    for (i, &f) in flags.iter().enumerate() {
        if f {
            packed |= 1 << i;
        }
    }
    write_int(buf, packed)
}

/// `write_type_var_likes`: `LIST_GEN` + bare size + N TypeVarLike
/// entries (TypeVarType / ParamSpecType / TypeVarTupleType). Inverse
/// of `read_type_var_likes` (wire.rs:642-668). Used for
/// `CallableType.variables`. Each entry is dispatched via `write_type`,
/// which handles all three TypeVar-like variants.
fn write_type_var_likes(buf: &mut WriteBuffer, items: &[Type]) -> Result<(), WireError> {
    write_tag(buf, LIST_GEN);
    write_int_bare(buf, items.len() as i64)?;
    for item in items {
        write_type(buf, item)?;
    }
    Ok(())
}

/// `write_type`: the `Type.write(WriteBuffer)` inverse of `read_type`.
///
/// Implements only the variants the set-ops visitors can produce:
/// `AnyType`, `NoneType`, `UninhabitedType`, `Instance` (args-less or
/// generic), `TypeType`. Other variants return `Err` so callers fail
/// loudly rather than emit malformed bytes that `Type.read()` would
/// reject.
/// `write_parameters`: inverse of `read_parameters` (wire.rs:818) and mirror
/// of `Parameters.write` in mypy/types.py:2115. Field order and bare-int kinds
/// must match the reader or the wire round-trip desynchronizes.
fn write_parameters(buf: &mut WriteBuffer, p: &Parameters) -> Result<(), WireError> {
    write_tag(buf, PARAMETERS);
    write_type_list(buf, &p.arg_types)?;
    write_int_list(buf, &p.arg_kinds)?;
    write_str_opt_list(buf, &p.arg_names)?;
    write_type_var_likes(buf, &p.variables)?;
    write_bool(buf, p.imprecise_arg_kinds);
    write_bool(buf, p.is_ellipsis_args);
    write_tag(buf, END_TAG);
    Ok(())
}

pub(crate) fn write_type(buf: &mut WriteBuffer, t: &Type) -> Result<(), WireError> {
    match t {
        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } => {
            write_tag(buf, ANY_TYPE);
            write_type_opt(buf, source_any.as_deref())?;
            write_int(buf, *type_of_any)?;
            write_str_opt(buf, missing_import_name.as_deref())?;
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::NoneType => {
            write_tag(buf, NONE_TYPE);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::ErasedType => {
            write_tag(buf, ERASED_TYPE);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::UninhabitedType { ambiguous } => {
            write_tag(buf, UNINHABITED_TYPE);
            write_bool(buf, *ambiguous);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            write_tag(buf, INSTANCE);
            if args.is_empty() && last_known_value.is_none() && extra_attrs.is_none() {
                match type_ref.as_str() {
                    "builtins.str" => write_tag(buf, INSTANCE_STR),
                    "builtins.function" => write_tag(buf, INSTANCE_FUNCTION),
                    "builtins.int" => write_tag(buf, INSTANCE_INT),
                    "builtins.bool" => write_tag(buf, INSTANCE_BOOL),
                    "builtins.object" => write_tag(buf, INSTANCE_OBJECT),
                    _ => {
                        write_tag(buf, INSTANCE_SIMPLE);
                        write_str_bare(buf, type_ref)?;
                    }
                }
                Ok(())
            } else {
                write_tag(buf, INSTANCE_GENERIC);
                write_str(buf, type_ref)?;
                write_type_list(buf, args)?;
                write_type_opt(buf, last_known_value.as_deref())?;
                match extra_attrs {
                    None => write_tag(buf, LITERAL_NONE),
                    Some(ea) => {
                        // Mirror Python's ExtraAttrs.write element order:
                        // attrs map, sorted(immutable), mod_name, END_TAG.
                        // Keys sorted: HashMap order is nondeterministic.
                        write_tag(buf, EXTRA_ATTRS);
                        let mut attrs: Vec<(&String, &Type)> = ea.attrs.iter().collect();
                        attrs.sort_unstable_by(|a, b| a.0.cmp(b.0));
                        write_tag(buf, DICT_STR_GEN);
                        write_int_bare(buf, attrs.len() as i64)?;
                        for (key, value) in attrs {
                            write_str_bare(buf, key)?;
                            write_type(buf, value)?;
                        }
                        let mut immutable: Vec<&String> = ea.immutable.iter().collect();
                        immutable.sort();
                        write_tag(buf, LIST_STR);
                        write_int_bare(buf, immutable.len() as i64)?;
                        for item in immutable {
                            write_str_bare(buf, item)?;
                        }
                        write_str_opt(buf, ea.mod_name.as_deref())?;
                        write_tag(buf, END_TAG);
                    }
                }
                write_tag(buf, END_TAG);
                Ok(())
            }
        }
        Type::TypeType { item, is_type_form } => {
            write_tag(buf, TYPE_TYPE);
            write_type(buf, item)?;
            write_bool(buf, *is_type_form);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            special_sig,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            write_tag(buf, CALLABLE_TYPE);
            // fallback is always an Instance (Python asserts the tag).
            write_type(buf, fallback)?;
            write_type_opt(buf, instance_type.as_deref())?;
            write_flags(
                buf,
                &[
                    *is_ellipsis_args,
                    *implicit,
                    *is_bound,
                    *from_concatenate,
                    *imprecise_arg_kinds,
                    *unpack_kwargs,
                    *from_type_type,
                ],
            )?;
            write_type_list(buf, arg_types)?;
            write_int_list(buf, arg_kinds)?;
            write_str_opt_list(buf, arg_names)?;
            write_type(buf, ret_type)?;
            write_str_opt(buf, name.as_deref())?;
            write_str_opt(buf, special_sig.as_deref())?;
            write_type_var_likes(buf, variables)?;
            write_type_opt(buf, type_guard.as_deref())?;
            write_type_opt(buf, type_is.as_deref())?;
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::Overloaded { items } => {
            write_tag(buf, OVERLOADED);
            write_type_list(buf, items)?;
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true,
            can_be_false,
            ..
        } => {
            write_tag(buf, UNION_TYPE);
            // `is_evaluated` / `original_str_*` are Rust-resident only
            // (Phase F0, #1349); the wire format matches
            // `UnionType.write` in types.py.
            write_type_list(buf, items)?;
            write_bool(buf, *uses_pep604_syntax);
            // Truthiness flags (cache wire layout >= 11): must mirror
            // `UnionType.write`/`read` in types.py so round-trips
            // preserve can_be_true/can_be_false instead of resetting

            // them to defaults (issue #201).
            write_bool(buf, *can_be_true);
            write_bool(buf, *can_be_false);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::LiteralType { fallback, value } => {
            write_tag(buf, LITERAL_TYPE);
            // `read_literal_type` requires the fallback to be an
            // Instance (it asserts INSTANCE tag). `write_type` emits
            // exactly that for an Instance.
            write_type(buf, fallback)?;
            write_literal_value(buf, value)?;
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => {
            // Field order mirrors `read_type_var_type`. meta_level is
            // written only when non-zero (backward-compatible append).
            write_tag(buf, TYPE_VAR_TYPE);
            write_str(buf, name)?;
            write_str(buf, fullname)?;
            write_int(buf, *raw_id)?;
            write_str(buf, namespace)?;
            write_type_list(buf, values)?;
            write_type(buf, upper_bound)?;
            write_type(buf, default)?;
            write_int(buf, *variance)?;
            if *meta_level != 0 {
                write_int(buf, *meta_level)?;
            }
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
            meta_level,
        } => {
            // Field order mirrors `read_param_spec_type` (wire.rs:790) and
            // `ParamSpecType.write` in mypy/types.py:911. The prefix is an
            // inlined Parameters record, not a nested tagged type.
            write_tag(buf, PARAM_SPEC_TYPE);
            write_parameters(buf, prefix)?;
            write_str(buf, name)?;
            write_str(buf, fullname)?;
            write_int(buf, *raw_id)?;
            write_str(buf, namespace)?;
            write_int(buf, *flavor)?;
            write_type(buf, upper_bound)?;
            write_type(buf, default)?;
            // meta_level is written only when non-zero
            // (backward-compatible append, mirroring types.py).
            if *meta_level != 0 {
                write_int(buf, *meta_level)?;
            }
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
            meta_level,
        } => {
            // Field order mirrors `read_type_var_tuple_type` (wire.rs:820) and
            // `TypeVarTupleType.write` in mypy/types.py:993. tuple_fallback is
            // an inlined Instance record.
            write_tag(buf, TYPE_VAR_TUPLE_TYPE);
            write_type(buf, tuple_fallback)?;
            write_str(buf, name)?;
            write_str(buf, fullname)?;
            write_int(buf, *raw_id)?;
            write_str(buf, namespace)?;
            write_type(buf, upper_bound)?;
            write_type(buf, default)?;
            write_int(buf, *min_len)?;
            // meta_level is written only when non-zero
            // (backward-compatible append, mirroring types.py).
            if *meta_level != 0 {
                write_int(buf, *meta_level)?;
            }
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            write_tag(buf, TUPLE_TYPE);
            // partial_fallback is always an Instance (reader asserts INSTANCE).
            write_type(buf, partial_fallback)?;
            write_type_list(buf, items)?;
            write_bool(buf, *implicit);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            write_tag(buf, TYPED_DICT_TYPE);
            write_type(buf, fallback)?;
            write_type_map(buf, items)?;
            write_str_set(buf, required_keys)?;
            write_str_set(buf, readonly_keys)?;
            write_bool(buf, *is_closed);
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::DeletedType { source } => {
            write_tag(buf, DELETED_TYPE);
            write_str_opt(buf, source.as_deref())?;
            write_tag(buf, END_TAG);
            Ok(())
        }
        Type::UnpackType { typ, .. } => {
            write_tag(buf, UNPACK_TYPE);
            // `from_star_syntax` is Rust-resident only (Phase F0, #1349);
            // the wire format matches `UnpackType.write` in types.py.
            write_type(buf, typ)?;
            write_tag(buf, END_TAG);
            Ok(())
        }

        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive,
        } => {
            write_tag(buf, TYPE_ALIAS_TYPE);
            write_type_list(buf, args)?;
            write_str(buf, type_ref)?;
            // Byte-lockstep with the types.py writer (types.py:553): the
            // flag rides as a tagged conditional int only when True, so
            // kernel-constructed aliases keep the default `false` shape.
            if *is_recursive {
                write_int(buf, 1)?;
            }
            write_tag(buf, END_TAG);
            Ok(())
        }

        Type::Parameters(p) => write_parameters(buf, p),

        Type::UnboundType {
            name,
            args,
            original_str_expr,
            original_str_fallback,
            ..
        } => {
            write_tag(buf, UNBOUND_TYPE);
            // `optional` / `empty_tuple_index` are Rust-resident only
            // (Phase F0, #1349); the wire format matches
            // `UnboundType.write` in types.py.
            write_str(buf, name)?;
            write_type_list(buf, args)?;
            write_str_opt(buf, original_str_expr.as_deref())?;
            write_str_opt(buf, original_str_fallback.as_deref())?;
            write_tag(buf, END_TAG);
            Ok(())
        }
    }
}

impl Type {
    /// Stable variant name for error messages (mirrors the Python class name).
    #[expect(dead_code)]
    fn variant_name(&self) -> &'static str {
        match self {
            Type::Instance { .. } => "Instance",
            Type::TypeAliasType { .. } => "TypeAliasType",
            Type::TypeVarType { .. } => "TypeVarType",
            Type::ParamSpecType { .. } => "ParamSpecType",
            Type::TypeVarTupleType { .. } => "TypeVarTupleType",
            Type::UnboundType { .. } => "UnboundType",
            Type::UnpackType { .. } => "UnpackType",
            Type::AnyType { .. } => "AnyType",
            Type::UninhabitedType { .. } => "UninhabitedType",
            Type::NoneType => "NoneType",
            Type::ErasedType => "ErasedType",
            Type::DeletedType { .. } => "DeletedType",
            Type::CallableType { .. } => "CallableType",
            Type::Overloaded { .. } => "Overloaded",
            Type::TupleType { .. } => "TupleType",
            Type::TypedDictType { .. } => "TypedDictType",
            Type::LiteralType { .. } => "LiteralType",
            Type::UnionType { .. } => "UnionType",
            Type::TypeType { .. } => "TypeType",
            Type::Parameters(_) => "Parameters",
        }
    }
}

/// Minimal short-int encoder for test helpers in other modules.
/// Mirrors `_write_short_int` 1-byte form (values -10..=117).
#[cfg(test)]
pub(crate) fn encode_short_int_for_test(value: i64) -> Vec<u8> {
    assert!(
        (MIN_ONE_BYTE_INT..=117).contains(&value),
        "test helper only supports 1-byte short-int range"
    );
    vec![((value - MIN_ONE_BYTE_INT) << 1) as u8]
}

/// Minimal args-less Instance wire blob for tests: INSTANCE (80) +
/// INSTANCE_SIMPLE (81) + short-int length + UTF-8 fullname.
#[cfg(test)]
pub(crate) fn encode_instance_simple_for_test(fullname: &str) -> Vec<u8> {
    let bytes = fullname.as_bytes();
    let mut blob = vec![INSTANCE, INSTANCE_SIMPLE];
    blob.extend(encode_short_int_for_test(bytes.len() as i64));
    blob.extend_from_slice(bytes);
    blob
}

// Python `==` semantics for wire-decoded types: mirrors each Variant's
// `__eq__` override (TypeVarType ignores variance and name); ALIAS_EQ_ACTIVE
// is Python's PyObject_RichCompareBool identity fast path on recursive aliases.
thread_local! {
    static ALIAS_EQ_ACTIVE: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
}

// RAII: a frame inserted into ALIAS_EQ_ACTIVE must pop on unwind. A leaked
// entry silently poisons every later comparison of that pair on the thread.
struct AliasEqFrame {
    key: (String, String),
    fresh: bool,
}

impl Drop for AliasEqFrame {
    fn drop(&mut self) {
        if self.fresh {
            ALIAS_EQ_ACTIVE.with(|c| c.borrow_mut().remove(&self.key));
        }
    }
}

pub(crate) fn py_type_eq(a: &Type, b: &Type) -> bool {
    match (a, b) {
        // Instance.__eq__ (types.py:1939): type identity + args +
        // last_known_value + extra_attrs. The wire carries `type_ref`
        // (fullname) in place of the live TypeInfo identity.
        (
            Type::Instance {
                type_ref: t1,
                args: a1,
                last_known_value: l1,
                extra_attrs: e1,
            },
            Type::Instance {
                type_ref: t2,
                args: a2,
                last_known_value: l2,
                extra_attrs: e2,
            },
        ) => {
            t1 == t2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(x, y)| py_type_eq(x, y))
                && match (l1, l2) {
                    (None, None) => true,
                    (Some(x), Some(y)) => py_type_eq(x, y),
                    _ => false,
                }
                && match (e1, e2) {
                    (None, None) => true,
                    (Some(x), Some(y)) => extra_attrs_py_eq(x, y),
                    _ => false,
                }
        }
        // TypeVarType.__eq__ (types.py:837): id + upper_bound + values +
        // default; `variance` and name/fullname are NOT part of the ==
        // contract, letting the trial-variance tvar match the NOT_READY.
        (
            Type::TypeVarType {
                raw_id: r1,
                namespace: n1,
                meta_level: m1,
                values: v1,
                upper_bound: u1,
                default: d1,
                ..
            },
            Type::TypeVarType {
                raw_id: r2,
                namespace: n2,
                meta_level: m2,
                values: v2,
                upper_bound: u2,
                default: d2,
                ..
            },
        ) => {
            r1 == r2
                && n1 == n2
                && m1 == m2
                && v1.len() == v2.len()
                && v1.iter().zip(v2.iter()).all(|(x, y)| py_type_eq(x, y))
                && py_type_eq(u1, u2)
                && py_type_eq(d1, d2)
        }
        // ParamSpecType.__eq__ (types.py:1011): id + flavor + prefix +
        // default (upper_bound is implied by flavor).
        (
            Type::ParamSpecType {
                raw_id: r1,
                namespace: n1,
                meta_level: m1,
                flavor: f1,
                prefix: p1,
                default: d1,
                ..
            },
            Type::ParamSpecType {
                raw_id: r2,
                namespace: n2,
                meta_level: m2,
                flavor: f2,
                prefix: p2,
                default: d2,
                ..
            },
        ) => {
            r1 == r2
                && n1 == n2
                && m1 == m2
                && f1 == f2
                && py_type_eq(d1, d2)
                && p1.arg_types.len() == p2.arg_types.len()
                && p1
                    .arg_types
                    .iter()
                    .zip(p2.arg_types.iter())
                    .all(|(x, y)| py_type_eq(x, y))
                && p1.arg_kinds == p2.arg_kinds
                && p1.arg_names == p2.arg_names
                && p1.is_ellipsis_args == p2.is_ellipsis_args
        }
        // TypeVarTupleType.__eq__ (types.py:1221): id + min_len + default.
        (
            Type::TypeVarTupleType {
                raw_id: r1,
                namespace: n1,
                meta_level: m1,
                min_len: l1,
                default: d1,
                ..
            },
            Type::TypeVarTupleType {
                raw_id: r2,
                namespace: n2,
                meta_level: m2,
                min_len: l2,
                default: d2,
                ..
            },
        ) => r1 == r2 && n1 == n2 && m1 == m2 && l1 == l2 && py_type_eq(d1, d2),
        // UnionType.__eq__ (types.py:3876): frozenset(items) only --
        // order-insensitive, uses_pep604_syntax excluded. Python unions
        // are de-duplicated, so a matching loop reproduces the frozenset.
        (
            Type::UnionType {
                items: i1,
                uses_pep604_syntax: _p1,
                ..
            },
            Type::UnionType {
                items: i2,
                uses_pep604_syntax: _p2,
                ..
            },
        ) => type_list_py_eq_bag(i1, i2),
        // AnyType.__eq__ (types.py:1534): isinstance check only — every
        // Any equals every Any, whatever its type_of_any/source_any.
        (Type::AnyType { .. }, Type::AnyType { .. }) => true,
        // CallableType.__eq__ (types.py:2949): ret_type, arg_types,
        // arg_names, arg_kinds, name, is_ellipsis_args, type_guard +
        // type_is, and fallback; flags/instance_type/variables excluded.
        (
            Type::CallableType {
                fallback: fb1,
                instance_type: _,
                is_ellipsis_args: e1,
                arg_types: a1,
                arg_kinds: k1,
                arg_names: n1,
                ret_type: rt1,
                name: nm1,
                variables: _,
                type_guard: g1,
                type_is: t1,
                ..
            },
            Type::CallableType {
                fallback: fb2,
                instance_type: _,
                is_ellipsis_args: e2,
                arg_types: a2,
                arg_kinds: k2,
                arg_names: n2,
                ret_type: rt2,
                name: nm2,
                variables: _,
                type_guard: g2,
                type_is: t2,
                ..
            },
        ) => {
            nm1 == nm2
                && e1 == e2
                && k1 == k2
                && n1 == n2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(x, y)| py_type_eq(x, y))
                && py_type_eq(rt1, rt2)
                && py_type_eq(fb1, fb2)
                && match (g1, g2) {
                    (None, None) => true,
                    (Some(x), Some(y)) => py_type_eq(x, y),
                    _ => false,
                }
                && match (t1, t2) {
                    (None, None) => true,
                    (Some(x), Some(y)) => py_type_eq(x, y),
                    _ => false,
                }
        }
        // Overloaded.__eq__ (types.py:3147): `self.items == other.items`,
        // i.e. elementwise py-eq.
        (Type::Overloaded { items: i1 }, Type::Overloaded { items: i2 }) => {
            i1.len() == i2.len() && i1.iter().zip(i2.iter()).all(|(x, y)| py_type_eq(x, y))
        }
        // TupleType.__eq__ (types.py:3252): items + partial_fallback;
        // `implicit` is not compared.
        (
            Type::TupleType {
                partial_fallback: fb1,
                items: i1,
                implicit: _,
            },
            Type::TupleType {
                partial_fallback: fb2,
                items: i2,
                implicit: _,
            },
        ) => {
            i1.len() == i2.len()
                && i1.iter().zip(i2.iter()).all(|(x, y)| py_type_eq(x, y))
                && py_type_eq(fb1, fb2)
        }
        // TypedDictType.__eq__ (types.py:3461): same key set paired
        // through `zip` (by key), py-eq on the paired values, plus
        // fallback / key sets / is_closed.
        (
            Type::TypedDictType {
                fallback: fb1,
                items: i1,
                required_keys: rk1,
                readonly_keys: ro1,
                is_closed: c1,
            },
            Type::TypedDictType {
                fallback: fb2,
                items: i2,
                required_keys: rk2,
                readonly_keys: ro2,
                is_closed: c2,
            },
        ) => {
            i1.len() == i2.len()
                && i1
                    .iter()
                    .all(|(k, v)| i2.iter().any(|(k2, v2)| k == k2 && py_type_eq(v, v2)))
                && rk1 == rk2
                && ro1 == ro2
                && c1 == c2
                && py_type_eq(fb1, fb2)
        }
        // LiteralType.__eq__ (types.py:3761): value + fallback.
        (
            Type::LiteralType {
                fallback: fb1,
                value: v1,
            },
            Type::LiteralType {
                fallback: fb2,
                value: v2,
            },
        ) => v1 == v2 && py_type_eq(fb1, fb2),
        // TypeType.__eq__ (types.py:4089): item + is_type_form.
        (
            Type::TypeType {
                item: it1,
                is_type_form: f1,
            },
            Type::TypeType {
                item: it2,
                is_type_form: f2,
            },
        ) => f1 == f2 && py_type_eq(it1, it2),
        // UnpackType.__eq__ (types.py:1463): `self.type == other.type`;
        // from_star_syntax is not compared.
        (Type::UnpackType { typ: t1, .. }, Type::UnpackType { typ: t2, .. }) => py_type_eq(t1, t2),
        // TypeAliasType.__eq__ (types.py:545): `self.alias == other.alias`
        // (node identity; the wire carries the fullname `type_ref`) +
        // `self.args == other.args`. `is_recursive` is not compared.
        (
            Type::TypeAliasType {
                type_ref: r1,
                args: a1,
                ..
            },
            Type::TypeAliasType {
                type_ref: r2,
                args: a2,
                ..
            },
        ) => {
            if r1 != r2 {
                false
            } else {
                let fresh =
                    ALIAS_EQ_ACTIVE.with(|c| c.borrow_mut().insert((r1.clone(), r2.clone())));
                match fresh {
                    true => {
                        // Fresh pair: mirror the structural
                        // `self.args == other.args` half of TypeAliasType
                        // __eq__ (types.py:545); Drop pops the frame.
                        let _guard = AliasEqFrame {
                            key: (r1.clone(), r2.clone()),
                            fresh: true,
                        };
                        a1.len() == a2.len()
                            && a1.iter().zip(a2.iter()).all(|(x, y)| py_type_eq(x, y))
                    }
                    false => {
                        // Re-entered pair (identity fast path): cut only
                        // when both sides' args agree byte-for-byte; a
                        // diverging nested arg is decided structurally.
                        if a1.is_empty() && a2.is_empty() || (a1 == a2) {
                            true
                        } else {
                            a1.len() == a2.len()
                                && a1.iter().zip(a2.iter()).all(|(x, y)| py_type_eq(x, y))
                        }
                    }
                }
            }
        }
        // UnboundType.__eq__ (types.py:1300): name/optional/original_str*;
        // args compare py-eq.
        (
            Type::UnboundType {
                name: nm1,
                args: a1,
                original_str_expr: e1,
                original_str_fallback: fb1,
                optional: o1,
                empty_tuple_index: _,
            },
            Type::UnboundType {
                name: nm2,
                args: a2,
                original_str_expr: e2,
                original_str_fallback: fb2,
                optional: o2,
                empty_tuple_index: _,
            },
        ) => {
            nm1 == nm2
                && o1 == o2
                && e1 == e2
                && fb1 == fb2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(x, y)| py_type_eq(x, y))
        }
        // Parameters.__eq__ (types.py:2392): arg_types/arg_names/
        // arg_kinds/is_ellipsis_args only.
        (Type::Parameters(p1), Type::Parameters(p2)) => {
            p1.arg_types.len() == p2.arg_types.len()
                && p1
                    .arg_types
                    .iter()
                    .zip(p2.arg_types.iter())
                    .all(|(x, y)| py_type_eq(x, y))
                && p1.arg_kinds == p2.arg_kinds
                && p1.arg_names == p2.arg_names
                && p1.is_ellipsis_args == p2.is_ellipsis_args
        }
        // Remaining variants are scalar-only shapes (NoneType, ErasedType,
        // UninhabitedType, DeletedType): the derived PartialEq matches
        // Python field for field; cross-variant pairs are never equal.
        _ => a == b,
    }
}

/// Python `frozenset(items) ==` for `UnionType.__eq__`: each item of `a`
/// must match a *distinct* py-equal item of `b`. Python unions are
/// de-duplicated, so the distinct-pairing loop reproduces the frozenset
/// semantics for bag-equal item lists.
fn type_list_py_eq_bag(a: &[Type], b: &[Type]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    // Python unions carry no duplicate items (make_simplified_union and
    // the union visitor dedupe), so a set-style one-shot pairing is exact.
    let mut taken = vec![false; b.len()];
    'outer: for x in a {
        for (y, y_taken) in b.iter().zip(taken.iter_mut()) {
            if !*y_taken && py_type_eq(x, y) {
                *y_taken = true;
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// `ExtraAttrs.__eq__` (types.py:1793): identical key sets (Python compares
/// the attrs dicts directly, so a key present only on one side is a miss),
/// py-eq per key, and the immutable set — `mod_name` is not part of the
/// == contract.
fn extra_attrs_py_eq(a: &ExtraAttrs, b: &ExtraAttrs) -> bool {
    a.attrs.len() == b.attrs.len()
        && a.attrs
            .iter()
            .all(|(k, v)| b.attrs.get(k).is_some_and(|w| py_type_eq(v, w)))
        && a.immutable.len() == b.immutable.len()
        && a.immutable.iter().all(|k| b.immutable.contains(k))
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
        assert!(!read_bool(&mut buf).unwrap());
        assert!(read_bool(&mut buf).unwrap());
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
        if (MIN_ONE_BYTE_INT..=117).contains(&value) {
            // 1-byte form.
            vec![((value - MIN_ONE_BYTE_INT) << 1) as u8]
        } else if (MIN_TWO_BYTES_INT..=16283).contains(&value) {
            // 2-byte form: low 2 bits = 01.
            let encoded = ((value - MIN_TWO_BYTES_INT) << 2) as u16 | TWO_BYTES_INT_BIT as u16;
            let le = encoded.to_le_bytes();
            vec![le[0], le[1]]
        } else if (MIN_FOUR_BYTES_INT..=536860911).contains(&value) {
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

    /// Round-trip through the production writer `write_int_bare`, not the
    /// reader-only oracle. Guards against the truncation bug where values
    /// above a tier's ceiling silently took the smaller width (e.g.
    /// `Literal[123]` corrupting to `Literal[-5]`), which the reader-only
    /// tests above cannot detect because `encode_int_for_test` is correct.
    fn write_round_trip_int(value: i64) -> i64 {
        let mut buf = WriteBuffer::new();
        write_int_bare(&mut buf, value).unwrap();
        let bytes = buf.into_bytes();
        let mut rb = ReadBuffer::new(&bytes);
        read_int_bare(&mut rb).unwrap()
    }

    #[test]
    fn write_int_bare_round_trips_all_tiers() {
        // 1-byte tier borders.
        assert_eq!(write_round_trip_int(-10), -10);
        assert_eq!(write_round_trip_int(0), 0);
        assert_eq!(write_round_trip_int(117), 117);
        // 2-byte tier borders (values just past the 1-byte ceiling are the
        // ones the old truncating writer corrupted).
        assert_eq!(write_round_trip_int(118), 118);
        assert_eq!(write_round_trip_int(123), 123);
        assert_eq!(write_round_trip_int(456), 456);
        assert_eq!(write_round_trip_int(16283), 16283);
        // 4-byte tier borders.
        assert_eq!(write_round_trip_int(16284), 16284);
        assert_eq!(write_round_trip_int(536860911), 536860911);
        // Long-int form just past the 4-byte ceiling.
        assert_eq!(write_round_trip_int(536860912), 536860912);
    }

    #[test]
    fn write_int_bare_negative_tiers() {
        assert_eq!(write_round_trip_int(-100), -100);
        assert_eq!(write_round_trip_int(-99), -99);
        assert_eq!(write_round_trip_int(-11), -11);
        assert_eq!(write_round_trip_int(-10000), -10000);
        assert_eq!(write_round_trip_int(-9999), -9999);
        assert_eq!(write_round_trip_int(-10001), -10001);
    }

    #[test]
    fn long_int_path() {
        // Just beyond the 4-byte short-int max — exercises LONG_INT_TRAILER.
        assert_eq!(round_trip_int(536860912), 536860912);
        assert_eq!(round_trip_int(-10001), -10001);
        assert_eq!(round_trip_int(1_000_000), 1_000_000);
        assert_eq!(round_trip_int(-1_000_000), -1_000_000);
    }

    // ----- Big-int literals (issue #1329) -----

    /// Serialize a `BigInt` literal through the production writer path and
    /// read it back through `read_int_literal`.
    fn round_trip_big(value: &BigInt) -> BigInt {
        let mut buf = WriteBuffer::new();
        write_literal_value(&mut buf, &LiteralValue::BigInt(value.clone())).unwrap();
        let bytes = buf.into_bytes();
        let mut rb = ReadBuffer::new(&bytes);
        let tag = read_tag(&mut rb).unwrap();
        match read_literal(&mut rb, tag).unwrap() {
            LiteralValue::BigInt(b) => b,
            other => panic!("expected BigInt variant, got {other:?}"),
        }
    }

    /// LE magnitude bytes of a u128 value (test helper).
    fn decimal_from_u128(v: u128) -> Vec<u8> {
        let mut out = Vec::new();
        let mut x = v;
        if x == 0 {
            out.push(0);
        }
        while x > 0 {
            out.push((x & 0xFF) as u8);
            x >>= 8;
        }
        out
    }

    #[test]
    fn big_int_round_trips_beyond_i64() {
        // 2**80 is the issue's regression value: well beyond i64 in both
        // directions.
        let pos = BigInt::from_le_bytes(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], false);
        let neg = BigInt::from_le_bytes(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], true);
        assert_eq!(pos.to_string(), "1208925819614629174706176");
        assert_eq!(neg.to_string(), "-1208925819614629174706176");
        assert_eq!(round_trip_big(&pos), pos);
        assert_eq!(round_trip_big(&neg), neg);
        // Wider magnitudes, including a >i128-scale value.
        let huge = BigInt::from_le_bytes(
            &(0..25).map(|i| (i * 17 + 3) as u8).collect::<Vec<u8>>(),
            false,
        );
        assert_eq!(round_trip_big(&huge), huge);
        // Zero is canonical: empty magnitude, and decodes as the i64 variant.
        assert_eq!(
            write_read_literal_value(LiteralValue::BigInt(BigInt {
                neg: false,
                magnitude: vec![]
            })),
            LiteralValue::Int(0)
        );
    }

    #[test]
    fn big_int_decimal_digits_chunk_padding() {
        // Chunk boundary at 10**19: lower chunks must zero-pad to 19 digits.
        let mag = decimal_from_u128(10_u128.pow(19) + 5);
        let big = BigInt::from_le_bytes(&mag, false);
        assert_eq!(big.to_string(), "10000000000000000005");
        let all_nines = BigInt::from_le_bytes(&decimal_from_u128(u128::MAX), false);
        assert_eq!(all_nines.to_string(), u128::MAX.to_string());
        // Negative zero is canonical zero.
        let zero = BigInt::from_le_bytes(&[0u8, 0], true);
        assert_eq!(zero.to_string(), "0");
    }

    #[test]
    fn big_int_canonical_equality() {
        // Leading zero bytes in the magnitude must not affect equality.
        let a = BigInt::from_le_bytes(&[0x42], false);
        let b = BigInt::from_le_bytes(&[0x42, 0x00, 0x00], false);
        assert_eq!(a, b);
        let neg_zero = BigInt::from_le_bytes(&[0x00], true);
        assert!(neg_zero.is_zero());
        assert_eq!(
            neg_zero,
            BigInt {
                neg: false,
                magnitude: vec![]
            }
        );
    }

    #[test]
    fn small_int_literal_keeps_i64_variant() {
        // A long-int-encoded value that still fits i64 (e.g. 10**12, which
        // the C writer emits in long form) must decode as `Int`, not
        // `BigInt`: the variant is a pure function of the value.
        let decoded = write_read_literal_value(LiteralValue::Int(1_000_000_000_000));
        assert_eq!(decoded, LiteralValue::Int(1_000_000_000_000));

        // i64 extremes through the same path.
        for edge in [i64::MAX, i64::MIN, 117, -10000] {
            let decoded = write_read_literal_value(LiteralValue::Int(edge));
            assert_eq!(decoded, LiteralValue::Int(edge), "edge {edge}");
        }
    }

    /// Tagged-literal round-trip through the production writer + reader.
    fn write_read_literal_value(value: LiteralValue) -> LiteralValue {
        let mut buf = WriteBuffer::new();
        write_literal_value(&mut buf, &value).unwrap();
        let bytes = buf.into_bytes();
        let mut rb = ReadBuffer::new(&bytes);
        let tag = read_tag(&mut rb).unwrap();
        read_literal(&mut rb, tag).unwrap()
    }

    #[test]
    fn long_int_header_cap_mirrors_c_writer() {
        // A magnitude of 268_435_456 bytes makes the size header exceed
        // MAX_FOUR_BYTES_INT; the C writer raises ValueError there
        // (librt_internal.c:813-816), so the Rust writer must error too.
        let huge = vec![0xAB; 268_435_456];
        let mut buf = WriteBuffer::new();
        let err = write_long_int_bytes(&mut buf, &huge, false);
        assert!(matches!(err, Err(WireError::Invalid(_))));
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
    /// Wire: ANY_TYPE(106), source_any=LITERAL_NONE(2),
    /// type_of_any=LITERAL_INT(3)+bare_int(0),
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
    ///   LIST_GEN(20) + size=1 + ANY_TYPE(106) + LITERAL_NONE + LITERAL_INT+0
    ///   + LITERAL_NONE + END_TAG,
    ///     LITERAL_NONE (no last_known_value),
    ///     LITERAL_NONE (no extra_attrs),
    ///     END_TAG(255).
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

    // ----- write_type round-trip (M8s) -----
    // Every test writes a Type then reads it back; the result must equal
    // the input. This is the exact contract Python's Type.read() will

    // rely on when decoding bytes Rust produces over FFI.

    fn round_trip(t: &Type) -> Type {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).expect("write_type failed");
        let bytes = buf.into_bytes();
        let mut rbuf = ReadBuffer::new(&bytes);
        read_type(&mut rbuf, None).expect("read_type failed")
    }

    #[test]
    fn write_then_read_any_type_round_trips() {
        let t = Type::AnyType {
            type_of_any: 3,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_none_type_round_trips() {
        assert_eq!(round_trip(&Type::NoneType), Type::NoneType);
    }

    #[test]
    fn write_then_read_uninhabited_type_round_trips() {
        assert_eq!(
            round_trip(&Type::UninhabitedType { ambiguous: true }),
            Type::UninhabitedType { ambiguous: true }
        );
        assert_eq!(
            round_trip(&Type::UninhabitedType { ambiguous: false }),
            Type::UninhabitedType { ambiguous: false }
        );
    }

    #[test]
    fn write_then_read_instance_simple_round_trips() {
        let t = Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_instance_simple_non_builtin_round_trips() {
        let t = Type::Instance {
            type_ref: "a.A".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_instance_extra_attrs_round_trips() {
        let t = module_instance_with_extra_attrs();
        assert_eq!(round_trip(&t), t);
    }

    /// mod_name-less variant: `write_str_opt` must emit LITERAL_NONE.
    #[test]
    fn write_then_read_instance_extra_attrs_no_mod_name_round_trips() {
        let mut t = module_instance_with_extra_attrs();
        if let Type::Instance { extra_attrs, .. } = &mut t {
            extra_attrs.as_mut().unwrap().mod_name = None;
        }
        assert_eq!(round_trip(&t), t);
    }

    /// The fast path (five singletons + INSTANCE_SIMPLE) must NOT fire for
    /// an args-less Instance that carries extra_attrs; it always takes
    /// INSTANCE_GENERIC so the extra_attrs element survives.
    #[test]
    fn write_instance_with_extra_attrs_uses_generic_path() {
        let t = module_instance_with_extra_attrs();
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &t).expect("write_type failed");
        let bytes = buf.into_bytes();
        assert_eq!(bytes[0], INSTANCE);
        assert_eq!(bytes[1], INSTANCE_GENERIC);
    }

    /// Byte-level order check mirroring Python's `ExtraAttrs.write`
    /// element order (attrs, sorted(immutable), mod_name, END_TAG).
    #[test]
    fn write_instance_extra_attrs_element_order_matches_read_extra_attrs() {
        let t = module_instance_with_extra_attrs();
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &t).expect("write_type failed");
        let bytes = buf.into_bytes();
        let mut rbuf = ReadBuffer::new(&bytes);
        assert_eq!(read_tag(&mut rbuf).unwrap(), INSTANCE);
        assert_eq!(read_tag(&mut rbuf).unwrap(), INSTANCE_GENERIC);
        assert!(read_str(&mut rbuf).is_ok());
        read_type_list(&mut rbuf).unwrap();
        assert_eq!(read_tag(&mut rbuf).unwrap(), LITERAL_NONE);
        assert_eq!(read_tag(&mut rbuf).unwrap(), EXTRA_ATTRS);
        assert!(read_type_map(&mut rbuf).is_ok());
        let immutable = read_str_list(&mut rbuf).unwrap();
        let mut sorted = immutable.clone();
        sorted.sort();
        assert_eq!(immutable, sorted);
        let _ = read_str_opt(&mut rbuf).unwrap();
        assert_eq!(read_tag(&mut rbuf).unwrap(), END_TAG);
        assert_eq!(read_tag(&mut rbuf).unwrap(), END_TAG);
    }

    fn module_instance_with_extra_attrs() -> Type {
        let mut attrs: HashMap<String, Type> = HashMap::new();
        attrs.insert(
            "func".to_string(),
            Type::AnyType {
                type_of_any: 2,
                source_any: None,
                missing_import_name: None,
            },
        );
        attrs.insert("unset".to_string(), Type::NoneType);
        Type::Instance {
            type_ref: "mypy.util".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: Some(ExtraAttrs {
                attrs,
                immutable: HashSet::from(["func".to_string(), "unset".to_string()]),
                mod_name: Some("mypy.util".to_string()),
            }),
        }
    }

    #[test]
    fn write_then_read_instance_generic_round_trips() {
        let t = Type::Instance {
            type_ref: "a.A".to_string(),
            args: vec![Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }],
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_type_type_round_trips() {
        let t = Type::TypeType {
            item: Box::new(Type::Instance {
                type_ref: "a.A".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            is_type_form: false,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_alias_default_shape_round_trips() {
        // Default shape: no trailing flag int at all, matching Python's
        // writer for a non-recursive alias.
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
            is_recursive: false,
        };
        assert_eq!(round_trip(&t), t);
        let buf = type_alias_bytes(&t);
        // END_TAG == 255 directly after the alias name: no flag record.
        assert_eq!(buf.last().copied(), Some(END_TAG));
        assert_eq!(read_alias_recursion_flag(&buf), Some(false));
    }

    #[test]
    fn write_then_read_alias_recursive_shape_round_trips() {
        // Recursive shape: the LITERAL_INT flag record rides before END_TAG
        // exactly as Python writes it (types.py:write, wave31 #1361).
        let t = Type::TypeAliasType {
            args: vec![Type::TypeAliasType {
                args: vec![],
                type_ref: "m.A".to_string(),
                is_recursive: false,
            }],
            type_ref: "m.A".to_string(),
            is_recursive: true,
        };
        let round = round_trip(&t);
        assert_eq!(round, t);
        assert_eq!(read_alias_recursion_flag(&type_alias_bytes(&t)), Some(true));
    }

    /// Serialize a top-level `TypeAliasType` and return the raw bytes.
    fn type_alias_bytes(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).expect("write_type failed");
        buf.into_bytes()
    }

    #[test]
    fn write_then_read_callable_type_minimal_round_trips() {
        // Minimal CallableType: empty args, NoneType ret, builtins.function
        // fallback, no variables/guard/type_is. Mirrors the shape produced
        // by combine_similar_callables when both operands are nullary.
        let t = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_then_read_callable_type_with_args_round_trips() {
        // CallableType with two positional int/str args, str ret, named.
        let t = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: true,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            // `true` here (unlike every other fixture) so the round-trip
            // assert exercises the 7th flag's bit position, not just
            // all-false symmetry.
            from_type_type: true,
            arg_types: vec![
                Type::Instance {
                    type_ref: "builtins.int".to_string(),
                    args: Vec::new(),
                    last_known_value: None,
                    extra_attrs: None,
                },
                Type::Instance {
                    type_ref: "builtins.str".to_string(),
                    args: Vec::new(),
                    last_known_value: None,
                    extra_attrs: None,
                },
            ],
            arg_kinds: vec![0, 0], // ARG_POS = 0
            arg_names: vec![None, None],
            ret_type: Box::new(Type::Instance {
                type_ref: "builtins.str".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            name: Some("f".to_string()),
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        assert_eq!(round_trip(&t), t);
    }

    #[test]
    fn write_type_round_trips_param_spec_type() {
        // ParamSpecType has a write arm (added with the typeanal/constraint
        // wire support), so it writes and reads back cleanly. If a future
        // variant is added without a write arm, `write_type` must error

        // rather than emit bytes Type.read() would reject.
        let prefix = Parameters {
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            variables: vec![],
            imprecise_arg_kinds: false,
            is_ellipsis_args: true,
        };
        let t = Type::ParamSpecType {
            prefix: Box::new(prefix),
            name: "P".to_string(),
            fullname: "P".to_string(),
            raw_id: -1,
            namespace: String::new(),
            flavor: 0,
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            meta_level: 0,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &t).expect("ParamSpecType must be writable");
        let bytes = buf.into_bytes();
        let mut rbuf = ReadBuffer::new(&bytes);
        let back = read_type(&mut rbuf, None).expect("ParamSpecType must round-trip");
        assert!(matches!(back, Type::ParamSpecType { .. }));
    }

    // ----- py_type_eq (Python == semantics for wire types) -----

    /// Self-referencing generic class instance: `C[T]` arg tvar with variance
    /// NOT_READY vs the same tvar with a trial variance — Python says the two
    /// `TypeVarType`s compare == (variance is not part of its __eq__).
    fn instance_with_tvar(variance: i64) -> Type {
        Type::Instance {
            type_ref: "__main__.Invariant".to_string(),
            args: vec![Type::TypeVarType {
                name: "T".to_string(),
                fullname: "__main__.T".to_string(),
                raw_id: 1,
                namespace: String::new(),
                values: Vec::new(),
                upper_bound: Box::new(Type::Instance {
                    type_ref: "builtins.object".to_string(),
                    args: Vec::new(),
                    last_known_value: None,
                    extra_attrs: None,
                }),
                default: Box::new(Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                }),
                variance,
                meta_level: 0,
            }],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn py_type_eq_ignores_tvar_variance() {
        // The testPEP695InferVarianceRecursive regression: member-side tvar
        // frozen NOT_READY (3) vs self-side trial tvar — Python compares
        // equal, so a self-returning method erases to Any.
        assert!(py_type_eq(&instance_with_tvar(3), &instance_with_tvar(0)));
        assert!(py_type_eq(&instance_with_tvar(3), &instance_with_tvar(2)));
        // ...but id, bound, values and default still matter.
        let mut wrong_raw_id = instance_with_tvar(3);
        if let Type::Instance { args, .. } = &mut wrong_raw_id {
            if let Type::TypeVarType { raw_id, .. } = &mut args[0] {
                *raw_id = 77;
            }
        }
        assert!(!py_type_eq(&instance_with_tvar(3), &wrong_raw_id));
    }

    #[test]
    fn py_type_eq_instance_shape() {
        // Different class fullname → Python Instance.__eq__ false.
        assert!(!py_type_eq(
            &instance_with_tvar(0),
            &Type::Instance {
                type_ref: "__main__.Other".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None
            }
        ));
        // Arg-count mismatch → false.
        assert!(!py_type_eq(
            &instance_with_tvar(1),
            &Type::Instance {
                type_ref: "__main__.Invariant".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None
            }
        ));
        // Truly identical → true.
        assert!(py_type_eq(&instance_with_tvar(2), &instance_with_tvar(2)));
    }

    #[test]
    fn py_type_eq_recursive_alias_back_ref_closes() {
        // A self-recursive alias vs an equally-shaped tree: the nested
        // back-ref pair hits the active-set cut (identity-fast-path
        // analogue) instead of recursing forever.
        let mk = |name: &str| Type::TypeAliasType {
            type_ref: name.to_string(),
            is_recursive: true,
            args: vec![Type::TypeAliasType {
                type_ref: name.to_string(),
                is_recursive: true,
                args: Vec::new(),
            }],
        };
        assert!(py_type_eq(&mk("__main__.A"), &mk("__main__.A")));
        assert!(!py_type_eq(&mk("__main__.A"), &mk("__main__.B")));

        // Args divergence settles before any back-ref cut.
        let inst = |inner: &str| Type::Instance {
            type_ref: inner.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        let mk_f = |inner: &str| Type::TypeAliasType {
            type_ref: "__main__.F".to_string(),
            is_recursive: true,
            args: vec![inst(inner)],
        };
        assert!(!py_type_eq(&mk_f("builtins.int"), &mk_f("builtins.str")));
        assert!(py_type_eq(&mk_f("builtins.int"), &mk_f("builtins.int")));
    }

    #[test]
    fn py_type_eq_reentered_alias_pair_divergent_args_is_false() {
        // Wave36 review: the old identity cut keyed the pair on
        // type_ref alone, so re-entered applications with diverging
        // nested args compared equal once the pair was active.
        let inst = |inner: &str| Type::Instance {
            type_ref: inner.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        // Outer A[int] nesting A[int] re-mentioned vs A[int,others],
        // nested divergence past the shared ref: outer == must be false.
        let mk = |name: &str, deep: &str| Type::TypeAliasType {
            type_ref: name.to_string(),
            is_recursive: true,
            args: vec![Type::TypeAliasType {
                type_ref: name.to_string(),
                is_recursive: true,
                args: vec![inst(deep)],
            }],
        };
        assert!(!py_type_eq(
            &mk("A", "builtins.int"),
            &mk("A", "builtins.str")
        ));
        assert!(py_type_eq(
            &mk("A", "builtins.int"),
            &mk("A", "builtins.int")
        ));
    }

    // ----- Phase F0 (#1349): Rust-resident plain-data fields -----
    // Wire never serializes these (writer arms match types.py *.write);
    // readers fill the Python class defaults. See doc/f0_coverage.md.

    fn f0_minimal_instance() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn f0_unbound(name: &str, optional: bool, empty_tuple_index: bool) -> Type {
        Type::UnboundType {
            name: name.to_string(),
            args: Vec::new(),
            original_str_expr: None,
            original_str_fallback: None,
            optional,
            empty_tuple_index,
        }
    }

    #[test]
    fn f0_unbound_type_reader_fills_defaults() {
        let back = round_trip(&f0_unbound("A", false, false));
        match back {
            Type::UnboundType {
                optional,
                empty_tuple_index,
                ..
            } => {
                assert!(!optional);
                assert!(!empty_tuple_index);
            }
            _ => panic!("expected UnboundType"),
        }
    }

    #[test]
    fn f0_unbound_type_nondefault_fields_are_wire_dropped() {
        let back = round_trip(&f0_unbound("A", true, true));
        match back {
            Type::UnboundType {
                optional,
                empty_tuple_index,
                ..
            } => {
                // Wire bytes carry no representation for these fields, so
                // the reader re-derives the Python defaults.
                assert!(!optional);
                assert!(!empty_tuple_index);
            }
            _ => panic!("expected UnboundType"),
        }
    }

    #[test]
    fn f0_unpack_type_reader_fills_from_star_syntax_default() {
        let t = Type::UnpackType {
            typ: Box::new(f0_minimal_instance()),
            from_star_syntax: false,
        };
        match round_trip(&t) {
            Type::UnpackType {
                from_star_syntax, ..
            } => assert!(!from_star_syntax),
            _ => panic!("expected UnpackType"),
        }
    }

    #[test]
    fn f0_unpack_type_nondefault_field_is_wire_dropped() {
        let t = Type::UnpackType {
            typ: Box::new(f0_minimal_instance()),
            from_star_syntax: true,
        };
        match round_trip(&t) {
            Type::UnpackType {
                from_star_syntax, ..
            } => {
                // Encodes the PEP 695 star syntax side flag only; not
                // serializable on the current wire layout.
                assert!(!from_star_syntax);
            }
            _ => panic!("expected UnpackType"),
        }
    }

    #[test]
    fn f0_callable_type_reader_round_trips_special_sig() {
        let mk = |special_sig: Option<String>| Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig,
        };
        match round_trip(&mk(None)) {
            Type::CallableType { special_sig, .. } => assert_eq!(special_sig, None),
            _ => panic!("expected CallableType"),
        }
        match round_trip(&mk(Some("partial".to_string()))) {
            Type::CallableType { special_sig, .. } => {
                assert_eq!(special_sig.as_deref(), Some("partial"))
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn f0_union_type_reader_fills_defaults() {
        let t = Type::UnionType {
            items: vec![f0_minimal_instance()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        match round_trip(&t) {
            Type::UnionType {
                is_evaluated,
                original_str_expr,
                original_str_fallback,
                ..
            } => {
                // Python default `is_evaluated = True`; original_str_* start
                // unpopulated until `make_union` records them.
                assert!(is_evaluated);
                assert_eq!(original_str_expr, None);
                assert_eq!(original_str_fallback, None);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn f0_coverage_doc_lists_every_wire_variant() {
        // The full-fidelity audit lives in doc/f0_coverage.md. If a variant
        // is added to `Type` without a doc section, this fails — the doc is
        // the class/field audit table the issue requires.
        let doc = include_str!("../doc/f0_coverage.md");
        for name in [
            "Instance",
            "TypeAliasType",
            "TypeVarType",
            "ParamSpecType",
            "TypeVarTupleType",
            "UnboundType",
            "UnpackType",
            "AnyType",
            "UninhabitedType",
            "NoneType",
            "ErasedType",
            "DeletedType",
            "CallableType",
            "Overloaded",
            "TupleType",
            "TypedDictType",
            "LiteralType",
            "UnionType",
            "TypeType",
            "Parameters",
        ] {
            assert!(
                doc.contains(name),
                "doc/f0_coverage.md is missing wire variant `{name}`"
            );
        }
        // The Rust-resident fields must be documented as wire gaps too.
        for field in [
            "optional",
            "empty_tuple_index",
            "from_star_syntax",
            "special_sig",
            "is_evaluated",
        ] {
            assert!(
                doc.contains(field),
                "doc/f0_coverage.md is missing F0 field `{field}`"
            );
        }
    }
}
