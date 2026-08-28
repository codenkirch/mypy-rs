//! Native port of `mypy.checker.conditional_types` (checker.py:9280-9422):
//! the isinstance/equality narrowing that splits a `current_type` into
//! `(proposed, remaining)` based on `proposed_type_ranges`.
//!
//! The Python side serializes each `TypeRange` as `item.write(buf)` (the
//! wire `Type` bytes `read_type` consumes) followed by a bool flag. Rust
//! decodes the list, runs every branch of the Python function, and returns
//! the two result types as wire blobs (or `None` to defer to Python).
//!
//! Every sub-step that Rust cannot decide propagates `?` over `Option`, so
//! a deferral anywhere in the chain falls back to the pure-Python path and
//! parity is preserved per call.

use pyo3::prelude::*;

use crate::checker_helpers::restrict_subtype_away_inner;
use crate::meet::overlap;
use crate::setops::{make_simplified_union, union_make_union};
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{read_bool_attr, read_str_list_attr, NativeTypeResolver, TypeResolver};
use crate::visitor::{flatten_nested_unions_inner, remove_dups_inner};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

/// One decoded `TypeRange`. Fields are read by
/// checker_stmts::rust_narrow_type_by_identity_equality (#1126), which
/// narrows against a single lower-bound range.
pub(crate) struct WireRange {
    pub(crate) item: Type,
    pub(crate) is_upper_bound: bool,
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Decode the `_serialize_type_ranges` byte layout: bare count, then per
/// range `read_type` + `read_bool`.
fn decode_ranges(bytes: &[u8]) -> Option<Vec<WireRange>> {
    let mut buf = ReadBuffer::new(bytes);
    let count = wire::read_int_bare(&mut buf).ok()?;
    if count < 0 {
        return None;
    }
    let mut ranges = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let item = wire::read_type(&mut buf, None).ok()?;
        let is_upper_bound = wire::read_bool(&mut buf).ok()?;
        ranges.push(WireRange {
            item,
            is_upper_bound,
        });
    }
    Some(ranges)
}

/// `get_proper_type` for the wire format. A `TypeAliasType` has no target
/// on the wire, so it defers (returns `None`); everything else is proper.
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// `UnionType.make_union` with the constructor's nested-union flattening
/// (types.py:3465, handle_type_alias_type=False): 0 items -> bottom, 1 item
/// -> that item, >1 -> `UnionType`. Flattening matters because the
/// NewType-unwrap and expansion steps can turn an item into a union.
fn make_union(items: &[Type]) -> Type {
    match items.len() {
        0 => Type::UninhabitedType { ambiguous: false },
        1 => items[0].clone(),
        _ => {
            let flat =
                flatten_nested_unions_inner(items, false, true).unwrap_or_else(|| items.to_vec());
            union_make_union(flat)
        }
    }
}

// ---------------------------------------------------------------------------
// expand_for_target (typeops.py:1292-1333), with enum support
// ---------------------------------------------------------------------------

