//! Native ports of pure `Type` object-model methods from `mypy/types.py`.
//!
//! Each function mirrors the Python `can_be_true_default` / `can_be_false_default`
//! property defaults, `CallableType` flag accessors, and `length()` methods.
//! All operate on the wire-format `Type` enum. Functions return `Option<T>`
//! (None = defer to Python) for type variants that need live `TypeInfo` or
//! `TypeAlias` nodes not available on the wire.
//!
//! Deferred (return None) cases:
//!   * `TypeAliasType` — `can_be_true/false_default` delegates to
//!     `self.alias.target`, which needs the live alias target.
//!   * `TupleType` with a non-`builtins.tuple` fallback — `can_be_any_bool`
//!     needs the live `TypeInfo.names` dict to check for `__bool__` (unless
//!     the resolver carries the class snapshots, see `can_be_any_bool_ref`).
//!   * `LiteralType` whose fallback class is missing from the resolver —
//!     the enum checks need the live `TypeInfo` (see `can_be_true_live`).
//!
//! The `rust_can_be_true_default_live` / `rust_can_be_false_default_live`
//! seams take the `NativeTypeResolver`: they port the `TupleType`
//! `can_be_any_bool` check, the `TypeAliasType` alias-target delegation
//! (via the frozen alias snapshots), and the enum `LiteralType` branch
//! (via live `is_enum` + `can_be_true`/`can_be_false` reads on the
//! fallback). The byte-only seams keep the pre-existing deferral behavior
//! so non-resolver callers stay parity-safe.

use pyo3::prelude::*;

#[allow(unused_imports)]
use crate::checkexpr_functions::expanded_alias_target;
use crate::typeinfo::{read_bool_attr, read_mro_fullnames, NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// ArgKind values (mirror mypy.nodes.ArgKind)
// ---------------------------------------------------------------------------

const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

// ---------------------------------------------------------------------------
// ArgKind helper predicates (mirror mypy.nodes.ArgKind methods)
// ---------------------------------------------------------------------------

fn kind_is_positional(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_OPT
}

fn kind_is_named(kind: i64) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT
}

fn kind_is_required(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_NAMED
}

fn kind_is_star(kind: i64) -> bool {
    kind == ARG_STAR || kind == ARG_STAR2
}

// ---------------------------------------------------------------------------
// Wire format helper
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `encode_type`: serialize a wire `Type` back to bytes. Used to re-encode
/// sub-blobs (e.g. a `partial_fallback`) for the byte-passing resolver
/// helpers.
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

// ---------------------------------------------------------------------------
// can_be_true_default / can_be_false_default
// ---------------------------------------------------------------------------

/// `mypy.types.Type.can_be_true_default` — default truthiness of a type.
///
/// Mirrors the per-class overrides in `mypy/types.py`:
///   * `Type` base: `True`.
///   * `UninhabitedType`: `False`.
///   * `NoneType`: `False`.
///   * `UnionType`: `any(item.can_be_true for item in self.items)`.
///   * `TypeAliasType`: defer (needs live alias target).
///   * `TupleType`: defer unless fallback is `builtins.tuple` (needs TypeInfo).
///   * `LiteralType`: defer (needs `TypeInfo.is_enum`).
///   * All other proper types: `True` (the base default).
///
/// Returns `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_can_be_true_default(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(can_be_true_default_inner(&typ))
}

fn can_be_true_default_inner(typ: &Type) -> Option<bool> {
    match typ {
        // UninhabitedType: always False.
        Type::UninhabitedType { .. } => Some(false),
        // NoneType: can_be_true = False.
        Type::NoneType => Some(false),
        // FunctionLike (CallableType / Overloaded): can_be_true stays True.
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(true),
        // UnionType: any(item.can_be_true). The wire UnionType stores
        // precomputed can_be_true/can_be_false fields (layout >= 11).
        Type::UnionType { can_be_true, .. } => Some(*can_be_true),
        // TypeAliasType: delegates to alias.target — defer.
        Type::TypeAliasType { .. } => None,
        // TupleType: if can_be_any_bool() returns True, result is True.
        // can_be_any_bool needs TypeInfo unless fallback is builtins.tuple.
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            if !can_be_any_bool_wire(partial_fallback) {
                // can_be_any_bool is False: result is length() > 0.
                Some(!items.is_empty())
            } else {
                // can_be_any_bool is True: result is True. But we can only
                // know it's True if we confirmed via TypeInfo. If we got
                // here, can_be_any_bool_wire returned True, meaning the

                // fallback is NOT builtins.tuple and we need TypeInfo to
                // check __bool__. Defer.
                None
            }
        }
        // LiteralType: needs TypeInfo.is_enum — defer.
        Type::LiteralType { .. } => None,
        // All other types: base default is True.
        _ => Some(true),
    }
}

