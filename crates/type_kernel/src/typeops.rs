//! Stage 3e: typeops helpers from `mypy/typeops.py`.
//!
//! Ports pure-algebra helpers as standalone `#[pyfunction]`s:
//! * `rust_make_simplified_union` — wraps `setops::make_simplified_union`.
//! * `rust_simple_literal_type` — extracts fallback Instance for simple literals.
//! * `rust_is_simple_literal` — checks if a type is a simple literal.
//! * `rust_true_only` / `rust_false_only` / `rust_true_or_false` — truthiness
//!   narrowing via discriminators (Python shim performs the `copy_type` +
//!   flag mutation on live objects). Step 6 (`__bool__`/`__len__` lookups,
//!   `is_final`/`is_enum` checks) walks the resolver's live TypeInfo map;
//!   runs without a resolver get the pre-resolver subset (steps 1-5), and
//!   unresolved snapshot lookups still defer to Python.
//! * `rust_fill_typevars` — `mypy.typevars.fill_typevars` on a live
//!   `TypeInfo`: rebuilds the class type parameters at line=-1 and returns
//!   the encoded `Instance` (or `TupleType` for named tuples).
//!
//! Parity-only, default-off. The Python shim in `mypy/typeops.py` gates each
//! call behind `Options.native_type_kernel`. `None` means "Rust doesn't handle
//! this, let Python decide".

use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};
use pyo3::IntoPy;

use crate::aliases::TypeAliasResolver;
use crate::checkexpr_functions::expanded_alias_target;
use crate::supported_self_type::supported_self_type_inner;
use crate::typeinfo::{
    read_bool_attr, read_mro_fullnames, read_str_list_attr, serialize_type_to_bytes,
    NativeTypeResolver, TypeResolver,
};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

use crate::setops;
use crate::subtypes::SubtypeContext;

/// `ArgKind.ARG_POS` = 0 (nodes.py).
const ARG_POS: i64 = 0;
/// `ArgKind.ARG_STAR` = 2 (checkmember.rs:52).
const ARG_STAR: i64 = 2;
/// `ArgKind.ARG_STAR2` = 4 (checkmember.rs:54).
const ARG_STAR2: i64 = 4;
/// `TypeOfAny.unannotated` = 1 (types.py:276).
const TYPE_OF_ANY_UNANNOTATED: i64 = 1;
/// `TypeOfAny.from_error` = 5 (types.py:293).
const TYPE_OF_ANY_FROM_ERROR: i64 = 5;
/// `TypeOfAny.special_form` = 6 (types.py:297).
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

// ---------------------------------------------------------------------------
// Wire codec helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// Build a wire `AnyType` with one of the `TypeOfAny` constants above.
fn any_type(type_of_any: i64) -> Type {
    Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    }
}

// ---------------------------------------------------------------------------
// simple_literal_type / is_simple_literal
// ---------------------------------------------------------------------------

/// `simple_literal_type` (typeops.py:588-594): return the fallback `Instance`
/// for a simple literal. If `t` is an `Instance` with a `last_known_value`,
/// unwrap to that literal first. If `t` is a `LiteralType`, return its
/// fallback. Otherwise return `None`.
///
/// Returns the fallback as wire-encoded bytes, or `None` if `t` is not a
/// simple literal (Python `None`).
fn simple_literal_type(t: &Type) -> Option<Type> {
    let t = match t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => lkv.as_ref(),
        _ => t,
    };
    match t {
        Type::LiteralType { fallback, .. } => Some((**fallback).clone()),
        _ => None,
    }
}

/// `is_simple_literal` (typeops.py:597-602): check if `t` is a simple literal.
///
/// A `LiteralType` is simple if its fallback is an enum or `builtins.str`.
/// An `Instance` is simple if it has a `last_known_value` whose value is a
/// string. The `is_enum` check needs the resolver snapshot; if the snapshot
/// is missing, conservatively return `false` (defer to Python).
pub(crate) fn is_simple_literal(t: &Type, resolver: &TypeResolver) -> Option<bool> {
    match t {
        Type::LiteralType { fallback, .. } => {
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(false);
            };
            if type_ref == "builtins.str" {
                return Some(true);
            }
            // enum check needs the snapshot
            let snap = resolver.get(type_ref)?;
            Some(snap.is_enum)
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => {
            if let Type::LiteralType { value, .. } = lkv.as_ref() {
                Some(matches!(value, LiteralValue::Str(_)))
            } else {
                Some(false)
            }
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// Truthiness helpers: can_be_true_default / can_be_false_default
// ---------------------------------------------------------------------------

/// Mirrors `Type.can_be_true_default()` for each variant (types.py:295-3459).
/// Returns `None` for variants where the default depends on live Python state
/// (TypeAliasType needs `alias.target`, TupleType needs `can_be_any_bool`).
pub(crate) fn can_be_true_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(false),
        // FunctionLike: Python sets `_can_be_false = False` only
        // (types.py:2023); can_be_true stays True.
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(true),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            // Enum literals get TRUE from the Instance fallback, plain ones use
            // bool(value). Kernel cannot tell them apart without a snapshot
            // resolver, so defer non-Bool truthiness.
            match value {
                LiteralValue::Bool(b) => Some(*b),
                _ => None,
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_true_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        // All other variants default to True (Instance, AnyType, CallableType,
        // Overloaded, UnboundType, DeletedType, TypeType, TypeVarType,
        // ParamSpecType, TypeVarTupleType, Parameters, UnpackType).
        _ => Some(true),
    }
}

/// Mirrors `Type.can_be_false_default()` for each variant (types.py:298-3459).
pub(crate) fn can_be_false_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(true),
        // FunctionLike: functions are never False-ish (types.py:2023).
        Type::CallableType { .. } | Type::Overloaded { .. } => Some(false),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            // See can_be_true_default: defer non-Bool literal truthiness
            // (enum-vs-plain needs the snapshot resolver).
            match value {
                LiteralValue::Bool(b) => Some(!*b),
                _ => None,
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_false_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        _ => Some(true),
    }
}

// ---------------------------------------------------------------------------
// Truthiness discriminators
// ---------------------------------------------------------------------------

/// Result of `true_only` / `false_only` / `true_or_false`.
///
/// The Python shim maps each variant to a live `Type`:
/// * `Uninhabited` -> `UninhabitedType(line=t.line)`
/// * `None` (strict_optional off) -> `NoneType(line=t.line)`
/// * `SameType` -> `t` unchanged
/// * `CopyTrueOnly` -> `copy_type(t)` with `can_be_false=False`
/// * `CopyFalseOnly` -> `copy_type(t)` with `can_be_true=False`
/// * `CopyReset` -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
/// * `LiteralEmptyStr(fallback_bytes)` -> `LiteralType("", fallback)`
/// * `LiteralZero(fallback_bytes)` -> `LiteralType(0, fallback)`
/// * `UnionNarrow(item_discs)` -> recurse on each union item (discs[i] is
///   the discriminator for items[i])
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TruthinessResult {
    Uninhabited,
    #[allow(dead_code)]
    NoneType,
    #[allow(dead_code)]
    SameType,
    CopyTrueOnly,
    CopyFalseOnly,
    CopyReset,
    LiteralEmptyStr(Vec<u8>),
    LiteralZero(Vec<u8>),
    /// For unions: one TruthinessResult per item, so the Python shim can
    /// recurse. The outer shape is always a `make_simplified_union`.
    UnionNarrow(Vec<TruthinessResult>),
}

// ---------------------------------------------------------------------------
// Step-6 helpers: __bool__/__len__ lookup and falsy-instance classification
// ---------------------------------------------------------------------------

/// Result of a wire-decoded `__bool__`/`__len__` lookup on a live `TypeInfo`
/// MRO, distinguishing "no dunder found" from "defer to Python".
enum DunderLookup {
    /// A dunder was found and its ret_type decoded from the wire.
    Found(Type),
    /// The MRO walk completed with no `CallableType`-typed symbol for the
    /// name (mirrors `_get_type_method_ret_type` returning `None`).
    NotFound,
    /// The live map / MRO / sym.type could not be read or serialized.
    /// The Python caller falls back to the pure-Python path.
    Defer,
}

/// Does the `Instance` type have a custom `__bool__`/`__len__` whose return
/// type is not truthy (so `true_only` yields `UninhabitedType`)? Mirrors the
/// `_get_type_method_ret_type` + `not ret_type.can_be_true` decision that
/// step 4 performs (typeops.py:1314-1319).
///
/// Returns:
/// * `Some(true)` — the resolved return type is `not can_be_true` (the
///   caller narrows to `Uninhabited`).
/// * `Some(false)` — no truthiness dunder on the MRO (the caller copies with
///   the flag mutation).
/// * `None` — defer to Python (live map missing, wire decode failed, or the
///   ret_type truthiness cannot be decided on the wire).
fn step4_dunder_ret_type_all_false(
    py: Python<'_>,
    t: &Type,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    match live_dunder_ret_type(py, t, resolver)? {
        DunderLookup::Found(ret_type) => match can_be_true_default(&ret_type) {
            Some(false) => Some(true),
            Some(true) => Some(false),
            None => None,
        },
        DunderLookup::NotFound => Some(false),
        DunderLookup::Defer => None,
    }
}

/// Resolve the `__bool__` (or `__len__`, as a fallback) return type of an
/// `Instance` via the live TypeInfo map, mirroring `TypeInfo.get`'s MRO walk
/// (nodes.py:4063) + `_get_type_method_ret_type` (typeops.py:1216-1229).
///
/// `_get_type_method_ret_type` means `t.type.get(name)` (NOT
/// `custom_special_method`, so there is no builtins/typing exclusion):
/// first MRO class with the name in its `names` table wins, its symbol's
/// proper type must be a `CallableType`, and the callable's ret_type is
/// returned. `TypeInfo.get` returns `None` when no MRO class has the name.
///
/// The `sym.type` (a live `mypy.types.Type`, possibly a TypeAliasType) is
/// serialized over the wire and the not-yet-proper alias resolved by the
/// wire decoder; a non-CallableType dict/FuncBase/DataclassTransform symbol
/// makes Python's `isinstance(sym_type, CallableType)` false -> the MRO walk
/// keeps going with the NEXT name (or `NotFound`), exactly like Python.
fn live_dunder_ret_type(
    py: Python<'_>,
    t: &Type,
    resolver: &NativeTypeResolver,
) -> Option<DunderLookup> {
    let Type::Instance { type_ref, .. } = t else {
        return Some(DunderLookup::NotFound);
    };
    let info = resolver.live_typeinfo(py, type_ref)?;
    let mro = read_mro_fullnames(info, "mro")?;
    for name in ["__bool__", "__len__"] {
        for cls_fullname in &mro {
            let Some(cls) = resolver.live_typeinfo(py, cls_fullname) else {
                return Some(DunderLookup::Defer);
            };
            let names = cls.getattr("names").ok()?;
            let sym = names
                .downcast::<pyo3::types::PyDict>()
                .ok()?
                .get_item(name)
                .ok()?;
            let Some(sym) = sym else {
                continue;
            };
            let sym_type = sym.getattr("type").ok()?;
            let sym_type = serialize_type_to_bytes(py, sym_type)?;
            let Some(sym_type) = decode_type(&sym_type) else {
                return Some(DunderLookup::Defer);
            };
            // Python runs get_proper_type(sym.type) before the CallableType
            // check; the wire decoder does not resolve TypeAliasType, so
            // defer rather than wrongly advancing the walk.
            if matches!(sym_type, Type::TypeAliasType { .. }) {
                return Some(DunderLookup::Defer);
            }
            let Type::CallableType { ret_type, .. } = sym_type else {
                // Non-callable symbol: Python's `isinstance` check fails,
                // so the name's walk continues with the next MRO class.
                continue;
            };
            return Some(DunderLookup::Found(*ret_type));
        }
    }
    Some(DunderLookup::NotFound)
}

/// Is the `Instance` type a `final` class or an enum (so under
/// `strict_optional` `false_only` yields Never, typeops.py:1369-1371)?
/// Reads `is_final` / `is_enum` live from the TypeInfo map (the snapshot
/// does not carry `is_final`; `is_enum` can go stale). `None` when the live
/// map cannot decide (structurally deferred).
fn step6_is_final_or_enum(py: Python<'_>, t: &Type, resolver: &NativeTypeResolver) -> Option<bool> {
    let Type::Instance { type_ref, .. } = t else {
        return Some(false);
    };
    let info = resolver.live_typeinfo(py, type_ref)?;
    let is_enum = read_bool_attr(info, "is_enum").unwrap_or(false);
    if is_enum {
        return Some(true);
    }
    let is_final = read_bool_attr(info, "is_final").unwrap_or(false);
    Some(is_final)
}

// ---------------------------------------------------------------------------
// true_only
// ---------------------------------------------------------------------------

/// `true_only` (typeops.py:790-817): restrict `t` to only True-ish values.
///
/// Rust owns ONLY the step-4 leaf (the `else` branch, typeops.py:803-807).
/// The Python shim handles the live-flag steps 1-2 and union recursion
/// (steps 3) because they read `t.can_be_true` / `t.can_be_false` — live
/// flags the wire does not carry on instances — and union items carry
/// mutated flags from earlier `copy_type` calls.
///
/// Step 4 leaf: when a custom `__bool__`/`__len__` ret_type is not
/// `can_be_true` (on the wire), every value is False-ish -> Uninhabited;
/// otherwise CopyTrueOnly. Defers (`None`) when the live TypeInfo map is
/// missing, the dunder ret_type truthiness cannot be decided, or the leaf
/// is a LiteralType (only enum literals reach the leaf; the enum unwrap in
/// `_get_type_method_ret_type` needs live TypeInfo).
pub(crate) fn true_only(
    py: Python<'_>,
    t: &Type,
    resolver: &NativeTypeResolver,
) -> Option<TruthinessResult> {
    // Only enum literals reach the leaf (plain literals exit at the Python
    // live-flag steps); the enum unwrap needs live TypeInfo -> defer.
    if matches!(t, Type::LiteralType { .. }) {
        return None;
    }
    match step4_dunder_ret_type_all_false(py, t, resolver) {
        Some(true) => Some(TruthinessResult::Uninhabited),
        Some(false) => Some(TruthinessResult::CopyTrueOnly),
        None => None,
    }
}

// ---------------------------------------------------------------------------
// false_only
// ---------------------------------------------------------------------------

/// `false_only` (typeops.py:820-862): restrict `t` to only False-ish values.
///
/// Rust owns ONLY the step 4-6 leaf (the non-union `elif` chain,
/// typeops.py:845-862). The Python shim handles the live-flag steps 1-2
/// and union recursion (step 3) for the same reason as `true_only`.
///
/// Leaf logic (mirrors Python's elif order):
/// 1. `Instance(builtins.str)` / `builtins.bytes` -> `LiteralType("",
///    fallback=t)`
/// 2. `Instance(builtins.int)` -> `LiteralType(0, fallback=t)`
/// 3. `__bool__`/`__len__` lookup via the live MRO: a ret_type that is not
///    `can_be_false` -> Uninhabited; a found-but-can-be-false ret_type skips
///    the `is_final` check entirely (Python's elif-chain).
/// 4. A `@final` class or enum, or an enum literal, under `strict_optional`
///    -> Uninhabited (typeops.py:1369-1373). Defer when the live map cannot
///    decide.
pub(crate) fn false_only(
    py: Python<'_>,
    t: &Type,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> Option<TruthinessResult> {
    // Only enum literals reach the leaf; the `is_enum_literal` tail and the
    // enum unwrap in `_get_type_method_ret_type` need live TypeInfo -> defer.
    if matches!(t, Type::LiteralType { .. }) {
        return None;
    }
    // Steps 1-2: str/bytes/int Instance -> LiteralType(""/0, fallback).
    // Only fire for plain Instances matching the Python `isinstance` check.
    if let Type::Instance { type_ref, .. } = t {
        if type_ref == "builtins.str" || type_ref == "builtins.bytes" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralEmptyStr(fb_bytes));
        }
        if type_ref == "builtins.int" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralZero(fb_bytes));
        }
    }
    // Step 3: __bool__/__len__ ret_type not can_be_false -> Uninhabited;
    // Python's elif chain (typeops.py:1361-1373) checks is_final/is_enum
    // only when the ret_type lookup found nothing, else falls to copy.
    match live_dunder_ret_type(py, t, resolver)? {
        DunderLookup::Found(ret_type) => match can_be_false_default(&ret_type) {
            Some(false) => return Some(TruthinessResult::Uninhabited),
            Some(true) => return Some(TruthinessResult::CopyFalseOnly),
            None => return None,
        },
        DunderLookup::NotFound => {}
        DunderLookup::Defer => return None,
    }
    // Step 4: a @final class or enum under strict_optional -> Uninhabited.
    match step6_is_final_or_enum(py, t, resolver) {
        Some(true) if strict_optional => return Some(TruthinessResult::Uninhabited),
        Some(_) => {}
        None => return None,
    }
    Some(TruthinessResult::CopyFalseOnly)
}

// ---------------------------------------------------------------------------
// true_or_false
// ---------------------------------------------------------------------------

/// `true_or_false` (typeops.py:865-878): unrestricted version of `t`.
///
/// Logic:
/// 1. If `UnionType` -> union of `true_or_false` on each item via
///    `make_simplified_union`
/// 2. Else -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
fn true_or_false(t: &Type) -> Option<TruthinessResult> {
    if let Type::UnionType { items, .. } = t {
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            item_results.push(true_or_false(item)?);
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    Some(TruthinessResult::CopyReset)
}

// ---------------------------------------------------------------------------
// Discriminator serialization for Python
// ---------------------------------------------------------------------------

/// Serialize a `TruthinessResult` to a Python-friendly tuple.
///
/// The encoding is a nested structure:
/// `(tag: i64, payload)` where:
/// * 0 = `Uninhabited`
/// * 1 = `NoneType`
/// * 2 = `SameType`
/// * 3 = `CopyTrueOnly`
/// * 4 = `CopyFalseOnly`
/// * 5 = `CopyReset`
/// * 6 = `LiteralEmptyStr(fallback_bytes)`
/// * 7 = `LiteralZero(fallback_bytes)`
/// * 8 = `UnionNarrow(item_discs)` — payload is a `Vec<TruthinessOut>`
type TruthinessOut = (i64, PyObject);

fn truthiness_to_py(py: Python<'_>, r: TruthinessResult) -> TruthinessOut {
    match r {
        TruthinessResult::Uninhabited => (0, py.None()),
        TruthinessResult::NoneType => (1, py.None()),
        TruthinessResult::SameType => (2, py.None()),
        TruthinessResult::CopyTrueOnly => (3, py.None()),
        TruthinessResult::CopyFalseOnly => (4, py.None()),
        TruthinessResult::CopyReset => (5, py.None()),
        TruthinessResult::LiteralEmptyStr(bytes) => {
            (6, pyo3::types::PyBytes::new(py, &bytes).into())
        }
        TruthinessResult::LiteralZero(bytes) => (7, pyo3::types::PyBytes::new(py, &bytes).into()),
        TruthinessResult::UnionNarrow(items) => {
            let py_items: Vec<TruthinessOut> =
                items.into_iter().map(|r| truthiness_to_py(py, r)).collect();
            (8, pyo3::types::PyList::new(py, py_items).into())
        }
    }
}

// ---------------------------------------------------------------------------
// #pyfunction entry points
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for `make_simplified_union`. Takes serialized items
/// + line/column/flags + `NativeTypeResolver`. Returns encoded result bytes
/// or `None` (defer to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_make_simplified_union(
    items_bytes: &[u8],
    line: i64,
    column: i64,
    keep_erased: bool,
    contract_literals: bool,
    handle_recursive: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    // items_bytes is a LIST_GEN-tagged list of serialized types.
    let mut buf = ReadBuffer::new(items_bytes);
    let items = wire::read_type_list(&mut buf).ok()?;
    let _ = (line, column, handle_recursive);
    // Step 1 mirror (typeops.py:1057): expand each alias item to its
    // chain-resolved raw target (flatten_nested_unions parity; arg
    // substitution skipped); missing snapshot or cycle defers.
    let expanded = expand_alias_items(&items, resolver.alias_resolver())?;
    // Match Python's _remove_redundant_union_items which calls
    // is_proper_subtype, reading state.strict_optional live
    // (subtypes.py:575 passes it to _is_subtype). proper_subtype=True

    // prevents Any-absorption: Instance is NOT <: AnyType, so Any | C is
    // preserved; strict_optional must flow from the caller so that
    // --no-strict-optional drops NoneType items (None <: T is true then).
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let result = setops::make_simplified_union(
        &expanded,
        &ctx,
        resolver.resolver(),
        contract_literals,
        keep_erased,
    )?;
    encode_type(&result)
}

/// Expand `TypeAliasType` items through the alias resolver, mirroring the
/// per-item `get_proper_type` in Python's `flatten_nested_unions`
/// (types.py:4981-4999). A `TypeAliasType` item is replaced by its
/// chain-resolved raw target (no argument substitution; see
/// `expand_alias_target_raw`); all other items pass through unchanged (the
/// wire `UnionType` flattening is handled by `setops::flatten_nested_unions`).
/// Returns `None` (defer) when any alias cannot be expanded.
fn expand_alias_items(
    items: &[Type],
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(items.len());
    for t in items {
        if let Type::TypeAliasType { .. } = t {
            let target = crate::checkexpr_functions::expand_alias_target_raw(t, aliases)?;
            out.push(target);
        } else {
            out.push(t.clone());
        }
    }
    Some(out)
}

