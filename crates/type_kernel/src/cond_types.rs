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
use crate::checkexpr_functions::get_proper_or_expand;
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

/// `UnionType.make_union` with the constructor's nested-union flattening
/// (types.py:3465, handle_type_alias_type=False): 0 items -> bottom, 1 item
/// -> that item, >1 -> `UnionType`. Flattening matters because the
/// NewType-unwrap and expansion steps can turn an item into a union.
fn make_union(items: &[Type]) -> Type {
    match items.len() {
        0 => Type::UninhabitedType { ambiguous: false },
        1 => items[0].clone(),
        _ => {
            let flat = flatten_nested_unions_inner(items, false, true, None, &mut Vec::new(), None)
                .unwrap_or_else(|| items.to_vec());
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
            let flat =
                flatten_nested_unions_inner(&relevant, true, true, None, &mut Vec::new(), None)?;
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

/// `TypeOfAny.special_form` = 6 (types.py:297).
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

fn any_special_form() -> Type {
    Type::AnyType {
        type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
        source_any: None,
        missing_import_name: None,
    }
}

/// `mypy.erasetype.shallow_erase_type_for_equality` (erasetype.py:493-508):
/// unions map item-wise and rebuild with `make_union`; an `Instance` with
/// args erases one arg per `defn.type_vars` slot (`erased_vars`,
/// typevartuples.py:28-35): a TypeVarTuple slot becomes
/// `UnpackType(tuple_fallback[Any])`, everything else
/// `AnyType(special_form)`; the fresh Instance drops
/// `last_known_value`. The slot count drives the result, not
/// `len(args)` (a variadic class absorbs several args into its TVT).
/// Everything else is identity.
fn shallow_erase_for_equality(t: &Type, resolver: &TypeResolver) -> Option<Type> {
    match t {
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(shallow_erase_for_equality(item, resolver)?);
            }
            Some(union_make_union(out))
        }
        Type::Instance { type_ref, args, .. } if !args.is_empty() => {
            let snap = resolver.get(type_ref)?;
            let mut args = Vec::with_capacity(snap.type_vars_with_variance.len());
            for (_, _, kind) in &snap.type_vars_with_variance {
                if *kind == 2 {
                    // TypeVarTuple slot: valid erasure is *tuple[Any, ...].
                    let bytes = snap.type_var_tuple_fallback.as_ref()?;
                    let mut fallback = decode_type(bytes)?;
                    match &mut fallback {
                        Type::Instance { args: fargs, .. } => {
                            fargs.clear();
                            fargs.push(any_special_form());
                        }
                        _ => return None,
                    }
                    args.push(Type::UnpackType {
                        typ: Box::new(fallback),
                        from_star_syntax: false,
                    });
                } else {
                    args.push(any_special_form());
                }
            }
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
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
    aliases: &crate::aliases::TypeAliasResolver,
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
        // target = get_proper_type(proposed_type_ranges[0].item)
        // (checker.py:10843-10844): expand an alias range item through the
        // alias snapshot.
        let target_owned = get_proper_or_expand(&ranges[0].item, aliases)?;
        let target = &target_owned;
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

    // Factorize over unions (checker.py:10854-10872): each item recurses
    // with default=item and both sides are simplified.
    let current_proper = get_proper_or_expand(&current, aliases)?;
    let p_current = &current_proper;
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
                aliases,
                py,
            )?;
            // `default` is always `Some(union_item)` here (never None), so
            // Python always receives `Type` payloads from the recursion;
            // a Rust None would diverge, so defer.
            yes_items.push(yes_type?);
            no_items.push(no_type?);
        }
        let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
        // Python's make_simplified_union is alias-transparent: dedup runs
        // on the expanded form (typeops.py step 2 + final get_proper_type).
        // Pre-expand the outputs; multi-item results are str-parity.
        let yes_items = yes_items
            .iter()
            .map(|t| get_proper_or_expand(t, aliases))
            .collect::<Option<Vec<_>>>()?;
        let no_items = no_items
            .iter()
            .map(|t| get_proper_or_expand(t, aliases))
            .collect::<Option<Vec<_>>>()?;
        let yes = make_simplified_union(&yes_items, &ctx, resolver, true, false)?;
        let no = make_simplified_union(&no_items, &ctx, resolver, true, false)?;
        return Some((Some(yes), Some(no)));
    }

    // proposed = make_simplified_union of the range items, then per-item
    // NewType unwrap, then proper expansion (checker.py:10874-10883). Range
    // items expand first, as Python's make_simplified_union does for dedup.
    let proposed_items: Vec<Type> = ranges
        .iter()
        .map(|r| get_proper_or_expand(&r.item, aliases))
        .collect::<Option<Vec<_>>>()?;
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let proposed = make_simplified_union(&proposed_items, &ctx, resolver, true, false)?;
    let mut items = match &proposed {
        Type::UnionType { items, .. } => items.clone(),
        _ => vec![proposed.clone()],
    };
    for item in &mut items {
        let proper = get_proper_or_expand(item, aliases)?;
        *item = unwrap_newtype(&proper, resolver)?;
    }
    let proposed = get_proper_or_expand(&make_union(&items), aliases)?;

    // Any current keeps the else-branch as current (checker.py:10885-10886).
    if matches!(p_current, Type::AnyType { .. }) {
        return Some((Some(proposed), Some(current)));
    }
    // Any proposed: no narrowing, else keeps default (checker.py:10887-10891).
    if matches!(proposed, Type::AnyType { .. }) {
        return Some((Some(proposed), default.cloned()));
    }

    if !ranges.iter().any(|r| r.is_upper_bound) {
        // Concrete proper subtype (checker.py:9381-9383): proposed covers
        // current, so the if-branch keeps default and the else is
        // unreachable. Python's is_proper_subtype expands operands.
        let proper_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
        match is_subtype(p_current, &proposed, &proper_ctx, resolver) {
            Some(true) => {
                return Some((
                    default.cloned(),
                    Some(Type::UninhabitedType { ambiguous: false }),
                ));
            }
            None => return None,
            Some(false) => {}
        }
        // Structural subtypes (checker.py:10898-10909): a Callable or
        // protocol target that `current` is a non-proper subtype of needs
        // restrict_subtype_away; `proposed` is already the proper form.
        let structural = match &proposed {
            Type::CallableType { .. } => true,
            Type::Instance { type_ref, .. } => resolver.get(type_ref)?.is_protocol,
            _ => false,
        };
        if structural {
            let pctx = SubtypeContext::new(false, false, false, true, false, strict_optional);
            match is_subtype(p_current, &proposed, &pctx, resolver) {
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
                        aliases,
                    )?;
                    return Some((default.cloned(), Some(restricted)));
                }
                _ => return None,
            }
        }
    }

    // from_equality: erase generic args so values with different generics
    // can compare equal (checker.py:10909-10914).
    let proposed = if from_equality {
        shallow_erase_for_equality(&proposed, resolver)?
    } else {
        proposed
    };

    // Overlap (checker.py:9404-9406): never of any proposed type -> the
    // if-branch is unreachable. A Rust `None` means "cannot decide", so
    // defer the whole call rather than assume overlap.
    if !overlap(
        // Python's is_overlapping_types expands alias operands internally,
        // so the decision must run on the expanded current.
        p_current,
        &proposed,
        strict_optional,
        true,
        false,
        resolver,
        0,
    )? {
        return Some((
            Some(Type::UninhabitedType { ambiguous: false }),
            default.cloned(),
        ));
    }

    // Only restrict when the type is precise, not bounded
    // (checker.py:9408-9416).
    let precise_items: Vec<Type> = ranges
        .iter()
        .filter(|r| !r.is_upper_bound)
        .map(|r| get_proper_or_expand(&r.item, aliases))
        .collect::<Option<Vec<_>>>()?;
    let precise_type = make_union(&precise_items);
    let remaining = restrict_subtype_away_inner(
        &current,
        &precise_type,
        consider_runtime_isinstance,
        strict_optional,
        resolver,
        aliases,
    )?;

    // Avoid widening (checker.py:10930-10932): Python checks
    // is_proper_subtype(p_current_type, proposed_type), the proper form of
    // current, so alias currents expand here too.
    let proper_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let proposed = match is_subtype(&current_proper, &proposed, &proper_ctx, resolver) {
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
        resolver.alias_resolver(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    fn serialize(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    fn tuple_fallback_bytes() -> Vec<u8> {
        // `builtins.tuple[Any, ...]`: the TypeVarTupleType.tuple_fallback
        // Instance with its single arg replaced by Any (typevartuples.py:33).
        serialize(&Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![Type::UninhabitedType { ambiguous: false }],
            last_known_value: None,
            extra_attrs: None,
        })
    }

    fn insert_snapshot(resolver: &mut TypeResolver, fullname: &str, slots: &[(&str, i64)]) {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.to_string(),
            ..Default::default()
        };
        for (name, kind) in slots {
            s.type_vars_with_variance.push((name.to_string(), 0, *kind));
        }
        if slots.iter().any(|&(_, kind)| kind == 2) {
            s.type_var_tuple_fallback = Some(tuple_fallback_bytes());
        }
        resolver.insert(fullname.to_string(), s);
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn any_of(kind: i64) -> Type {
        Type::AnyType {
            type_of_any: kind,
            source_any: None,
            missing_import_name: None,
        }
    }

    // ------------------------------------------------------------------
    // shallow_erase_for_equality
    // ------------------------------------------------------------------

    #[test]
    fn erase_eq_generic_class_single_slot() {
        // list[int] -> list[Any] (AnyType(TypeOfAny.special_form)).
        let mut r = TypeResolver::new();
        insert_snapshot(&mut r, "builtins.list", &[("T", 0)]);
        let out = shallow_erase_for_equality(
            &instance("builtins.list", vec![instance("builtins.int", vec![])]),
            &r,
        )
        .unwrap();
        match out {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.list");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Type::AnyType { type_of_any, .. } => assert_eq!(*type_of_any, 6),
                    other => panic!("expected AnyType(special_form), got {other:?}"),
                }
            }
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn erase_eq_variadic_slot_becomes_unpack_tuple_any() {
        // class C[*Ts, U]; C[int, str, bool] has 3 args but 2 slots, so the
        // erased instance carries exactly 2 args: [Unpack(tuple[Any]), Any].
        let mut r = TypeResolver::new();
        insert_snapshot(&mut r, "mod.C", &[("Ts", 2), ("U", 0)]);
        let three_args = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
            instance("builtins.bool", vec![]),
        ];
        let out = shallow_erase_for_equality(&instance("mod.C", three_args), &r).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert_eq!(args.len(), 2, "slot count, not len(args)");
                match &args[0] {
                    Type::UnpackType { typ, .. } => match &**typ {
                        Type::Instance {
                            type_ref,
                            args: fargs,
                            ..
                        } => {
                            assert_eq!(type_ref, "builtins.tuple");
                            assert_eq!(fargs.len(), 1);
                            match &fargs[0] {
                                Type::AnyType { type_of_any, .. } => {
                                    assert_eq!(*type_of_any, 6)
                                }
                                other => panic!("expected AnyType, got {other:?}"),
                            }
                        }
                        other => panic!("expected tuple fallback Instance, got {other:?}"),
                    },
                    other => panic!("expected UnpackType, got {other:?}"),
                }
                assert!(matches!(args[1], Type::AnyType { type_of_any: 6, .. }));
            }
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn erase_eq_paramspec_slot_erases_to_any() {
        // erased_vars treats ParamSpec like a plain TypeVar: AnyType.
        let mut r = TypeResolver::new();
        insert_snapshot(&mut r, "mod.P", &[("P", 1)]);
        let out = shallow_erase_for_equality(&instance("mod.P", vec![any_of(0)]), &r).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], Type::AnyType { type_of_any: 6, .. }));
            }
            other => panic!("expected Instance, got {other:?}"),
        }
    }

    #[test]
    fn erase_eq_union_erases_item_wise() {
        // list[int] | set[str] -> list[Any] | set[Any], union rebuilt.
        let mut r = TypeResolver::new();
        insert_snapshot(&mut r, "builtins.list", &[("T", 0)]);
        insert_snapshot(&mut r, "builtins.set", &[("T", 0)]);
        let union = Type::UnionType {
            items: vec![
                instance("builtins.list", vec![instance("builtins.int", vec![])]),
                instance("builtins.set", vec![instance("builtins.str", vec![])]),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let out = shallow_erase_for_equality(&union, &r).unwrap();
        match out {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                for item in &items {
                    match item {
                        Type::Instance { args, .. } => {
                            assert_eq!(args.len(), 1);
                            assert!(matches!(args[0], Type::AnyType { type_of_any: 6, .. }));
                        }
                        other => panic!("expected Instance, got {other:?}"),
                    }
                }
            }
            other => panic!("expected UnionType, got {other:?}"),
        }
    }

    #[test]
    fn erase_eq_empty_args_and_non_instance_identity() {
        // Empty-args instances and other shapes pass through unchanged
        // (erasetype.py:498-499, 508).
        let r = TypeResolver::new();
        let bare = instance("builtins.int", vec![]);
        let out = shallow_erase_for_equality(&bare, &r).unwrap();
        match out {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.int");
                assert!(args.is_empty());
            }
            other => panic!("expected Instance, got {other:?}"),
        }
        let uninst = any_of(1);
        let out = shallow_erase_for_equality(&uninst, &r).unwrap();
        assert!(matches!(out, Type::AnyType { type_of_any: 1, .. }));
    }

    #[test]
    fn erase_eq_missing_snapshot_defers() {
        // A generic instance without a snapshot defers (None), so the
        // whole conditional_types call falls back to Python.
        let r = TypeResolver::new();
        let out = shallow_erase_for_equality(
            &instance("mod.Unsnapshotted", vec![instance("builtins.int", vec![])]),
            &r,
        );
        assert!(out.is_none());
    }

    #[test]
    fn erase_eq_variadic_missing_fallback_defers() {
        // A TVT slot whose tuple_fallback blob is unreadable defers.
        let mut r = TypeResolver::new();
        let mut s = TypeInfoSnapshot {
            fullname: "mod.C".to_string(),
            name: "C".to_string(),
            ..Default::default()
        };
        s.type_vars_with_variance.push(("Ts".to_string(), 0, 2));
        r.insert("mod.C".to_string(), s);
        let out = shallow_erase_for_equality(&instance("mod.C", vec![any_of(0)]), &r);
        assert!(out.is_none());
    }
}