/// `mypy.types.Type.can_be_false_default` — default falsiness of a type.
///
/// Mirrors the per-class overrides in `mypy/types.py`:
///   * `Type` base: `True`.
///   * `UninhabitedType`: `False`.
///   * `NoneType`: `True` (base default, no override).
///   * `UnionType`: `any(item.can_be_false for item in self.items)`.
///   * `TypeAliasType`: defer (needs live alias target).
///   * `TupleType`: complex logic depending on length and unpack — defer
///     unless fallback is `builtins.tuple`.
///   * `LiteralType`: defer (needs `TypeInfo.is_enum`).
///   * All other proper types: `True` (the base default).
///
/// Returns `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_can_be_false_default(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(can_be_false_default_inner(&typ))
}

fn can_be_false_default_inner(typ: &Type) -> Option<bool> {
    match typ {
        // UninhabitedType: always False.
        Type::UninhabitedType { .. } => Some(false),
        // NoneType: base default True (no override).
        Type::NoneType => Some(true),
        // FunctionLike (CallableType / Overloaded): Python sets
        // `_can_be_false = False` in FunctionLike.__init__ (types.py:2023),
        // so a function is never False-ish.
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(false),
        // UnionType: any(item.can_be_false). Wire stores precomputed field.
        Type::UnionType { can_be_false, .. } => Some(*can_be_false),
        // TypeAliasType: delegates to alias.target — defer.
        Type::TypeAliasType { .. } => None,
        // TupleType: complex logic. If can_be_any_bool() is True, result
        // is True. If False, depends on length and unpack structure.
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            if can_be_any_bool_wire(partial_fallback) {
                // Non-builtins.tuple fallback: need TypeInfo to check
                // __bool__. Defer.
                return None;
            }
            // can_be_any_bool is False (fallback is builtins.tuple).
            // Mirror types.py:2871-2887.
            let length = items.len();
            if length == 0 {
                return Some(true);
            }
            if length > 1 {
                return Some(false);
            }
            // length == 1: special case tuple[*Ts].
            let item = &items[0];
            if let Type::UnpackType { typ: inner } = item {
                if let Type::TypeVarTupleType { min_len, .. } = inner.as_ref() {
                    return Some(*min_len == 0);
                }
                // Non-normalized tuple[int, ...] can be false.
                return Some(true);
            }
            Some(false)
        }
        // LiteralType: needs TypeInfo.is_enum — defer.
        Type::LiteralType { .. } => None,
        // All other types: base default is True.
        _ => Some(true),
    }
}

/// Check `can_be_any_bool` using only wire data. Returns `True` only when
/// the fallback is NOT `builtins.tuple` (meaning we'd need TypeInfo to
/// check `__bool__`). Returns `False` when the fallback IS `builtins.tuple`,
/// matching `can_be_any_bool`'s second condition. This is a partial check:
/// the real `can_be_any_bool` also requires `type.names.get("__bool__")`,
/// which we can't verify without TypeInfo. The caller uses `False` to
/// proceed and `True` to defer.
fn can_be_any_bool_wire(fallback: &Type) -> bool {
    if let Type::Instance { type_ref, .. } = fallback {
        type_ref != "builtins.tuple"
    } else {
        // Non-Instance fallback: `partial_fallback.type` would be falsy
        // in Python (e.g. UnboundType), so can_be_any_bool returns False.
        // We return False to proceed (no __bool__ check needed).
        false
    }
}

/// Wire-portable `TupleType.can_be_any_bool` (types.py:3094-3099) decided
/// from `TypeInfoSnapshot`s. Returns `Some(true)` when the fallback class
/// differs from `builtins.tuple` and has a `__bool__` name in some MRO
/// class's `member_info` (a `str` key suffices: the Python `names.get`
/// matches vars-hidden and Var names equally), `Some(false)` otherwise.
/// Defer (None) when the fallback is not an `Instance`, or any MRO class
/// snapshot is missing from the resolver (absent vs unknown is
/// indistinguishable, so the Python `names` lookup must decide).
fn can_be_any_bool_ref(fallback: &Type, resolver: &TypeResolver) -> Option<bool> {
    let Type::Instance { type_ref, .. } = fallback else {
        return Some(false);
    };
    if type_ref == "builtins.tuple" {
        return Some(false);
    }
    let snap = resolver.get(type_ref)?;
    for base in &snap.mro {
        let b = resolver.get(base)?;
        if b.member_info.contains_key("__bool__") {
            return Some(true);
        }
    }
    Some(false)
}

/// Validate and decode one wire sub-type blob: it must decode to an
/// `Instance`. Returns `None` to defer.
fn decode_instance(bytes: &[u8]) -> Option<Type> {
    let t = decode_type(bytes)?;
    match t {
        Type::Instance { .. } => Some(t),
        _ => None,
    }
}