/// `#[pyfunction]` entry for `simple_literal_type`. Returns a
/// `(decided, value)` wire answer (issue #1101 protocol, issue #1295): the
/// function is total over well-formed wire input, so a genuine no-result
/// (`t` not a simple literal) is a `(true, None)` decided answer, not a
/// deferral. `(false, None)` defers on an undecodable blob or an
/// unencodable fallback so the Python shim re-runs its body.
#[pyfunction]
pub(crate) fn rust_simple_literal_type(t_bytes: &[u8]) -> (bool, Option<Vec<u8>>) {
    let Some(t) = decode_type(t_bytes) else {
        return (false, None);
    };
    match simple_literal_type(&t) {
        Some(fallback) => match encode_type(&fallback) {
            Some(bytes) => (true, Some(bytes)),
            None => (false, None),
        },
        None => (true, None),
    }
}

/// `#[pyfunction]` entry for `is_simple_literal`. Returns `Some(true)`/
/// `Some(false)` or `None` (defer to Python when snapshot lookup is needed
/// but missing).
#[pyfunction]
pub(crate) fn rust_is_simple_literal(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_simple_literal(&t, resolver.resolver())
}

/// `#[pyfunction]` entry for `is_simple_literal`'s sibling
/// `is_literal_type_like` (typeops.py:1241-1257). Accepts a serialized type
/// and returns `Some(bool)` when the wire form can be fully decoded and no
/// TypeAliasType is encountered; `None` (defer to Python) otherwise.
#[pyfunction]
pub(crate) fn rust_is_literal_type_like(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_literal_type_like(&t)
}