/// `mypy.typeops.try_expanding_sum_type_to_union` (typeops.py:1292-1333),
/// the subset `conditional_types` needs to expand a bool or enum target.
///
/// Mirrors `typeops::try_expanding_sum_type_to_union_inner` but reads enum
/// members live via the resolver's installed live TypeInfo map (the frozen
/// snapshot's `enum_members` can go stale for nonmember members). Defers
/// (`None`) when the live read fails, a recursive alias appears inside a
/// union rebuild, or the target span needs a live read the map cannot give.
///
/// Also used by checker_stmts::rust_narrow_type_by_identity_equality (#1126).
pub(crate) fn expand_for_target<'py>(
    typ: &Type,
    target_fullname: Option<&str>,
    strict_optional: bool,
    native: &NativeTypeResolver,
    py: Python<'py>,
) -> Option<Type> {
    if let Type::TypeAliasType { .. } = typ {
        return None;
    }
    match typ {
        Type::UnionType { items, .. } => {
            let relevant: Vec<Type> = if strict_optional {
                items.clone()
            } else {
                items
                    .iter()
                    .filter(|i| !matches!(i, Type::NoneType))
                    .cloned()
                    .collect()
            };
            // flatten_nested_unions(relevant, type_alias_type=True,
            // recursive=True): a recursive alias inside yields None (defer).
            let flat = flatten_nested_unions_inner(&relevant, true, true)?;
            let deduped = remove_dups_inner(&flat);
            let mut out = Vec::with_capacity(deduped.len());
            for item in &deduped {
                out.push(expand_for_target(
                    item,
                    target_fullname,
                    strict_optional,
                    native,
                    py,
                )?);
            }
            Some(make_union(&out))
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
                return Some(make_union(&[lit(true), lit(false)]));
            }
            // Enum expansion reads members live (coerce_to_literal pattern,
            // typeops.rs:1253): a snapshot member list can be stale.
            let info = native.live_typeinfo(py, type_ref)?;
            let is_enum = read_bool_attr(info, "is_enum").unwrap_or(false);
            if !is_enum {
                return Some(typ.clone());
            }
            let members = read_str_list_attr(info, "enum_members").unwrap_or_default();
            let mut items = Vec::with_capacity(members.len());
            for name in members {
                items.push(Type::LiteralType {
                    fallback: Box::new(typ.clone()),
                    value: LiteralValue::Str(name),
                });
            }
            if items.is_empty() {
                return Some(typ.clone());
            }
            Some(make_union(&items))
        }
        _ => Some(typ.clone()),
    }
}

// ---------------------------------------------------------------------------
// shallow_erase_type_for_equality (erasetype.py:418-429)
// ---------------------------------------------------------------------------

/// `mypy.erasetype.shallow_erase_type_for_equality` (erasetype.py:418-429):
/// unions map item-wise and rebuild with `make_union`; an `Instance` with
/// args defers (erasing type vars to the right `AnyType` is not portable);
/// everything else is identity. Mirrors `checker_stmts::shallow_erase_for_equality`.
fn shallow_erase_for_equality(t: &Type) -> Option<Type> {
    match t {
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(shallow_erase_for_equality(item)?);
            }
            Some(union_make_union(out))
        }
        Type::Instance { args, .. } if !args.is_empty() => None,
        _ => Some(t.clone()),
    }
}

/// Unwrap NewType instances down to their first base (checker.py:9368-9369):
/// `while isinstance(item, Instance) and item.type.is_newtype: item =
/// item.type.bases[0]`. Defers when a NewType has no decodable first base.
fn unwrap_newtype(t: &Type, resolver: &TypeResolver) -> Option<Type> {
    let mut cur = t.clone();
    loop {
        let Type::Instance { type_ref, .. } = &cur else {
            return Some(cur);
        };
        let snap = resolver.get(type_ref)?;
        if !snap.is_newtype {
            return Some(cur);
        }
        let first = snap.bases.first()?;
        let mut buf = ReadBuffer::new(first);
        cur = wire::read_type(&mut buf, None).ok()?;
    }
}

// ---------------------------------------------------------------------------
// conditional_types (checker.py:9280-9422)
// ---------------------------------------------------------------------------