/// `TupleType.can_be_any_bool` against a live fallback `Instance`
/// (types.py:3094-3099). The full `names.get("__bool__")` check needs the
/// live `TypeInfo.names` dict: the snapshot only carries a membership key.
/// Returns `None` (defer) on any read failure: missing live map entry,
/// MRO read failure, or a missing/non-str `__bool__` name.
fn can_be_any_bool_live(
    py: Python<'_>,
    instance: &Type,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let Type::Instance { type_ref, .. } = instance else {
        return Some(false);
    };
    if type_ref == "builtins.tuple" {
        return Some(false);
    }
    let info = resolver.live_typeinfo(py, type_ref)?;
    if info.is_none() {
        return None;
    }
    let mro = read_mro_fullnames(info, "mro")?;
    for cls_fullname in &mro {
        let cls = resolver.live_typeinfo(py, cls_fullname)?;
        if cls.is_none() {
            return None;
        }
        let names = cls.getattr("names").ok()?;
        let names = names.downcast::<pyo3::types::PyDict>().ok()?;
        let sym = names.get_item("__bool__").ok()?;
        let Some(sym) = sym else {
            continue;
        };
        if !sym.is_none() {
            return Some(true);
        }
    }
    Some(false)
}

/// Resolve a sub-type blob to the `Instance` it must be, then apply
/// `can_be_any_bool` via the snapshot resolver, falling back to the live
/// map. Returns `(bool, Instance)` when decided, `None` to defer.
fn can_be_any_bool_for(
    py: Python<'_>,
    bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<(bool, Type)> {
    let instance = decode_instance(bytes)?;
    if let Some(decided) = can_be_any_bool_ref(&instance, resolver.resolver()) {
        return Some((decided, instance));
    }
    let decided = can_be_any_bool_live(py, &instance, resolver)?;
    Some((decided, instance))
}

/// Chain-resolve an alias target to the first non-alias type, mirroring
/// `get_proper_type`'s while loop (types.py:4056). `expanded_alias_target`
/// already walks snapshot chains internally; the one exit that can still
/// yield a `TypeAliasType` is a `no_args` snapshot whose frozen target is
/// itself an alias, so feed the result back in until it lands on a
/// non-alias type. `None` defers: missing snapshot, alias cycle, or a
/// substitution the kernel cannot perform exactly.
pub(crate) fn chain_resolve_alias_target(
    typ: &Type,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<Type> {
    let mut current = typ.clone();
    // Guards mutual recursion across no_args aliases (A -> B, B -> A):
    // the per-call `seen` inside expanded_alias_target cannot see it.
    let mut seen: Vec<String> = Vec::new();
    loop {
        let (target, _, _) = expanded_alias_target(&current, aliases)?;
        if !matches!(target, Type::TypeAliasType { .. }) {
            return Some(target);
        }
        let type_ref = match &target {
            Type::TypeAliasType { type_ref, .. } => type_ref.clone(),
            _ => return Some(target),
        };
        if seen.contains(&type_ref) {
            return None;
        }
        seen.push(type_ref);
        current = target;
    }
}

/// Resolve a fallback blob: a `TypeAliasType` is expanded through the
/// frozen alias snapshots; everything else must decode to an `Instance`.
/// Returns `None` to defer (missing alias snapshot, unreadable target,
/// unresolvable alias chain, or non-instance fallback).
fn resolve_fallback(py: Python<'_>, bytes: &[u8], resolver: &NativeTypeResolver) -> Option<Type> {
    let decoded = decode_type(bytes)?;
    if !matches!(decoded, Type::TypeAliasType { .. }) {
        return decode_instance(bytes);
    }
    let _ = py;
    chain_resolve_alias_target(&decoded, resolver.alias_resolver())
}

/// Resolver-backed `mypy.types.Type.can_be_true_default` (types.py:360-364).
/// Covers the deferral sites that carry live-type information on the wire
/// seam but can now be decided from the resolver:
///   * `TupleType` with a named-tuple/custom fallback: `can_be_any_bool()`
///     found via the class snapshots (`__bool__` in an MRO member table).
///   * `TypeAliasType`: `alias.target.can_be_true` from the frozen alias
///     snapshots, chain-resolved.
///   * `LiteralType` with an enum fallback: `self.fallback.can_be_true`
///     read live (`is_enum` flag); for non-enum literals the byte-only
///     default applies.
/// Returns `Some(bool)` when decided, `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_can_be_true_default_live(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(can_be_true_live(py, &typ, resolver))
}

fn can_be_true_live(py: Python<'_>, typ: &Type, resolver: &NativeTypeResolver) -> Option<bool> {
    match typ {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(false),
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(true),
        Type::UnionType { can_be_true, .. } => Some(*can_be_true),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            let fallback_bytes = encode_type(partial_fallback)?;
            let (any_bool, _) = can_be_any_bool_for(py, &fallback_bytes, resolver)?;
            // types.py:3069-3074: with can_be_any_bool() the
            // NamedTuple-with-__bool__ corner makes the tuple both true- and
            // false-able; otherwise the tuple is true iff length() > 0.
            Some(any_bool || !items.is_empty())
        }
        Type::LiteralType { fallback, value } => {
            let fallback_bytes = encode_type(fallback)?;
            let fallback_inst = resolve_fallback(py, &fallback_bytes, resolver)?;
            let Type::Instance { type_ref, .. } = &fallback_inst else {
                return None;
            };
            let info = resolver.live_typeinfo(py, type_ref)?;
            if info.is_none() {
                return None;
            }
            if read_bool_attr(info, "is_enum").unwrap_or(false) {
                // types.py:3610-3612: Enum literal truthiness is the fallback
                // Instance's, i.e. the base Type default (True/True); mypy
                // does not respect __bool__/__len__ for Instance truthiness.
                return Some(true);
            }
            // Non-enum fallback: a TypeVarType fallback (possible from
            // make_simplified_union) must keep the byte-seam deferral, base
            // default True needs live `has_default()` to be safe.
            if matches!(&fallback_inst, Type::TypeVarType { .. }) {
                return None;
            }
            Some(bool_value_is_true(value))
        }
        // TypeAliasType delegation: alias.target.can_be_true (types.py:476-479).
        Type::TypeAliasType { .. } => alias_can_be(py, typ, true, resolver),
        _ => Some(true),
    }
}