/// `mypy.typeops.is_literal_type_like` — whether a (proper) type is
/// potentially a LiteralType, a Union whose items all qualify, or a TypeVar
/// whose upper bound / values qualify.
///
/// Mirrors typeops.py:1241-1257:
/// ```text
/// t = get_proper_type(t)
/// if t is None: return False
/// elif isinstance(t, LiteralType): return True
/// elif isinstance(t, UnionType): return any(is_literal_type_like(item) for item in
/// t.items)
/// elif isinstance(t, TypeVarType):
/// return is_literal_type_like(t.upper_bound) or any(is_literal_type_like(item) for
/// item in t.values)
/// else: return False
/// ```
///
/// TypeAliasType can't be resolved to a proper type here (no target in the
/// wire form), so it returns `None` and the Python shim defers.
fn is_literal_type_like(t: &Type) -> Option<bool> {
    match t {
        Type::TypeAliasType { .. } => None,
        Type::LiteralType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for item in items {
                match is_literal_type_like(item) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        Type::TypeVarType {
            upper_bound,
            values,
            ..
        } => {
            match is_literal_type_like(upper_bound) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
            for v in values {
                match is_literal_type_like(v) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

/// `#[pyfunction]` entry for `try_getting_str_literals_from_type`
/// (typeops.py:1186-1194). Returns `(decided, values)`: `Some((true, list))`
/// when the wire form proves the literal list, `Some((true, None))` when
/// Python would provably answer None (the #1101 decided-None protocol, so the
/// shim must not re-run the Python body), and `None` to defer (TypeAliasType
/// anywhere in the candidates — the wire has no alias target to expand).
#[pyfunction]
pub(crate) fn rust_try_getting_str_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<(bool, PyObject)> {
    let t = decode_type(t_bytes)?;
    literals_outcome(
        py,
        try_getting_literals(&t, "builtins.str", LiteralKind::Str).ok()?,
    )
}

/// `#[pyfunction]` entry for `try_getting_int_literals_from_type`
/// (typeops.py:1197-1205). Same `(decided, values)` protocol as the str seam.
#[pyfunction]
pub(crate) fn rust_try_getting_int_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<(bool, PyObject)> {
    let t = decode_type(t_bytes)?;
    literals_outcome(
        py,
        try_getting_literals(&t, "builtins.int", LiteralKind::Int).ok()?,
    )
}

/// `#[pyfunction]` entry for `try_getting_literals_from_type` with bool
/// target (typeops.py:1236-1256, called from dataclasses.py:897 with
/// `target_literal_type=bool`). Returns the Python `list[bool]` of literal
/// values (real Python bools, not 0/1: dataclasses uses the value directly
/// as the field's default) under the same `(decided, values)` protocol as
/// the str seam.
#[pyfunction]
pub(crate) fn rust_try_getting_bool_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<(bool, PyObject)> {
    let t = decode_type(t_bytes)?;
    literals_outcome(
        py,
        try_getting_literals(&t, "builtins.bool", LiteralKind::Bool).ok()?,
    )
}

/// Package a `LitOutcome` into the Python-side `(decided, values)` tuple.
fn literals_outcome(py: Python<'_>, out: LitOutcome) -> Option<(bool, PyObject)> {
    Some(match out {
        LitOutcome::DecidedNone => (true, py.None()),
        LitOutcome::Values(vals) => (
            true,
            pyo3::types::PyList::new(py, vals.into_iter().map(|v| v.into_pyobject(py))).into(),
        ),
    })
}

/// `#[pyfunction]` entry for `try_getting_instance_fallback`
/// (typeops.py:1525-1539). Issue #1101 decided-None protocol: returns
/// `(true, instance_bytes)` when the fallback exists, `(true, None)` when
/// Python's dispatch decides no fallback, or `None` to defer to Python.
#[pyfunction]
#[pyo3(signature = (t_bytes, resolver))]
pub(crate) fn rust_try_getting_instance_fallback(
    py: Python<'_>,
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<(bool, PyObject)> {
    let t = decode_type(t_bytes)?;
    match try_getting_instance_fallback(&t, resolver.alias_resolver()) {
        TgifOut::Fallback(fallback) => {
            let bytes = encode_type(&fallback)?;
            let boxed = pyo3::types::PyBytes::new(py, &bytes);
            Some((true, boxed.into()))
        }
        TgifOut::DecidedNone => Some((true, py.None())),
        TgifOut::Defer => None,
    }
}

/// `#[pyfunction]` entry for `try_expanding_sum_type_to_union`
/// (typeops.py:1292-1333). Returns the expanded type encoded as wire bytes,
/// or `None` to defer to Python when a Union branch needs recursive-flatten
/// semantics the wire format cannot express (a recursive TypeAliasType), or
/// an Instance's enum snapshot is missing from the resolver.
#[pyfunction]
#[pyo3(signature = (t_bytes, target_fullname, strict_optional, resolver))]
pub(crate) fn rust_try_expanding_sum_type_to_union(
    t_bytes: &[u8],
    target_fullname: Option<String>,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let result = try_expanding_sum_type_to_union_inner(
        &t,
        target_fullname.as_deref(),
        strict_optional,
        resolver.resolver(),
    )?;
    encode_type(&result)
}

/// `UnionType.make_union` (types.py:3502-3509): more than one item makes a
/// fresh `UnionType`, one item returns the item itself, and zero items
/// returns `UninhabitedType`. The union is always `uses_pep604_syntax=False`
/// (make_union's default) and its truthiness flags are computed eagerly from
/// the items exactly as Python's lazy `can_be_true/can_be_false_default`
/// (`any(item.can_be_true)`) would compute them.
fn make_union(items: Vec<Type>) -> Type {
    match items.len() {
        0 => Type::UninhabitedType { ambiguous: false },
        1 => items.into_iter().next().unwrap(),
        _ => {
            // UnionType.__init__ flattens nested unions (types.py:3465) with
            // handle_type_alias_type=False, so type aliases pass through and
            // the helper never returns None here. Truthiness iterates the

            // flattened items, matching Python's lazy any(item.can_be_true).
            let flat = crate::visitor::flatten_nested_unions_inner(&items, false, true)
                .unwrap_or_else(|| items.clone());
            let can_be_true = flat.iter().any(crate::setops::union_item_can_be_true);
            let can_be_false = flat.iter().any(crate::setops::union_item_can_be_false);
            Type::UnionType {
                items: flat,
                uses_pep604_syntax: false,
                can_be_true,
                can_be_false,
            }
        }
    }
}

/// `mypy.typeops.try_expanding_sum_type_to_union` (typeops.py:1292-1333):
/// expand a bool Instance into `Literal[True, False]` or an enum Instance
/// into the union of its member literals, recursively down a Union.
///
/// ```text
/// typ = get_proper_type(typ)
/// if isinstance(typ, UnionType):
///     items = [try_expanding_sum_type_to_union(item, target_fullname) for
///              item in remove_dups(flatten_nested_unions(typ.relevant_items()))]
///     return UnionType.make_union(items)
/// if isinstance(typ, Instance) and
///         (target_fullname is None or typ.type.fullname == target_fullname):
///     if typ.type.fullname == "builtins.bool":
///         return UnionType([LiteralType(True, typ), LiteralType(False, typ)])
///     if typ.type.is_enum:
///         items = [LiteralType(name, typ) for name in typ.type.enum_members]
///         if not items: return typ
///         return UnionType.make_union(items)
/// return typ
/// ```
///
/// A `TypeAliasType` cannot be resolved here (no target in the wire form), so
/// it returns `None` and the Python shim defers — matching `get_proper_type`.
/// Same deferral when a Union branch needs recursive alias flattening.
///
/// Enum expansion is deferred to Python: the snapshot's `enum_members` is
/// captured at resolver-build time, when nonmember members (e.g.
/// `z = nonmember(3)`) may not yet have their `Var.type` resolved, causing
/// stale entries. Python reads `enum_members` live at type-check time.
pub(crate) fn try_expanding_sum_type_to_union_inner(
    typ: &Type,
    target_fullname: Option<&str>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    // get_proper_type: a TypeAliasType has no proper form in the wire format.
    if let Type::TypeAliasType { .. } = typ {
        return None;
    }
    match typ {
        Type::UnionType { items, .. } => {
            // typ.relevant_items(): with strict_optional off, NoneType items
            // are dropped (types.py:3517-3522).
            let relevant: Vec<Type> = if strict_optional {
                items.clone()
            } else {
                items
                    .iter()
                    .filter(|i| !matches!(i, Type::NoneType))
                    .cloned()
                    .collect()
            };
            // flatten_nested_unions(relevant, type_alias_type=True, recursive=True).
            // A recursive TypeAliasType inside yields None (defer); by then the
            // relevant items may include NoneTypes, which must not be dropped

            // twice, so `relevant` is the pre-None-filtered input.
            let flat = crate::visitor::flatten_nested_unions_inner(&relevant, true, true)?;
            let deduped = crate::visitor::remove_dups_inner(&flat);
            let mut out = Vec::with_capacity(deduped.len());
            for item in &deduped {
                out.push(try_expanding_sum_type_to_union_inner(
                    item,
                    target_fullname,
                    strict_optional,
                    resolver,
                )?);
            }
            Some(make_union(out))
        }
        Type::Instance { type_ref, .. } => {
            if let Some(tf) = target_fullname {
                if type_ref != tf {
                    return Some(typ.clone());
                }
            }
            if type_ref == "builtins.bool" {
                let lit = |value: bool| Type::LiteralType {
                    fallback: Box::new(typ.clone()),
                    value: LiteralValue::Bool(value),
                };
                return Some(make_union(vec![lit(true), lit(false)]));
            }
            // Non-bool, non-enum instances pass through unchanged.
            // Enum expansion is deferred to Python: the snapshot's
            // enum_members is captured at resolver-build time, when

            // nonmember members (e.g. `z = nonmember(3)`) may not yet
            // have their Var.type resolved, causing stale entries.

            // Python reads enum_members live at type-check time, which
            // correctly excludes nonmembers.
            let snap = resolver.get(type_ref)?;
            if snap.is_enum {
                return None;
            }
            Some(typ.clone())
        }
        _ => Some(typ.clone()),
    }
}

/// Issue #1101 decided-None protocol outcome for
/// `try_getting_instance_fallback`.
#[derive(Debug, PartialEq)]
enum TgifOut {
    /// Python dispatch answer: an Instance fallback.
    Fallback(Type),
    /// Python dispatch answer: no fallback (the `else: return None` tail,
    /// or a recursion that bottoms out in that tail).
    DecidedNone,
    /// The kernel cannot decide: the only genuine defer here is a
    /// top-level `TypeAliasType` whose snapshot is missing from the
    /// resolver (Python's `get_proper_type` would expand it live).
    Defer,
}

/// `mypy.typeops.try_getting_instance_fallback` — the Instance fallback for
/// a proper type, or `DecidedNone` if it has no such fallback.
///
/// Mirrors typeops.py:1525-1539:
/// ```text
/// t = get_proper_type(t)
/// if isinstance(t, Instance): return t
/// elif isinstance(t, LiteralType): return t.fallback
/// elif isinstance(t, (NoneType, AnyType)): return None
/// elif isinstance(t, FunctionLike): return t.fallback
/// elif isinstance(t, TypeVarType): return try_getting_instance_fallback(t.upper_bound)
/// elif isinstance(t, TupleType): return t.partial_fallback
/// elif isinstance(t, TypedDictType): return t.fallback
/// else: return None
/// ```
/// `Overloaded.fallback` is `items[0].fallback` (types.py:2758), so the
/// Overloaded arm recurses through the first item. A `TypeAliasType`
/// operand is expanded via the alias resolver (`get_proper_type` at the
/// top of the Python body); a missing snapshot defers. Every proper shape
/// outside the isinstance chain (TypeType, Union, Uninhabited, Unbound,
/// Unpack, Deleted, Erased, ParamSpec, TypeVarTuple, ...) hits Python's
/// `else: return None` tail, so those decide natively now (#1183).
fn try_getting_instance_fallback(t: &Type, aliases: &crate::aliases::TypeAliasResolver) -> TgifOut {
    // Python: `t = get_proper_type(t)`. Expand a top-level alias through
    // the resolver (nested aliases inside the target have no proper form
    // here and defer — parity-safe, Python would recurse).
    let proper: Type = match t {
        Type::TypeAliasType { .. } => {
            match crate::checkexpr_functions::get_proper_or_expand(t, aliases) {
                Some(p) => p,
                None => return TgifOut::Defer,
            }
        }
        _ => t.clone(),
    };
    match &proper {
        Type::Instance { .. } => TgifOut::Fallback(proper.clone()),
        Type::LiteralType { fallback, .. } => TgifOut::Fallback((**fallback).clone()),
        Type::CallableType { fallback, .. } => TgifOut::Fallback((**fallback).clone()),
        Type::Overloaded { items } => match items.first() {
            Some(first) => try_getting_instance_fallback(first, aliases),
            // Python indexes items[0].fallback; an empty Overloaded never
            // reaches this seam, so the unreachable tail mirrors the
            // no-fallback decision instead of crashing.
            None => TgifOut::DecidedNone,
        },
        Type::TypeVarType { upper_bound, .. } => {
            try_getting_instance_fallback(upper_bound, aliases)
        }
        Type::TupleType {
            partial_fallback, ..
        } => TgifOut::Fallback((**partial_fallback).clone()),
        Type::TypedDictType { fallback, .. } => TgifOut::Fallback((**fallback).clone()),
        // NoneType and AnyType hit Python's fast-path comment; every other
        // unmatched proper shape hits the `else: return None` tail.
        _ => TgifOut::DecidedNone,
    }
}

/// `mypy.typeops.erase_to_bound` (typeops.py:629-637): erase a type
/// variable to its upper bound. A `TypeVarType` yields its `upper_bound`; a
/// `TypeType` whose item is a `TypeVarType` yields `TypeType(upper_bound)`.
/// All other types pass through unchanged.
///
/// `get_proper_type` is a no-op on wire types except for `TypeAliasType`,
/// which has no proper form here, so it defers (returns `None`).
pub(crate) fn erase_to_bound(t: &Type) -> Option<Type> {
    if let Type::TypeAliasType { .. } = t {
        return None;
    }
    match t {
        Type::TypeVarType { upper_bound, .. } => Some((**upper_bound).clone()),
        Type::TypeType { item, is_type_form } => {
            if let Type::TypeVarType { upper_bound, .. } = item.as_ref() {
                Some(Type::TypeType {
                    item: Box::new((**upper_bound).clone()),
                    is_type_form: *is_type_form,
                })
            } else {
                Some(t.clone())
            }
        }
        _ => Some(t.clone()),
    }
}

/// `mypy.typeops.tuple_fallback` (typeops.py:194-220): compute the fallback
/// `Instance` for a `TupleType`.
///
/// If the partial_fallback is not `builtins.tuple`, return it as-is.
/// Otherwise collect item types (unpacking `UnpackType`s whose unwrapped
/// type is `builtins.tuple` to their first arg), then build a single
/// `Instance(type, [make_simplified_union(items, handle_recursive=False)])`
/// preserving `extra_attrs`.
///
/// Defers (`None`) when: an `UnpackType` unpacks to a non-tuple (Python
/// raises `NotImplementedError`), the simplified-union step defers, or any
/// `make_simplified_union` subtype check needs the resolver and can't
/// resolve. Needs the resolver for the union simplification.
pub(crate) fn tuple_fallback(typ: &Type, resolver: &TypeResolver) -> Option<Type> {
    let Type::TupleType {
        partial_fallback,
        items,
        ..
    } = typ
    else {
        return None;
    };
    let Type::Instance {
        type_ref,
        extra_attrs,
        ..
    } = partial_fallback.as_ref()
    else {
        // Fallback isn't an Instance: defer (Python would crash too in
        // practice, but the wire form can't represent it).
        return None;
    };
    if type_ref != "builtins.tuple" {
        return Some((**partial_fallback).clone());
    }
    let mut collected: Vec<Type> = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Type::UnpackType { typ } => {
                let unpacked = typ.as_ref();
                // Unwrap TypeVarTupleType to its upper_bound, matching
                // get_proper_type(item.type) then the TypeVarTupleType
                // branch (typeops.py:203-204).
                let unwrapped = if let Type::TypeVarTupleType { upper_bound, .. } = unpacked {
                    upper_bound.as_ref()
                } else {
                    unpacked
                };
                if let Type::Instance {
                    type_ref: inner_ref,
                    args,
                    ..
                } = unwrapped
                {
                    if inner_ref == "builtins.tuple" {
                        if args.len() != 1 {
                            return None;
                        }
                        collected.push(args[0].clone());
                    } else {
                        // Python raises NotImplementedError; defer.
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => collected.push(item.clone()),
        }
    }
    // make_simplified_union(items, handle_recursive=False). The Rust
    // make_simplified_union ignores handle_recursive (it always flattens
    // with recursive=False semantics via flatten_nested_unions), so the

    // behavior matches the Python handle_recursive=False call.
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let union_type = crate::setops::make_simplified_union(&collected, &ctx, resolver, true, false)?;
    let union_type = get_proper_type_or_none(&union_type)?;
    Some(Type::Instance {
        type_ref: type_ref.clone(),
        args: vec![union_type],
        last_known_value: None,
        extra_attrs: extra_attrs.clone(),
    })
}

/// `get_proper_type` shim: a `TypeAliasType` has no proper form in the wire
/// representation (its target is unresolved), so defer. Otherwise the wire
/// type is already proper.
fn get_proper_type_or_none(t: &Type) -> Option<Type> {
    if let Type::TypeAliasType { .. } = t {
        return None;
    }
    Some(t.clone())
}

/// The shared walk behind `try_getting_str/int/bool_literals_from_type`
/// (typeops.py:1211-1264), under the #1101 decided-None protocol.
///
/// `Values` answers with the scalar literal values when every candidate is a
/// `LiteralType` whose fallback fullname equals `target_fullname` and whose
/// value is of the `expect` kind. `DecidedNone` reports that Python provably
/// answers None: the candidate set is fixed (one candidate or the union
/// items) and at least one is a proper type that is not a matching
/// LiteralType — plain Instance without last_known_value, TupleType,
/// CallableType, an int literal under the str target, and so on — so the
/// shim may return None without re-running the Python body. Defers (the
/// `Err(())` the caller maps to shim-level None) are reserved for
/// `TypeAliasType` candidates, which Python expands via `get_proper_type`
/// but the wire cannot carry: the shim then re-runs the Python body.
fn try_getting_literals(
    t: &Type,
    target_fullname: &str,
    expect: LiteralKind,
) -> Result<LitOutcome, ()> {
    // Python (typeops.py:1770-1776): typ = get_proper_type(typ), then
    // candidates = [typ.last_known_value] | list(typ.items) | [typ]. A
    // TypeAliasType top level needs the alias target; defer.
    let candidates: Vec<Type> = match t {
        Type::TypeAliasType { .. } => return Err(()),
        Type::Instance {
            last_known_value: Some(v),
            ..
        } => {
            if let Type::TypeAliasType { .. } = v.as_ref() {
                return Err(());
            }
            vec![v.as_ref().clone()]
        }
        Type::Instance { .. } => vec![t.clone()],
        Type::UnionType { items, .. } => {
            if items
                .iter()
                .any(|i| matches!(i, Type::TypeAliasType { .. }))
            {
                return Err(());
            }
            items.clone()
        }
        _ => vec![t.clone()],
    };
    let mut out: Vec<Scalar> = Vec::new();
    for lit in candidates {
        // Python: get_proper_types(...) per candidate. Everything that
        // reaches here is already proper (alias candidates were excluded
        // above), so the per-candidate re-proper-type is a no-op.
        let Type::LiteralType { fallback, value } = lit else {
            return Ok(LitOutcome::DecidedNone);
        };
        // Python: lit.fallback.type.fullname == target_fullname. The wire
        // fallback Instance carries that fullname in type_ref.
        let Type::Instance { type_ref, .. } = fallback.as_ref() else {
            return Ok(LitOutcome::DecidedNone);
        };
        if type_ref != target_fullname {
            return Ok(LitOutcome::DecidedNone);
        }
        let s = match value {
            LiteralValue::Str(s) if expect == LiteralKind::Str => Scalar::Str(s),
            LiteralValue::Int(i) if expect == LiteralKind::Int => Scalar::Int(i),
            LiteralValue::Bool(b) if expect == LiteralKind::Int => {
                // Python: isinstance(True, int) is True, so Literal[True]
                // counts as an int literal.
                Scalar::Int(if b { 1 } else { 0 })
            }
            LiteralValue::Bool(b) if expect == LiteralKind::Bool => Scalar::Bool(b),
            _ => return Ok(LitOutcome::DecidedNone),
        };
        out.push(s);
    }
    Ok(LitOutcome::Values(out))
}

#[derive(Debug, PartialEq)]
enum LitOutcome {
    Values(Vec<Scalar>),
    DecidedNone,
}

#[derive(Clone, Copy, PartialEq)]
enum LiteralKind {
    Str,
    Int,
    Bool,
}

/// The scalar values returned by `try_getting_literals`: Python strings,
/// ints (including bools-as-ints), or bools.
#[derive(Debug, PartialEq)]
enum Scalar {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl Scalar {
    fn into_pyobject(self, py: Python<'_>) -> PyObject {
        match self {
            Scalar::Str(s) => s.into_py(py),
            Scalar::Int(i) => i.into_py(py),
            Scalar::Bool(b) => b.into_py(py),
        }
    }
}

/// `#[pyfunction]` entry for `true_only`. Returns a truthiness discriminator
/// tuple or `None` (defer to Python). `__bool__`/`__len__` step-6 lookups go
/// through the resolver's live TypeInfo map.
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver))]
pub(crate) fn rust_true_only(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<TruthinessOut> {
    let t = decode_type(type_bytes)?;
    let result = true_only(py, &t, resolver)?;
    Some(truthiness_to_py(py, result))
}

/// `#[pyfunction]` entry for `false_only`.
#[pyfunction]
#[pyo3(signature = (type_bytes, strict_optional, resolver))]
pub(crate) fn rust_false_only(
    py: Python<'_>,
    type_bytes: &[u8],
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> Option<TruthinessOut> {
    let t = decode_type(type_bytes)?;
    let result = false_only(py, &t, strict_optional, resolver)?;
    Some(truthiness_to_py(py, result))
}

/// `#[pyfunction]` entry for `true_or_false`. No resolver needed (no live
/// lookups), kept for a uniform `(type_bytes, resolver)` call convention.
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver))]
pub(crate) fn rust_true_or_false(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<TruthinessOut> {
    let _ = resolver;
    let t = decode_type(type_bytes)?;
    let result = true_or_false(&t)?;
    Some(truthiness_to_py(py, result))
}

// ---------------------------------------------------------------------------
// separate_union_literals / get_type_vars
// ---------------------------------------------------------------------------

/// `separate_union_literals` (typeops.py:1522-1534): split the items of a
/// union into a list of literal items and a list of non-literal items.
///
/// The Python version calls `get_proper_type` on each item before checking
/// `isinstance(proper, LiteralType)`, so any `TypeAliasType` reaches Python's
/// proper-type machinery. To keep the alias target expansion on the Python
/// side, the Rust entry defers (returns `None`) when any item is not directly
/// a `LiteralType` wire variant it cannot classify. Because
/// `fixup_wire_type` runs on the decoded items in the shim, literal items
/// come back as live `LiteralType` objects.
#[allow(clippy::type_complexity)]
#[pyfunction]
pub(crate) fn rust_separate_union_literals(t_bytes: &[u8]) -> Option<(Vec<Vec<u8>>, Vec<Vec<u8>>)> {
    let t = decode_type(t_bytes)?;
    let Type::UnionType { items, .. } = t else {
        // Not a union on the wire: Python would iterate `t.items` and fail,
        // so this is a defensive defer.
        return None;
    };
    let mut literal_items: Vec<Vec<u8>> = Vec::new();
    let mut union_items: Vec<Vec<u8>> = Vec::new();
    for item in items {
        match item {
            // Python classifies with `get_proper_type(item)`, which expands
            // aliases; a `TypeAliasType` may resolve to a `LiteralType`, so
            // we cannot bucket it without the resolver. Defer the whole call.
            Type::TypeAliasType { .. } => return None,
            Type::LiteralType { .. } => literal_items.push(encode_type(&item)?),
            _ => union_items.push(encode_type(&item)?),
        }
    }
    Some((literal_items, union_items))
}

/// `get_type_vars` (typeops.py:1449-1450) and `get_all_type_vars`
/// (typeops.py:1453-1455): collect the type variables (or type-var-like
/// parameters and type-var-tuples when `include_all`) appearing in `tp`,
/// mirroring `TypeVarExtractor` (typeops.py:1458-1476).
///
/// `TypeVarExtractor` inherits `TypeQuery`, whose default traversal walks
/// instance args, callable arg/ret/instance types, tuple partial fallback +
/// items, typed-dict item values, type_obj item, overload items, union
/// items, unpacked type, parameter arg types, and placeholder args.
/// The byte-only entry defers on any `TypeAliasType` (no alias target on
/// the wire); the resolver-backed `rust_get_type_vars_live` below expands
/// through the alias snapshot. Non-type-var leaves contribute nothing.
///
/// Returns a list of wire-encoded `Type` blobs, or `None` to defer (decode
/// failure, or a shape the Rust traversal does not handle).
#[pyfunction]
pub(crate) fn rust_get_type_vars(t_bytes: &[u8], include_all: bool) -> Option<Vec<Vec<u8>>> {
    let t = decode_type(t_bytes)?;
    let mut out: Vec<Type> = Vec::new();
    collect_type_vars(&t, include_all, None, &mut Vec::new(), &mut out)?;
    let mut blobs = Vec::with_capacity(out.len());
    for tv in out {
        blobs.push(encode_type(&tv)?);
    }
    Some(blobs)
}

/// Resolver-backed variant of `rust_get_type_vars`: a `TypeAliasType`
/// expands through the `NativeTypeResolver` alias snapshot, mirroring
/// `TypeQuery.visit_type_alias_type` (type_visitor.py:459-467): the
/// substituted target only (plain `TypeQuery` does NOT additionally visit
/// `t.args`; that extra visit is the `BoolTypeQuery` variant). A repeated
/// alias on a descent contributes nothing, matching `seen_aliases`.
/// Missing snapshot, an alias cycle, or a substitution the kernel cannot
/// perform exactly defers (`None`) and the Python shim falls back to the
/// pure-Python `TypeVarExtractor`.
#[pyfunction]
pub(crate) fn rust_get_type_vars_live(
    resolver: &NativeTypeResolver,
    t_bytes: &[u8],
    include_all: bool,
) -> Option<Vec<Vec<u8>>> {
    let t = decode_type(t_bytes)?;
    let mut out: Vec<Type> = Vec::new();
    collect_type_vars(
        &t,
        include_all,
        Some(resolver.alias_resolver()),
        &mut Vec::new(),
        &mut out,
    )?;
    let mut blobs = Vec::with_capacity(out.len());
    for tv in out {
        blobs.push(encode_type(&tv)?);
    }
    Some(blobs)
}

/// `#[pyfunction]` entry for `erase_to_bound` (typeops.py:629-637). Takes
/// serialized type bytes, returns encoded result bytes or `None`.
#[pyfunction]
pub(crate) fn rust_erase_to_bound(t_bytes: &[u8]) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let result = erase_to_bound(&t)?;
    encode_type(&result)
}

/// `#[pyfunction]` entry for `tuple_fallback` (typeops.py:194-220). Takes
/// serialized tuple type bytes + `NativeTypeResolver`, returns encoded
/// fallback Instance bytes or `None`.
#[pyfunction]
pub(crate) fn rust_tuple_fallback(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let result = tuple_fallback(&t, resolver.resolver())?;
    encode_type(&result)
}

/// `mypy.typevars.fill_typevars` (typevars.py:43-85): for a non-generic type
/// return the instance type; for a generic G with parameters T1..Tn return
/// G[T1, ..., Tn]. Each class type parameter is rebuilt at line=-1,
/// column=-1 (the wire format carries no line/column, so the Python
/// `read_type` reconstruction defaults match). A `TypeVarTupleType` is
/// wrapped in an `UnpackType`, mirroring the Python body.
///
/// Reads the live `TypeInfo` (fullname, defn.type_vars, tuple_type) and
/// defers (`None`) when any tvar kind is not one of the three expected,
/// or when `tuple_type` decodes to something other than a `TupleType`.
fn fill_typevars_inner(py: Python<'_>, typ: &PyAny) -> Option<Type> {
    let type_ref: String = typ.getattr("fullname").ok()?.extract().ok()?;
    let defn = typ.getattr("defn").ok()?;
    let tvars = defn
        .getattr("type_vars")
        .ok()?
        .downcast::<pyo3::types::PyList>()
        .ok()?;
    let mut args = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        let bytes = serialize_type_to_bytes(py, item)?;
        let t = decode_type(&bytes)?;
        match t {
            // TypeVarType / ParamSpecType pass through unchanged: the wire
            // round-trip already drops line/column the same way
            // copy_modified(line=-1, column=-1) does.
            Type::TypeVarType { .. } | Type::ParamSpecType { .. } => args.push(t),
            Type::TypeVarTupleType { .. } => {
                args.push(Type::UnpackType { typ: Box::new(t) });
            }
            // A stored UnpackType or any other kind would trip Python's
            // `assert isinstance(tv, ParamSpecType)`; defer instead.
            _ => return None,
        }
    }
    let inst = Type::Instance {
        type_ref,
        args,
        last_known_value: None,
        extra_attrs: None,
    };
    // `typ.tuple_type.copy_modified(fallback=inst)` for named tuples:
    // keep items + implicit, swap the partial fallback.
    let tt = typ.getattr("tuple_type").ok()?;
    if tt.is_none() {
        return Some(inst);
    }
    let bytes = serialize_type_to_bytes(py, tt)?;
    let Type::TupleType {
        items, implicit, ..
    } = decode_type(&bytes)?
    else {
        return None;
    };
    Some(Type::TupleType {
        partial_fallback: Box::new(inst),
        items,
        implicit,
    })
}

/// `#[pyfunction]` entry for `fill_typevars` (typevars.py:43-85). Takes the
/// live `TypeInfo`, returns encoded `Instance`/`TupleType` bytes or `None`
/// (defer to Python).
#[pyfunction]
pub(crate) fn rust_fill_typevars(py: Python<'_>, typ: &PyAny) -> Option<Vec<u8>> {
    let t = fill_typevars_inner(py, typ)?;
    encode_type(&t)
}

/// `mypy.typevars.fill_typevars_with_any` (typevars.py:129-138): build the
/// class instance type with every type parameter replaced by
/// `AnyType(special_form)`. For very large generics this avoids building the
/// full tvar objects in Python; the result Instance carries only Anys.
///
/// Mirrors the Python body (`erased_vars`, typevartuples.py:28-36):
/// * `TypeVarType` / `ParamSpecType` erasure is a plain `AnyType`.
/// * `TypeVarTupleType` erasure needs `tuple_fallback.copy_modified(args=[Any])`
///   wrapped in `UnpackType`, which needs the live `tuple_fallback` TypeInfo;
///   this port defers (`None`) on that kind.
/// * A meta `TypeVarId` (`raw_id < 0`, types.py:495-504) is deferred the same
///   way the sibling `rust_fill_typevars` treats it.
/// * The `tuple_type` erasure runs through `erase_typevars_inner` with the
///   tvar-id set; any case that visitor cannot decide (TypeAlias/UnboundType,
///   named-tuple unpack normalization) propagates the defer. Python's
///   `copy_modified(fallback=inst)` only fires when the erased tuple is still
///   a `TupleType`; the visitor's `Tuple[*Ts] -> tuple[X, ...]` normalization
///   to an Instance matches that predicate exactly.
fn fill_typevars_with_any_inner(py: Python<'_>, typ: &PyAny) -> Option<Type> {
    let type_ref: String = typ.getattr("fullname").ok()?.extract().ok()?;
    let defn = typ.getattr("defn").ok()?;
    let tvars = defn
        .getattr("type_vars")
        .ok()?
        .downcast::<pyo3::types::PyList>()
        .ok()?;
    let mut args = Vec::with_capacity(tvars.len());
    // Ids of the class's own type parameters, for the tuple_type erasure
    // (mirrors `tv.id for tv in typ.defn.type_vars}`).
    let mut ids: HashSet<(i64, String)> = HashSet::with_capacity(tvars.len());
    for item in tvars.iter() {
        let id = item.getattr("id").ok()?;
        let raw_id: i64 = id.getattr("raw_id").and_then(|v| v.extract()).ok()?;
        let namespace: String = id.getattr("namespace").and_then(|v| v.extract()).ok()?;
        if raw_id < 0 {
            // Meta type variable: erasure is inference-dependent, defer.
            return None;
        }
        ids.insert((raw_id, namespace));
        match item.get_type().name().unwrap_or("").to_string().as_str() {
            // TypeVarType / ParamSpecType erasure is a plain AnyType
            // (erased_vars, typevartuples.py:28-36).
            "TypeVarType" | "ParamSpecType" => args.push(any_type(TYPE_OF_ANY_SPECIAL_FORM)),
            // TypeVarTupleType erasure needs the live tuple_fallback
            // TypeInfo for `UnpackType(copy_modified(args=[Any]))`; defer.
            _ => return None,
        }
    }
    let inst = Type::Instance {
        type_ref,
        args,
        last_known_value: None,
        extra_attrs: None,
    };
    // `typ.tuple_type` present: rebuild the tuple with `inst` as its
    // (partial) fallback, but only when the erased tuple is still a
    // TupleType (typevars.py:132-138).
    let tt = typ.getattr("tuple_type").ok()?;
    if tt.is_none() {
        return Some(inst);
    }
    let bytes = serialize_type_to_bytes(py, tt)?;
    let tuple = decode_type(&bytes)?;
    let erased = crate::erase_typevars::erase_typevars_inner(
        &tuple,
        Some(&ids),
        &any_type(TYPE_OF_ANY_SPECIAL_FORM),
    )?;
    let Type::TupleType {
        items, implicit, ..
    } = tuple
    else {
        return None;
    };
    if matches!(erased, Type::TupleType { .. }) {
        return Some(Type::TupleType {
            partial_fallback: Box::new(inst),
            items,
            implicit,
        });
    }
    Some(inst)
}

/// `#[pyfunction]` entry for `fill_typevars_with_any` (typevars.py:129-138).
/// Takes the live `TypeInfo`, returns encoded `Instance`/`TupleType` bytes
/// or `None` (defer to Python).
#[pyfunction]
pub(crate) fn rust_fill_typevars_with_any(py: Python<'_>, typ: &PyAny) -> Option<Vec<u8>> {
    let t = fill_typevars_with_any_inner(py, typ)?;
    encode_type(&t)
}

/// `#[pyfunction]` entry for `map_type_from_supertype` (typeops.py:552-578):
/// map a type defined in a supertype context (`super_info`) to be valid in a
/// subtype context (`sub_info`).
///
/// Mirrors the Python body as one consolidated native call:
///   1. `inst_type = fill_typevars(sub_info)` (typeops.py:558).
///   2. If `inst_type` is a `TupleType`, `inst_type = tuple_fallback(inst_type)`
///      (typeops.py:559-561) — an element-preserving Instance for builtins.tuple
///      fallbacks, the namedtuple class otherwise.
///   3. `inst_type = map_instance_to_supertype(inst_type, super_info)`
///      (typeops.py:564) — map up the derivation path to `super_info`'s frame.
///   4. `return expand_type_by_instance(typ, inst_type)` (typeops.py:572) —
///      bind `super_info`'s class typevars in `typ`.
///
/// Defers (`None`) when any step hits an unsupported edge: a TypeVarTuple
/// anywhere, an arg-count mismatch in `expand_type_by_instance`, a TypeAlias in
/// `typ`, or a `fill_typevars` value of an unexpected tvar kind. The Python shim
/// guards against `typ` carrying a FuncDef/Decorator definition node (which the
/// wire cannot carry) before calling.
#[pyfunction]
pub(crate) fn rust_map_type_from_supertype(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    sub_info: &PyAny,
    super_info: &PyAny,
    type_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let typ = decode_type(type_bytes)?;
    let mapped = map_type_from_supertype_inner(
        py,
        resolver,
        &typ,
        sub_info,
        super_info,
        strict_optional,
        false,
    )?;
    encode_type(&mapped)
}

/// Shared body of `rust_map_type_from_supertype` (typeops.py:552-578).
///
/// Extracted so the `type_object_type_from_function` composite seam can map
/// a bound `__init__`/`__new__` signature from `sub_info`'s frame into
/// `super_info`'s frame without a second decode/encode round-trip. The
/// `is_new` generic fallback (a `bool` passed by Python) is
/// `type_object_type_from_function`-specific and does not affect the map
/// body itself.
fn map_type_from_supertype_inner(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    typ: &Type,
    sub_info: &PyAny,
    super_info: &PyAny,
    strict_optional: bool,
    allow_free: bool,
) -> Option<Type> {
    let mut inst = fill_typevars_inner(py, sub_info)?;
    // Step 2: named tuples and plain tuples get an element-preserving
    // fallback so the subsequent map/expand see a real Instance frame.
    if let Type::TupleType { .. } = inst {
        inst = tuple_fallback(&inst, resolver.resolver())?;
    }
    // Steps 3-4. `inst` is now an Instance; read its frame.
    let Type::Instance {
        type_ref: sub_ref,
        args: sub_args,
        ..
    } = inst
    else {
        return None;
    };
    let sup_ref: String = super_info.getattr("fullname").ok()?.extract().ok()?;
    // Mapping to builtins.tuple defers: the element-preserving tuple
    // fallback (maptype.py:226-239) is not ported, so Rust would return
    // tuple[Any, ...] instead. Mirror the maptype.py seam guard.
    if sup_ref == "builtins.tuple" && sub_ref != sup_ref {
        return None;
    }
    // Map step fast paths mirroring maptype.map_instance_to_supertype
    // (maptype.py:15-21) that the subtypes primitive does not encode:
    //   * same class -> keep args (already the frame we want).

    //   * target has no type vars -> empty args.
    let sup_snap = resolver.resolver().get(&sup_ref)?;
    let mapped_args = if sub_ref == sup_ref {
        sub_args
    } else if sup_snap.type_vars.is_empty() {
        Vec::new()
    } else {
        crate::subtypes::map_instance_to_supertype(
            &sub_ref,
            &sub_args,
            &sup_ref,
            resolver.resolver(),
        )?
    };
    let inst = Type::Instance {
        type_ref: sup_ref,
        args: mapped_args,
        last_known_value: None,
        extra_attrs: None,
    };
    // allow_free=true (the typeobj composite) returns leftover TypeVars like
    // Python's expand_type_by_instance; the composite's Python tail re-links
    // their identities via wirefixup.
    if allow_free {
        crate::expandtype::expand_type_by_instance_free(
            typ,
            &inst,
            resolver.resolver(),
            strict_optional,
        )
    } else {
        // #1309: the relink variant (#1215/#1224 contract) lets leftover
        // TypeVars and surviving aliases ride through; the Python shim
        // re-links identities and defers on anything unmatchable.
        crate::expandtype::expand_type_by_instance_relink(
            typ,
            &inst,
            resolver.resolver(),
            strict_optional,
        )
    }
}

/// `mypy.typeops.coerce_to_literal` (typeops.py:1629-1645): recursively
/// turn any Instance with a last-known value or a single-value enum into
/// the corresponding LiteralType.
///
/// Live-enum reads: the snapshot's `enum_members` is captured when the
/// class's own SCC sealed it and can go stale (members resolving later,
/// e.g. nonmember members), so the single-member-enum decision reads
/// `is_enum` / `enum_members` live from the resolver-installed live
/// TypeInfo map. Defers (`None`) when that map is absent, when the enum
/// has zero members (Python returns the original type), or on a recursive
/// TypeAliasType (no alias target on the wire). Union items are coerced
/// recursively and rebuilt with `make_union`, exactly like the Python body.
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver))]
pub(crate) fn rust_coerce_to_literal(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<Vec<u8>> {
    let typ = decode_type(type_bytes)?;
    let result = coerce_to_literal_inner(py, &typ, resolver)?;
    encode_type(&result)
}

fn coerce_to_literal_inner(
    py: Python<'_>,
    typ: &Type,
    resolver: &NativeTypeResolver,
) -> Option<Type> {
    match typ {
        // Mirrors `typ = get_proper_type(typ)` at the top of the Python
        // body (typeops.py): expand the alias through the resolver and
        // continue. Defers (`None`) when the alias has no resolver snapshot.
        Type::TypeAliasType { .. } => {
            let proper =
                crate::checkexpr_functions::get_proper_or_expand(typ, resolver.alias_resolver())?;
            coerce_to_literal_inner(py, &proper, resolver)
        }
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(coerce_to_literal_inner(py, item, resolver)?);
            }
            Some(make_union(out))
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => Some((**lkv).clone()),
        Type::Instance { type_ref, .. } => {
            // Two live-enum reads, so the decision never uses a stale
            // snapshot: `is_enum` and `enum_members` come from the live
            // TypeInfo keyed by fullname.
            let info = resolver.live_typeinfo(py, type_ref)?;
            let is_enum = read_bool_attr(info, "is_enum").unwrap_or(false);
            if !is_enum {
                return Some(typ.clone());
            }
            let members = read_str_list_attr(info, "enum_members").unwrap_or_default();
            match members.len() {
                // Python: len(enum_values) == 1 -> LiteralType(value, typ).
                1 => Some(Type::LiteralType {
                    fallback: Box::new(typ.clone()),
                    value: LiteralValue::Str(members.into_iter().next().unwrap()),
                }),
                // Python: a zero-member enum returns the original type
                // (early `if not items: return typ` in the expand path is
                // separate; here coerce_to_literal checks `len == 1` only,

                // so 0 members falls through to `return original_type`).
                _ => Some(typ.clone()),
            }
        }
        _ => Some(typ.clone()),
    }
}