/// `mypy.checker.conditional_types` (checker.py:9280-9422), Rust subset.
///
/// Mirrors the Python branch-for-branch:
///   * `None` ranges -> `(current, default)`.
///   * empty ranges -> `(Uninhabited, default)`.
///   * single bool/enum-literal range -> expand `current` by the target.
///   * Union current -> recurse per item with `default=item`, then
///     `make_simplified_union` both sides.
///   * proposed = `make_simplified_union` of the range items, NewType
///     unwrapped, flattened `make_union` (typeops::make_union).
///   * Any currents / proposed short-circuit; concrete-subtype and
///     structural-subtype branches; equality erasure; overlap;
///     restrict_subtype_away; avoid-widening.
///
/// Returns `Some((yes, no))` on success, `None` to defer to Python.
#[allow(clippy::too_many_arguments)]
pub(crate) fn conditional_types_inner(
    current: &Type,
    ranges: Option<&[WireRange]>,
    default: Option<&Type>,
    consider_runtime_isinstance: bool,
    from_equality: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
    native: &NativeTypeResolver,
    py: Python<'_>,
) -> Option<(Option<Type>, Option<Type>)> {
    let ranges = match ranges {
        None => return Some((Some(current.clone()), default.cloned())),
        Some(r) => r,
    };
    if ranges.is_empty() {
        return Some((
            Some(Type::UninhabitedType { ambiguous: false }),
            default.cloned(),
        ));
    }

    // current_type is mutated by the len()==1 expansion (checker.py:9337),
    // so it must be owned here.
    let mut current = current.clone();
    if ranges.len() == 1 {
        let target = get_proper_or_none(&ranges[0].item)?;
        if let Type::LiteralType {
            fallback,
            value: LiteralValue::Bool(_),
            ..
        } = target
        {
            let Type::Instance { type_ref, .. } = &**fallback else {
                return None;
            };
            current = expand_for_target(&current, Some(type_ref), strict_optional, native, py)?;
        } else if let Type::LiteralType {
            fallback,
            value: LiteralValue::Str(_),
            ..
        } = target
        {
            let Type::Instance { type_ref, .. } = &**fallback else {
                return None;
            };
            let snap = resolver.get(type_ref)?;
            if snap.is_enum {
                current = expand_for_target(&current, Some(type_ref), strict_optional, native, py)?;
            }
        }
    }

    // Factorize over unions (checker.py:9342-9360): each item recurses with
    // default=item and both sides are simplified.
    let p_current = get_proper_or_none(&current)?;
    if let Type::UnionType { items, .. } = p_current {
        let mut yes_items = Vec::with_capacity(items.len());
        let mut no_items = Vec::with_capacity(items.len());
        for union_item in items {
            let (yes_type, no_type) = conditional_types_inner(
                union_item,
                Some(ranges),
                Some(union_item),
                consider_runtime_isinstance,
                from_equality,
                strict_optional,
                resolver,
                native,
                py,
            )?;
            // `default` is always `Some(union_item)` here (never None), so
            // Python always receives `Type` payloads from the recursion;
            // a Rust None would diverge, so defer.
            yes_items.push(yes_type?);
            no_items.push(no_type?);
        }
        let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
        let yes = make_simplified_union(&yes_items, &ctx, resolver, true)?;
        let no = make_simplified_union(&no_items, &ctx, resolver, true)?;
        return Some((Some(yes), Some(no)));
    }

    // proposed = make_simplified_union of the range items, then NewType
    // unwrap, then flattened make_union (checker.py:9362-9371).
    let proposed_items: Vec<Type> = ranges.iter().map(|r| r.item.clone()).collect();
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let proposed = make_simplified_union(&proposed_items, &ctx, resolver, true)?;
    let proposed_proper = get_proper_or_none(&proposed).cloned();
    let mut items = match proposed_proper {
        Some(Type::UnionType { items, .. }) => items,
        _ => vec![proposed.clone()],
    };
    for item in &mut items {
        *item = unwrap_newtype(item, resolver)?;
    }
    let proposed = make_union(&items);

    // Any current keeps the else-branch as current (checker.py:9373-9374).
    if matches!(get_proper_or_none(&current)?, Type::AnyType { .. }) {
        return Some((Some(proposed), Some(current)));
    }
    // Any proposed: no narrowing, else keeps default (checker.py:9375-9379).
    if matches!(get_proper_or_none(&proposed)?, Type::AnyType { .. }) {
        return Some((Some(proposed), default.cloned()));
    }

    if !ranges.iter().any(|r| r.is_upper_bound) {
        // Concrete proper subtype (checker.py:9381-9383): proposed covers
        // current, so the if-branch keeps default and the else is unreachable.
        let proper_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
        match is_subtype(&current, &proposed, &proper_ctx, resolver) {
            Some(true) => {
                return Some((
                    default.cloned(),
                    Some(Type::UninhabitedType { ambiguous: false }),
                ));
            }
            None => return None,
            Some(false) => {}
        }
        // Structural subtypes (checker.py:9386-9397): a Callable or protocol
        // target that `current` is a non-proper subtype of needs
        // restrict_subtype_away to rule out the unequal-but-overlapping value.
        let structural = match get_proper_or_none(&proposed) {
            Some(Type::CallableType { .. }) => true,
            Some(Type::Instance { type_ref, .. }) => resolver.get(type_ref)?.is_protocol,
            _ => false,
        };
        if structural {
            let pctx = SubtypeContext::new(false, false, false, true, false, strict_optional);
            match is_subtype(&current, &proposed, &pctx, resolver) {
                // Rust is_subtype only proves the structural-subtype side
                // (Some(true)); anything else can diverge from Python, so
                // defer and let the pure-Python path decide.
                Some(true) => {
                    let restricted = restrict_subtype_away_inner(
                        &current,
                        default.unwrap_or(&proposed),
                        consider_runtime_isinstance,
                        strict_optional,
                        resolver,
                    )?;
                    return Some((default.cloned(), Some(restricted)));
                }
                _ => return None,
            }
        }
    }

    // from_equality: erase generic args so values with different generics
    // can compare equal (checker.py:9399-9402).
    let proposed = if from_equality {
        shallow_erase_for_equality(&proposed)?
    } else {
        proposed
    };

    // Overlap (checker.py:9404-9406): never of any proposed type -> the
    // if-branch is unreachable. A Rust `None` means "cannot decide", so
    // defer the whole call rather than assume overlap.
    match overlap(
        &current,
        &proposed,
        strict_optional,
        true,
        false,
        resolver,
        0,
    ) {
        Some(false) => {
            return Some((
                Some(Type::UninhabitedType { ambiguous: false }),
                default.cloned(),
            ));
        }
        Some(true) => {}
        None => return None,
    }

    // Only restrict when the type is precise, not bounded
    // (checker.py:9408-9416).
    let precise_items: Vec<Type> = ranges
        .iter()
        .filter(|r| !r.is_upper_bound)
        .map(|r| r.item.clone())
        .collect();
    let precise_type = make_union(&precise_items);
    let remaining = restrict_subtype_away_inner(
        &current,
        &precise_type,
        consider_runtime_isinstance,
        strict_optional,
        resolver,
    )?;

    // Avoid widening (checker.py:9419-9420).
    let proper_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let proposed = match is_subtype(&current, &proposed, &proper_ctx, resolver) {
        Some(true) => default.cloned().unwrap_or(current),
        None => return None,
        Some(false) => proposed,
    };

    Some((Some(proposed), Some(remaining)))
}