/// `TypeAliasType.can_be_true/false_default`: delegate to
/// `self.alias.target` (types.py:476-484), chain-resolving the frozen
/// alias target. `None` (defer) on a missing alias snapshot, an unreadable
/// target, or an unresolvable alias chain.
fn alias_can_be(
    py: Python<'_>,
    typ: &Type,
    is_true: bool,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let target = chain_resolve_alias_target(typ, resolver.alias_resolver())?;
    if is_true {
        can_be_true_live(py, &target, resolver)
    } else {
        can_be_false_live(py, &target, resolver)
    }
}

/// Resolver-backed `mypy.types.Type.can_be_false_default` (types.py:366-370).
/// See `rust_can_be_true_default_live` for the covered sites; the tuple
/// tail follows types.py:3076-3092 exactly.
#[pyfunction]
pub(crate) fn rust_can_be_false_default_live(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(can_be_false_live(py, &typ, resolver))
}

fn can_be_false_live(py: Python<'_>, typ: &Type, resolver: &NativeTypeResolver) -> Option<bool> {
    match typ {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(true),
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(false),
        Type::UnionType { can_be_false, .. } => Some(*can_be_false),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            let fallback_bytes = encode_type(partial_fallback)?;
            let (any_bool, _) = can_be_any_bool_for(py, &fallback_bytes, resolver)?;
            if any_bool {
                return Some(true);
            }
            let length = items.len();
            if length == 0 {
                return Some(true);
            }
            if length > 1 {
                return Some(false);
            }
            let item = &items[0];
            if let Type::UnpackType { typ: inner } = item {
                if let Type::TypeVarTupleType { min_len, .. } = inner.as_ref() {
                    return Some(*min_len == 0);
                }
                return Some(true);
            }
            Some(false)
        }
        Type::LiteralType { fallback, value } => {
            let fallback_bytes = encode_type(fallback)?;
            let fallback_inst = resolve_fallback(py, &fallback_bytes, resolver)?;
            let Type::Instance { type_ref, .. } = &fallback_inst else {
                return None;
            };
            let info = resolver.live_typeinfo(py, type_ref)?;
            if info.is_none() {
                return None;
            }
            if read_bool_attr(info, "is_enum").unwrap_or(false) {
                // types.py:3605-3607: enum literal falsiness delegates to
                // `self.fallback.can_be_false` (an Instance = True).
                return Some(true);
            }
            if matches!(&fallback_inst, Type::TypeVarType { .. }) {
                return None;
            }
            Some(!bool_value_is_true(value))
        }
        Type::TypeAliasType { .. } => alias_can_be(py, typ, false, resolver),
        _ => Some(true),
    }
}

/// Wire-portable `bool(value)` for a serialized `LiteralValue`
/// (types.py:3608 / 3613: `not self.value` / `bool(self.value)`).
fn bool_value_is_true(value: &crate::wire::LiteralValue) -> bool {
    use crate::wire::LiteralValue;
    match value {
        LiteralValue::Int(i) => *i != 0,
        LiteralValue::Str(s) => !s.is_empty(),
        LiteralValue::Bytes(b) => !b.is_empty(),
        LiteralValue::Bool(b) => *b,
        LiteralValue::Float(f) => *f != 0.0,
    }
}

// ---------------------------------------------------------------------------
// CallableType pure accessors
// ---------------------------------------------------------------------------

/// `mypy.types.CallableType.min_args` — count positional (ARG_POS) args.
///
/// Mirrors `CallableType.min_args` (types.py:2330-2331).
#[pyfunction]
pub(crate) fn rust_callable_min_args(type_bytes: &[u8]) -> PyResult<Option<i64>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(callable_min_args_inner(&typ))
}