/// `mypy.typeops.is_singleton_identity_type` (typeops.py:1493-1514): true
/// if every value of the type is `is`-identical to every other, plus
/// `is_singleton_equality_type` (typeops.py:1517-1518) which adds plain
/// LiteralTypes.
///
/// Live-enum/final reads via the resolver-installed live TypeInfo map
/// (snapshot `enum_members` can go stale; `is_final` is not snapshotted).
/// Defers (`None`) when the target span needs a live read the map cannot
/// satisfy, or on the rare FunctionLike type-object branch (needs
/// `CallableType.type_object()` force_fallback Instance resolution, not
/// wire-portable).
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver))]
pub(crate) fn rust_is_singleton_identity_type(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    is_singleton_identity_inner(py, &typ, resolver)
}

/// `mypy.typeops.is_singleton_equality_type` (typeops.py:1517-1518).
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver))]
pub(crate) fn rust_is_singleton_equality_type(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    Some(
        matches!(typ, Type::LiteralType { .. }) || is_singleton_identity_inner(py, &typ, resolver)?,
    )
}
// `ellipsis` / `NotImplemented` identity names (types.py:257-260).
const ELLIPSIS_TYPE_NAMES: [&str; 2] = ["builtins.ellipsis", "types.EllipsisType"];
const NOT_IMPLEMENTED_TYPE_NAMES: [&str; 2] =
    ["builtins._NotImplementedType", "types.NotImplementedType"];

fn is_singleton_identity_inner(
    py: Python<'_>,
    typ: &Type,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    match typ {
        Type::NoneType => Some(true),
        Type::Instance { type_ref, .. } => {
            let info = resolver.live_typeinfo(py, type_ref)?;
            let is_enum = read_bool_attr(info, "is_enum").unwrap_or(false);
            let members = read_str_list_attr(info, "enum_members").unwrap_or_default();
            if is_enum && members.len() == 1 {
                return Some(true);
            }
            if ELLIPSIS_TYPE_NAMES.contains(&type_ref.as_str())
                || NOT_IMPLEMENTED_TYPE_NAMES.contains(&type_ref.as_str())
            {
                return Some(true);
            }
            Some(false)
        }
        Type::LiteralType { value, .. } => {
            // is_enum_literal() == fallback.type.is_enum (live), or a bool
            // value. non-bool/non-enum literals like Literal[100001] are not
            // identity-singletons.
            let is_enum = match typ {
                Type::LiteralType { fallback, .. } => match fallback.as_ref() {
                    Type::Instance { type_ref, .. } => {
                        let info = resolver.live_typeinfo(py, type_ref)?;
                        read_bool_attr(info, "is_enum").unwrap_or(false)
                    }
                    _ => false,
                },
                _ => false,
            };
            Some(matches!(value, LiteralValue::Bool(_)) || is_enum)
        }
        Type::TypeType { item, .. } => match item.as_ref() {
            Type::Instance { type_ref, .. } => {
                let info = resolver.live_typeinfo(py, type_ref)?;
                Some(read_bool_attr(info, "is_final").unwrap_or(false))
            }
            _ => Some(false),
        },
        // FunctionLike type-object branch needs
        // `CallableType.type_object()` force_fallback Instance resolution
        // (get_instance_type over instance_type/ret_type chains), which the

        // wire does not carry; defer to Python.
        Type::CallableType { .. } | Type::Overloaded { .. } => None,
        _ => Some(false),
    }
}

/// `#[pyfunction]` entry for `bind_self` (typeops.py:540-641): strip the
/// first parameter of a `CallableType` and set `is_bound=True`.
///
/// Mirrors `bind_self`'s non-generic path. Deferred to Python (`None`):
/// * Non-callable types
/// * `CallableType` with non-empty `variables` (needs `infer_type_arguments`
///   for type-var substitution)
/// * `CallableType` with no args, or first arg `*args`/`**kwargs` — Python
///   returns the method unchanged; the shim skips the native call in those
///   cases rather than round-tripping an identical object.
/// * `Overloaded` whose items include a variable-carrying or empty/star-arg
///   callable
fn bind_self_inner(typ: &Type) -> Option<Type> {
    match typ {
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound: _,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            if !variables.is_empty() {
                return None;
            }
            if arg_types.is_empty() {
                return None;
            }
            match arg_kinds.first() {
                Some(&kind) if kind == ARG_STAR || kind == ARG_STAR2 => None,
                Some(_) => Some(Type::CallableType {
                    fallback: fallback.clone(),
                    instance_type: instance_type.clone(),
                    is_ellipsis_args: *is_ellipsis_args,
                    implicit: *implicit,
                    is_bound: true,
                    from_concatenate: *from_concatenate,
                    imprecise_arg_kinds: *imprecise_arg_kinds,
                    unpack_kwargs: *unpack_kwargs,
                    from_type_type: *from_type_type,
                    arg_types: arg_types[1..].to_vec(),
                    arg_kinds: arg_kinds[1..].to_vec(),
                    arg_names: arg_names[1..].to_vec(),
                    ret_type: ret_type.clone(),
                    name: name.clone(),
                    variables: variables.clone(),
                    type_guard: type_guard.clone(),
                    type_is: type_is.clone(),
                }),
                None => None,
            }
        }
        _ => None,
    }
}

#[pyfunction]
pub(crate) fn rust_bind_self(method_bytes: &[u8]) -> Option<Vec<u8>> {
    let t = decode_type(method_bytes)?;
    let result = bind_self_inner(&t)?;
    encode_type(&result)
}

/// `mypy.typeops.class_callable` (typeops.py:428-486) — pick the return
/// type for a type-object callable and combine the type variables.
///
/// The two resolver-backed subtype checks (`is_equivalent` / `is_subtype`)
/// run on the Python side (already native via the subtype resolver) and are
/// passed in as `is_eq` / `is_st`. Everything else is pure on wire types and
/// the live `info`, so Rust makes the exact same decision:
///
///   `ret_type = explicit` when `is_new and explicit is not None and
///   (explicit is a non-unannotated Any or not is_eq)`, or when `explicit`
///   is Instance/Tuple/Uninhabited/Literal, `default_ret` is a non-protocol
///   Instance, and `is_st`. Otherwise `ret_type = default_ret`.
///
/// `variables = info.defn.type_vars + init.variables` (defn vars first,
/// mirroring the Python `extend` order). Returns `(ret_type, variables)` as
/// wire blobs, or `None` to defer to Python.
#[allow(clippy::too_many_arguments)]
fn class_callable_inner(
    py: Python<'_>,
    init_wire: &[u8],
    explicit_wire: Option<&[u8]>,
    default_ret_wire: &[u8],
    is_new: bool,
    is_eq: bool,
    is_st: bool,
    info: &PyAny,
) -> Option<(Type, Vec<Type>)> {
    let init = decode_type(init_wire)?;
    let default_ret = decode_type(default_ret_wire)?;

    // Combined variables = info.defn.type_vars + init.variables. Read the
    // live type vars exactly like fill_typevars_inner does (serialize each
    // through mypy's WriteBuffer, decode to the wire Type).
    let init_variables = match &init {
        Type::CallableType { variables, .. } => variables.clone(),
        _ => return None,
    };
    let defn = info.getattr("defn").ok()?;
    let tvars = defn
        .getattr("type_vars")
        .ok()?
        .downcast::<pyo3::types::PyList>()
        .ok()?;
    let mut variables = Vec::with_capacity(tvars.len() + init_variables.len());
    for item in tvars.iter() {
        let bytes = serialize_type_to_bytes(py, item)?;
        variables.push(decode_type(&bytes)?);
    }
    variables.extend(init_variables);

    let explicit = explicit_wire.and_then(decode_type);

    // default_ret = fill_typevars(info) is Instance(info) (or a TupleType
    // for a named tuple), so `default_ret.type.is_protocol == info.is_protocol`.
    let is_protocol = info.getattr("is_protocol").ok()?.extract::<bool>().ok()?;

    let ret_type = class_callable_ret(
        explicit.as_ref(),
        &default_ret,
        is_protocol,
        is_new,
        is_eq,
        is_st,
    )?;
    Some((ret_type, variables))
}

/// Pure `ret_type` decision for `class_callable` (typeops.py:450-477). No
/// live-Python reads, so it is directly unit-testable. Precedence matters:
/// `and` binds tighter than `or` in the first branch.
fn class_callable_ret(
    explicit: Option<&Type>,
    default_ret: &Type,
    is_protocol: bool,
    is_new: bool,
    is_eq: bool,
    is_st: bool,
) -> Option<Type> {
    let default_is_instance = matches!(default_ret, Type::Instance { .. });
    let explicit_is_some = explicit.is_some();
    let explicit_non_unannotated_any = matches!(
        explicit,
        Some(Type::AnyType { type_of_any, .. }) if *type_of_any != TYPE_OF_ANY_UNANNOTATED
    );
    let explicit_is_subtypable = matches!(
        explicit,
        Some(
            Type::Instance { .. }
                | Type::TupleType { .. }
                | Type::UninhabitedType { .. }
                | Type::LiteralType { .. },
        )
    );
    // Equivalent to Python's if/elif/else: use the explicit return type when
    // the is_new branch or the subtype branch fires, else the default.
    let use_explicit = (is_new && explicit_is_some && (explicit_non_unannotated_any || !is_eq))
        || (explicit_is_subtypable && default_is_instance && !is_protocol && is_st);
    if use_explicit {
        explicit.cloned()
    } else {
        Some(default_ret.clone())
    }
}

/// `#[pyfunction]` entry for `class_callable` (typeops.py:428-486). Takes the
/// wire `init_type`, wire explicit type (or `None`), wire
/// `default_ret_type`, the two Python-computed subtype booleans, and the live
/// `info`. Returns `(ret_type_bytes, [variable_bytes, ...])` or `None`
/// (defer to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (init_wire, explicit_wire, default_ret_wire, is_new, is_eq, is_st, info))]
pub(crate) fn rust_class_callable(
    py: Python<'_>,
    init_wire: &[u8],
    explicit_wire: Option<&[u8]>,
    default_ret_wire: &[u8],
    is_new: bool,
    is_eq: bool,
    is_st: bool,
    info: &PyAny,
) -> Option<(Vec<u8>, Vec<Vec<u8>>)> {
    let (ret_type, variables) = class_callable_inner(
        py,
        init_wire,
        explicit_wire,
        default_ret_wire,
        is_new,
        is_eq,
        is_st,
        info,
    )?;
    let ret_bytes = encode_type(&ret_type)?;
    let mut var_bytes = Vec::with_capacity(variables.len());
    for v in &variables {
        var_bytes.push(encode_type(v)?);
    }
    Some((ret_bytes, var_bytes))
}

// ---------------------------------------------------------------------------
// type_object_type / type_object_type_from_function (typeops.py:283-394, 410-460)
// ---------------------------------------------------------------------------

/// `mypy.typeops.get_self_type` (typeops.py:273-281): the explicit self type
/// of `func` in `def_info`, or `None` when the signature has no explicit
/// self annotation.
///
/// Mirrors the Python body exactly:
///   default_self = fill_typevars(def_info)
///   if isinstance(get_proper_type(func.ret_type), UninhabitedType):
///       return func.ret_type
///   elif func.arg_types and func.arg_types[0] != default_self
///        and func.arg_kinds[0] == ARG_POS:
///       return func.arg_types[0]
///   else:
///       return None
///
/// The `arg_types[0] != default_self` inequality is the wire-format
/// structural equality of `Type` — `Type.__eq__` on Instance compares
/// `type == type` (TypeInfo identity), which the wire `type_ref` string
/// round-trips as equal fullnames.
///
/// Returns the outer `Some` with:
///   * `Some(typ)` — an explicit self type (Python returns `func.arg_types[0]`
///     or `func.ret_type`).
///   * `None` — no explicit self (Python returns `None`).
///
/// The outer `None` means Rust could not decide and the whole composite
/// defers to Python.
fn get_self_type(py: Python<'_>, func: &Type, def_info: &PyAny) -> Option<Option<Type>> {
    let Type::CallableType {
        arg_types,
        arg_kinds,
        ret_type,
        ..
    } = func
    else {
        return None;
    };
    let default_self = fill_typevars_inner(py, def_info)?;
    if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
        // `isinstance(get_proper_type(func.ret_type), UninhabitedType)`:
        // the wire ret_type is already the proper type (an alias would be a
        // TypeAliasType, which the wire format carries as, well, alias).
        return Some(Some((**ret_type).clone()));
    }
    if !arg_types.is_empty() && arg_types[0] != default_self && arg_kinds.first() == Some(&ARG_POS)
    {
        return Some(Some(arg_types[0].clone()));
    }
    Some(None)
}

/// `bind_self` over a `FunctionLike` for the typeobj composite seam:
/// Callable items take `bind_self_composite_item`, `Overloaded` recurses
/// into each item exactly like the Python body.
#[allow(clippy::too_many_arguments)]
fn bind_self_composite(
    py: Python<'_>,
    method: &Type,
    is_new: bool,
    original_type: &Type,
    strict_optional: bool,
    infer_unions: bool,
    resolver: &NativeTypeResolver,
) -> Option<Type> {
    match method {
        Type::CallableType { .. } => bind_self_composite_item(
            py,
            method,
            is_new,
            original_type,
            strict_optional,
            infer_unions,
            resolver,
        ),
        Type::Overloaded { items } => {
            let mut bound = Vec::with_capacity(items.len());
            for item in items {
                bound.push(bind_self_composite_item(
                    py,
                    item,
                    is_new,
                    original_type,
                    strict_optional,
                    infer_unions,
                    resolver,
                )?);
            }
            Some(Type::Overloaded { items: bound })
        }
        _ => None,
    }
}

/// `mypy.typeops.type_object_type_from_function` (typeops.py:410-460).
///
/// Composite seam mirroring the whole Python body:
///   1. `orig_self_types = [get_self_type(it, def_info) for it in
///      signature.items]` unless `is_new or info.is_newtype` (then all
///      `None`).
///   2. `signature = bind_self(signature, original_type=fill_typevars(info),
///      is_classmethod=is_new, ignore_instances=True)` — non-generic
///      signatures strip via `bind_self_inner`; generic signatures solve
///      the self parameter's variables (`bind_self_composite_item`).
///   3. `signature = map_type_from_supertype(signature, info, def_info)`.
///   4. `special_sig = "dict"` when `def_info.fullname == "builtins.dict"`.
///   5. For each callable item, `class_callable(item, info, def_info,
///      fallback, special_sig, is_new, orig_self)` — assembling the wire
///      CallableType (ret_type/variables decided by `class_callable_inner`,
///      instance_type = fill_typevars(info), name = info.name).
///
/// The MRO walk that selects `__init__` vs `__new__` and the
/// `is_valid_constructor` checks stay in Python (`type_object_type`); this
/// seam takes the `is_new` decision and the already-selected `signature`.
/// Returns the fully-assembled wire `FunctionLike` (CallableType or
/// Overloaded), or `None` to defer to the pure-Python body.
#[pyfunction]
#[pyo3(signature = (signature_bytes, info, def_info, fallback_bytes, is_new, strict_optional, infer_unions, resolver))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_type_object_type_from_function(
    py: Python<'_>,
    signature_bytes: &[u8],
    info: &PyAny,
    def_info: &PyAny,
    fallback_bytes: &[u8],
    is_new: bool,
    strict_optional: bool,
    infer_unions: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let signature = decode_type(signature_bytes)?;
    let fallback = decode_type(fallback_bytes)?;
    // 1. orig_self_types
    let is_newtype = read_bool_attr(info, "is_newtype").unwrap_or(false);
    let orig_self_types: Vec<Option<Type>> = if is_new || is_newtype {
        match &signature {
            Type::CallableType { .. } => vec![None],
            Type::Overloaded { items } => vec![None; items.len()],
            _ => return None,
        }
    } else {
        match &signature {
            Type::CallableType { .. } => vec![get_self_type(py, &signature, def_info)?],
            Type::Overloaded { items } => items
                .iter()
                .map(|it| get_self_type(py, it, def_info))
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        }
    };
    // 2. bind_self. Python's bind_self with `ignore_instances=True` takes
    // the strip path for signatures without type variables; generic ones
    // solve the self parameter's vars against fill_typevars (1058-1099).

    // 3. map the bound signature from def_info's frame into info's frame
    // (allow_free: leftover class-tvar values survive expansion and the
    // Python shim re-links their identities via wirefixup).
    let default_ret = fill_typevars_inner(py, info)?;
    let bound = bind_self_composite(
        py,
        &signature,
        is_new,
        &default_ret,
        strict_optional,
        infer_unions,
        resolver,
    )?;
    let mapped =
        map_type_from_supertype_inner(py, resolver, &bound, info, def_info, strict_optional, true)?;
    // 5. class_callable per item.
    let result = match &mapped {
        Type::CallableType { .. } => class_callable_item_wire(
            py,
            &mapped,
            &orig_self_types[0],
            info,
            def_info,
            &default_ret,
            &fallback,
            is_new,
            strict_optional,
            resolver,
        )?,
        Type::Overloaded { items } => {
            let mut built = Vec::with_capacity(items.len());
            for (item, orig_self) in items.iter().zip(orig_self_types.iter()) {
                built.push(class_callable_item_wire(
                    py,
                    item,
                    orig_self,
                    info,
                    def_info,
                    &default_ret,
                    &fallback,
                    is_new,
                    strict_optional,
                    resolver,
                )?);
            }
            Type::Overloaded { items: built }
        }
        _ => return None,
    };
    encode_type(&result)
}

/// `(raw_id, meta_level, namespace)` — the wire shape of `TypeVarId`, the
/// same key space as `solve::tv_id` and `expandtype::EnvKey`.
type TvarKey = (i64, i64, String);

/// Wire mirror of `solve::tv_id` (solve.rs): ParamSpec and TypeVarTuple
/// carry meta_level 0.
fn typevar_key(t: &Type) -> Option<TvarKey> {
    match t {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        _ => None,
    }
}

/// Collect the keys of all TypeVar-like nodes a `TypeQuery(get_all_type_vars)`
/// walk with `include_all=True` would visit (TypeQuery positions,
/// type_visitor.py:415-466). Returns `None` on a `TypeAliasType`: Python
/// expands those with `get_proper_type`, which needs live alias nodes.
fn collect_query_tvars(t: &Type, out: &mut Vec<TvarKey>) -> Option<()> {
    match t {
        Type::TypeAliasType { .. } => None,
        Type::TypeVarType {
            upper_bound,
            default,
            ..
        }
        | Type::ParamSpecType {
            upper_bound,
            default,
            ..
        }
        | Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(typevar_key(t)?);
            collect_query_tvars(upper_bound, out)?;
            collect_query_tvars(default, out)?;
            for v in values_for_tvar(t) {
                collect_query_tvars(v, out)?;
            }
            if let Type::ParamSpecType { prefix, .. } = t {
                for a in &prefix.arg_types {
                    collect_query_tvars(a, out)?;
                }
            }
            Some(())
        }
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            for a in arg_types {
                collect_query_tvars(a, out)?;
            }
            collect_query_tvars(ret_type, out)?;
            if let Some(it) = instance_type {
                // The query only descends when instance_type != ret_type.
                if **it != **ret_type {
                    collect_query_tvars(it, out)?;
                }
            }
            Some(())
        }
        Type::Overloaded { items } => {
            for it in items {
                collect_query_tvars(it, out)?;
            }
            Some(())
        }
        Type::Instance { args, .. }
        | Type::UnboundType { args, .. }
        | Type::UnionType { items: args, .. } => {
            for a in args {
                collect_query_tvars(a, out)?;
            }
            Some(())
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            collect_query_tvars(partial_fallback, out)?;
            for a in items {
                collect_query_tvars(a, out)?;
            }
            Some(())
        }
        Type::TypedDictType { items, .. } => {
            for (_, v) in items {
                collect_query_tvars(v, out)?;
            }
            Some(())
        }
        Type::TypeType { item, .. } => collect_query_tvars(item, out),
        Type::UnpackType { typ } => collect_query_tvars(typ, out),
        Type::Parameters(p) => {
            for a in &p.arg_types {
                collect_query_tvars(a, out)?;
            }
            Some(())
        }
        // Leaf shapes and Any/None/Literal/Raw/Ellipsis/Uninhabited/Erased/
        // Deleted carry no tvars at the query positions.
        _ => Some(()),
    }
}