// ---------------------------------------------------------------------------
// pyfunction entry
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry mirroring `rust_narrow_type_by_identity_equality`:
/// returns `(yes_bytes, no_bytes)` or `None` (defer to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[pyo3(signature = (
    current_bytes,
    ranges_bytes,
    default_bytes,
    consider_runtime_isinstance,
    from_equality,
    strict_optional,
    resolver
))]
pub(crate) fn rust_conditional_types(
    py: Python<'_>,
    current_bytes: &[u8],
    ranges_bytes: Option<&[u8]>,
    default_bytes: Option<&[u8]>,
    consider_runtime_isinstance: bool,
    from_equality: bool,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> Option<(Option<Vec<u8>>, Option<Vec<u8>>)> {
    let current = decode_type(current_bytes)?;
    let ranges = match ranges_bytes {
        Some(bytes) => Some(decode_ranges(bytes)?),
        None => None,
    };
    let default = match default_bytes {
        Some(bytes) => Some(decode_type(bytes)?),
        None => None,
    };
    let (yes, no) = conditional_types_inner(
        &current,
        ranges.as_deref(),
        default.as_ref(),
        consider_runtime_isinstance,
        from_equality,
        strict_optional,
        resolver.resolver(),
        resolver,
        py,
    )?;
    let yes_blob = match yes {
        Some(t) => Some(encode_type(&t)?),
        None => None,
    };
    let no_blob = match no {
        Some(t) => Some(encode_type(&t)?),
        None => None,
    };
    Some((yes_blob, no_blob))
}