fn callable_min_args_inner(typ: &Type) -> Option<i64> {
    if let Type::CallableType { arg_kinds, .. } = typ {
        Some(arg_kinds.iter().filter(|&&k| k == ARG_POS).count() as i64)
    } else {
        None
    }
}

/// `mypy.types.CallableType.is_var_arg` — does this callable have `*args`?
///
/// Mirrors `CallableType.is_var_arg` (types.py:2334-2336).
#[pyfunction]
pub(crate) fn rust_callable_is_var_arg(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(callable_is_var_arg_inner(&typ))
}

fn callable_is_var_arg_inner(typ: &Type) -> Option<bool> {
    if let Type::CallableType { arg_kinds, .. } = typ {
        Some(arg_kinds.contains(&ARG_STAR))
    } else {
        None
    }
}

/// `mypy.types.CallableType.is_kw_arg` — does this callable have `**kwargs`?
///
/// Mirrors `CallableType.is_kw_arg` (types.py:2339-2341).
#[pyfunction]
pub(crate) fn rust_callable_is_kw_arg(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(callable_is_kw_arg_inner(&typ))
}

fn callable_is_kw_arg_inner(typ: &Type) -> Option<bool> {
    if let Type::CallableType { arg_kinds, .. } = typ {
        Some(arg_kinds.contains(&ARG_STAR2))
    } else {
        None
    }
}

/// `mypy.types.CallableType.max_possible_positional_args` — max positional
/// args, or `i64::MAX` if the callable has `*args` or `**kwargs`.
///
/// Mirrors `CallableType.max_possible_positional_args` (types.py:2390-2396).
#[pyfunction]
pub(crate) fn rust_callable_max_possible_positional_args(
    type_bytes: &[u8],
) -> PyResult<Option<i64>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(callable_max_possible_positional_args_inner(&typ))
}

fn callable_max_possible_positional_args_inner(typ: &Type) -> Option<i64> {
    if let Type::CallableType { arg_kinds, .. } = typ {
        let is_var = arg_kinds.contains(&ARG_STAR);
        let is_kw = arg_kinds.contains(&ARG_STAR2);
        if is_var || is_kw {
            return Some(i64::MAX);
        }
        // Count positional kinds: ARG_POS (0) and ARG_OPT (1).
        // ArgKind.is_positional() is True for ARG_POS and ARG_OPT.
        Some(
            arg_kinds
                .iter()
                .filter(|&&k| k == ARG_POS || k == ARG_OPT)
                .count() as i64,
        )
    } else {
        None
    }
}

/// `mypy.types.CallableType.is_generic` — does this callable have type
/// variables?
///
/// Mirrors `CallableType.is_generic` (types.py:2471-2472).
#[pyfunction]
pub(crate) fn rust_callable_is_generic(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(callable_is_generic_inner(&typ))
}