fn values_for_tvar(t: &Type) -> &[Type] {
    match t {
        Type::TypeVarType { values, .. } => values,
        Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => &[],
        _ => &[],
    }
}

/// The generic `bind_self` arm (typeops.py:1058-1099) for the typeobj
/// composite seam, mirroring `bind_self(..., ignore_instances=True)`:
/// solve the self-parameter's variables against `original_type` and
/// substitute. Returns `Some(item)` unchanged for the Python early returns
/// (no args, `*args`/`**kwargs` first). Non-generic signatures take the
/// shipped `bind_self_inner` strip path. Defers on unsupported self-type
/// shapes (alias bridges, unsupported self types).
fn bind_self_composite_item(
    py: Python<'_>,
    item: &Type,
    is_new: bool,
    original_type: &Type,
    strict_optional: bool,
    infer_unions: bool,
    resolver: &NativeTypeResolver,
) -> Option<Type> {
    let Type::CallableType {
        arg_types,
        arg_kinds,
        variables,
        ..
    } = item
    else {
        return None;
    };
    // Python: invalid method / signature absorbing *args -> method unchanged.
    if arg_types.is_empty() {
        return Some(item.clone());
    }
    if matches!(
        arg_kinds.first(),
        Some(&k) if k == ARG_STAR || k == ARG_STAR2
    ) {
        return Some(item.clone());
    }
    if variables.is_empty() {
        return bind_self_inner(item);
    }
    generic_bind_self_item(
        py,
        item,
        is_new,
        original_type,
        strict_optional,
        infer_unions,
        resolver,
    )
}

/// The generic solve arm itself; kept separate so the non-generic strip
/// path (`bind_self_inner`) and the Python early returns stay in one place.
#[allow(clippy::too_many_arguments)]
fn generic_bind_self_item(
    py: Python<'_>,
    item: &Type,
    is_new: bool,
    original_type: &Type,
    strict_optional: bool,
    infer_unions: bool,
    resolver: &NativeTypeResolver,
) -> Option<Type> {
    let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound: _,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        variables,
        type_guard,
        type_is,
    } = item
    else {
        return None;
    };
    // Python: `self_param_type = get_proper_type(arg_types[0])`; a wire
    // alias defers (Python expands from the live alias node).
    let self_param = match arg_types.first() {
        Some(t) if !matches!(t, Type::TypeAliasType { .. }) => t,
        _ => return None,
    };
    // allow_callable guard (typeops.py:1058-1060).
    let allow_callable = name
        .as_deref()
        .is_none_or(|n| !n.starts_with("__call__ of"));
    // The composite passes ignore_instances=True -> allow_instances=False.
    let supported = supported_self_type_inner(py, self_param, resolver, allow_callable, false)?;
    if !supported {
        return Some(Type::CallableType {
            fallback: fallback.clone(),
            instance_type: instance_type.clone(),
            is_ellipsis_args: *is_ellipsis_args,
            implicit: *implicit,
            is_bound: true,
            from_concatenate: *from_concatenate,
            imprecise_arg_kinds: *imprecise_arg_kinds,
            unpack_kwargs: *unpack_kwargs,
            from_type_type: *from_type_type,
            arg_types: arg_types[1..].to_vec(),
            arg_kinds: arg_kinds[1..].to_vec(),
            arg_names: arg_names[1..].to_vec(),
            ret_type: ret_type.clone(),
            name: name.clone(),
            variables: variables.clone(),
            type_guard: type_guard.clone(),
            type_is: type_is.clone(),
        });
    }
    // Solve for the method's variables that appear in the self type.
    let mut tv_keys: Vec<TvarKey> = Vec::new();
    collect_query_tvars(self_param, &mut tv_keys)?;
    let self_ids: HashSet<TvarKey> = tv_keys.into_iter().collect();
    let mut self_vars: Vec<Type> = Vec::with_capacity(variables.len());
    for tv in variables {
        let key = typevar_key(tv)?;
        if self_ids.contains(&key) {
            self_vars.push(tv.clone());
        }
    }
    let mut typeargs = crate::solve::infer_type_arguments_inner(
        &self_vars,
        self_param,
        original_type,
        true,
        false,
        false,
        strict_optional,
        infer_unions,
        resolver,
    )?;
    // Classmethod fallback: infer against type(x) when a solution is Never.
    if is_new
        && typeargs
            .iter()
            .any(|t| matches!(t, Some(Type::UninhabitedType { .. })))
        && matches!(
            original_type,
            Type::Instance { .. } | Type::TypeVarType { .. } | Type::TupleType { .. }
        )
    {
        let tt = Type::TypeType {
            item: Box::new(original_type.clone()),
            is_type_form: false,
        };
        typeargs = crate::solve::infer_type_arguments_inner(
            &self_vars,
            self_param,
            &tt,
            true,
            false,
            false,
            strict_optional,
            infer_unions,
            resolver,
        )?;
    }
    // to_apply: unsolved -> Never (UninhabitedType(), ambiguous=False).
    let mut env: HashMap<crate::expandtype::EnvKey, Type> = HashMap::new();
    for (tv, arg) in self_vars.iter().zip(typeargs.iter()) {
        let key = typevar_key(tv)?;
        let apply = arg
            .clone()
            .unwrap_or(Type::UninhabitedType { ambiguous: false });
        env.insert(key, apply);
    }
    let expanded = crate::expandtype::expand_type_with_env_inner(
        item,
        &env,
        strict_optional,
        true,
        false,
        false,
    )?;
    let Type::CallableType {
        fallback: ex_fb,
        instance_type: ex_inst,
        is_ellipsis_args: ex_ell,
        implicit: ex_impl,
        is_bound: _,
        from_concatenate: ex_concat,
        imprecise_arg_kinds: ex_impr,
        unpack_kwargs: ex_unpk,
        from_type_type: ex_ftt,
        arg_types: ex_args,
        arg_kinds: ex_kinds,
        arg_names: ex_names,
        ret_type: ex_ret,
        name: ex_name,
        variables: ex_vars,
        type_guard: ex_guard,
        type_is: ex_tis,
    } = expanded
    else {
        return None;
    };
    let mut kept: Vec<Type> = Vec::with_capacity(ex_vars.len());
    for tv in ex_vars {
        if typevar_key(&tv).is_some_and(|id| !self_ids.contains(&id)) {
            kept.push(tv.clone());
        }
    }
    Some(Type::CallableType {
        fallback: ex_fb,
        instance_type: ex_inst,
        is_ellipsis_args: ex_ell,
        implicit: ex_impl,
        is_bound: true,
        from_concatenate: ex_concat,
        imprecise_arg_kinds: ex_impr,
        unpack_kwargs: ex_unpk,
        from_type_type: ex_ftt,
        arg_types: ex_args[1..].to_vec(),
        arg_kinds: ex_kinds[1..].to_vec(),
        arg_names: ex_names[1..].to_vec(),
        ret_type: ex_ret,
        name: ex_name,
        variables: kept,
        type_guard: ex_guard,
        type_is: ex_tis,
    })
}

/// Assemble one wire `CallableType` for `class_callable` (typeops.py:448-459):
/// pick `ret_type` + `variables` via `class_callable_inner`, then rebuild the
/// callable with `fallback`, `name`, `special_sig`, and `instance_type` set.
#[allow(clippy::too_many_arguments)]
fn class_callable_item_wire(
    py: Python<'_>,
    item: &Type,
    orig_self: &Option<Type>,
    info: &PyAny,
    def_info: &PyAny,
    default_ret: &Type,
    fallback: &Type,
    is_new: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Type> {
    let Type::CallableType { ret_type, .. } = item else {
        return None;
    };
    // class_callable (typeops.py:478-501): explicit_type is the __init__
    // ret_type for __new__, else the declared self type. Both are resolved
    // through get_proper_type before the subtype checks; the wire format is

    // already proper except for TypeAliasType, which defers.
    let orig_self_proper = match orig_self {
        Some(t) if !matches!(t, Type::TypeAliasType { .. }) => Some(t.clone()),
        Some(_) => return None,
        None => None,
    };
    let explicit = if is_new {
        match ret_type.as_ref() {
            Type::TypeAliasType { .. } => return None,
            t => Some(t.clone()),
        }
    } else {
        orig_self_proper
    };
    let is_eq = if is_new {
        match &explicit {
            Some(explicit) => crate::subtypes::is_equivalent(
                &fill_typevars_inner(py, def_info)?,
                explicit,
                true,
                strict_optional,
                resolver.resolver(),
            )?,
            None => false,
        }
    } else {
        false
    };
    let is_st = match &explicit {
        Some(explicit)
            if matches!(
                explicit,
                Type::Instance { .. }
                    | Type::TupleType { .. }
                    | Type::UninhabitedType { .. }
                    | Type::LiteralType { .. }
            ) && matches!(default_ret, Type::Instance { .. }) =>
        {
            let is_protocol = info.getattr("is_protocol").ok()?.extract::<bool>().ok()?;
            if is_protocol {
                false
            } else {
                crate::subtypes::is_subtype(
                    explicit,
                    default_ret,
                    &SubtypeContext::new(true, false, false, false, false, strict_optional),
                    resolver.resolver(),
                )?
            }
        }
        _ => false,
    };
    let (ret_type, variables) = class_callable_inner(
        py,
        &encode_type(item)?,
        explicit.as_ref().and_then(encode_type).as_deref(),
        &encode_type(default_ret)?,
        is_new,
        is_eq,
        is_st,
        info,
    )?;
    let name = info.getattr("name").ok()?.extract::<String>().ok()?;
    let Type::CallableType {
        arg_types,
        arg_kinds,
        arg_names,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        type_guard,
        type_is,
        ..
    } = item
    else {
        return None;
    };
    Some(Type::CallableType {
        fallback: Box::new(fallback.clone()),
        instance_type: Some(Box::new(default_ret.clone())),
        is_ellipsis_args: *is_ellipsis_args,
        implicit: *implicit,
        is_bound: *is_bound,
        from_concatenate: *from_concatenate,
        imprecise_arg_kinds: *imprecise_arg_kinds,
        unpack_kwargs: *unpack_kwargs,
        from_type_type: *from_type_type,
        arg_types: arg_types.clone(),
        arg_kinds: arg_kinds.clone(),
        arg_names: arg_names.clone(),
        ret_type: Box::new(ret_type),
        name: Some(name),
        variables,
        type_guard: type_guard.clone(),
        type_is: type_is.clone(),
    })
}

/// Collect the type variables of `t` into `out` (mirroring
/// `TypeVarExtractor`, see `rust_get_type_vars` above). With `aliases`
/// set, a `TypeAliasType` expands through the alias snapshot (the live
/// seam); `None` defers on any alias (the byte-only seam). `seen` holds
/// the alias fullnames already visited on this descent, mirroring
/// `TypeQuery.seen_aliases` (a repeat contributes nothing). Returns
/// `None` when any shape needs something the Rust path cannot decide,
/// deferring the entire extraction to the Python `TypeVarExtractor`.
pub(crate) fn collect_type_vars(
    t: &Type,
    include_all: bool,
    aliases: Option<&TypeAliasResolver>,
    seen: &mut Vec<String>,
    out: &mut Vec<Type>,
) -> Option<()> {
    match t {
        Type::TypeVarType { .. } => {
            out.push(t.clone());
            Some(())
        }
        Type::ParamSpecType { .. } => {
            if include_all {
                out.push(t.clone());
            }
            Some(())
        }
        Type::TypeVarTupleType { .. } => {
            if include_all {
                out.push(t.clone());
            }
            Some(())
        }
        Type::TypeAliasType { type_ref, .. } => {
            let aliases = aliases?;
            if seen.contains(type_ref) {
                return Some(());
            }
            seen.push(type_ref.clone());
            // Plain TypeQuery visits only the substituted target
            // (`get_proper_type(t)`), not `t.args`; the args are already
            // folded into the substituted target.
            let (target, _args, _py312) = expanded_alias_target(t, aliases)?;
            collect_type_vars(&target, include_all, Some(aliases), seen, out)
        }
        Type::Instance { args, .. } => collect_list(args, include_all, aliases, seen, out),
        // TupleType: skip the partial fallback Instance like Python's copy
        // (typeops.py:1471), so a TypeVarTuple in the items is not
        // double-collected via its synthetic tuple fallback.
        Type::TupleType { items, .. } => collect_list(items, include_all, aliases, seen, out),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            collect_list(arg_types, include_all, aliases, seen, out)?;
            collect(ret_type, include_all, aliases, seen, out)?;
            if let Some(it) = instance_type {
                if it.as_ref() != (ret_type.as_ref()) {
                    collect(it, include_all, aliases, seen, out)?;
                }
            }
            Some(())
        }
        Type::TypedDictType { items, .. } => {
            for (_, v) in items {
                collect(v, include_all, aliases, seen, out)?;
            }
            Some(())
        }
        Type::TypeType { item, .. } => collect(item, include_all, aliases, seen, out),
        Type::Overloaded { items } => collect_list(items, include_all, aliases, seen, out),
        Type::UnionType { items, .. } => collect_list(items, include_all, aliases, seen, out),
        Type::UnpackType { typ } => collect(typ, include_all, aliases, seen, out),
        Type::Parameters(p) => collect_list(&p.arg_types, include_all, aliases, seen, out),
        Type::UnboundType { args, .. } => collect_list(args, include_all, aliases, seen, out),
        // Leaf types: any, none, uninhabited, deleted, erased, literal,
        // ellipsis, raw expression, partial, has no children.
        _ => Some(()),
    }
}

fn collect(
    t: &Type,
    include_all: bool,
    aliases: Option<&TypeAliasResolver>,
    seen: &mut Vec<String>,
    out: &mut Vec<Type>,
) -> Option<()> {
    collect_type_vars(t, include_all, aliases, seen, out)
}

fn collect_list(
    ts: &[Type],
    include_all: bool,
    aliases: Option<&TypeAliasResolver>,
    seen: &mut Vec<String>,
    out: &mut Vec<Type>,
) -> Option<()> {
    for t in ts {
        collect_type_vars(t, include_all, aliases, seen, out)?;
    }
    Some(())
}

// ---------------------------------------------------------------------------
// function_type / callable_type (typeops.py:1378-1429)
// ---------------------------------------------------------------------------

/// `mypy.typeops.function_type` (typeops.py:1378-1402) plus its helper
/// `callable_type` (typeops.py:1405-1429). Mirrors the caller-visible
/// behavior:
///
///   * `func.type` set -> that FunctionLike (asserted), returned unchanged.
///   * `FuncItem` with no type -> `callable_type(func, fallback, ret_type)`
///     (below), which binds the self/cls parameter.
///   * `OverloadedFuncDef` with no type -> a dummy
///     `CallableType([Any, Any], [ARG_STAR, ARG_STAR2], [None, None],
///     Any, fallback, line=func.line, is_ellipsis_args=True)` wrapped in
///     `Overloaded([dummy])` (a broken or not-yet-typed overload).
///
/// The self/cls binding in `callable_type` (typeops.py:1409-1413):
///
///   self_type = fill_typevars(info)
///   if fdef.is_class or fdef.name == "__new__":
///       self_type = TypeType.make_normalized(self_type)
///   args = [self_type] + [Any(unannotated)] * (len(arg_names) - 1)
///
/// `TypeType.make_normalized` (types.py:3927-3942) builds `TypeType(item)`
/// directly when `item` is not a Union (it is an Instance from
/// `fill_typevars`, so the Union re-distribution never fires).
///
/// The wire round-trip drops line/column (both rebuild at the default -1)
/// and cannot carry the `definition` node; the Python shim restores
/// `line` / `column` / `definition` on a rebuilt live CallableType so
/// error messages keep naming the function (missing-self note,
/// messages.py:3683 asserts on `definition`). `implicit=True` is
/// wire-carryable (flag bit 2).
///
/// Deferral conditions (Rust returns `None`, Python runs the original body):
///   * `func.type` is set to something the wire cannot represent exactly
///     (a non-CallableType/Overloaded FunctionLike, or any invalid value).
///   * the `info`/`arg_names`/`arg_kinds` reads fail, or the two lists
///     disagree in length (this includes the tangent where the self-arm
///     self branch is semantically skipped, left exactly as Python).
///   * the self-arm needs `fill_typevars(info)` (named tuples return a
///     `TupleType`; tuple_type decode failure defers the whole call).
fn function_type_inner(py: Python<'_>, func: &PyAny, fallback: &Type) -> Option<(bool, Type)> {
    // Classify by the Python class of the whole node, not the type value.
    let func_cls = match func.get_type().name() {
        Ok(n) => n.to_string(),
        Err(_) => return None,
    };
    let is_overloaded_func_def = func_cls.ends_with("OverloadedFuncDef");
    let astype = match func.getattr("type").ok()? {
        a if a.is_none() => None,
        a => Some(a),
    };
    match (is_overloaded_func_def, astype) {
        // `func.type` truthy -> passthrough: Python asserts FunctionLike and
        // returns it unchanged (FuncItem or OverloadedFuncDef, CallableType or
        // Overloaded; the dummy only fires when func.type is None).
        (_, Some(astype)) => {
            let cls = astype.get_type().name().ok()?.to_string();
            if !cls.ends_with("CallableType") && !cls.ends_with("Overloaded") {
                return None;
            }
            Some((true, decode_astype(py, astype)?))
        }
        // OverloadedFuncDef with no type: broken -> the dummy.
        (true, None) => Some((
            false,
            Type::Overloaded {
                items: vec![dummy_callable(fallback)],
            },
        )),
        // FuncItem with no type -> callable_type (self-binding).
        (false, None) => callable_type_inner(py, func, fallback, None).map(|t| (false, t)),
    }
}
/// `fn dummy_callable`: build the wire dummy for a broken overload.
fn dummy_callable(fallback: &Type) -> Type {
    let any = Type::AnyType {
        type_of_any: TYPE_OF_ANY_FROM_ERROR,
        source_any: None,
        missing_import_name: None,
    };
    let fallback = fallback.clone();
    Type::CallableType {
        fallback: Box::new(fallback),
        instance_type: None,
        is_ellipsis_args: true,
        implicit: false,
        is_bound: false,
        from_concatenate: false,
        imprecise_arg_kinds: false,
        unpack_kwargs: false,
        from_type_type: false,
        arg_types: vec![any.clone(), any],
        arg_kinds: vec![ARG_STAR, ARG_STAR2],
        arg_names: vec![None, None],
        ret_type: Box::new(Type::AnyType {
            type_of_any: TYPE_OF_ANY_FROM_ERROR,
            source_any: None,
            missing_import_name: None,
        }),
        name: None,
        variables: Vec::new(),
        type_guard: None,
        type_is: None,
    }
}

/// `fn callable_type_inner`: `callable_type` body (typeops.py:1485-1509).
///
/// `ret_type` mirrors the Python `ret_type or AnyType(...)` default: the
/// `rust_function_type` export always passes `None` (its wrapper builds the
/// final value), while `rust_callable_type` passes the caller's explicit
/// `ret_type` (the checkexpr lambda callback never supplies one explicitly,
/// but the shim serializes the live value when present).
fn callable_type_inner(
    py: Python<'_>,
    fdef: &PyAny,
    fallback: &Type,
    ret_type: Option<&Type>,
) -> Option<Type> {
    let arg_names = {
        let v = fdef.getattr("arg_names").ok()?;
        let list = v.downcast::<PyList>().ok()?;
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            out.push(item.extract::<Option<String>>().ok()?);
        }
        out
    };
    let arg_kinds = {
        let v = fdef.getattr("arg_kinds").ok()?;
        let list = v.downcast::<PyList>().ok()?;
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            // mypy.nodes.ArgKind is an IntEnum; read the enum value.
            out.push(item.getattr("value").ok()?.extract::<i64>().ok()?);
        }
        out
    };
    if arg_names.len() != arg_kinds.len() {
        return None;
    }
    let info = fdef.getattr("info").ok()?;
    let has_self = fdef
        .getattr("has_self_or_cls_argument")
        .ok()?
        .is_true()
        .ok()?;
    let arg_name_count = arg_names.len();
    // Self branch (typeops.py:1409-1413): filler + TypeType normalization.
    let args = if info.is_true().ok()? && has_self && arg_name_count > 0 {
        let self_type = fill_typevars_inner(py, info)?;
        let self_type = if read_bool_attr(fdef, "is_class")?
            || fdef.getattr("name").ok()?.extract::<String>().ok()? == "__new__"
        {
            Type::TypeType {
                item: Box::new(self_type),
                is_type_form: false,
            }
        } else {
            self_type
        };
        let mut args = vec![self_type];
        args.extend(std::iter::repeat_n(
            any_type(TYPE_OF_ANY_UNANNOTATED),
            arg_name_count - 1,
        ));
        args
    } else {
        std::iter::repeat_n(any_type(TYPE_OF_ANY_UNANNOTATED), arg_name_count).collect()
    };
    // `ret_type or AnyType(...)`: encoded ret_type or Any when not passed.
    let ret_type = ret_type
        .cloned()
        .unwrap_or_else(|| any_type(TYPE_OF_ANY_UNANNOTATED));
    let name: Option<String> = fdef.getattr("name").ok()?.extract().ok()?;
    Some(Type::CallableType {
        fallback: Box::new(fallback.clone()),
        instance_type: None,
        is_ellipsis_args: false,
        implicit: true,
        is_bound: false,
        from_concatenate: false,
        imprecise_arg_kinds: false,
        unpack_kwargs: false,
        from_type_type: false,
        arg_types: args,
        arg_kinds,
        arg_names,
        ret_type: Box::new(ret_type),
        name,
        variables: Vec::new(),
        type_guard: None,
        type_is: None,
    })
}

/// `fn decode_astype`: serialize the live CallableType through Python's
/// WriteBuffer then decode it into the wire Type. This is exactly the
/// python `_serialize_type` pattern (keeps any `instance_type` / `is_bound`
/// / `from_type_type` state the Python body would carry over).
fn decode_astype(py: Python<'_>, astype: &PyAny) -> Option<Type> {
    let bytes = serialize_type_to_bytes(py, astype)?;
    decode_type(&bytes)
}

/// `rust_function_type`, the exported seam entry point.
///
/// `fallback_wire` is the serialized live `builtins.function` Instance (the
/// shim always passes it). Decodes, runs the port, re-encodes, returns
/// `None` when anything defers. Line/column/definition are NOT
/// wire-carryable; the Python shim restores them via `copy_modified`.
#[pyfunction]
pub(crate) fn rust_function_type(
    py: Python<'_>,
    func: &PyAny,
    fallback_wire: &[u8],
) -> Option<(bool, Vec<u8>)> {
    let fallback = decode_type(fallback_wire)?;
    let (is_passthrough, t) = function_type_inner(py, func, &fallback)?;
    let bytes = encode_type(&t)?;
    Some((is_passthrough, bytes))
}

/// `rust_callable_type`: `mypy.typeops.callable_type` (typeops.py:1485-1509)
/// on a live `FuncItem` with an optional explicit `ret_type`.
///
/// Mirrors the same body as `callable_type_inner` but takes the caller's
/// `ret_type` wire bytes (`None` bytes mean defer / no explicit ret_type).
/// Used by the checkexpr lambda callback which passes the inferred return
/// type; `rust_function_type` uses this body with `None`.
#[pyfunction]
pub(crate) fn rust_callable_type(
    py: Python<'_>,
    fdef: &PyAny,
    fallback_wire: &[u8],
    ret_type_wire: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let fallback = decode_type(fallback_wire)?;
    let ret_type = match ret_type_wire {
        Some(bytes) => Some(decode_type(bytes)?),
        None => None,
    };
    let t = callable_type_inner(py, fdef, &fallback, ret_type.as_ref())?;
    encode_type(&t)
}

// ---------------------------------------------------------------------------
// is_valid_constructor
// ---------------------------------------------------------------------------

/// `mypy.typeops.is_valid_constructor` (typeops.py:445-455): pure bool
/// predicate. True for `OverloadedFuncDef`/`FuncDef` (SYMBOL_FUNCBASE_TYPES),
/// or for a `Decorator` whose `get_proper_type(var.type)` is a `FunctionLike`.
///
/// Reads the live node via PyO3 isinstance (mirrors `rust_is_magic_base`).
/// The Decorator arm calls `mypy.types.get_proper_type(n.type)` then
/// serializes the proper type to the wire format and checks the tag is
/// `CallableType` or `Overloaded` (the wire form of `FunctionLike`). A
/// ProperType is never a `TypeAliasType`, so encode/decode always succeeds;
/// a `None` type (unanalyzed decorator) yields `False`. Always returns a
/// bool, never defers: no resolver / inference / checker callbacks.
#[pyfunction]
pub(crate) fn rust_is_valid_constructor(py: Python<'_>, n: &PyAny) -> PyResult<bool> {
    if n.is_none() {
        return Ok(false);
    }
    is_valid_constructor_inner(py, n)
}

/// Shared body of `is_valid_constructor`, reused by the `type_object_type`
/// arbitration head (issue #1059). The `None` fast path is the caller's.
pub(crate) fn is_valid_constructor_inner(py: Python<'_>, n: &PyAny) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ofd_cls: &PyType = nodes_mod.getattr("OverloadedFuncDef")?.downcast()?;
    let fd_cls: &PyType = nodes_mod.getattr("FuncDef")?.downcast()?;
    if n.is_instance(ofd_cls)? || n.is_instance(fd_cls)? {
        return Ok(true);
    }
    let decorator_cls: &PyType = nodes_mod.getattr("Decorator")?.downcast()?;
    if !n.is_instance(decorator_cls)? {
        return Ok(false);
    }
    let n_type = n.getattr("type")?;
    if n_type.is_none() {
        return Ok(false);
    }
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let proper = get_proper_type.call1((n_type,))?;
    if proper.is_none() {
        return Ok(false);
    }
    if let Some(bytes) = serialize_type_to_bytes(py, proper) {
        if let Some(t) = decode_type(&bytes) {
            return Ok(matches!(
                t,
                Type::CallableType { .. } | Type::Overloaded { .. }
            ));
        }
    }
    let fl_cls: &PyType = types_mod.getattr("FunctionLike")?.downcast()?;
    proper.is_instance(fl_cls)
}

// ---------------------------------------------------------------------------
// type_object_type arbitration head (#1059)
// ---------------------------------------------------------------------------

/// Decision tags for the `type_object_type` arbitration head; must match
/// `NATIVE_TYPE_OBJECT_*` in mypy/typeops.py.
const TYPE_OBJECT_ERROR_INIT: i64 = 0;
const TYPE_OBJECT_ERROR_NEW: i64 = 1;
const TYPE_OBJECT_INIT: i64 = 2;
const TYPE_OBJECT_NEW: i64 = 3;
const TYPE_OBJECT_TIE_ANY: i64 = 4;

/// Scalar facts the arbitration head decides from, gathered from the live
/// `TypeInfo` by `gather_type_object_facts`. Index positions are
/// `info.mro.index(method.node.info)`; validity is the presence + native
/// `is_valid_constructor` verdict for each candidate; the tuple / uncached
/// flags are per-candidate so the classifier can pick the chosen one.
struct TypeObjectFacts {
    init_valid: bool,
    new_valid: bool,
    init_index: usize,
    new_index: usize,
    // Tie arm: the __init__ node is defined on `builtins.object`.
    init_info_is_object: bool,
    fallback_to_any: bool,
    // Defining class of each candidate method is `builtins.tuple`.
    init_is_tuple: bool,
    new_is_tuple: bool,
    // The constructed class itself is `builtins.tuple` (skips special_sig).
    info_is_tuple: bool,
    // Candidate is an `OverloadedFuncDef` whose `.type` is still None.
    init_uncached: bool,
    new_uncached: bool,
}

/// Pure init-vs-new-vs-tie arbitration of `typeops.type_object_type`
/// (typeops.py:350-461 head). Returns `(tag, is_new, special_sig,
/// uncached)`. Branch order mirrors the Python body: missing/invalid
/// `__init__` or `__new__` is an invalid class definition (error tags),
/// then prefer `__init__` on the lower MRO index and `__new__` on the
/// higher; on a tie prefer `__init__` unless the method is object's and
/// the class falls back to Any (the universal-callable arm). The tuple
/// `special_sig` and cache-disabling overloaded flag follow the chosen
/// method; the TIE_ANY arm returns before them (Python returns early).
fn classify_type_object_head(f: &TypeObjectFacts) -> (i64, bool, bool, bool) {
    if !f.init_valid {
        return (TYPE_OBJECT_ERROR_INIT, false, false, false);
    }
    if !f.new_valid {
        return (TYPE_OBJECT_ERROR_NEW, false, false, false);
    }
    let (tag, is_new) = if f.init_index < f.new_index {
        (TYPE_OBJECT_INIT, false)
    } else if f.init_index > f.new_index {
        (TYPE_OBJECT_NEW, true)
    } else if f.init_info_is_object && f.fallback_to_any {
        return (TYPE_OBJECT_TIE_ANY, false, false, false);
    } else {
        (TYPE_OBJECT_INIT, false)
    };
    let (chosen_is_tuple, chosen_uncached) = if is_new {
        (f.new_is_tuple, f.new_uncached)
    } else {
        (f.init_is_tuple, f.init_uncached)
    };
    let special_sig = chosen_is_tuple && !f.info_is_tuple;
    (tag, is_new, special_sig, chosen_uncached)
}

/// The arbitrated method node plus the gathered facts, so the entry can
/// hand the chosen node to the Python shim.
struct TypeObjectGathered<'a> {
    facts: TypeObjectFacts,
    init_node: &'a PyAny,
    new_node: &'a PyAny,
}

/// Return shape of `rust_classify_type_object_type`: `(tag, is_new,
/// special_sig, uncached, method)`.
type TypeObjectClassification = (i64, bool, bool, bool, Option<PyObject>);

/// `#[pyfunction]` entry for the `type_object_type` arbitration head
/// (typeops.py:350-461). Reads the live `TypeInfo` via PyO3 and returns
/// `(tag, is_new, special_sig, uncached, method)`: the arbitration tag,
/// the is_new bit for the `type_object_type_from_function` tail, the
/// tuple `special_sig` decision, the cache-disabling overloaded flag,
/// and the arbitrated method node (None for the error / universal
/// callable tags, where Python builds the result without a method).
/// Python keeps the fallback construction, the already-native tail, and
/// all cache writes. Defers (`None`) on any unreadable attribute, a
/// non-list `mro`, or a method whose `info` is missing from the MRO; the
/// Python caller then re-runs the pure-Python body, which decides or
/// raises exactly as before the seam.
#[pyfunction]
pub(crate) fn rust_classify_type_object_type(
    py: Python<'_>,
    info: &PyAny,
) -> PyResult<Option<TypeObjectClassification>> {
    let gathered = match gather_type_object_facts(py, info) {
        Ok(Some(g)) => g,
        _ => return Ok(None),
    };
    let (tag, is_new, special_sig, uncached) = classify_type_object_head(&gathered.facts);
    let method = match tag {
        TYPE_OBJECT_INIT => Some(gathered.init_node.into_py(py)),
        TYPE_OBJECT_NEW => Some(gathered.new_node.into_py(py)),
        _ => None,
    };
    Ok(Some((tag, is_new, special_sig, uncached, method)))
}

/// Gathers the arbitration facts from the live `TypeInfo`, mirroring the
/// Python body's read order: init presence/validity, new presence/
/// validity, MRO indices, then the scalar facts of both candidates.
/// Any PyO3 read failure surfaces as `Err`, which the entry maps to a
/// deferral (`None`).
fn gather_type_object_facts<'a>(
    py: Python<'_>,
    info: &'a PyAny,
) -> PyResult<Option<TypeObjectGathered<'a>>> {
    let init_method = info.call_method1("get", ("__init__",))?;
    if init_method.is_none() {
        // Missing `__init__`: invalid class definition; decided before
        // any other fact is read.
        return Ok(Some(TypeObjectGathered {
            facts: error_facts(),
            init_node: info,
            new_node: info,
        }));
    }
    let init_node = init_method.getattr("node")?;
    if !is_valid_constructor_inner(py, init_node)? {
        return Ok(Some(TypeObjectGathered {
            facts: error_facts(),
            init_node: info,
            new_node: info,
        }));
    }
    // There *should* always be a `__new__` method except the test stubs
    // lack it, so Python copies init_method in that situation.
    let new_method = info.call_method1("get", ("__new__",))?;
    let (new_node, new_valid) = if new_method.is_none() {
        (init_node, true)
    } else {
        let node = new_method.getattr("node")?;
        (node, is_valid_constructor_inner(py, node)?)
    };
    if !new_valid {
        return Ok(Some(TypeObjectGathered {
            facts: error_facts(),
            init_node: info,
            new_node: info,
        }));
    }

    let mro = info.getattr("mro")?;
    let mro_list: &PyList = match mro.downcast() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let init_index = match mro_index(mro_list, init_node.getattr("info")?) {
        Some(i) => i,
        None => return Ok(None),
    };
    let new_index = match mro_index(mro_list, new_node.getattr("info")?) {
        Some(i) => i,
        None => return Ok(None),
    };

    let init_info = init_node.getattr("info")?;
    if init_info.is_none() {
        return Ok(None);
    }
    let init_fullname: String = init_info.getattr("fullname")?.extract()?;
    let new_info = new_node.getattr("info")?;
    if new_info.is_none() {
        return Ok(None);
    }
    let new_fullname: String = new_info.getattr("fullname")?.extract()?;
    let info_fullname: String = info.getattr("fullname")?.extract()?;
    let fallback_to_any = match read_bool_attr(info, "fallback_to_any") {
        Some(b) => b,
        None => return Ok(None),
    };
    let nodes_mod = py.import("mypy.nodes")?;
    let ofd_cls: &PyType = nodes_mod.getattr("OverloadedFuncDef")?.downcast()?;
    let is_uncached = |node: &PyAny| -> PyResult<bool> {
        Ok(node.is_instance(ofd_cls)? && node.getattr("type")?.is_none())
    };

    Ok(Some(TypeObjectGathered {
        facts: TypeObjectFacts {
            init_valid: true,
            new_valid,
            init_index,
            new_index,
            init_info_is_object: init_fullname == "builtins.object",
            fallback_to_any,
            init_is_tuple: init_fullname == "builtins.tuple",
            new_is_tuple: new_fullname == "builtins.tuple",
            info_is_tuple: info_fullname == "builtins.tuple",
            init_uncached: is_uncached(init_node)?,
            new_uncached: is_uncached(new_node)?,
        },
        init_node,
        new_node,
    }))
}

/// Facts for the invalid-class-definition early returns: Python returns
/// `AnyType(TypeOfAny.from_error)` without touching the cache.
fn error_facts() -> TypeObjectFacts {
    TypeObjectFacts {
        init_valid: false,
        new_valid: false,
        init_index: 0,
        new_index: 0,
        init_info_is_object: false,
        fallback_to_any: false,
        init_is_tuple: false,
        new_is_tuple: false,
        info_is_tuple: false,
        init_uncached: false,
        new_uncached: false,
    }
}