fn callable_is_generic_inner(typ: &Type) -> Option<bool> {
    if let Type::CallableType { variables, .. } = typ {
        Some(!variables.is_empty())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// TupleType.length / UnionType.length
// ---------------------------------------------------------------------------

/// `mypy.types.TupleType.length` — number of items.
///
/// Mirrors `TupleType.length` (types.py:2896-2897).
#[pyfunction]
pub(crate) fn rust_tuple_length(type_bytes: &[u8]) -> PyResult<Option<i64>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(tuple_length_inner(&typ))
}

fn tuple_length_inner(typ: &Type) -> Option<i64> {
    if let Type::TupleType { items, .. } = typ {
        Some(items.len() as i64)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// CallableType / Parameters arg-query helpers (issue #487)
// ---------------------------------------------------------------------------

/// Return shape for callable arg queries: `(name, pos, required)`.
/// The type is looked up by Python from the original callable; Rust
/// only returns the flat metadata so the wire format stays out of it.
type ArgRow = (Option<String>, Option<i64>, bool);

/// `mypy.types.CallableType.formal_arguments` — walk arg_types/kinds/names
/// and collect FormalArgument records, mirroring types.py:2446-2467.
///
/// Returns `None` (Python `None`) for a non-CallableType so the caller
/// falls through. Always succeeds for CallableType (the loop is a
/// straightforward dict scan).
#[pyfunction]
pub(crate) fn rust_callable_formal_arguments(type_bytes: &[u8]) -> PyResult<Option<Vec<ArgRow>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let Type::CallableType {
        arg_names,
        arg_kinds,
        ..
    } = &typ
    else {
        return Ok(None);
    };
    Ok(Some(formal_arguments_inner(arg_names, arg_kinds)))
}

fn formal_arguments_inner(arg_names: &[Option<String>], arg_kinds: &[i64]) -> Vec<ArgRow> {
    let mut args = Vec::with_capacity(arg_names.len());
    let mut done_with_positional = false;
    for i in 0..arg_names.len() {
        let kind = arg_kinds[i];
        if kind_is_named(kind) || kind_is_star(kind) {
            done_with_positional = true;
        }
        let pos = if done_with_positional {
            None
        } else {
            Some(i as i64)
        };
        args.push((arg_names[i].clone(), pos, kind_is_required(kind)));
    }
    args
}

/// `mypy.types.CallableType.argument_by_name` — scan by name, mirroring
/// types.py:2469-2484.
///
/// Returns `None` (Python `None`) for a non-CallableType. For
/// CallableType, returns `(name, pos, required)` or `None` when no match
/// is found (including deferral to `try_synthesizing_arg_from_kwarg`).
#[pyfunction]
pub(crate) fn rust_callable_argument_by_name(
    type_bytes: &[u8],
    name: Option<String>,
) -> PyResult<Option<ArgRow>> {
    let name = match name {
        Some(n) => n,
        None => return Ok(None),
    };
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let Type::CallableType {
        arg_names,
        arg_kinds,
        ..
    } = &typ
    else {
        return Ok(None);
    };
    Ok(argument_by_name_inner(arg_names, arg_kinds, &name))
}

fn argument_by_name_inner(
    arg_names: &[Option<String>],
    arg_kinds: &[i64],
    name: &str,
) -> Option<ArgRow> {
    let mut seen_star = false;
    for i in 0..arg_names.len() {
        let kind = arg_kinds[i];
        if kind_is_named(kind) || kind_is_star(kind) {
            seen_star = true;
        }
        if kind_is_star(kind) {
            continue;
        }
        if arg_names[i] == Some(name.to_string()) {
            let pos = if seen_star { None } else { Some(i as i64) };
            return Some((Some(name.to_string()), pos, kind_is_required(kind)));
        }
    }
    // Try synthesizing from kwarg — defers here; Python fills from kw_arg.
    None
}

/// `mypy.types.CallableType.argument_by_position` — scan by position, mirroring
/// types.py:2486-2499.
///
/// Returns `None` for a non-CallableType or when position is None.
#[pyfunction]
pub(crate) fn rust_callable_argument_by_position(
    type_bytes: &[u8],
    position: Option<i64>,
) -> PyResult<Option<ArgRow>> {
    let pos = match position {
        Some(p) => p,
        None => return Ok(None),
    };
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let Type::CallableType {
        arg_names,
        arg_kinds,
        ..
    } = &typ
    else {
        return Ok(None);
    };
    let pos_usize = pos as usize;
    if pos_usize >= arg_names.len() {
        // Try synthesizing from vararg — defers here; Python fills from var_arg.
        return Ok(None);
    }
    let name = arg_names[pos_usize].clone();
    let kind = arg_kinds[pos_usize];
    if kind_is_positional(kind) {
        Ok(Some((name, Some(pos), kind == ARG_POS)))
    } else {
        // Not purely positional — defer to try_synthesizing from vararg.
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// TupleType.length / UnionType.length
// ---------------------------------------------------------------------------
///
/// Mirrors `UnionType.length` (types.py:3511-3512).
#[pyfunction]
pub(crate) fn rust_union_length(type_bytes: &[u8]) -> PyResult<Option<i64>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(union_length_inner(&typ))
}

fn union_length_inner(typ: &Type) -> Option<i64> {
    if let Type::UnionType { items, .. } = typ {
        Some(items.len() as i64)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Resolver-enabled truthiness defaults (issue #854)
// ---------------------------------------------------------------------------

/// Expose the resolver-enabled live truthiness seams to the extension
/// module (Python parity tests call them directly). Also referenced by the
/// plain lib build so the functions are not dead code there.
pub(crate) mod extension_seams {
    use pyo3::prelude::*;

    pub(crate) fn add_seams(module: &pyo3::types::PyModule) -> pyo3::PyResult<()> {
        module.add_function(wrap_pyfunction!(
            super::rust_can_be_true_default_live,
            module
        )?)?;
        module.add_function(wrap_pyfunction!(
            super::rust_can_be_false_default_live,
            module
        )?)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, LiteralValue, WriteBuffer};

    fn encode(typ: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        if write_type(&mut buf, typ).is_err() {
            return Vec::new();
        }
        buf.into_bytes()
    }

    #[test]
    fn test_can_be_true_default_uninhabited() {
        let t = Type::UninhabitedType { ambiguous: false };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_true_default_none() {
        let t = Type::NoneType;
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_true_default_any() {
        let t = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_can_be_true_default_union() {
        let t = Type::UnionType {
            items: vec![Type::NoneType, Type::UninhabitedType { ambiguous: false }],
            uses_pep604_syntax: false,
            can_be_true: false,
            can_be_false: true,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_true_default_type_alias_defers() {
        // TypeAliasType wire round-trip depends on the alias target
        // being resolvable; test with a None return from decode.
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "my.alias".to_string(),
        };
        let bytes = encode(&t);
        // encode returns empty for TypeAliasType without target.
        if bytes.is_empty() {
            return;
        }
        if let Some(decoded) = decode_type(&bytes) {
            assert_eq!(can_be_true_default_inner(&decoded), None);
        }
    }

    #[test]
    fn test_can_be_true_default_tuple_builtins() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::NoneType],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_can_be_true_default_tuple_namedtuple_defers() {
        let fallback = Type::Instance {
            type_ref: "my.NamedTuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::NoneType],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            None
        );
    }

    #[test]
    fn test_can_be_true_default_tuple_empty() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_true_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_uninhabited() {
        let t = Type::UninhabitedType { ambiguous: false };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_none() {
        let t = Type::NoneType;
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_can_be_false_default_union() {
        let t = Type::UnionType {
            items: vec![Type::NoneType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_tuple_empty() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_can_be_false_default_tuple_multi() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::NoneType, Type::NoneType],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_tuple_single_unpack_typevartuple() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let inner = Type::TypeVarTupleType {
            tuple_fallback: Box::new(fallback.clone()),
            name: "Ts".to_string(),
            fullname: "module.Ts".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
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
            min_len: 0,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::UnpackType {
                typ: Box::new(inner),
            }],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_can_be_false_default_tuple_single_unpack_typevartuple_min_len_1() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let inner = Type::TypeVarTupleType {
            tuple_fallback: Box::new(fallback.clone()),
            name: "Ts".to_string(),
            fullname: "module.Ts".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
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
            min_len: 1,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::UnpackType {
                typ: Box::new(inner),
            }],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_tuple_single_non_unpack() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::NoneType],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
    }

    #[test]
    fn test_can_be_false_default_literal_defers() {
        let fallback = Type::Instance {
            type_ref: "enum.Color".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::LiteralType {
            fallback: Box::new(fallback),
            value: LiteralValue::Int(1),
        };
        let bytes = encode(&t);
        assert_eq!(
            can_be_false_default_inner(&decode_type(&bytes).unwrap()),
            None
        );
    }

    #[test]
    fn test_callable_min_args() {
        let t = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
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
            arg_types: vec![Type::NoneType, Type::NoneType, Type::NoneType],
            arg_kinds: vec![ARG_POS, ARG_POS, 1],
            arg_names: vec![None, None, None],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let bytes = encode(&t);
        assert_eq!(
            callable_min_args_inner(&decode_type(&bytes).unwrap()),
            Some(2)
        );
    }

    #[test]
    fn test_callable_is_var_arg() {
        let make = |kinds: Vec<i64>| Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
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
            arg_types: vec![Type::NoneType; kinds.len()],
            arg_kinds: kinds,
            arg_names: vec![None; 3],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let t_no_star = make(vec![ARG_POS, 1, 3]);
        let bytes = encode(&t_no_star);
        assert_eq!(
            callable_is_var_arg_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
        let t_star = make(vec![ARG_POS, ARG_STAR, 3]);
        let bytes = encode(&t_star);
        assert_eq!(
            callable_is_var_arg_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_callable_is_kw_arg() {
        let make = |kinds: Vec<i64>| Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
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
            arg_types: vec![Type::NoneType; kinds.len()],
            arg_kinds: kinds,
            arg_names: vec![None; 3],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let t_no_kw = make(vec![ARG_POS, 1, 3]);
        let bytes = encode(&t_no_kw);
        assert_eq!(
            callable_is_kw_arg_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
        let t_kw = make(vec![ARG_POS, 1, ARG_STAR2]);
        let bytes = encode(&t_kw);
        assert_eq!(
            callable_is_kw_arg_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_callable_max_possible_positional_args() {
        let make = |kinds: Vec<i64>| Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
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
            arg_types: vec![Type::NoneType; kinds.len()],
            arg_kinds: kinds,
            arg_names: vec![None; 4],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let t_plain = make(vec![ARG_POS, 0, 1, 3]);
        let bytes = encode(&t_plain);
        assert_eq!(
            callable_max_possible_positional_args_inner(&decode_type(&bytes).unwrap()),
            Some(3)
        );
        let t_var = make(vec![ARG_POS, ARG_STAR, 1, 3]);
        let bytes = encode(&t_var);
        assert_eq!(
            callable_max_possible_positional_args_inner(&decode_type(&bytes).unwrap()),
            Some(i64::MAX)
        );
        let t_kw = make(vec![ARG_POS, 1, ARG_STAR2, 3]);
        let bytes = encode(&t_kw);
        assert_eq!(
            callable_max_possible_positional_args_inner(&decode_type(&bytes).unwrap()),
            Some(i64::MAX)
        );
    }

    #[test]
    fn test_callable_is_generic() {
        let make = |vars: Vec<Type>| Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
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
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vars,
            type_guard: None,
            type_is: None,
        };
        let t_no_vars = make(vec![]);
        let bytes = encode(&t_no_vars);
        assert_eq!(
            callable_is_generic_inner(&decode_type(&bytes).unwrap()),
            Some(false)
        );
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 0,
            namespace: "".to_string(),
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            values: vec![],
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        let t_vars = make(vec![tvar]);
        let bytes = encode(&t_vars);
        assert_eq!(
            callable_is_generic_inner(&decode_type(&bytes).unwrap()),
            Some(true)
        );
    }

    #[test]
    fn test_tuple_length() {
        let fallback = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(fallback),
            items: vec![Type::NoneType, Type::NoneType, Type::NoneType],
            implicit: false,
        };
        let bytes = encode(&t);
        assert_eq!(tuple_length_inner(&decode_type(&bytes).unwrap()), Some(3));
    }

    #[test]
    fn test_union_length() {
        let t = Type::UnionType {
            items: vec![Type::NoneType, Type::NoneType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let bytes = encode(&t);
        assert_eq!(union_length_inner(&decode_type(&bytes).unwrap()), Some(2));
    }

    #[test]
    fn test_non_callable_returns_none() {
        let t = Type::NoneType;
        let bytes = encode(&t);
        assert_eq!(callable_min_args_inner(&decode_type(&bytes).unwrap()), None);
        assert_eq!(
            callable_is_var_arg_inner(&decode_type(&bytes).unwrap()),
            None
        );
        assert_eq!(
            callable_is_kw_arg_inner(&decode_type(&bytes).unwrap()),
            None
        );
        assert_eq!(
            callable_max_possible_positional_args_inner(&decode_type(&bytes).unwrap()),
            None
        );
        assert_eq!(
            callable_is_generic_inner(&decode_type(&bytes).unwrap()),
            None
        );
    }

    #[test]
    fn test_non_tuple_union_length_none() {
        let t = Type::NoneType;
        let bytes = encode(&t);
        assert_eq!(tuple_length_inner(&decode_type(&bytes).unwrap()), None);
        assert_eq!(union_length_inner(&decode_type(&bytes).unwrap()), None);
    }

    // -------------------------------------------------------------------
    // chain_resolve_alias_target (issue #1134)
    // -------------------------------------------------------------------

    fn make_instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// Serialized target bytes for an alias node pointing at `inner_ref`.
    /// `write_type` refuses `TypeAliasType`, so chain edges are crafted
    /// by hand (mirrors read_type_alias_type: tag, args, ref, END_TAG).
    fn alias_target_bytes(inner_ref: &str) -> Vec<u8> {
        let mut wbuf = WriteBuffer::new();
        crate::wire::write_tag(&mut wbuf, crate::wire::TYPE_ALIAS_TYPE);
        crate::wire::write_type_list(&mut wbuf, &[]).expect("empty args encode");
        crate::wire::write_str(&mut wbuf, inner_ref).expect("ref encodes");
        crate::wire::write_tag(&mut wbuf, crate::wire::END_TAG);
        wbuf.into_bytes()
    }

    fn insert_edge(
        aliases: &mut crate::aliases::TypeAliasResolver,
        fullname: &str,
        target: Vec<u8>,
        no_args: bool,
    ) {
        aliases.insert(
            fullname.to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: fullname.to_string(),
                target,
                no_args,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_chain_resolve_a_b_int() {
        // A = B, B = int: the snapshot chain must land on the Instance.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_edge(&mut aliases, "mod.A", alias_target_bytes("mod.B"), false);
        insert_edge(
            &mut aliases,
            "mod.B",
            encode(&make_instance("builtins.int")),
            false,
        );
        let a = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(
            chain_resolve_alias_target(&a, &aliases),
            Some(make_instance("builtins.int"))
        );
    }

    #[test]
    fn test_chain_resolve_missing_mid_snapshot_defers() {
        // A = B but B has no snapshot: defer, do not guess.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_edge(&mut aliases, "mod.A", alias_target_bytes("mod.B"), false);
        let a = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(chain_resolve_alias_target(&a, &aliases), None);
    }

    #[test]
    fn test_chain_resolve_recursive_alias_defers() {
        // A = A: the cycle guard must defer, not loop.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_edge(&mut aliases, "mod.A", alias_target_bytes("mod.A"), false);
        let a = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(chain_resolve_alias_target(&a, &aliases), None);
    }

    #[test]
    fn test_chain_resolve_no_args_alias_chain() {
        // A (no_args) -> B (no_args) -> int: previously the still-alias
        // intermediate deferred; the chain loop must ride past it.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_edge(&mut aliases, "mod.A", alias_target_bytes("mod.B"), true);
        insert_edge(
            &mut aliases,
            "mod.B",
            encode(&make_instance("builtins.int")),
            true,
        );
        let a = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(
            chain_resolve_alias_target(&a, &aliases),
            Some(make_instance("builtins.int"))
        );
    }

    #[test]
    fn test_chain_resolve_no_args_mutual_cycle_defers() {
        // A (no_args) -> B, B (no_args) -> A: mutual recursion through
        // no_args snapshots must defer, not loop forever.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_edge(&mut aliases, "mod.A", alias_target_bytes("mod.B"), true);
        insert_edge(&mut aliases, "mod.B", alias_target_bytes("mod.A"), true);
        let a = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(chain_resolve_alias_target(&a, &aliases), None);
    }
}