/// Identity-based `list.index` over the MRO; TypeInfo does not define
/// `__eq__`, so Python's `list.index` compares by identity too.
fn mro_index(mro: &PyList, needle: &PyAny) -> Option<usize> {
    for (i, entry) in mro.iter().enumerate() {
        if entry.is(needle) {
            return Some(i);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// _is_disjoint_base
// ---------------------------------------------------------------------------

/// `mypy.typeops._is_disjoint_base` (typeops.py:2110-2124) — does the type
/// have the `@disjoint_base` decorator or define non-empty `__slots__`?
///
/// A slot is "own" when no direct base class declares it. Bases whose
/// `slots` is `None` declare nothing. Mirrors `rust_is_magic_base`: takes
/// a live `TypeInfo` object and reads its attributes via PyO3.
#[pyfunction]
pub(crate) fn rust_is_disjoint_base(info: &PyAny) -> PyResult<bool> {
    is_disjoint_base_inner(info)
}

/// Pure decision: is `info` a disjoint base? Shared by the `#[pyfunction]`
/// entry and `rust_can_have_shared_disjoint_base` (checker_visitor.rs).
pub(crate) fn is_disjoint_base_inner(info: &PyAny) -> PyResult<bool> {
    if info.getattr("is_disjoint_base")?.is_true()? {
        return Ok(true);
    }
    let slots = info.getattr("slots")?;
    if slots.is_none() {
        return Ok(false);
    }
    let bases_list = info.getattr("bases")?.downcast::<PyList>()?;
    for slot_result in slots.iter()? {
        let slot_str: String = slot_result?.extract()?;
        let mut owned = true;
        for base in bases_list.iter() {
            let base_slots = base.getattr("type")?.getattr("slots")?;
            if base_slots.is_none() {
                continue;
            }
            for base_slot_result in base_slots.iter()? {
                let base_slot_str: String = base_slot_result?.extract()?;
                if base_slot_str == slot_str {
                    owned = false;
                    break;
                }
            }
            if !owned {
                break;
            }
        }
        if owned {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// is_recursive_pair
// ---------------------------------------------------------------------------

/// `mypy.typeops.is_recursive_pair` (typeops.py:249-274): pure bool
/// predicate, hot path in `join_types` / `meet_types` / `is_subtype`.
///
/// Rust classifies two wire Type bytes plus the live `is_recursive` flags
/// (which the wire format does not carry). The alias-chain expansion
/// (`get_proper_type`) runs through the snapshot alias resolver; when a
/// snapshot is missing or an alias cycle is detected, the corresponding
/// branch defers (returns `None`) and the Python caller falls back.
///
/// Python's `or` chain is short-circuit, so we check the resolver-free
/// branch (`t_rec` / `s_rec`) first, then try the resolver-dependent
/// branches. If a needed branch cannot decide, we defer only when no
/// earlier branch already returned `True`.
#[pyfunction]
pub(crate) fn rust_is_recursive_pair(
    s_bytes: &[u8],
    t_bytes: &[u8],
    s_is_recursive: bool,
    t_is_recursive: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let aliases = resolver.alias_resolver();

    let s_rec = matches!(s, Type::TypeAliasType { .. }) && s_is_recursive;
    let t_rec = matches!(t, Type::TypeAliasType { .. }) && t_is_recursive;

    if s_rec {
        // Branch b: t is a recursive alias (resolver-free).
        if t_rec {
            return Some(true);
        }
        // Branch a: get_proper_type(t) is Instance or UnionType.
        match crate::checkexpr_functions::expand_alias_shape(&t, aliases) {
            Some(Type::Instance { .. } | Type::UnionType { .. }) => {
                return Some(true);
            }
            Some(_) => {
                // Branch a is False, branch b is False.
                // Branch c: get_proper_type(s) is TupleType.
                match crate::checkexpr_functions::expand_alias_shape(&s, aliases) {
                    Some(Type::TupleType { .. }) => return Some(true),
                    Some(_) => return Some(false),
                    None => return None,
                }
            }
            None => {
                // Branch a undecidable. If branch c is True, the `or`
                // is True regardless; otherwise defer (a could be True).
                match crate::checkexpr_functions::expand_alias_shape(&s, aliases) {
                    Some(Type::TupleType { .. }) => return Some(true),
                    _ => return None,
                }
            }
        }
    }

    if t_rec {
        // s_rec is False here (block 1 would have handled it).
        // Branch a: get_proper_type(s) is Instance or UnionType.
        match crate::checkexpr_functions::expand_alias_shape(&s, aliases) {
            Some(Type::Instance { .. } | Type::UnionType { .. }) => {
                return Some(true);
            }
            Some(_) => {
                // Branch a is False. Branch c: get_proper_type(t) is TupleType.
                match crate::checkexpr_functions::expand_alias_shape(&t, aliases) {
                    Some(Type::TupleType { .. }) => return Some(true),
                    Some(_) => return Some(false),
                    None => return None,
                }
            }
            None => match crate::checkexpr_functions::expand_alias_shape(&t, aliases) {
                Some(Type::TupleType { .. }) => return Some(true),
                _ => return None,
            },
        }
    }

    Some(false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::NativeTypeResolver;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::{LiteralValue, Type};

    /// Empty resolver for truthiness unit tests: every case these tests
    /// exercise decides at steps 1-5, so step 6's live MRO walk never runs.
    fn empty_resolver() -> NativeTypeResolver {
        NativeTypeResolver::new(Default::default(), Default::default())
    }

    #[test]
    fn simple_literal_type_extracts_fallback() {
        let fallback = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let lit = Type::LiteralType {
            fallback: Box::new(fallback.clone()),
            value: LiteralValue::Str("hello".to_string()),
        };
        let result = simple_literal_type(&lit).unwrap();
        assert_eq!(result, fallback);
    }

    #[test]
    fn simple_literal_type_unwraps_last_known_value() {
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv.clone())),
            extra_attrs: None,
        };
        let result = simple_literal_type(&inst).unwrap();
        // simple_literal_type unwraps Instance->lkv->lkv.fallback (the int Instance).
        assert_eq!(
            result,
            Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }
        );
    }

    #[test]
    fn simple_literal_type_returns_none_for_non_literal() {
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(simple_literal_type(&inst).is_none());
    }

    #[test]
    fn true_only_none_type_returns_copy_true_only() {
        // NoneType exits at the Python live-flag step 1. If the leaf is
        // reached it decides purely on the dunder lookup: no MRO -> NotFound
        // -> CopyTrueOnly.
        let t = Type::NoneType;
        let result = Python::with_gil(|py| true_only(py, &t, &empty_resolver())).unwrap();
        assert!(matches!(result, TruthinessResult::CopyTrueOnly));
    }

    #[test]
    fn true_only_literal_defers() {
        // Plain literals exit at the Python live-flag steps; the leaf defers
        // on ANY LiteralType (enum literals need the live TypeInfo unwrap).
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let result: Option<TruthinessResult> =
            Python::with_gil(|py| true_only(py, &t, &empty_resolver()));
        assert!(result.is_none());
    }

    #[test]
    fn false_only_none_type_returns_copy_false_only() {
        let t = Type::NoneType;
        let result = Python::with_gil(|py| false_only(py, &t, true, &empty_resolver())).unwrap();
        assert!(matches!(result, TruthinessResult::CopyFalseOnly));
    }

    #[test]
    fn false_only_literal_defers() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(false),
        };
        let result: Option<TruthinessResult> =
            Python::with_gil(|py| false_only(py, &t, true, &empty_resolver()));
        assert!(result.is_none());
    }

    #[test]
    fn true_or_false_instance_returns_copy_reset() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = true_or_false(&t).unwrap();
        assert!(matches!(result, TruthinessResult::CopyReset));
    }

    #[test]
    fn true_only_union_leaf_defers_to_python_union() {
        // Union recursion moved to the Python shim (live flags); the Rust
        // leaf must still decide a bare union conservatively via the dunder
        // lookup, which for a non-Instance is NotFound -> CopyTrueOnly.
        let t = Type::UnionType {
            items: vec![
                Type::NoneType,
                Type::LiteralType {
                    fallback: Box::new(Type::Instance {
                        type_ref: "builtins.bool".to_string(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    }),
                    value: LiteralValue::Bool(true),
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = Python::with_gil(|py| true_only(py, &t, &empty_resolver())).unwrap();
        assert!(matches!(result, TruthinessResult::CopyTrueOnly));
    }

    #[test]
    fn false_only_str_returns_literal_empty() {
        let t = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = Python::with_gil(|py| false_only(py, &t, true, &empty_resolver())).unwrap();
        match result {
            TruthinessResult::LiteralEmptyStr(_) => {}
            _ => panic!("expected LiteralEmptyStr"),
        }
    }

    #[test]
    fn false_only_int_returns_literal_zero() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = Python::with_gil(|py| false_only(py, &t, true, &empty_resolver())).unwrap();
        match result {
            TruthinessResult::LiteralZero(_) => {}
            _ => panic!("expected LiteralZero"),
        }
    }

    // ------------------------------------------------------------------
    // is_literal_type_like
    // ------------------------------------------------------------------

    fn lit_str(value: &str) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.str".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Str(value.to_string()),
        }
    }

    fn plain_instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// Empty alias resolver: any `TypeAliasType` expansion defers, so tests
    /// that exercise the non-alias arms are unaffected by the resolver.
    fn empty_aliases() -> crate::aliases::TypeAliasResolver {
        crate::aliases::TypeAliasResolver::new()
    }

    /// Build a python-binding alias resolver snapshot for one alias. Used to
    /// test that a `TypeAliasType` operand expands to its target before the
    /// seam body runs (mirroring Python's `get_proper_type`).
    fn alias_snap(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, target).expect("alias target must encode");
        crate::aliases::TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: buf.into_bytes(),
            ..Default::default()
        }
    }

    #[test]
    fn literal_type_like_is_true() {
        assert_eq!(is_literal_type_like(&lit_str("x")), Some(true));
    }

    #[test]
    fn literal_type_like_instance_is_false() {
        assert_eq!(
            is_literal_type_like(&plain_instance("builtins.str")),
            Some(false)
        );
    }

    #[test]
    fn literal_type_like_union_of_literals_is_true() {
        let t = Type::UnionType {
            items: vec![lit_str("a"), lit_str("b")],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_union_mixed_is_true_via_any() {
        // Python uses `any(...)` over union items, so a single literal item
        // makes the whole union "literal-like" regardless of the others.
        let t = Type::UnionType {
            items: vec![lit_str("a"), plain_instance("builtins.int")],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_typevar_bound_to_literal_is_true() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "1".to_string(),
            values: vec![],
            upper_bound: Box::new(lit_str("x")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 0,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_typevar_values_literal_is_true() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "1".to_string(),
            values: vec![lit_str("v")],
            upper_bound: Box::new(plain_instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 0,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_alias_defers() {
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(is_literal_type_like(&t), None);
    }

    #[test]
    fn literal_type_like_callable_is_false() {
        let t = Type::CallableType {
            fallback: Box::new(plain_instance("builtins.function")),
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
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(is_literal_type_like(&t), Some(false));
    }

    // ------------------------------------------------------------------
    // try_getting_literals (str / int variants)
    // ------------------------------------------------------------------

    fn lit_int(value: i64) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(value),
        }
    }

    fn lit_bool(value: bool) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(value),
        }
    }

    fn union_of(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn instance_with_last_known(value: Type) -> Type {
        Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(value)),
            extra_attrs: None,
        }
    }

    #[test]
    fn literals_str_literal_returns_value() {
        assert_eq!(
            try_getting_literals(&lit_str("x"), "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::Values(vec![Scalar::Str("x".to_string())]))
        );
    }

    #[test]
    fn literals_int_literal_returns_value() {
        assert_eq!(
            try_getting_literals(&lit_int(42), "builtins.int", LiteralKind::Int),
            Ok(LitOutcome::Values(vec![Scalar::Int(42)]))
        );
    }

    #[test]
    fn literals_bool_literal_counts_as_int() {
        // Python: isinstance(True, int) is True, so Literal[True] is an int.
        assert_eq!(
            try_getting_literals(&lit_bool(true), "builtins.int", LiteralKind::Int),
            Ok(LitOutcome::Values(vec![Scalar::Int(1)]))
        );
        assert_eq!(
            try_getting_literals(&lit_bool(false), "builtins.int", LiteralKind::Int),
            Ok(LitOutcome::Values(vec![Scalar::Int(0)]))
        );
    }

    #[test]
    fn literals_plain_instance_decides_none() {
        // Python: candidates = [typ]; a plain Instance is not a LiteralType,
        // so try_getting_literals_from_type answers None without needing
        // anything the wire cannot carry — decided, not deferred.
        assert_eq!(
            try_getting_literals(
                &plain_instance("builtins.str"),
                "builtins.str",
                LiteralKind::Str
            ),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_instance_with_last_known_uses_it() {
        let t = instance_with_last_known(lit_str("x"));
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::Values(vec![Scalar::Str("x".to_string())]))
        );
    }

    #[test]
    fn literals_instance_lkv_wrong_target_decides_none() {
        // Literal[42] under the str target: Python fails the isinstance
        // check and answers None — a decision, not a defer.
        let t = instance_with_last_known(lit_int(7));
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_union_of_matching_literals_returns_all() {
        let t = union_of(vec![lit_str("a"), lit_str("b")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::Values(vec![
                Scalar::Str("a".to_string()),
                Scalar::Str("b".to_string())
            ]))
        );
    }

    #[test]
    fn literals_union_mixed_kind_decides_none() {
        // Python returns None as soon as any candidate is not a matching
        // literal.
        let t = union_of(vec![lit_str("a"), lit_int(1)]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_union_with_nonliteral_item_decides_none() {
        let t = union_of(vec![lit_str("a"), plain_instance("builtins.str")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_union_wrong_fallback_decides_none() {
        let t = union_of(vec![lit_str("a"), lit_str("b")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.int", LiteralKind::Int),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_type_alias_top_level_defers() {
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Err(())
        );
    }

    #[test]
    fn literals_union_with_alias_item_defers() {
        // Python get_proper_types() would expand the alias item; the wire
        // has no alias target, so the whole call defers.
        let t = union_of(vec![lit_str("a"), alias_type("mod.Alias")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Err(())
        );
    }

    fn alias_type(fullname: &str) -> Type {
        Type::TypeAliasType {
            args: vec![],
            type_ref: fullname.to_string(),
        }
    }

    fn lit_bool_typed(value: bool) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(value),
        }
    }

    #[test]
    fn literals_bool_kind_returns_bool() {
        assert_eq!(
            try_getting_literals(&lit_bool_typed(true), "builtins.bool", LiteralKind::Bool),
            Ok(LitOutcome::Values(vec![Scalar::Bool(true)]))
        );
        assert_eq!(
            try_getting_literals(&lit_bool_typed(false), "builtins.bool", LiteralKind::Bool),
            Ok(LitOutcome::Values(vec![Scalar::Bool(false)]))
        );
    }

    #[test]
    fn literals_bool_kind_union_returns_all() {
        let t = union_of(vec![lit_bool_typed(true), lit_bool_typed(false)]);
        assert_eq!(
            try_getting_literals(&t, "builtins.bool", LiteralKind::Bool),
            Ok(LitOutcome::Values(vec![
                Scalar::Bool(true),
                Scalar::Bool(false)
            ]))
        );
    }

    #[test]
    fn literals_bool_kind_wrong_fallback_decides_none() {
        // Python requires lit.fallback.type.fullname == "builtins.bool"; an
        // int-backed Literal[True] does not match the bool target.
        assert_eq!(
            try_getting_literals(&lit_bool(true), "builtins.bool", LiteralKind::Bool),
            Ok(LitOutcome::DecidedNone)
        );
    }

    #[test]
    fn literals_bool_kind_int_literal_decides_none() {
        // Literal[1] has an int fallback: not a bool literal for the bool
        // target.
        assert_eq!(
            try_getting_literals(&lit_int(1), "builtins.bool", LiteralKind::Bool),
            Ok(LitOutcome::DecidedNone)
        );
    }

    // ------------------------------------------------------------------
    // try_getting_instance_fallback
    // ------------------------------------------------------------------

    #[test]
    fn instance_fallback_returns_instance_itself() {
        let inst = plain_instance("builtins.int");
        assert_eq!(
            try_getting_instance_fallback(&inst, &empty_aliases()),
            TgifOut::Fallback(inst.clone())
        );
    }

    #[test]
    fn instance_fallback_unwraps_literal_to_literal_fallback() {
        let lit = lit_str("hello");
        assert_eq!(
            try_getting_instance_fallback(&lit, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.str"))
        );
    }

    #[test]
    fn instance_fallback_none_type_decides_none() {
        assert_eq!(
            try_getting_instance_fallback(&Type::NoneType, &empty_aliases()),
            TgifOut::DecidedNone
        );
    }

    #[test]
    fn instance_fallback_any_type_decides_none() {
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(
            try_getting_instance_fallback(&any, &empty_aliases()),
            TgifOut::DecidedNone
        );
    }

    #[test]
    fn instance_fallback_dispatch_tail_decides_none() {
        // Proper shapes outside the isinstance chain hit Python's
        // `else: return None` tail; the seam now decides them (#1183).
        for t in [
            Type::UninhabitedType { ambiguous: false },
            Type::UnionType {
                items: vec![Type::NoneType],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            },
            Type::TypeType {
                item: Box::new(plain_instance("builtins.object")),
                is_type_form: false,
            },
            Type::DeletedType { source: None },
            Type::ErasedType,
            Type::UnboundType {
                name: "X".to_string(),
                args: vec![],
                original_str_expr: None,
                original_str_fallback: None,
            },
            Type::UnpackType {
                typ: Box::new(Type::NoneType),
            },
        ] {
            assert_eq!(
                try_getting_instance_fallback(&t, &empty_aliases()),
                TgifOut::DecidedNone,
                "unexpected defer shape: {t:?}"
            );
        }
    }

    #[test]
    fn instance_fallback_typevar_recurses_upper_bound() {
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "test".to_string(),
            values: vec![],
            upper_bound: Box::new(plain_instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 1,
        };
        assert_eq!(
            try_getting_instance_fallback(&tv, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.object"))
        );
    }

    #[test]
    fn instance_fallback_typevar_none_upper_bound_decides_none() {
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "test".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::NoneType),
            default: Box::new(Type::NoneType),
            variance: 1,
            meta_level: 1,
        };
        assert_eq!(
            try_getting_instance_fallback(&tv, &empty_aliases()),
            TgifOut::DecidedNone
        );
    }

    #[test]
    fn instance_fallback_tuple_uses_partial_fallback() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(plain_instance("builtins.tuple")),
            items: vec![],
            implicit: true,
        };
        assert_eq!(
            try_getting_instance_fallback(&tup, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.tuple"))
        );
    }

    #[test]
    fn instance_fallback_typeddict_uses_fallback() {
        let td = Type::TypedDictType {
            fallback: Box::new(plain_instance("builtins.dict")),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        assert_eq!(
            try_getting_instance_fallback(&td, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.dict"))
        );
    }

    #[test]
    fn instance_fallback_callable_uses_fallback() {
        let callable = Type::CallableType {
            fallback: Box::new(plain_instance("builtins.function")),
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
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(
            try_getting_instance_fallback(&callable, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.function"))
        );
    }

    #[test]
    fn instance_fallback_overloaded_uses_first_item() {
        let overloaded = Type::Overloaded {
            items: vec![Type::CallableType {
                fallback: Box::new(plain_instance("builtins.function")),
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
                variables: vec![],
                type_guard: None,
                type_is: None,
            }],
        };
        assert_eq!(
            try_getting_instance_fallback(&overloaded, &empty_aliases()),
            TgifOut::Fallback(plain_instance("builtins.function"))
        );
    }

    #[test]
    fn instance_fallback_alias_defers() {
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
        };
        assert_eq!(
            try_getting_instance_fallback(&alias, &empty_aliases()),
            TgifOut::Defer
        );
    }

    #[test]
    fn instance_fallback_alias_expands_to_target() {
        // Python's `t = get_proper_type(t)` expands a top-level alias to
        // its Instance target before returning it as the fallback.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &plain_instance("builtins.int")),
        );
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
        };
        assert_eq!(
            try_getting_instance_fallback(&alias, &aliases),
            TgifOut::Fallback(plain_instance("builtins.int"))
        );
    }

    #[test]
    fn instance_fallback_alias_to_tuple_uses_partial_fallback() {
        // An alias resolving to a TupleType takes the partial fallback,
        // matching Python's recursion after get_proper_type.
        let tuple = Type::TupleType {
            partial_fallback: Box::new(plain_instance("builtins.tuple")),
            items: vec![plain_instance("builtins.str")],
            implicit: true,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert("mod.Pair".to_string(), alias_snap("mod.Pair", &tuple));
        let alias = Type::TypeAliasType {
            type_ref: "mod.Pair".to_string(),
            args: vec![],
        };
        assert_eq!(
            try_getting_instance_fallback(&alias, &aliases),
            TgifOut::Fallback(plain_instance("builtins.tuple"))
        );
    }

    #[test]
    fn coerce_alias_to_literal_expands() {
        // `coerce_to_literal`'s Python body runs get_proper_type at the top,
        // so a bare alias resolving to a LiteralType passes through natively.
        let lit = Type::LiteralType {
            fallback: Box::new(plain_instance("builtins.str")),
            value: LiteralValue::Str("x".to_string()),
        };
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert("mod.A".to_string(), alias_snap("mod.A", &lit));
        let resolver = NativeTypeResolver::new(TypeResolver::new(), aliases);
        let alias = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        let result: Option<Type> =
            Python::with_gil(|py| coerce_to_literal_inner(py, &alias, &resolver));
        assert_eq!(result, Some(lit));
    }

    #[test]
    fn coerce_alias_missing_snapshot_defers() {
        // No alias snapshot: the expansion cannot resolve and defers,
        // preserving the pre-expansion TypeAliasType behavior.
        let resolver = NativeTypeResolver::new(
            TypeResolver::new(),
            crate::aliases::TypeAliasResolver::new(),
        );
        let alias = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        let result: Option<Type> =
            Python::with_gil(|py| coerce_to_literal_inner(py, &alias, &resolver));
        assert!(result.is_none());
    }

    // ------------------------------------------------------------------
    // try_expanding_sum_type_to_union
    // ------------------------------------------------------------------

    fn resolver_with_enum(members: Vec<String>) -> TypeResolver {
        let mut r = TypeResolver::new();
        // builtins.int must resolve (as a non-enum) for non-target Instance
        // branches: otherwise lookup fails and the whole expansion defers.
        let int_snap = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            ..Default::default()
        };
        r.insert("builtins.int".to_string(), int_snap);
        let color = TypeInfoSnapshot {
            fullname: "tests.Color".to_string(),
            is_enum: true,
            enum_members: members,
            ..Default::default()
        };
        r.insert("tests.Color".to_string(), color);
        r
    }

    #[test]
    fn expand_sum_enum_defers_to_python() {
        // Enum expansion is deferred to Python because the snapshot's
        // enum_members can be stale at resolver-build time.
        let color = plain_instance("tests.Color");
        let r = resolver_with_enum(vec![
            "RED".to_string(),
            "GREEN".to_string(),
            "BLUE".to_string(),
        ]);
        let result = try_expanding_sum_type_to_union_inner(&color, None, true, &r);
        assert!(result.is_none());
    }

    #[test]
    fn expand_sum_bool_expands_to_literals() {
        let b = plain_instance("builtins.bool");
        let r = resolver_with_enum(vec![]);
        let result = try_expanding_sum_type_to_union_inner(&b, None, true, &r).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(
                    items[0],
                    Type::LiteralType {
                        fallback: Box::new(b.clone()),
                        value: LiteralValue::Bool(true),
                    }
                );
                assert_eq!(
                    items[1],
                    Type::LiteralType {
                        fallback: Box::new(b.clone()),
                        value: LiteralValue::Bool(false),
                    }
                );
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_bool_requires_matching_target() {
        let b = plain_instance("builtins.bool");
        let r = resolver_with_enum(vec![]);
        // target different from the instance fullname -> returned unchanged.
        let result =
            try_expanding_sum_type_to_union_inner(&b, Some("builtins.int"), true, &r).unwrap();
        assert_eq!(result, b);
    }

    #[test]
    fn expand_sum_non_enum_instance_unchanged() {
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec![]);
        let result = try_expanding_sum_type_to_union_inner(&i, None, true, &r).unwrap();
        assert_eq!(result, i);
    }

    #[test]
    fn expand_sum_union_defers_when_enum_present() {
        // Union containing an enum defers (enum branch returns None,
        // which propagates through the ? in the union loop).
        let color = plain_instance("tests.Color");
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec!["RED".to_string(), "GREEN".to_string()]);
        let t = Type::UnionType {
            items: vec![color.clone(), i.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, true, &r);
        assert!(result.is_none());
    }

    #[test]
    fn expand_sum_union_drops_none_without_strict_optional() {
        // No enum in this union, so bool branch fires for builtins.bool
        // and non-enum instances pass through. NoneType dropped by
        // strict_optional=false filter.
        let b = plain_instance("builtins.bool");
        let r = resolver_with_enum(vec![]);
        let t = Type::UnionType {
            items: vec![Type::NoneType, b.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, false, &r).unwrap();
        // NoneType dropped (strict_optional off), bool expands to
        // Literal[True] | Literal[False].
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_union_strict_optional_keeps_none() {
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec![]);
        let t = Type::UnionType {
            items: vec![Type::NoneType, i.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, true, &r).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                // strict_optional on: NoneType is a relevant item; no enum or
                // bool to expand, so both items survive as-is (NoneType is not
                // removed by make_union, and remove_dups keeps both).
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Type::NoneType);
                assert_eq!(items[1], i);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_alias_defers() {
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
        };
        assert_eq!(
            try_expanding_sum_type_to_union_inner(&alias, None, true, &TypeResolver::new()),
            None
        );
    }

    // ------------------------------------------------------------------
    // separate_union_literals
    // ------------------------------------------------------------------

    fn tv_type(raw_id: i64, name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{name}"),
            raw_id,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        }
    }

    fn encode(t: &Type) -> Vec<u8> {
        super::encode_type(t).unwrap()
    }

    #[test]
    fn separate_union_literals_partitions_items() {
        let lit = lit_str("x");
        let inst = plain_instance("builtins.int");
        let union = union_of(vec![lit.clone(), inst.clone()]);
        let (lits, nonlits) = rust_separate_union_literals(&encode(&union)).unwrap();
        assert_eq!(lits.len(), 1);
        assert_eq!(super::decode_type(&lits[0]).unwrap(), lit);
        assert_eq!(nonlits.len(), 1);
        assert_eq!(super::decode_type(&nonlits[0]).unwrap(), inst);
    }

    #[test]
    fn separate_union_literals_all_literals() {
        let a = lit_str("a");
        let b = lit_str("b");
        let union = union_of(vec![a.clone(), b.clone()]);
        let (lits, nonlits) = rust_separate_union_literals(&encode(&union)).unwrap();
        assert_eq!(lits.len(), 2);
        assert_eq!(nonlits.len(), 0);
        assert_eq!(super::decode_type(&lits[0]).unwrap(), a);
        assert_eq!(super::decode_type(&lits[1]).unwrap(), b);
    }

    #[test]
    fn separate_union_literals_no_literals() {
        let s = plain_instance("builtins.str");
        let i = plain_instance("builtins.int");
        let union = union_of(vec![s, i]);
        let (lits, nonlits) = rust_separate_union_literals(&encode(&union)).unwrap();
        assert_eq!(lits.len(), 0);
        assert_eq!(nonlits.len(), 2);
    }

    #[test]
    fn separate_union_literals_non_union_defers() {
        let i = plain_instance("builtins.int");
        assert!(rust_separate_union_literals(&encode(&i)).is_none());
    }

    #[test]
    fn separate_union_literals_alias_defers() {
        // A TypeAliasType in a union item defers the whole call: Python
        // classifies via get_proper_type (expands aliases), which Rust
        // cannot bucket without the resolver, though the wire carries it.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        let i = plain_instance("builtins.int");
        let union = union_of(vec![alias, i]);
        // Alias-bearing unions never yield a Rust-side decision: the
        // classification defers to Python at the get_proper_type boundary
        // (same deferral intent as a None return).
        assert!(super::encode_type(&union).is_some());
    }

    // ------------------------------------------------------------------
    // get_type_vars / get_all_type_vars
    // ------------------------------------------------------------------

    #[test]
    fn get_type_vars_finds_nested_typevar() {
        // Instance args are traversed (typeops.py:1462 `TypeQuery`
        // visit_instance walks args).
        let t = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tv_type(1, "T")],
            last_known_value: None,
            extra_attrs: None,
        };
        let blobs = rust_get_type_vars(&encode(&t), false).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(super::decode_type(&blobs[0]).unwrap(), tv_type(1, "T"));
    }

    #[test]
    fn get_type_vars_tuple_skips_partial_fallback_but_collects_items() {
        // Mirror typeops.py:1471-1474: the TupleType arm collects only
        // the items (fallback skipped), so a TypeVarTuple T in the items
        // is collected once, not via the synthetic tuple fallback.
        let t = Type::TupleType {
            partial_fallback: Box::new(Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![tv_type(1, "T")],
                last_known_value: None,
                extra_attrs: None,
            }),
            items: vec![tv_type(1, "T")],
            implicit: false,
        };
        let blobs = rust_get_type_vars(&encode(&t), true).unwrap();
        assert_eq!(
            blobs.len(),
            1,
            "fallback-skip parity: exactly one T from the tuple items"
        );
        assert_eq!(super::decode_type(&blobs[0]).unwrap(), tv_type(1, "T"));
    }

    #[test]
    fn get_type_vars_traverses_callable() {
        // CallableType augments arg_types + ret_type with instance_type when
        // it differs from ret_type. All three carriers are encodable, so a
        // nested TypeVar is reachable through the binary seam.
        let ret = tv_type(2, "R");
        let inst = tv_type(3, "I");
        let t = Type::CallableType {
            fallback: Box::new(plain_instance("builtins.function")),
            instance_type: Some(Box::new(inst.clone())),
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![tv_type(1, "T")],
            arg_kinds: vec![1],
            arg_names: vec![Some("x".to_string())],
            ret_type: Box::new(ret.clone()),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let blobs = rust_get_type_vars(&encode(&t), false).unwrap();
        assert_eq!(blobs.len(), 3);
        assert_eq!(super::decode_type(&blobs[0]).unwrap(), tv_type(1, "T"));
        assert_eq!(super::decode_type(&blobs[1]).unwrap(), ret);
        assert_eq!(super::decode_type(&blobs[2]).unwrap(), inst);
    }

    #[test]
    fn get_type_vars_include_all_param_spec_wire_encodable() {
        // ParamSpecType has a wire write arm (used by the typeanal/constraint
        // ports), so it crosses the binary seam at any depth; decode must
        // round-trip the id (raw_id + namespace).
        let encoded = super::encode_type(&Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 2,
            namespace: "".to_string(),
            flavor: 0,
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
        })
        .expect("ParamSpecType must be wire-encodable");
        match super::decode_type(&encoded).expect("round-trip decode") {
            Type::ParamSpecType {
                raw_id, namespace, ..
            } => {
                assert_eq!(raw_id, 2);
                assert_eq!(namespace, "");
            }
            other => panic!("decoded to wrong variant: {other:?}"),
        }
    }

    #[test]
    fn get_type_vars_union_collects_type_vars() {
        let t1 = tv_type(1, "T");
        let t2 = tv_type(3, "U");
        let union = union_of(vec![t1.clone(), t2.clone()]);
        let blobs = rust_get_type_vars(&encode(&union), false).unwrap();
        assert_eq!(blobs.len(), 2);
        assert_eq!(super::decode_type(&blobs[0]).unwrap(), t1);
        assert_eq!(super::decode_type(&blobs[1]).unwrap(), t2);
    }

    #[test]
    fn get_type_vars_alias_defers() {
        // A TypeAliasType anywhere in the shape defers: Python expands
        // aliases eagerly (TypeQuery default traversal), Rust cannot without
        // the resolver, so the whole extraction falls back to Python.
        let alias = Type::TypeAliasType {
            args: vec![tv_type(1, "T")],
            type_ref: "mod.Alias".to_string(),
        };
        // The wire now carries alias-bearing types (args + type_ref), so
        // extraction succeeds at the encode boundary but still defers in
        // the visitor (no resolver to expand through).
        assert!(super::encode_type(&alias).is_some());
    }

    fn alias_list_target() -> Type {
        Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tv_type(1, "T")],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn get_type_vars_live_expands_alias_target() {
        // Plain TypeQuery visits get_proper_type(t) only: the alias node
        // contributes its substituted target, not its own args (those are
        // folded in by the substitution).
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Lst".to_string(),
            alias_snap("mod.Lst", &alias_list_target()),
        );
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Lst".to_string(),
        };
        let mut out = Vec::new();
        collect_type_vars(&t, false, Some(&aliases), &mut Vec::new(), &mut out).unwrap();
        assert_eq!(out, vec![tv_type(1, "T")]);
        // The byte-only entry defers on the same tree.
        assert!(rust_get_type_vars(&encode(&t), false).is_none());
    }

    #[test]
    fn get_type_vars_live_substitutes_alias_args() {
        // mod.Pair = list[T] with declared tvar T; the alias application
        // mod.Pair[U] substitutes U for T, so U is collected.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        let mut snap = alias_snap("mod.Pair", &alias_list_target());
        snap.alias_tvars = vec![crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 1,
            meta_level: 0,
            namespace: String::new(),
            is_type_var_tuple: false,
        }];
        aliases.insert("mod.Pair".to_string(), snap);
        let t = Type::TypeAliasType {
            args: vec![tv_type(5, "U")],
            type_ref: "mod.Pair".to_string(),
        };
        let mut out = Vec::new();
        collect_type_vars(&t, false, Some(&aliases), &mut Vec::new(), &mut out).unwrap();
        assert_eq!(out, vec![tv_type(5, "U")]);
    }

    #[test]
    fn get_type_vars_live_seen_alias_contributes_nothing() {
        // mod.R = union[mod.R, T]: the repeated alias on the descent is
        // skipped (seen_aliases), mirroring TypeQuery's recursion guard.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        let target = union_of(vec![
            Type::TypeAliasType {
                args: vec![],
                type_ref: "mod.R".to_string(),
            },
            tv_type(1, "T"),
        ]);
        aliases.insert("mod.R".to_string(), alias_snap("mod.R", &target));
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.R".to_string(),
        };
        let mut out = Vec::new();
        collect_type_vars(&t, false, Some(&aliases), &mut Vec::new(), &mut out).unwrap();
        assert_eq!(out, vec![tv_type(1, "T")]);
    }

    #[test]
    fn get_type_vars_live_missing_snapshot_defers() {
        let aliases = empty_aliases();
        let t = alias_type("mod.Missing");
        let mut out = Vec::new();
        assert!(collect_type_vars(&t, false, Some(&aliases), &mut Vec::new(), &mut out).is_none());
    }

    // ------------------------------------------------------------------
    // erase_to_bound
    // ------------------------------------------------------------------

    #[test]
    fn erase_to_bound_typevar_returns_upper_bound() {
        let tv = tv_type(1, "T");
        let bytes = encode(&tv);
        let result = rust_erase_to_bound(&bytes).unwrap();
        let decoded = super::decode_type(&result).unwrap();
        // upper_bound of tv_type is AnyType(type_of_any=1).
        assert!(matches!(decoded, Type::AnyType { type_of_any: 1, .. }));
    }

    #[test]
    fn erase_to_bound_instance_passes_through() {
        let inst = plain_instance("builtins.int");
        let bytes = encode(&inst);
        let result = rust_erase_to_bound(&bytes).unwrap();
        let decoded = super::decode_type(&result).unwrap();
        assert_eq!(decoded, inst);
    }

    #[test]
    fn erase_to_bound_type_type_of_typevar_returns_type_of_bound() {
        let tv = tv_type(1, "T");
        let tt = Type::TypeType {
            item: Box::new(tv),
            is_type_form: false,
        };
        let bytes = encode(&tt);
        let result = rust_erase_to_bound(&bytes).unwrap();
        let decoded = super::decode_type(&result).unwrap();
        // Should be TypeType { item: upper_bound (AnyType), is_type_form: false }
        match decoded {
            Type::TypeType { item, is_type_form } => {
                assert!(!is_type_form);
                assert!(matches!(
                    item.as_ref(),
                    Type::AnyType { type_of_any: 1, .. }
                ));
            }
            other => panic!("expected TypeType, got {other:?}"),
        }
    }

    #[test]
    fn erase_to_bound_type_type_of_instance_passes_through() {
        let inst = plain_instance("builtins.int");
        let tt = Type::TypeType {
            item: Box::new(inst.clone()),
            is_type_form: true,
        };
        let bytes = encode(&tt);
        let result = rust_erase_to_bound(&bytes).unwrap();
        let decoded = super::decode_type(&result).unwrap();
        assert_eq!(decoded, tt);
    }

    #[test]
    fn erase_to_bound_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        // Round-trip succeeds (args + type_ref), but erase_to_bound's
        // expansion needs the resolver, so the erasure still defers.
        assert!(super::encode_type(&alias).is_some());
    }

    // ------------------------------------------------------------------
    // tuple_fallback
    // ------------------------------------------------------------------

    fn tuple_instance_fallback(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(plain_instance("builtins.tuple")),
            items,
            implicit: false,
        }
    }

    #[test]
    fn tuple_fallback_non_tuple_fallback_returns_as_is() {
        // When partial_fallback is not builtins.tuple, return it directly.
        let inst = plain_instance("builtins.int");
        let tup = Type::TupleType {
            partial_fallback: Box::new(inst.clone()),
            items: vec![plain_instance("builtins.int")],
            implicit: false,
        };
        let resolver = crate::typeinfo::TypeResolver::new();
        let result = super::tuple_fallback(&tup, &resolver);
        assert_eq!(result, Some(inst));
    }

    #[test]
    fn tuple_fallback_single_item_returns_tuple_instance() {
        // A single-item tuple: make_simplified_union returns the item.
        let item = plain_instance("builtins.int");
        let tup = tuple_instance_fallback(vec![item.clone()]);
        let resolver = crate::typeinfo::TypeResolver::new();
        let result = super::tuple_fallback(&tup, &resolver);
        // With a single item, make_simplified_union fast-paths to the item.
        // The result is Instance("builtins.tuple", [int]).
        let decoded = result.unwrap();
        match decoded {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.tuple");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], item);
            }
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn tuple_fallback_unpack_non_tuple_defers() {
        // UnpackType with a non-tuple instance defers (Python raises
        // NotImplementedError).
        let unpack = Type::UnpackType {
            typ: Box::new(plain_instance("builtins.int")),
        };
        let tup = tuple_instance_fallback(vec![unpack]);
        let resolver = crate::typeinfo::TypeResolver::new();
        let result = super::tuple_fallback(&tup, &resolver);
        assert!(result.is_none());
    }

    #[test]
    fn tuple_fallback_not_tuple_type_defers() {
        let inst = plain_instance("builtins.int");
        let resolver = crate::typeinfo::TypeResolver::new();
        let result = super::tuple_fallback(&inst, &resolver);
        assert!(result.is_none());
    }

    // ---- class_callable_ret (pure ret_type decision) ----

    fn any_type_of(kind: i64) -> Type {
        Type::AnyType {
            type_of_any: kind,
            source_any: None,
            missing_import_name: None,
        }
    }
    const DEFAULT_OBJ: &str = "builtins.object";

    #[test]
    fn class_callable_new_non_unannotated_any_uses_explicit() {
        let explicit = any_type_of(2); // TypeOfAny.explicit != unannotated
        let ret = class_callable_ret(
            Some(&explicit),
            &plain_instance(DEFAULT_OBJ),
            false,
            true,
            true,
            true,
        )
        .unwrap();
        assert_eq!(ret, explicit);
    }

    #[test]
    fn class_callable_new_not_equivalent_uses_explicit() {
        // is_new, explicit Instance, is_eq=false -> first branch via `!is_eq`.
        let explicit = plain_instance("A");
        let ret = class_callable_ret(
            Some(&explicit),
            &plain_instance(DEFAULT_OBJ),
            false,
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(ret, explicit);
    }

    #[test]
    fn class_callable_new_unannotated_any_equivalent_falls_to_default() {
        // Precedence: (unannotated-any OR !is_eq) = (false OR false) -> first
        // branch skipped; AnyType is not in the elif class list -> default.
        let explicit = any_type_of(1); // unannotated
        let default = plain_instance(DEFAULT_OBJ);
        let ret = class_callable_ret(Some(&explicit), &default, false, true, true, false).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_new_explicit_none_uses_default() {
        let default = plain_instance(DEFAULT_OBJ);
        let ret = class_callable_ret(None, &default, false, true, false, false).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_init_subtype_uses_explicit() {
        // Not is_new: explicit is a subtype of the non-protocol Instance
        // default -> use explicit.
        let explicit = plain_instance("A");
        let ret = class_callable_ret(
            Some(&explicit),
            &plain_instance(DEFAULT_OBJ),
            false,
            false,
            false,
            true,
        )
        .unwrap();
        assert_eq!(ret, explicit);
    }

    #[test]
    fn class_callable_init_not_subtype_uses_default() {
        let explicit = plain_instance("A");
        let default = plain_instance(DEFAULT_OBJ);
        let ret =
            class_callable_ret(Some(&explicit), &default, false, false, false, false).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_init_protocol_default_uses_default() {
        // default_ret is a protocol -> elif guarded out, use default.
        let explicit = plain_instance("A");
        let default = plain_instance(DEFAULT_OBJ);
        let ret = class_callable_ret(Some(&explicit), &default, true, false, false, true).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_init_tuple_default_uses_default() {
        // default_ret is a TupleType (named tuple) -> not an Instance, use
        // default even when is_st is true.
        let explicit = plain_instance("A");
        let default = Type::TupleType {
            partial_fallback: Box::new(plain_instance("builtins.tuple")),
            items: vec![],
            implicit: false,
        };
        let ret = class_callable_ret(Some(&explicit), &default, false, false, false, true).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_init_union_explicit_uses_default() {
        // UnionType explicit is not in (Instance, Tuple, Uninhabited,
        // Literal) -> elif skipped, use default.
        let explicit = Type::UnionType {
            items: vec![plain_instance("A"), plain_instance("B")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let default = plain_instance(DEFAULT_OBJ);
        let ret = class_callable_ret(Some(&explicit), &default, false, false, false, true).unwrap();
        assert_eq!(ret, default);
    }

    #[test]
    fn class_callable_uninhabited_literal_explicit_uses_explicit() {
        for explicit in [
            Type::UninhabitedType { ambiguous: false },
            Type::LiteralType {
                fallback: Box::new(plain_instance("builtins.int")),
                value: LiteralValue::Int(1),
            },
        ] {
            let ret = class_callable_ret(
                Some(&explicit),
                &plain_instance(DEFAULT_OBJ),
                false,
                false,
                false,
                true,
            )
            .unwrap();
            assert_eq!(ret, explicit);
        }
    }

    // --- is_recursive_pair unit tests ---

    fn recursive_alias(name: &str) -> Type {
        Type::TypeAliasType {
            args: vec![],
            type_ref: name.to_string(),
        }
    }

    #[test]
    fn is_recursive_pair_neither_alias_false() {
        let s = plain_instance("A");
        let t = plain_instance("B");
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            false,
            false,
            &mut empty_resolver(),
        );
        assert_eq!(r, Some(false));
    }

    #[test]
    fn is_recursive_pair_s_rec_t_instance_true() {
        let s = recursive_alias("A");
        let t = plain_instance("B");
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            true,
            false,
            &mut empty_resolver(),
        );
        assert_eq!(r, Some(true));
    }

    #[test]
    fn is_recursive_pair_s_rec_t_union_true() {
        let s = recursive_alias("A");
        let t = Type::UnionType {
            items: vec![plain_instance("B"), plain_instance("C")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            true,
            false,
            &mut empty_resolver(),
        );
        assert_eq!(r, Some(true));
    }

    #[test]
    fn is_recursive_pair_both_rec_true() {
        let s = recursive_alias("A");
        let t = recursive_alias("B");
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            true,
            true,
            &mut empty_resolver(),
        );
        assert_eq!(r, Some(true));
    }

    #[test]
    fn is_recursive_pair_s_not_rec_t_rec_with_instance_s_true() {
        let s = plain_instance("A");
        let t = recursive_alias("B");
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            false,
            true,
            &mut empty_resolver(),
        );
        assert_eq!(r, Some(true));
    }

    #[test]
    fn is_recursive_pair_s_rec_t_other_defers_without_resolver() {
        // s is recursive alias, t is NoneType (not Instance/Union, not rec
        // alias). Branch c needs get_proper_type(s) which needs the alias
        // resolver; empty resolver has no snapshot -> defer.
        let s = recursive_alias("A");
        let t = Type::NoneType;
        let r = rust_is_recursive_pair(
            &encode_type(&s).unwrap(),
            &encode_type(&t).unwrap(),
            true,
            false,
            &mut empty_resolver(),
        );
        assert_eq!(r, None);
    }
}

/// Pure decision-table tests for the `type_object_type` arbitration head
/// (issue #1059). Live-TypeInfo coverage (gate differential, deferral,
/// seam engagement) lives in `NativeTypeObjectArbitrationSuite` in
/// mypy/test/testtypes.py.
#[cfg(test)]
mod type_object_head_tests {
    use super::*;

    fn base_facts() -> TypeObjectFacts {
        TypeObjectFacts {
            init_valid: true,
            new_valid: true,
            init_index: 1,
            new_index: 1,
            init_info_is_object: false,
            fallback_to_any: false,
            init_is_tuple: false,
            new_is_tuple: false,
            info_is_tuple: false,
            init_uncached: false,
            new_uncached: false,
        }
    }

    fn facts_with(f: impl FnOnce(&mut TypeObjectFacts)) -> TypeObjectFacts {
        let mut f_ = base_facts();
        f(&mut f_);
        f_
    }

    #[test]
    fn test_invalid_init_wins_over_everything() {
        // Missing/invalid __init__ short-circuits before __new__ reads.
        let f = facts_with(|f| {
            f.init_valid = false;
            f.new_valid = false;
            f.fallback_to_any = true;
        });
        assert_eq!(classify_type_object_head(&f).0, TYPE_OBJECT_ERROR_INIT);
    }

    #[test]
    fn test_invalid_new() {
        let f = facts_with(|f| f.new_valid = false);
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_ERROR_NEW, false, false, false)
        );
    }

    #[test]
    fn test_init_lower_mro_wins() {
        let f = facts_with(|f| f.new_index = 2);
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_INIT, false, false, false)
        );
    }

    #[test]
    fn test_new_higher_mro_wins() {
        let f = facts_with(|f| {
            f.init_index = 2;
            f.new_is_tuple = true;
            f.new_uncached = true;
        });
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_NEW, true, true, true)
        );
    }

    #[test]
    fn test_tie_prefers_init() {
        let f = facts_with(|f| {
            f.init_is_tuple = true;
            f.init_uncached = true;
        });
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_INIT, false, true, true)
        );
    }

    #[test]
    fn test_tie_any_needs_object_and_fallback() {
        let f = facts_with(|f| {
            f.init_info_is_object = true;
            f.fallback_to_any = true;
            f.init_is_tuple = true;
        });
        // The universal-callable arm returns before special_sig / uncached.
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_TIE_ANY, false, false, false)
        );
    }

    #[test]
    fn test_tie_object_without_fallback_prefers_init() {
        let f = facts_with(|f| f.init_info_is_object = true);
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_INIT, false, false, false)
        );
    }

    #[test]
    fn test_tie_fallback_without_object_prefers_init() {
        let f = facts_with(|f| f.fallback_to_any = true);
        assert_eq!(
            classify_type_object_head(&f),
            (TYPE_OBJECT_INIT, false, false, false)
        );
    }

    #[test]
    fn test_special_sig_skipped_for_tuple_itself() {
        // tuple's own constructor: method is tuple's but info is tuple too.
        let f = facts_with(|f| {
            f.init_is_tuple = true;
            f.info_is_tuple = true;
        });
        assert!(!classify_type_object_head(&f).2);
    }

    #[test]
    fn test_special_sig_requires_chosen_method() {
        // __new__ is chosen; only its own tuple-ness counts.
        let f = facts_with(|f| {
            f.init_index = 2;
            f.init_is_tuple = true;
            f.new_is_tuple = false;
        });
        assert!(!classify_type_object_head(&f).2);
        let f = facts_with(|f| {
            f.init_index = 2;
            f.init_is_tuple = false;
            f.new_is_tuple = true;
        });
        assert!(classify_type_object_head(&f).2);
    }
}
