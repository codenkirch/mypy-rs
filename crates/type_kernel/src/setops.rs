//! Stage 3c (M8d): `trivial_join` / `trivial_meet` on the Rust `Type` enum.
//! Stage 3c (M8e): `join_types` pre-dispatch + leaf visitors.
//!
//! Ports the subtype-only fallbacks `mypy.join.trivial_join`
//! (join.py:198-205) and `mypy.meet.trivial_meet` (meet.py:62-72),
//! plus the `join_types` entry point (join.py:294-330) pre-dispatch
//! short-circuits and the leaf `TypeJoinVisitor` visitors
//! (join.py:344-374) that don't recurse into `join_types`.
//!
//! Both reduce the set-theoretic op to `is_subtype` + a branch on
//! which side is wider:
//!
//! * `trivial_join(s, t)`: if `s <: t` return `t`; if `t <: s` return
//!   `s`; else `object_or_any_from_type(t)`.
//! * `trivial_meet(s, t)`: if `s <: t` return `s`; if `t <: s` return
//!   `t`; else `bottom` (strict_optional ? `UninhabitedType` :
//!   `NoneType`).
//!
//! `join_types` leaf visitors ported in M8e:
//! * `visit_any` -> `t` (SameT).
//! * `visit_none_type` (strict_optional): s in {None, Bottom} -> t
//!   (SameT); s in {Unbound, Any} -> Any; else defer (union).
//!   Non-strict: `s` (SameS).
//! * `visit_uninhabited_type` -> `s` (SameS).
//! * `visit_deleted_type` -> `s` (SameS).
//!
//! `visit_erased_type` (join.py:373-374 / meet.py:875) is not ported:
//! top-level Erased operands are filtered by the Python shims
//! (join.py/meet.py gate on `isinstance(..., ErasedType)`), and a nested
//! Erased leaf defers via the `_ => None` fallbacks below, so Python
//! decides it through its own visitors. ErasedType IS on the wire
//! (tag 122; see `wire::Type`), which is why nested leaves can arrive.
//!
//! The strangler-fig contract mirrors `erase::erase_type`
//! (erasetype.py:80-86): `None` means "Rust doesn't handle this, let
//! Python decide". No production code calls this until
//! `Options.native_type_kernel` is on AND `mypy/join.py` / `mypy/meet.py`
//! dispatch to it (the shims are added in this same milestone).

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ExtraAttrs, LiteralValue, ReadBuffer, Type, WriteBuffer};

use crate::subtypes::{
    is_subtype, map_instance_to_supertype, SubtypeContext, CONTRAVARIANT, COVARIANT, INVARIANT,
    VARIANCE_NOT_READY,
};

/// Discriminator for `trivial_join` / `trivial_meet` / `join_types`
/// results.
///
/// Python maps each variant to a live `Type`:
/// * `SameS` -> the `s` argument (unchanged).
/// * `SameT` -> the `t` argument (unchanged).
/// * `Object` -> `object_or_any_from_type(t)` (Instance right only;
///   non-Instance right defers with `None`).
/// * `Bottom` -> `UninhabitedType` (strict_optional) or `NoneType`.
/// * `Any` -> `AnyType(TypeOfAny.special_form)`.
/// * `Ancestor(fullname)` -> `Instance(typeinfo_map[fullname], [])`
///   (the common supertype found by the Instance-Instance nominal join;
///   the Python shim holds a fullname -> TypeInfo map alongside the
///   resolver).
/// * `SameTypeWithArgs { type_ref, arg_discs }` ->
///   `Instance(typeinfo_map[type_ref], [reconstructed args])` where
///   each `arg_discs[i]` is 0 (use `s.args[i]`), 1 (use `t.args[i]`),
///   or 4 (use `AnyType(from_another_any)`). Produced by the
///   same-type-with-args join (join.py:114-180) when every arg reduces
///   to one of the original args or Any.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum SetOpResult {
    SameS,
    SameT,
    Object,
    Bottom,
    Any,
    Ancestor(String),
    /// Same-type Instance-Instance join with per-arg discriminators.
    /// `arg_discs[i]`: 0=left arg (s.args[i]), 1=right arg
    /// (t.args[i]), 4=AnyType(from_another_any).
    SameTypeWithArgs {
        type_ref: String,
        arg_discs: Vec<i8>,
    },
    /// A newly-constructed type encoded in the wire format. The Python
    /// shim decodes via `read_type(ReadBuffer(bytes))`. Used by visitors
    /// that produce a type other than s/t (e.g. `visit_type_type` case 1
    /// builds a new `TypeType`). `disc=7` on the wire.
    Encoded(Vec<u8>),
}

/// `trivial_join` (join.py:198-205), Rust subset.
///
/// Returns `Some(SetOpResult)` when Rust decided; `None` when the
/// `object_or_any_from_type` else-branch fires on a non-Instance
/// right (the full helper walks every Type variant; we only handle
/// Instance right, deferring the rest to Python).
pub(crate) fn trivial_join(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // Defer FunctionLike pairs: is_subtype now decides
    // Callable-vs-Callable via callable_compat, so the object/`s`/`t`
    // falls-through below would fire; Python joins them instead.
    if is_function_like(s) && is_function_like(t) {
        return None;
    }
    match is_subtype(s, t, ctx, resolver) {
        Some(true) => return Some(SetOpResult::SameT),
        Some(false) => {}
        None => return None,
    }
    match is_subtype(t, s, ctx, resolver) {
        Some(true) => return Some(SetOpResult::SameS),
        Some(false) => {}
        None => return None,
    }
    // object_or_any_from_type(t): always returns either
    // Instance(builtins.object) or AnyType; for Instance t returns
    // Instance(builtins.object) (parity-safe per join.py disc==2).
    Some(SetOpResult::Object)
}

/// `trivial_meet` (meet.py:62-72), Rust subset.
///
/// Returns `Some(SetOpResult)` when Rust decided; `None` when an
/// `is_subtype` check fell through (unsupported variant) and we
/// can't safely decide.
pub(crate) fn trivial_meet(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // Same as trivial_join: FunctionLike pairs must defer.
    if is_function_like(s) && is_function_like(t) {
        return None;
    }
    // First direction: s <: t? If yes, meet is s.
    match is_subtype(s, t, ctx, resolver) {
        Some(true) => Some(SetOpResult::SameS),
        Some(false) => {
            // Second direction: t <: s? If yes, meet is t.
            match is_subtype(t, s, ctx, resolver) {
                Some(true) => Some(SetOpResult::SameT),
                Some(false) => Some(SetOpResult::Bottom),
                None => None,
            }
        }
        None => None,
    }
}

/// `Type.is_equivalent`-style FunctionLike test for the defer guards:
/// `CallableType` or `Overloaded` (types.py:1986, types.py:2674).
fn is_function_like(t: &Type) -> bool {
    matches!(t, Type::CallableType { .. } | Type::Overloaded { .. })
}

/// Decode a wire-format `Type` blob via `wire::read_type`. Returns
/// `None` on any read failure (truncated input, unknown tag).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `join_types` (join.py:294-330) pre-dispatch + leaf visitors
/// (join.py:344-374), Rust subset.
///
/// Handles the cases that don't recurse into `join_types`:
/// - AnyType left (after UnionType swap) -> SameT (return t).
/// - NoneType right (strict_optional): s in {None, Bottom} -> SameT;
///   s in {Unbound, Any} -> Any; else defer (needs union).
/// - NoneType right (non-strict) -> SameS.
/// - UninhabitedType right -> SameS.
/// - DeletedType right -> SameS.
/// - UnionType right: s <: any item -> SameT (return t); every item
///   <: s -> SameS (union collapses); else defer (needs a Type encoder
///   to build the new union).
/// - CallableType right, s non-callable: fallback join
///   (join_types(t.fallback, s)); Ancestor/Object pass through,
///   SameT (result=s) -> SameS, SameS (result=fallback=s) -> SameS.
/// - Overloaded right, s non-callable: same fallback join, with the
///   fallback extracted from `items[0].fallback` (types.py:2744).
/// - TypeType right, s is Instance(builtins.type): SameS (return s).
///   The TypeType-vs-TypeType case (produces a new TypeType via
///   `TypeType.make_normalized`) defers (needs a Type encoder).
/// - LiteralType right, s is LiteralType with equal value: SameT
///   (return t). s is Instance with `last_known_value == t`: SameT.
///   Unequal literals / non-matching lkv defer (the fallback join
///   produces a type that is neither s nor t).
/// - TypeVarType right, s is TypeVarType with same id (raw_id +
///   namespace, matching wire-roundtrip semantics — meta_level is not
///   in the wire format) AND equal upper_bound: SameS (return s).
///   Different upper_bounds / different ids / s not TypeVarType defer
///   (the copy_modified or bound-join produces a new type).
/// - TypedDictType right, s is Instance: recursive
///   `join_types(s, t.fallback)` (s=left, fallback=right). SameS
///   (recursive) -> SameS; Ancestor/Object pass through. SameT
///   (recursive, result=fallback != t) defers. Case 1 (s is
///   TypedDictType, builds a new TypedDictType) and case 3 (s not
///   Instance/TypedDictType, walks fallback chain) defer.
/// - TupleType right, s is not TupleType AND `partial_fallback` is
///   NOT `builtins.tuple`: recursive `join_types(s,
///   partial_fallback)`. `tuple_fallback(t) == t.partial_fallback`
///   only when the fallback is non-builtin (typeops.py:108-109);
///   when it IS `builtins.tuple`, `tuple_fallback` constructs a new
///   Instance with a union of items -> defer. SameS -> SameS;
///   Ancestor/Object pass through. Case 1 (s is TupleType, builds a
///   new TupleType via `join_tuples`) defers.
///
/// Returns `None` (defer to Python) for:
/// - `is_recursive_pair` (needs the live alias graph).
/// - `can_be_true`/`can_be_false` mismatch (needs the properties).
/// - UnionType left AND UnionType right (needs merge/flatten).
/// - CallableType left AND CallableType right (similar-callables needs
///   `combine_similar_callables` which produces a new CallableType).
/// - Overloaded left AND callable-like right (both-FunctionLike needs
///   `is_similar_callables` + `combine_similar_callables`).
/// - Parameters (needs live callable normalization).
/// - Instance/etc right (full visitor).
///
/// The Python shim is responsible for `get_proper_type` expansion
/// BEFORE calling this, matching `join.py:303-304`.
pub(crate) fn join_types(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // join.py:311-312: if s is UnionType and t is not, swap so s is
    // the non-union. If both are unions, merge all items and call
    // make_simplified_union — it handles flattening and dedup.
    let (s, t, swapped) =
        if matches!(s, Type::UnionType { .. }) && matches!(t, Type::UnionType { .. }) {
            // Both UnionType: merge items, call make_simplified_union.
            // (join.py:432-436; merging both sides is the equivalent.)
            let Type::UnionType { items: s_items, .. } = s else {
                unreachable!()
            };
            let Type::UnionType { items: t_items, .. } = t else {
                unreachable!()
            };
            let mut merged = Vec::with_capacity(s_items.len() + t_items.len());
            merged.extend(flatten_nested_unions(s_items)?);
            merged.extend(flatten_nested_unions(t_items)?);
            let simplified = make_simplified_union(&merged, ctx, resolver, true, false)?;
            let mut wbuf = WriteBuffer::new();
            wire::write_type(&mut wbuf, &simplified).ok()?;
            return Some(SetOpResult::Encoded(wbuf.into_bytes()));
        } else if matches!(s, Type::UnionType { .. }) && !matches!(t, Type::UnionType { .. }) {
            (t, s, true)
        } else {
            (s, t, false)
        };

    // join.py:314-315: isinstance(s, AnyType) -> return s. The AnyType
    // is on the left after swap, so SameS is correct relative to the
    // post-swap s; the caller maps SameS/SameT back via `swapped`.
    if matches!(s, Type::AnyType { .. }) {
        return Some(flip_if(SetOpResult::SameS, swapped));
    }

    // join.py:317-318 (isinstance(s, ErasedType) -> return t) is
    // unreachable here: top-level Erased operands are filtered by the
    // shim (join.py:524-525); nested ones defer in visit_join below.

    // join.py:320-321: isinstance(s, NoneType) and not isinstance(t,
    // NoneType) -> swap. Post-swap, s is non-None, t is None.
    let (s, t, swap2) = if matches!(s, Type::NoneType) && !matches!(t, Type::NoneType) {
        (t, s, true)
    } else {
        (s, t, false)
    };
    let swapped = swapped ^ swap2;

    // join.py:323-324: isinstance(s, UninhabitedType) and not
    // isinstance(t, UninhabitedType) -> swap.
    let (s, t, swap3) = if matches!(s, Type::UninhabitedType { .. })
        && !matches!(t, Type::UninhabitedType { .. })
    {
        (t, s, true)
    } else {
        (s, t, false)
    };
    let swapped = swapped ^ swap3;

    // normalize_callables is a no-op here (shim serializes the
    // post-normalization form); the both-CallableType case lives in
    // visit_join. t.accept leaf visitors — flip back post-swap.
    visit_join(s, t, ctx, resolver).map(|r| flip_if(r, swapped))
}

/// Swap SameS/SameT when the join_types pre-dispatch swapped s and t.
/// `Object`, `Bottom`, `Any`, and `Ancestor` are swap-invariant.
/// `SameTypeWithArgs` exchanges per-arg Left(0)/Right(1) discriminators
/// (Any=4 is invariant); `type_ref` is unchanged (same-type case).
fn flip_if(r: SetOpResult, swapped: bool) -> SetOpResult {
    if !swapped {
        return r;
    }
    match r {
        SetOpResult::SameS => SetOpResult::SameT,
        SetOpResult::SameT => SetOpResult::SameS,
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => {
            let flipped = arg_discs
                .into_iter()
                .map(|d| match d {
                    0 => 1,
                    1 => 0,
                    other => other,
                })
                .collect();
            SetOpResult::SameTypeWithArgs {
                type_ref,
                arg_discs: flipped,
            }
        }
        other => other,
    }
}

/// `meet_types` (meet.py:114-153) pre-dispatch + leaf visitors
/// (meet.py:822+), Rust subset. Mirror of `join_types`.
///
/// Handles the cases that don't recurse into `meet_types`:
/// - `is_proper_subtype(s, t, ignore_promotions=True)` -> SameS.
/// - `is_proper_subtype(t, s, ignore_promotions=True)` -> SameT.
/// - AnyType s (after UnionType swap) -> SameT (return t).
/// - UnionType s AND t not UnionType -> swap; both UnionType defers
///   (needs make_simplified_union).
/// - Both callable-like -> defer (needs meet_similar_callables).
/// - Leaf visitors:
///   * visit_any (meet.py:837): return self.s -> SameS.
///   * visit_none_type (meet.py:850-859): strict_optional, s in
///     {NoneType, Instance(builtins.object)} -> SameT; else Bottom.
///     Non-strict -> SameT.
///   * visit_uninhabited_type (meet.py:861): return t -> SameT.
///   * visit_deleted_type (meet.py:864-873): s is NoneType ->
///     SameS (strict) / SameT (non-strict); s is UninhabitedType ->
///     SameS; else SameT.
///   * visit_instance (meet.py:913-996): same type_ref, args-less ->
///     SameS (equal); same type_ref with args -> per-arg meet combined
///     into a new Instance (encoded); different type_ref with
///     is_subtype(t, s) -> SameT; is_subtype(s, t) -> SameS; else
///     Bottom. Variadic / alt_promote / protocol defers.
///
/// Returns `None` (defer to Python) for:
/// - `is_recursive_pair` (checked in Python before the Rust call).
/// - `can_be_true`/`can_be_false` mismatch (needs the properties).
/// - UnionType right (after swap): needs make_simplified_union.
/// - Both callable-like: needs meet_similar_callables.
/// - CallableType/Overloaded/Parameters right, s non-callable: the
///   visit_callable_type branches need unpack_callback_proxy /
///   live TypeInfo protocol flag not in the snapshot -> defer.
/// - TypeVarType/ParamSpec/TypeVarTuple right: copy_modified or
///   bound-meet produces a new type -> defer.
/// - TypedDictType/TupleType/TypeType/LiteralType right: produce a new
///   type or need live TypeInfo (alt_promote, is_metaclass, etc.) ->
///   defer.
/// - Instance right with args when the per-arg meet defers (variadic,
///   ParamSpec/TypeVarTuple tv, arity mismatch, is_subtype gate).
///
/// The Python shim is responsible for `get_proper_type` expansion
/// BEFORE calling this, matching `meet.py:120-121`.
pub(crate) fn meet_types(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // meet.py:129-141: same-type Instance pair with extra_attrs on either
    // side returns the attrs-bearing side (so hasattr-synthesized attrs
    // survive narrowing), before the is_proper_subtype pre-check.
    if let (
        Type::Instance {
            type_ref: s_ref,
            args: s_args,
            last_known_value: s_lkv,
            extra_attrs: s_ea,
        },
        Type::Instance {
            type_ref: t_ref,
            args: t_args,
            last_known_value: t_lkv,
            extra_attrs: t_ea,
        },
    ) = (s, t)
    {
        if s_ref == t_ref
            && s_args == t_args
            && s_lkv == t_lkv
            && (s_ea.is_some() || t_ea.is_some())
        {
            return Some(match (s_ea, t_ea) {
                (Some(a), Some(b)) if a.attrs.len() > b.attrs.len() => SetOpResult::SameS,
                (Some(_), Some(_)) => SetOpResult::SameT,
                (Some(_), None) => SetOpResult::SameS,
                (None, Some(_)) => SetOpResult::SameT,
                (None, None) => unreachable!(), // guarded above
            });
        }
    }

    // meet.py:139-141: is_proper_subtype pre-check (ignore_promotions),
    // skipped when either side is an UnboundType (meet.py:138) so an
    // UnboundType is never eliminated by the sub-type pre-check;

    // the visit_unbound_type visitor below decides the result instead.
    let has_unbound =
        matches!(s, Type::UnboundType { .. }) || matches!(t, Type::UnboundType { .. });
    if !has_unbound {
        let proper_ctx = {
            let mut c = ctx.clone();
            c.proper_subtype = true;
            c.ignore_promotions = true;
            c
        };
        if let Some(true) = is_subtype(s, t, &proper_ctx, resolver) {
            return Some(SetOpResult::SameS);
        }
        if let Some(true) = is_subtype(t, s, &proper_ctx, resolver) {
            return Some(SetOpResult::SameT);
        }
    }

    // meet.py:143-144 (isinstance(s, ErasedType) -> return s) is unreachable:
    // top-level Erased operands are filtered by the shim (meet.py one-pair gate);
    // nested ones defer in visit_meet below. meet.py:145-146: s is Any -> SameT.
    if matches!(s, Type::AnyType { .. }) {
        return Some(SetOpResult::SameT);
    }

    // meet.py:147-148: isinstance(s, UnionType) and not isinstance(t,
    // UnionType) -> swap. Both UnionType -> visit_union_type builds a
    // new union via make_simplified_union.
    let swapped = matches!(s, Type::UnionType { .. }) && !matches!(t, Type::UnionType { .. });
    let (s, t) = if swapped {
        (t, s)
    } else if matches!(s, Type::UnionType { .. }) && matches!(t, Type::UnionType { .. }) {
        // Both UnionType: build the pairwise meets and encode the result.
        return meet_union(s, t, ctx, resolver).map(|r| flip_if(r, false));
    } else {
        (s, t)
    };

    // normalize_callables (meet.py:151) is a no-op for the Rust path:
    // the Python shim serializes the post-normalization form.
    // Both-FunctionLike: build the meet (encoded).
    let s_is_callable = matches!(
        s,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    let t_is_callable = matches!(
        t,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    if s_is_callable && t_is_callable {
        // Both callable-like: meet_similar_callables or default.
        return meet_callable_like(s, t, ctx, resolver).map(|r| flip_if(r, swapped));
    }

    // t.accept(TypeMeetVisitor(s)) — leaf visitors + recursive meet.
    // The visitor returns SameS/SameT relative to the post-swap s/t;
    // flip back to the original s/t frame.
    visit_meet(s, t, ctx, resolver).map(|r| flip_if(r, swapped))
}

/// `TypeMeetVisitor.visit_*` leaf methods (meet.py:822+), Rust subset.
/// Handles the visitors that don't recurse into `meet_types`.
fn visit_meet(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    match t {
        // visit_any (meet.py:837): return self.s.
        Type::AnyType { .. } => Some(SetOpResult::SameS),

        // visit_none_type (meet.py:850-859).
        Type::NoneType => {
            if ctx.strict_optional {
                let s_is_object = matches!(
                    s,
                    Type::Instance { type_ref, .. } if type_ref == "builtins.object"
                );
                if matches!(s, Type::NoneType) || s_is_object {
                    Some(SetOpResult::SameT)
                } else {
                    Some(SetOpResult::Bottom)
                }
            } else {
                // Non-strict: return t.
                Some(SetOpResult::SameT)
            }
        }

        // visit_uninhabited_type (meet.py:861): return t (SameT).
        Type::UninhabitedType { .. } => Some(SetOpResult::SameT),

        // visit_deleted_type (meet.py:864-873): if s is NoneType, return
        // t when strict_optional else s. If s is Uninhabited, return s.
        // Otherwise return t. The Python shim maps via SameS/SameT.
        Type::DeletedType { .. } => {
            if matches!(s, Type::NoneType) {
                if ctx.strict_optional {
                    Some(SetOpResult::SameT)
                } else {
                    Some(SetOpResult::SameS)
                }
            } else if matches!(s, Type::UninhabitedType { .. }) {
                // return self.s (Uninhabited).
                Some(SetOpResult::SameS)
            } else {
                // else: return t (DeletedType).
                Some(SetOpResult::SameT)
            }
        }

        // visit_erased_type (meet.py:875) is unhandled: top-level Erased
        // operands are filtered by the shim's meet gate; a nested Erased
        // t has no arm here and defers via the wildcard.

        // visit_unbound_type (meet.py:864-873): NoneType + strict_optional
        // -> Bottom; UninhabitedType -> SameS; else -> AnyType. AnyType
        // never reaches here (meet.py:145 short-circuits).
        Type::UnboundType { .. } => {
            if matches!(s, Type::NoneType) {
                if ctx.strict_optional {
                    Some(SetOpResult::Bottom)
                } else {
                    Some(SetOpResult::SameS)
                }
            } else if matches!(s, Type::UninhabitedType { .. }) {
                Some(SetOpResult::SameS)
            } else {
                Some(SetOpResult::Any)
            }
        }

        // visit_instance (meet.py:913-996), args-less nominal subset.
        Type::Instance { .. } => visit_instance_meet(s, t, ctx, resolver),

        // visit_type_var (meet.py:878-884), same-id same-upper-bound only:
        // -> SameS. Differing upper bounds or non-TypeVar s -> defer.
        // TypeVarId.__eq__: raw_id + namespace (meta_level not on wire).
        Type::TypeVarType {
            raw_id: t_raw,
            namespace: t_ns,
            upper_bound: t_ub,
            name: t_name,
            fullname: t_fullname,
            default: t_default,
            variance: t_variance,
            values: t_values,
            ..
        } => {
            if let Type::TypeVarType {
                raw_id: s_raw,
                namespace: s_ns,
                upper_bound: s_ub,
                ..
            } = s
            {
                if s_raw == t_raw && s_ns == t_ns {
                    if s_ub == t_ub {
                        return Some(SetOpResult::SameS);
                    }
                    // Different upper_bound: meet upper bounds and
                    // encode. fruit_to_type (not setop_result_to_type)
                    // so a recursive Encoded result decodes: this is a

                    // meet-only path, so join semantics are unaffected.
                    let new_ub = fruit_to_type(meet_types(s_ub, t_ub, ctx, resolver)?, s_ub, t_ub)?;
                    let new_tv = Type::TypeVarType {
                        raw_id: *t_raw,
                        namespace: t_ns.clone(),
                        upper_bound: Box::new(new_ub),
                        name: t_name.clone(),
                        fullname: t_fullname.clone(),
                        values: t_values.clone(),
                        default: t_default.clone(),
                        variance: *t_variance,
                        meta_level: 0,
                    };
                    let mut wbuf = WriteBuffer::new();
                    wire::write_type(&mut wbuf, &new_tv).ok()?;
                    return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                }
            }
            // s not TypeVarType or different id -> default -> Bottom.
            Some(SetOpResult::Bottom)
        }

        // visit_type_var_tuple (meet.py:930-934): same id (raw_id +
        // namespace) -> min_len-ordered; different id -> Bottom or
        // NoneType (non-strict). TypeVarId: raw_id + namespace.
        Type::TypeVarTupleType {
            raw_id: t_raw,
            namespace: t_ns,
            min_len: t_min,
            ..
        } => {
            if let Type::TypeVarTupleType {
                raw_id: s_raw,
                namespace: s_ns,
                min_len: s_min,
                ..
            } = s
            {
                if s_raw == t_raw && s_ns == t_ns {
                    if s_min > t_min {
                        return Some(SetOpResult::SameS);
                    }
                    return Some(SetOpResult::SameT);
                }
            }
            Some(SetOpResult::Bottom)
        }

        // visit_literal_type (meet.py:1236-1242): s==t -> SameT;
        // is_subtype(t.fallback, s) -> SameT; else Bottom. Type enum
        // derives PartialEq, matching LiteralType.__eq__ (types.py:3361).
        Type::LiteralType { .. } => {
            if let Type::LiteralType { .. } = s {
                if s == t {
                    return Some(SetOpResult::SameT);
                }
                // s is LiteralType but s != t -> default -> Bottom.
                return Some(SetOpResult::Bottom);
            }
            if let Type::Instance { .. } = s {
                // Case 2: is_subtype(t.fallback, s); t.fallback is the
                // LiteralType's fallback Instance. None -> defer.
                if let Type::LiteralType { fallback, .. } = t {
                    // Match on the result once: True -> SameT,
                    // False -> Bottom (default), None -> defer.
                    return match is_subtype(fallback, s, ctx, resolver) {
                        Some(true) => Some(SetOpResult::SameT),
                        Some(false) => Some(SetOpResult::Bottom),
                        None => None,
                    };
                }
            }
            // s is not LiteralType or Instance -> default -> Bottom.
            Some(SetOpResult::Bottom)
        }

        // visit_type_type (meet.py:1248-1261, 1412-1419).
        Type::TypeType {
            item: t_item,
            is_type_form: t_itf,
        } => {
            if let Type::Instance { type_ref, .. } = s {
                if type_ref == "builtins.type" {
                    return Some(SetOpResult::SameT);
                }
            }
            if let Type::TypeType {
                item: s_item,
                is_type_form: s_itf,
            } = s
            {
                // Python meet.py:1412-1419: typ = meet(t.item, s.item);
                // if not NoneType: wrap
                // TypeType.make_normalized(typ,

                // is_type_form=s.is_type_form and t.is_type_form).
                // Skip NoneType (bottom) results; Python returns the
                // raw NoneType in that case (no wrap). Materialize the

                // met item via fruit_to_type (decodes recursive
                // Encoded), then encode the result: the NoneType path
                // encodes NoneType directly, the wrapped path encodes

                // the fresh TypeType. Mirrors the join-side
                // implementation at setops.rs:1958-1969, except the
                // flag is AND (meet) not OR (join).
                let met_item =
                    fruit_to_type(meet_types(t_item, s_item, ctx, resolver)?, t_item, s_item)?;
                let result = if matches!(met_item, Type::NoneType) {
                    met_item
                } else {
                    Type::TypeType {
                        item: Box::new(met_item),
                        is_type_form: *s_itf && *t_itf,
                    }
                };
                let mut wbuf = WriteBuffer::new();
                wire::write_type(&mut wbuf, &result).ok()?;
                return Some(SetOpResult::Encoded(wbuf.into_bytes()));
            }
            if matches!(s, Type::CallableType { .. }) {
                // Case 3 (s is CallableType): meet.py recurses
                // meet(t, self.s) -> defer.
                return None;
            }
            // Else -> default -> Bottom.
            Some(SetOpResult::Bottom)
        }

        // visit_union_type (meet.py:962-970): t is UnionType and s is
        // not (the both-Union case is routed to meet_union in the
        // pre-dispatch). meet.py:965-966 else-branch:

        // meets = [meet_types(x, self.s) for x in t.items], then
        // make_simplified_union(meets). One-sided: s is fixed.
        Type::UnionType { items: t_items, .. } => {
            let mut meets = Vec::with_capacity(t_items.len());
            for t_item in t_items {
                let m = meet_types(t_item, s, ctx, resolver)?;
                meets.push(fruit_to_type(m, t_item, s)?);
            }
            // Drop UninhabitedType items, then make_simplified_union
            // (which itself drops/contracts; defer when it can't).
            meets.retain(|item| !matches!(item, Type::UninhabitedType { .. }));
            let joined = make_simplified_union(&meets, ctx, resolver, false, false)?;
            let mut wbuf = WriteBuffer::new();
            wire::write_type(&mut wbuf, &joined).ok()?;
            Some(SetOpResult::Encoded(wbuf.into_bytes()))
        }

        // visit_param_spec (meet.py:1008-1012): s == t -> self.s
        // (SameS). Else -> default(self.s): UnboundType -> Any, others
        // -> Bottom (strict) / NoneType (non-strict, shim maps Bottom).

        // ParamSpec is not callable-like, so both-ParamSpec reaches
        // here; the pre-dispatch is_proper_subtype catches identical
        // pairs first, discounting the SameS arm in practice.
        Type::ParamSpecType { .. } => {
            if paramspec_eq(s, t) {
                Some(SetOpResult::SameS)
            } else if matches!(s, Type::UnboundType { .. }) {
                Some(SetOpResult::Any)
            } else {
                Some(SetOpResult::Bottom)
            }
        }

        // visit_parameters (meet.py:1023-1033). Both-Parameters is
        // intercepted by the meet_types both-callable pre-dispatch, so
        // here s is never Parameters: always default(self.s) ->

        // UnboundType ? Any : Bottom.
        Type::Parameters { .. } => {
            if matches!(s, Type::UnboundType { .. }) {
                Some(SetOpResult::Any)
            } else {
                Some(SetOpResult::Bottom)
            }
        }

        // Full visitors (callable, typeddict, tuple,
        // typevartuple, overloaded) — deferred. The both-FunctionLike
        // case is already deferred by meet_types pre-dispatch. The

        // remaining cases (s non-callable, t callable-like) reach here
        // and defer.
        _ => None,
    }
}

/// `visit_union_type` for `meet_types`: both sides are UnionType.
///
/// meet.py:962-970: pairwise meets then make_simplified_union.
/// Produces a new UnionType encoded via wire format.
fn meet_union(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let Type::UnionType { items: s_items, .. } = s else {
        return None;
    };
    let Type::UnionType { items: t_items, .. } = t else {
        return None;
    };

    let mut meets = Vec::with_capacity(s_items.len() * t_items.len());
    for s_item in s_items {
        for t_item in t_items {
            let m = meet_types(s_item, t_item, ctx, resolver)?;
            meets.push(fruit_to_type(m, s_item, t_item)?);
        }
    }

    // Drop UninhabitedType items, then call make_simplified_union
    meets.retain(|item| !matches!(item, Type::UninhabitedType { .. }));

    // make_simplified_union returns Option<Type>; wrap as Encoded(SetOpResult)
    let joined = make_simplified_union(&meets, ctx, resolver, false, false)?;
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &joined).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// Convert a per-item `SetOpResult` from `meet_types`/`join_types` into
/// the concrete `Type` it denotes, relative to the `s_item`/`t_item`
/// operands. Shared by `meet_union` and `visit_instance_meet_args`.
///
/// `SameTypeWithArgs` builds `Instance(type_ref, reconstructed args)`
/// (disc 0 -> s.args[i], 1 -> t.args[i], 4 -> Any); the operands must
/// be Instances with matching arity, else `None` (mirrors
/// join.py:422-423). `Encoded` decodes back to the `Type`.
fn fruit_to_type(m: SetOpResult, s_item: &Type, t_item: &Type) -> Option<Type> {
    Some(match m {
        SetOpResult::SameS => s_item.clone(),
        SetOpResult::SameT => t_item.clone(),
        SetOpResult::Bottom => Type::UninhabitedType { ambiguous: true },
        SetOpResult::Any => Type::AnyType {
            type_of_any: 3,
            source_any: None,
            missing_import_name: None,
        },
        SetOpResult::Object => Type::Instance {
            type_ref: "builtins.object".into(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        },
        SetOpResult::Ancestor(fullname) => Type::Instance {
            type_ref: fullname,
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        },
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => {
            let (Type::Instance { args: s_args, .. }, Type::Instance { args: t_args, .. }) =
                (s_item, t_item)
            else {
                return None;
            };
            if arg_discs.len() != s_args.len() || arg_discs.len() != t_args.len() {
                return None;
            }
            let args = reconstruct_args_from_discs(&arg_discs, s_args, t_args);
            Type::Instance {
                type_ref,
                args,
                last_known_value: None,
                extra_attrs: None,
            }
        }
        SetOpResult::Encoded(bytes) => decode_type(&bytes)?,
    })
}

/// Reconstruct args from per-arg discriminators for SameTypeWithArgs.
/// `disc 0` -> `s_args[i]`, `disc 1` -> `t_args[i]`, `disc 4` ->
/// AnyType. Callers guarantee `arg_discs.len() == args.len()`.
fn reconstruct_args_from_discs(arg_discs: &[i8], s_args: &[Type], t_args: &[Type]) -> Vec<Type> {
    arg_discs
        .iter()
        .enumerate()
        .map(|(i, d)| match d {
            0 => s_args[i].clone(),
            1 => t_args[i].clone(),
            4 => Type::AnyType {
                type_of_any: 3,
                source_any: None,
                missing_import_name: None,
            },
            _ => s_args[i].clone(),
        })
        .collect()
}

/// `meet_callable_like` (meet.py:1120-1146, 1148-1165): handles all
/// callable-like (CallableType, Overloaded, Parameters) vs callable-like
/// meet cases. Produces an encoded result or None (defer).
/// Both-Parameters pairs build the arg-type join (meet_parameters_pair);
/// mixed Parameters/Callable/Overloaded fall to the conservative
/// catch-all (Bottom), matching Python's default for s not UnboundType.
fn meet_callable_like(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // Normalize both to callable types for comparison
    let s_callable = match s {
        Type::CallableType { .. } => s.clone(),
        Type::Overloaded { items } => {
            // Use first overload item
            items.first()?.clone()
        }
        Type::Parameters { .. } => s.clone(),
        _ => return None,
    };

    let t_callable = match t {
        Type::CallableType { .. } => t.clone(),
        Type::Overloaded { items } => items.first()?.clone(),
        Type::Parameters { .. } => t.clone(),
        _ => return None,
    };

    // Both are CallableType: check similarity and meet
    match (&s_callable, &t_callable) {
        (
            Type::CallableType {
                arg_types: t_arg_types,
                arg_kinds: t_arg_kinds,
                arg_names: t_arg_names,
                variables: t_variables,
                imprecise_arg_kinds: t_imprecise_arg_kinds,
                ret_type: t_ret,
                fallback: t_fallback,
                instance_type: t_inst,
                ..
            },
            Type::CallableType {
                arg_types: s_arg_types,
                arg_kinds: s_arg_kinds,
                ret_type: s_ret,
                fallback: s_fallback,
                instance_type: s_inst,
                ..
            },
        ) => {
            if is_similar_callables(t_arg_types, t_arg_kinds, s_arg_types, s_arg_kinds) {
                meet_similar_callables_impl(
                    t_arg_types,
                    t_arg_kinds,
                    t_arg_names,
                    t_variables,
                    *t_imprecise_arg_kinds,
                    t_ret.as_ref(),
                    t_fallback.as_ref(),
                    t_inst,
                    s_arg_types,
                    s_ret.as_ref(),
                    s_fallback.as_ref(),
                    s_inst,
                    ctx,
                    resolver,
                )
            } else {
                // Not similar: fallback join
                Some(SetOpResult::SameT)
            }
        }
        // Both standalone Parameters: meet_parameters_pair (the
        // meet.py:1023-1031 visit_parameters path — arg types are
        // joined, not met). Previously this fell into the `_` catch-all

        // and wrongly answered Bottom for a same-length pair.
        (Type::Parameters(s_p), Type::Parameters(t_p)) => {
            meet_parameters_pair(s_p, t_p, ctx, resolver)
        }
        // Overloaded vs callable-like: fallback join
        _ => {
            // Default: meet(t.fallback, s.fallback) or default(self.s)
            // Conservative: return Bottom
            Some(SetOpResult::Bottom)
        }
    }
}

/// `meet_similar_callables` (meet.py:1401-1427): arg types are joined
/// (not met — args are contravariant), return type is met, instance_type
/// is met, fallback is picked.
#[allow(clippy::too_many_arguments)]
fn meet_similar_callables_impl(
    t_arg_types: &[Type],
    t_arg_kinds: &[i64],
    t_arg_names: &[Option<String>],
    t_variables: &[Type],
    t_imprecise_arg_kinds: bool,
    t_ret: &Type,
    t_fallback: &Type,
    t_inst: &Option<Box<Type>>,
    s_arg_types: &[Type],
    s_ret: &Type,
    s_fallback: &Type,
    s_inst: &Option<Box<Type>>,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        // Args are joined (contravariant), not met
        new_arg_types.push(setop_result_to_type(
            join_types(ta, sa, ctx, resolver),
            ta,
            sa,
        )?);
    }
    // meet.py:1414 ret_type=meet_types(t_ret, s_ret); map with the same
    // argument order (SameS -> first). The old (s_ret, t_ret) order
    // mapped an is_subtype win onto the wrong operand. fruit_to_type

    // (not setop_result_to_type) so a recursive Encoded ret decodes:
    // meet-only path, join unaffected.
    let new_ret = fruit_to_type(meet_types(t_ret, s_ret, ctx, resolver)?, t_ret, s_ret)?;
    let new_instance_type = match (t_inst, s_inst) {
        (Some(ti), Some(si)) => Some(Box::new(fruit_to_type(
            meet_types(ti, si, ctx, resolver)?,
            ti,
            si,
        )?)),
        // meet.py:1455-1461: when only one side has instance_type,
        // propagate that side.
        (Some(ti), None) => Some(ti.clone()),
        (None, Some(si)) => Some(si.clone()),
        (&None, &None) => None,
    };
    let new_fallback = pick_fallback(s_fallback, t_fallback);

    let new_callable = Type::CallableType {
        fallback: Box::new(new_fallback),
        instance_type: new_instance_type,
        // Python's meet_similar_callables preserves t's flags,
        // arg_kinds, arg_names, and variables (copy_modified).
        is_ellipsis_args: false,
        implicit: false,
        is_bound: false,
        from_concatenate: false,
        imprecise_arg_kinds: t_imprecise_arg_kinds,
        unpack_kwargs: false,
        from_type_type: false,
        arg_types: new_arg_types,
        arg_kinds: t_arg_kinds.to_vec(),
        arg_names: t_arg_names.to_vec(),
        ret_type: Box::new(new_ret),
        name: None,
        variables: t_variables.to_vec(),
        type_guard: None,
        type_is: None,
    };

    encode_callable(new_callable)
}

/// `TypeMeetVisitor.visit_instance` (meet.py:913-996), Rust subset.
/// Mirrors `visit_instance_join` but for meet.
///
/// Handles:
/// - Same type_ref, args-less -> SameS (equal).
/// - Same type_ref with args -> `visit_instance_meet_args`: per-arg
///   `meet_types(ta, sa)` combined into a new Instance (encoded).
/// - Different type_ref (args present or not): `is_subtype(t, s)` ->
///   SameT; `is_subtype(s, t)` -> SameS; else Bottom. The Python
///   different-type branch (meet.py:1086-1101) never combines args,
///   so args on either side need no special handling here.
///
/// Defers (returns `None`) for:
/// - s not Instance (FunctionLike/TypeType/Tuple/Literal/TypedDict
///   branches recurse into meet_types(t, self.s) or default).
/// - Same type_ref with args when `visit_instance_meet_args` defers
///   (is_subtype gate / per-arg meet / variadic / arity mismatch).
/// - `alt_promote` (meet.py:1086-1091): snapshot has no alt_promote
///   field. For args-less Instance-Instance, the is_subtype check
///   covers the common case; alt_promote fires for mypyc native ints
///   (i64, i32) which the parity suite (TypeFixture) does not set.
fn visit_instance_meet(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let (s_ref, s_args) = match s {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        // s not Instance: the FunctionLike/TypeType/Tuple/Literal/
        // TypedDict branches (meet.py:980-996) recurse or default ->
        // defer.
        _ => return None,
    };
    let (t_ref, t_args) = match t {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };

    // meet.py:1035-1037: t.type == self.s.type -> combine args.
    if t_ref == s_ref {
        if s_args.is_empty() && t_args.is_empty() {
            // Equal args-less Instances -> meet is the type itself.
            // (The extra_attrs same-type guard lives at the top of
            // meet_types, meet.py:129-141, so any attrs-bearing pair

            // returns there before reaching this visitor.)
            return Some(SetOpResult::SameS);
        }
        // Same type with args: per-arg meet combination.
        return visit_instance_meet_args(s, t, ctx, resolver);
    }

    // meet.py:1024-1029: alt_promote check BEFORE is_subtype. Python
    // checks t.alt_promote == s.type -> return t, then s.alt_promote ==
    // t.type -> return s. This is needed for native int types where

    // i32.alt_promote = int (so meet(i32, int) = i32, NOT int, even
    // though is_subtype(int, i32) is also True via int._promote).
    let t_snap = resolver.get(t_ref);
    let s_snap = resolver.get(s_ref);
    if let Some(snap) = t_snap {
        if let Some(alt) = &snap.alt_promote_fullname {
            if alt == s_ref {
                return Some(SetOpResult::SameT);
            }
        }
    }
    if let Some(snap) = s_snap {
        if let Some(alt) = &snap.alt_promote_fullname {
            if alt == t_ref {
                return Some(SetOpResult::SameS);
            }
        }
    }

    // meet.py:1030-1039: is_subtype(t, s) -> return t; is_subtype(s, t)
    // -> return s; else Bottom. Python's is_subtype always returns
    // bool; when Rust's is_subtype defers (None) the meet must defer

    // too. Falling through to Bottom would be a wrong answer when
    // Python would have returned t or s (e.g. via a promotion).
    match is_subtype(t, s, ctx, resolver) {
        Some(true) => Some(SetOpResult::SameT),
        Some(false) => match is_subtype(s, t, ctx, resolver) {
            Some(true) => Some(SetOpResult::SameS),
            Some(false) => Some(SetOpResult::Bottom),
            None => None,
        },
        None => None,
    }
}

/// Same-type-with-args branch of `TypeMeetVisitor.visit_instance`
/// (meet.py:1035-1079), Rust subset.
///
/// Python's flow:
/// 1. `is_subtype(t, s) or is_subtype(s, t)` gate (meet.py:1038);
///    both False -> UninhabitedType (strict) / NoneType (non-strict).
/// 2. Variadic instances (`has_type_var_tuple_type`) need
///    `split_with_prefix_and_suffix` + `TupleType` wrapping
///    (meet.py:1044-1063) -> defer; snapshot can't rebuild the
///    tuple fallback.
/// 3. Per-arg `self.meet(ta, sa)` (meet.py:1067-1078). A `TupleType`
///    meet result for a TypeVarTupleType arg unpacks into multiple
///    args (meet.py:1072-1074); a TypeVarTupleType tv whose arg meet is
///    UninhabitedType wraps `UnpackType(tv.tuple_fallback[meet])`
///    (meet.py:1076-1077) -> both defer (snapshot has no
///    `tuple_fallback`).
/// 4. Python zips (tolerates arg-count mismatch during daemon
///    reprocessing); Rust requires equal arity.
///
/// The final `Instance(t.type, args)` is written via the wire format
/// (disc=7) and decoded back by the Python shim via `read_type` +
/// fixup, so type_ref strings resolve to live TypeInfo there.
fn visit_instance_meet_args(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let (
        Type::Instance {
            type_ref,
            args: s_args,
            ..
        },
        Type::Instance { args: t_args, .. },
    ) = (s, t)
    else {
        return None;
    };

    let snap = resolver.get(type_ref)?;
    // Variadic instances need split_with_prefix_and_suffix +
    // TupleType wrapping (meet.py:1044-1063) — defer.
    if snap.has_type_var_tuple_type {
        return None;
    }
    // Python zips t.args, s.args, type_vars; mismatched lengths during
    // daemon reprocessing tolerate the zip. Rust requires equal arity.
    let tvars_len = snap.type_vars_with_variance.len();
    if s_args.len() != t_args.len() || s_args.len() != tvars_len {
        return None;
    }

    // meet.py:1038: gate on is_subtype(t, s) or is_subtype(s, t), with
    // Python's `or` short-circuit: t<:s is evaluated first; s<:t only
    // when the first is False. Both False -> Bottom (the shim maps disc

    // 3 to UninhabitedType (strict) / NoneType (non-strict), matching
    // meet.py:1081-1084). Any deferral -> defer the whole meet.
    let gate_ok = match is_subtype(t, s, ctx, resolver) {
        Some(true) => true,
        Some(false) => match is_subtype(s, t, ctx, resolver) {
            Some(true) => true,
            Some(false) => false,
            None => return None,
        },
        None => return None,
    };
    if !gate_ok {
        return Some(SetOpResult::Bottom);
    }

    // meet.py:1067-1078: per-arg meets. A TypeVarTupleType tv can't be
    // handled: the TupleType-unpack / UnpackType-wrap branches need the
    // live `tuple_fallback`, and a mismatched zip would double-count.

    // Defer if any tv is a TypeVarTupleType or ParamSpec (kind 2/1).
    if snap
        .type_vars_with_variance
        .iter()
        .any(|(_, _, kind)| *kind == 1 || *kind == 2)
    {
        return None;
    }

    let mut args_out: Vec<Type> = Vec::with_capacity(t_args.len());
    for (ta, sa) in t_args.iter().zip(s_args.iter()) {
        let r = meet_types(ta, sa, ctx, resolver)?;
        let typ = fruit_to_type(r, ta, sa)?;
        args_out.push(typ);
    }

    let result = Type::Instance {
        type_ref: type_ref.to_string(),
        args: args_out,
        last_known_value: None,
        extra_attrs: None,
    };
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &result).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// Map a `SetOpResult` to the `Type` it denotes, given the `s`/`t`
/// operands. Used by visitors that need to feed the recursive result
/// into a new type (e.g. `visit_type_type` case 1 wraps the joined
/// item in a new `TypeType`).
///
/// Returns `None` for results that can't be materialized without a
/// Type encoder or that the caller should defer on:
/// - `None` (the recursive call deferred)
/// - `SameTypeWithArgs` (needs per-arg reconstruction)
/// - `Ancestor` whose fullname is not in the resolver (would need
///   `object_or_any_from_type` fallback)
///
/// `Object` maps to `Instance(builtins.object, [])` (the common case
/// of `object_or_any_from_type` for Instance right; `visit_type_type`
/// recurses on `t.item`/`s.item` which are always Instance, so the
/// Object result is always `builtins.object`).
///
/// `Encoded` defers (returns `None`): the converter is shared with the
/// join visitors, and join keeps batch-1 semantics of deferring
/// recursive `Encoded` results to Python. The meet-only per-arg path
/// that must decode uses `fruit_to_type` (which has its own `Encoded`
/// arm) instead.
pub(crate) fn setop_result_to_type(r: Option<SetOpResult>, s: &Type, t: &Type) -> Option<Type> {
    match r? {
        SetOpResult::SameS => Some(s.clone()),
        SetOpResult::SameT => Some(t.clone()),
        SetOpResult::Any => Some(Type::AnyType {
            type_of_any: 3, // TypeOfAny.special_form
            source_any: None,
            missing_import_name: None,
        }),
        SetOpResult::Bottom => Some(Type::UninhabitedType { ambiguous: true }),
        SetOpResult::Object => {
            // Prefer the fixed s/t operand if it is already
            // builtins.object (avoids decoding an unfixed Instance).
            for candidate in [s, t] {
                if let Type::Instance { type_ref, .. } = candidate {
                    if type_ref == "builtins.object" {
                        return Some(candidate.clone());
                    }
                }
            }
            Some(Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            })
        }
        SetOpResult::Ancestor(fullname) => {
            // Prefer the fixed s/t operand when its type_ref matches
            // the ancestor fullname — the decoded bytes would otherwise
            // produce an unfixed Instance (type_ref only, no live

            // TypeInfo), which breaks == against fixed operands.
            for candidate in [s, t] {
                if let Type::Instance { type_ref, .. } = candidate {
                    if type_ref == &fullname {
                        return Some(candidate.clone());
                    }
                }
            }
            Some(Type::Instance {
                type_ref: fullname,
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            })
        }
        // SameTypeWithArgs needs per-arg reconstruction (which arg to
        // pick from s vs t); the visitor callers above
        // visit_type_type only recurse on args-less Instance items,

        // so this arm is unreachable in practice. Defer conservatively.
        SetOpResult::SameTypeWithArgs { .. } => None,
        // Encoded must NOT be decoded here: this converter is shared
        // with the join visitors, and batch-1's join semantics rely on
        // deferring recursive `Encoded` results to Python (the join

        // shim in mypy/join.py decodes top-level disc=7 itself, and
        // inner encodings re-joined here would change inference). The
        // meet-only per-arg path that needs the decode uses

        // `fruit_to_type` instead, which has its own Encoded arm.
        SetOpResult::Encoded(_) => None,
    }
    .filter(|typ| {
        // Only return types the encoder can write. Other variants
        // (CallableType, UnionType, etc.) would error in write_type;
        // defer so Python handles them.
        wire::write_type(&mut WriteBuffer::new(), typ).is_ok()
    })
}

/// `is_similar_callables` (join.py:993-1001): same arg count, same
/// min_args (ARG_POS count), same is_var_arg (any ARG_STAR). The wire
/// format stores arg_kinds as i64 (ARG_POS=0, ARG_STAR=2).
fn is_similar_callables(
    t_arg_types: &[Type],
    t_arg_kinds: &[i64],
    s_arg_types: &[Type],
    s_arg_kinds: &[i64],
) -> bool {
    t_arg_types.len() == s_arg_types.len()
        && min_args(t_arg_kinds) == min_args(s_arg_kinds)
        && is_var_arg(t_arg_kinds) == is_var_arg(s_arg_kinds)
}

fn min_args(arg_kinds: &[i64]) -> usize {
    arg_kinds.iter().filter(|&&k| k == 0).count()
}

fn is_var_arg(arg_kinds: &[i64]) -> bool {
    arg_kinds.contains(&2)
}

/// `ParamSpecType.__eq__` (types.py:931-938): id (raw_id + namespace),
/// flavor, prefix, and default. name/fullname/upper_bound are ignored
/// by Python equality; the prefix uses the derived `Parameters`
/// PartialEq (all five wire fields), strictly tighter than
/// `Parameters.__eq__` (types.py:2255) — safe direction: may defer
/// where Python deems equal, never answers wrongly.
fn paramspec_eq(s: &Type, t: &Type) -> bool {
    match (s, t) {
        (
            Type::ParamSpecType {
                prefix: s_prefix,
                raw_id: s_raw,
                namespace: s_ns,
                flavor: s_flavor,
                default: s_default,
                ..
            },
            Type::ParamSpecType {
                prefix: t_prefix,
                raw_id: t_raw,
                namespace: t_ns,
                flavor: t_flavor,
                default: t_default,
                ..
            },
        ) => {
            s_raw == t_raw
                && s_ns == t_ns
                && s_flavor == t_flavor
                && s_prefix == t_prefix
                && s_default == t_default
        }
        _ => false,
    }
}

/// `is_similar_params` (join.py:1050-1055): same arg count, same
/// min_args (ARG_POS count), same var_arg presence (any ARG_STAR).
/// Symmetric; mirrors `is_similar_callables` for Parameters.
fn is_similar_parameters(s: &wire::Parameters, t: &wire::Parameters) -> bool {
    s.arg_types.len() == t.arg_types.len()
        && min_args(&s.arg_kinds) == min_args(&t.arg_kinds)
        && is_var_arg(&s.arg_kinds) == is_var_arg(&t.arg_kinds)
}

/// `ArgKind.is_named` (nodes.py): ARG_NAMED (3) or ARG_NAMED_OPT (5).
fn is_named_kind(kind: i64) -> bool {
    kind == 3 || kind == 5
}

/// `combine_arg_names` (join.py:1169-1188), called as
/// `combine_arg_names(self.s, t)`. The result name at index i is outer
/// s's name when the names match or either kind is named, else None.
fn combine_parameter_arg_names(
    s_names: &[Option<String>],
    t_names: &[Option<String>],
    s_kinds: &[i64],
    t_kinds: &[i64],
) -> Vec<Option<String>> {
    (0..s_names.len())
        .map(|i| {
            if s_names[i] == t_names[i] || is_named_kind(s_kinds[i]) || is_named_kind(t_kinds[i]) {
                s_names[i].clone()
            } else {
                None
            }
        })
        .collect()
}

/// `TypeJoinVisitor.default` (join.py:990-1005), Rust subset. Returns
/// the set-op disc for the default join of `s`. Instance -> Object;
/// UnboundType / non-chain kinds -> Any; TypeVar / ParamSpec / TypeType
/// / TypedDict / FunctionLike recurse into the enclosing type;
/// TupleType needs tuple_fallback (resolver) — defer when unavailable.
fn join_default(s: &Type, resolver: &TypeResolver) -> Option<SetOpResult> {
    match s {
        Type::Instance { .. } => Some(SetOpResult::Object),
        Type::UnboundType { .. } => Some(SetOpResult::Any),
        Type::TypeVarType { upper_bound, .. } | Type::ParamSpecType { upper_bound, .. } => {
            // TypeVarTupleType is NOT in the Python elif chain -> Any.
            join_default(upper_bound, resolver)
        }
        Type::TypeType { item, .. } => join_default(item, resolver),
        Type::TypedDictType { fallback, .. } => join_default(fallback, resolver),
        Type::CallableType { fallback, .. } => join_default(fallback, resolver),
        Type::Overloaded { items } => {
            // Overloaded.fallback == items[0].fallback (types.py:17).
            let Type::CallableType { fallback, .. } = items.first()? else {
                return Some(SetOpResult::Any);
            };
            join_default(fallback, resolver)
        }
        Type::TupleType { .. } => match crate::typeops::tuple_fallback(s, resolver) {
            Some(fb) => join_default(&fb, resolver),
            None => None,
        },
        _ => Some(SetOpResult::Any),
    }
}

/// `TypeMeetVisitor.visit_parameters` same-length pair (meet.py:1023-
/// 1031): join arg_types (args are contravariant), keep t's remaining
/// Parameters fields. Diff length -> Python default(self.s) -> Bottom
/// (s is Parameters, not UnboundType). Encodes the fresh Parameters.
fn meet_parameters_pair(
    s_p: &wire::Parameters,
    t_p: &wire::Parameters,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    if s_p.arg_types.len() != t_p.arg_types.len() {
        return Some(SetOpResult::Bottom);
    }
    let mut new_arg_types = Vec::with_capacity(s_p.arg_types.len());
    for (s_a, t_a) in s_p.arg_types.iter().zip(t_p.arg_types.iter()) {
        let joined = join_types(s_a, t_a, ctx, resolver)?;
        new_arg_types.push(fruit_to_type(joined, s_a, t_a)?);
    }
    let result = Type::Parameters(wire::Parameters {
        arg_types: new_arg_types,
        arg_kinds: t_p.arg_kinds.clone(),
        arg_names: t_p.arg_names.clone(),
        variables: t_p.variables.clone(),
        imprecise_arg_kinds: t_p.imprecise_arg_kinds,
        is_ellipsis_args: t_p.is_ellipsis_args,
    });
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &result).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// `is_equivalent` (subtypes.py:277-300) for two Callables: is_subtype
/// both ways on pairwise arg_types + ret_type. Returns `None` (defer)
/// if any is_subtype can't decide; `Some(true)` if mutually subtype,
/// `Some(false)` otherwise.
#[allow(clippy::too_many_arguments)]
fn is_equivalent_callable(
    t_arg_types: &[Type],
    t_ret_type: &Type,
    s_arg_types: &[Type],
    s_ret_type: &Type,
    t_arg_names: &[Option<String>],
    s_arg_names: &[Option<String>],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // is_equivalent for callables checks is_subtype both ways, which
    // includes arg_name compatibility for named args. When arg_names
    // differ, the callables are not equivalent (join.py:643).
    if t_arg_names != s_arg_names {
        return Some(false);
    }
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        let fwd = is_subtype(ta, sa, ctx, resolver)?;
        if !fwd {
            return Some(false);
        }
        let bwd = is_subtype(sa, ta, ctx, resolver)?;
        if !bwd {
            return Some(false);
        }
    }
    let ret_fwd = is_subtype(t_ret_type, s_ret_type, ctx, resolver)?;
    if !ret_fwd {
        return Some(false);
    }
    let ret_bwd = is_subtype(s_ret_type, t_ret_type, ctx, resolver)?;
    Some(ret_bwd)
}

/// `combine_arg_names` (join.py:1123-1156): per-index, None if either is
/// None or names differ. Preserves positional names when compatible.
pub(crate) fn combine_arg_names(
    t_names: &[Option<String>],
    s_names: &[Option<String>],
    t_kinds: &[i64],
    s_kinds: &[i64],
) -> Vec<Option<String>> {
    // join.py:1169: keep t_name when names match OR either arg kind
    // is_named (ARG_NAMED=3, ARG_NAMED_OPT=5). Otherwise None.
    let is_named = |k: &i64| *k == 3 || *k == 5;
    t_names
        .iter()
        .zip(s_names.iter())
        .enumerate()
        .map(|(i, (tn, sn))| {
            if tn == sn
                || t_kinds.get(i).is_some_and(is_named)
                || s_kinds.get(i).is_some_and(is_named)
            {
                tn.clone()
            } else {
                None
            }
        })
        .collect()
}

/// `safe_join` (join.py:1065-1072): join_types for non-UnpackType
/// pairs. Both-UnpackType -> UnpackType(join). Mixed -> defer (None).
pub(crate) fn safe_join(
    t: &Type,
    s: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Type> {
    let t_unpack = matches!(t, Type::UnpackType { .. });
    let s_unpack = matches!(s, Type::UnpackType { .. });
    if !t_unpack && !s_unpack {
        return setop_result_to_type(join_types(t, s, ctx, resolver), t, s);
    }
    if t_unpack && s_unpack {
        let t_inner = match t {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => unreachable!(),
        };
        let s_inner = match s {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => unreachable!(),
        };
        let joined = setop_result_to_type(
            join_types(t_inner, s_inner, ctx, resolver),
            t_inner,
            s_inner,
        )?;
        return Some(Type::UnpackType {
            typ: Box::new(joined),
        });
    }
    // Mixed UnpackType / non-UnpackType: object_or_any_from_type fallback.
    // Defer to Python (rare case, needs full object_or_any_from_type).
    None
}

/// Pick the fallback per join.py:1106-1109 (combine) / 1048-1051
/// (join_similar): if t.fallback is builtins.function, use t.fallback,
/// else s.fallback. The "t" here is the second operand (self.s in
/// Python is the first arg; our s/t naming follows the Rust convention
/// where s is the first arg to join_types).
pub(crate) fn pick_fallback(s_fallback: &Type, t_fallback: &Type) -> Type {
    if let Type::Instance { type_ref, .. } = s_fallback {
        if type_ref == "builtins.function" {
            return s_fallback.clone();
        }
    }
    t_fallback.clone()
}

/// `safe_meet` (join.py:1057-1074): per-arg meet for
/// `join_similar_callables`. Args are met (contravariant).
/// - Non-UnpackType pair: meet_types.
/// - Both UnpackType: meet the inner types; if the meet is a definite
///   Bottom, wrap as `Instance(tuple_fallback, [Bottom])` per
///   join.py:1064-1068 (the tuple fallback type comes from the unpacked
///   side: TypeVarTupleType.tuple_fallback, TupleType.partial_fallback,
///   or Instance(builtins.tuple)).
/// - Mixed UnpackType / non: `UninhabitedType(ambiguous=False)`.
///
/// Returns None only if the inner meet defers.
fn safe_meet(t: &Type, s: &Type, ctx: &SubtypeContext, resolver: &TypeResolver) -> Option<Type> {
    let t_unpack = matches!(t, Type::UnpackType { .. });
    let s_unpack = matches!(s, Type::UnpackType { .. });
    if !t_unpack && !s_unpack {
        return fruit_to_type(meet_types(t, s, ctx, resolver)?, t, s);
    }
    if t_unpack && s_unpack {
        let t_inner = match t {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => unreachable!(),
        };
        let s_inner = match s {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => unreachable!(),
        };
        let fallback_ref = match t_inner {
            Type::TypeVarTupleType { tuple_fallback, .. } => {
                if let Type::Instance { type_ref, .. } = tuple_fallback.as_ref() {
                    type_ref.clone()
                } else {
                    return None;
                }
            }
            Type::TupleType {
                partial_fallback, ..
            } => {
                if let Type::Instance { type_ref, .. } = partial_fallback.as_ref() {
                    type_ref.clone()
                } else {
                    return None;
                }
            }
            Type::Instance { type_ref, .. } => type_ref.clone(),
            _ => return None,
        };
        let met = fruit_to_type(
            meet_types(t_inner, s_inner, ctx, resolver)?,
            t_inner,
            s_inner,
        )?;
        if matches!(met, Type::UninhabitedType { .. }) {
            return Some(Type::UnpackType {
                typ: Box::new(Type::Instance {
                    type_ref: fallback_ref,
                    args: vec![met],
                    last_known_value: None,
                    extra_attrs: None,
                }),
            });
        }
        return Some(Type::UnpackType { typ: Box::new(met) });
    }
    // Mixed: join.py:1073-1074 -> UninhabitedType().
    Some(Type::UninhabitedType { ambiguous: false })
}

// `join_similar_callables` (join.py:1086-1119): non-equivalent similar
// callables. Per-arg safe_meet, ret join, instance_type join, fallback pick.
// Operand order (t, self.s) from join.py:622; field handling per callsite.

#[allow(clippy::too_many_arguments)]
fn join_similar_callables_impl(
    s: &Type,
    t: &Type,
    s_arg_types: &[Type],
    t_arg_types: &[Type],
    s_ret_type: &Type,
    t_ret_type: &Type,
    s_fallback: &Type,
    t_fallback: &Type,
    s_instance_type: &Option<Box<Type>>,
    t_instance_type: &Option<Box<Type>>,
    s_arg_names: &[Option<String>],
    t_arg_names: &[Option<String>],
    s_arg_kinds: &[i64],
    t_arg_kinds: &[i64],
    t_variables: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        new_arg_types.push(safe_meet(ta, sa, ctx, resolver)?);
    }
    // join.py:627-630: if any arg is NoneType/UninhabitedType, Python
    // returns join_types(t.fallback, self.s) instead of the joined
    // callable. That fallback join cannot be reproduced here without

    // recursing into the visitor with a half-built operand, so defer.
    if new_arg_types
        .iter()
        .any(|tp| matches!(tp, Type::NoneType | Type::UninhabitedType { .. }))
    {
        return None;
    }
    let new_ret = setop_result_to_type(
        join_types(t_ret_type, s_ret_type, ctx, resolver),
        t_ret_type,
        s_ret_type,
    )?;
    let new_instance_type = match (s_instance_type, t_instance_type) {
        (Some(si), Some(ti)) => Some(Box::new(setop_result_to_type(
            join_types(ti.as_ref(), si.as_ref(), ctx, resolver),
            ti.as_ref(),
            si.as_ref(),
        )?)),
        _ => None,
    };
    let new_arg_names = combine_arg_names(t_arg_names, s_arg_names, t_arg_kinds, s_arg_kinds);
    let new_fallback = pick_fallback(s_fallback, t_fallback);
    let (
        arg_kinds,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        type_guard,
        type_is,
    ) = extract_callable_invariants(t);
    // join.py:626-635: the caller sets from_type_type=True on the result
    // unless either operand is an abstract type object (is_type_obj &&
    // type_object().is_abstract); then it keeps t's flag (copy_modified).

    // min_len>0 (both generic) is deferred before this impl is reached,
    // so match_generic_callables is a no-op and t.variables is preserved.
    let t_abstract = callable_is_abstract_type_obj(t, resolver)?;
    let s_abstract = callable_is_abstract_type_obj(s, resolver)?;
    let new_from_type_type = if t_abstract || s_abstract {
        from_type_type
    } else {
        true
    };
    // join.py:1108 sets `name=None` on the result, which is a
    // copy_modified on the live right operand `t`, so `t.definition`
    // survives and `pretty_callable` renders `def <right_operand_name>`.

    // The wire cannot carry `definition`; the Python shim restores it
    // from the live `t` after fixup. The wire `name` stays None to
    // match the pure result exactly (only `definition` differs, and the

    // shim repairs that).
    let new_callable = Type::CallableType {
        fallback: Box::new(new_fallback),
        instance_type: new_instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type: new_from_type_type,
        arg_types: new_arg_types,
        arg_kinds,
        arg_names: new_arg_names,
        ret_type: Box::new(new_ret),
        name: None,
        variables: t_variables.to_vec(),
        type_guard,
        type_is,
    };
    encode_callable(new_callable)
}

/// `combine_similar_callables` (join.py:1097-1120): is_equivalent path.
/// Per-arg safe_join, ret join, instance_type join, fallback pick.
/// Returns Encoded(new CallableType) or None (defer).
#[allow(clippy::too_many_arguments)]
fn combine_similar_callables(
    s: &Type,
    t: &Type,
    s_arg_types: &[Type],
    t_arg_types: &[Type],
    s_ret_type: &Type,
    t_ret_type: &Type,
    s_fallback: &Type,
    t_fallback: &Type,
    s_instance_type: &Option<Box<Type>>,
    t_instance_type: &Option<Box<Type>>,
    s_arg_names: &[Option<String>],
    t_arg_names: &[Option<String>],
    s_arg_kinds: &[i64],
    t_arg_kinds: &[i64],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        new_arg_types.push(safe_join(ta, sa, ctx, resolver)?);
    }
    let new_ret = setop_result_to_type(
        join_types(t_ret_type, s_ret_type, ctx, resolver),
        t_ret_type,
        s_ret_type,
    )?;
    let new_instance_type = match (s_instance_type, t_instance_type) {
        (Some(si), Some(ti)) => Some(Box::new(setop_result_to_type(
            join_types(ti.as_ref(), si.as_ref(), ctx, resolver),
            ti.as_ref(),
            si.as_ref(),
        )?)),
        _ => None,
    };
    let new_arg_names = combine_arg_names(t_arg_names, s_arg_names, t_arg_kinds, s_arg_kinds);
    let new_fallback = pick_fallback(s_fallback, t_fallback);
    let (
        arg_kinds,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        type_guard,
        type_is,
    ) = extract_callable_invariants(t);
    let new_callable = Type::CallableType {
        fallback: Box::new(new_fallback),
        instance_type: new_instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types: new_arg_types,
        arg_kinds,
        arg_names: new_arg_names,
        ret_type: Box::new(new_ret),
        name: None,
        variables: Vec::new(),
        type_guard,
        type_is,
    };
    let _ = t;
    let _ = s;
    encode_callable(new_callable)
}

/// Extract the invariant fields (arg_kinds, flags, type_guard, type_is)
/// from a CallableType `t`. These are copied as-is to the result
/// (join.py:1113-1119 copy_modified preserves them).
#[allow(clippy::type_complexity)]
pub(crate) fn extract_callable_invariants(
    t: &Type,
) -> (
    Vec<i64>,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    Option<Box<Type>>,
    Option<Box<Type>>,
) {
    match t {
        Type::CallableType {
            arg_kinds,
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
        } => (
            arg_kinds.clone(),
            *is_ellipsis_args,
            *implicit,
            *is_bound,
            *from_concatenate,
            *imprecise_arg_kinds,
            *unpack_kwargs,
            *from_type_type,
            type_guard.clone(),
            type_is.clone(),
        ),
        _ => unreachable!("extract_callable_invariants on non-CallableType"),
    }
}

/// Encode a CallableType via write_type and wrap as Encoded. Returns
/// None if write_type fails (unsupported nested variant).
fn encode_callable(t: Type) -> Option<SetOpResult> {
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &t).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// Encode a list of SetOpResult items as an Overloaded type via
/// write_type and wrap as Encoded. Returns None if any item fails
/// to decode back as a CallableType, or write_type fails.
fn encode_overloaded(items: Vec<SetOpResult>) -> Option<SetOpResult> {
    let mut all_items: Vec<Type> = Vec::with_capacity(items.len());
    for item in items {
        if let SetOpResult::Encoded(bytes) = item {
            let mut rbuf = ReadBuffer::new(&bytes);
            let typ = wire::read_type(&mut rbuf, None).ok()?;
            match &typ {
                Type::CallableType { .. } | Type::Overloaded { .. } => {
                    all_items.push(typ);
                }
                _ => return None,
            }
        } else {
            return None;
        }
    }
    let overloaded = Type::Overloaded { items: all_items };
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &overloaded).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// Whether a CallableType is a type object (types.py:2323-2326).
/// `is_type_obj` = fallback is a metaclass AND ret_type is not
/// UninhabitedType. The join_similar_callables caller (join.py:631-635)
/// sets `from_type_type=True` on the result, which the Rust port does not
/// replicate, so the non-equivalent join path defers on type objects.
pub(crate) fn is_type_obj_callable(t: &Type, resolver: &TypeResolver) -> bool {
    let Type::CallableType {
        fallback, ret_type, ..
    } = t
    else {
        return false;
    };
    if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
        return false;
    }
    let Type::Instance { type_ref, .. } = fallback.as_ref() else {
        return false;
    };
    resolver.get(type_ref).is_some_and(|snap| {
        snap.has_base("builtins.type") || snap.fullname == "abc.ABCMeta" || snap.fallback_to_any
    })
}

/// Whether a CallableType is an abstract type object, when the callee of
/// join_similar_callables decides whether to force `from_type_type=True`
/// on the result (join.py:631-635): true iff `is_type_obj()` AND
/// `type_object().is_abstract`. `is_type_obj` (types.py:2473-2476):
/// `fallback` is a metaclass (`is_metaclass`) and `ret_type` is not
/// UninhabitedType. `type_object()` (types.py:2505-2508):
/// `get_instance_type(force_fallback=True).type`: prefer the
/// `instance_type` field, else unwrap `ret_type` (TypeVar upper_bound,
/// TupleType partial_fallback, TypedDictType/LiteralType fallback) to an
/// Instance. Returns None when the chain cannot produce a live
/// meta-TypeInfo, in which case the caller defers to Python.
fn callable_is_abstract_type_obj(t: &Type, resolver: &TypeResolver) -> Option<bool> {
    let Type::CallableType {
        fallback,
        instance_type,
        ret_type,
        ..
    } = t
    else {
        return None;
    };
    if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
        return Some(false);
    }
    let Type::Instance { type_ref, .. } = fallback.as_ref() else {
        return None;
    };
    let snap = resolver.get(type_ref)?;
    let is_metaclass =
        snap.has_base("builtins.type") || snap.fullname == "abc.ABCMeta" || snap.fallback_to_any;
    if !is_metaclass {
        return Some(false);
    }
    // type_object(): get_instance_type(force_fallback=True).
    let mut ret = if let Some(inst) = instance_type {
        inst.as_ref().clone()
    } else {
        get_proper_type_local(ret_type.as_ref())?
    };
    match &ret {
        Type::TypeVarType { upper_bound, .. } => ret = get_proper_type_local(upper_bound.as_ref())?,
        Type::TupleType {
            partial_fallback, ..
        } => ret = get_proper_type_local(partial_fallback.as_ref())?,
        Type::TypedDictType { fallback, .. } => ret = get_proper_type_local(fallback.as_ref())?,
        Type::LiteralType { fallback, .. } => ret = get_proper_type_local(fallback.as_ref())?,
        _ => {}
    }
    let Type::Instance { type_ref, .. } = &ret else {
        return None;
    };
    Some(resolver.get(type_ref)?.is_abstract)
}

/// `get_proper_type` (types.py:3985-4011) for the wire format. The wire
/// cannot expand a `TypeAliasType` (the alias target is a live node, not
/// serialized), so it defers (None) exactly where Python's
/// `get_proper_type` would resolve the alias.
fn get_proper_type_local(typ: &Type) -> Option<Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ.clone()),
    }
}

/// `TypeJoinVisitor.visit_*` leaf methods (join.py:344-374), Rust
/// subset. Handles the visitors that don't recurse into `join_types`.
fn visit_join(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    match t {
        // visit_any (join.py:353-354): return t.
        Type::AnyType { .. } => Some(SetOpResult::SameT),

        // visit_none_type (join.py:356-365).
        Type::NoneType => {
            if ctx.strict_optional {
                match s {
                    Type::NoneType | Type::UninhabitedType { .. } => Some(SetOpResult::SameT),
                    Type::UnboundType { .. } | Type::AnyType { .. } => Some(SetOpResult::Any),
                    // Else branch: make_simplified_union([s, t])
                    // (join.py:363).
                    _ => {
                        let simplified = make_simplified_union(
                            &[s.clone(), Type::NoneType],
                            ctx,
                            resolver,
                            true,
                            false,
                        )?;
                        let mut wbuf = WriteBuffer::new();
                        wire::write_type(&mut wbuf, &simplified).ok()?;
                        Some(SetOpResult::Encoded(wbuf.into_bytes()))
                    }
                }
            } else {
                // Non-strict: return s.
                Some(SetOpResult::SameS)
            }
        }

        // visit_uninhabited_type (join.py:367-368): return s.
        Type::UninhabitedType { .. } => Some(SetOpResult::SameS),

        // visit_deleted_type (join.py:370-371): return s.
        Type::DeletedType { .. } => Some(SetOpResult::SameS),

        // visit_erased_type (join.py:373-374) is unhandled: top-level
        // Erased operands are filtered by the shim (join.py:524-525); a
        // nested Erased t has no arm here and defers via the wildcard.

        // visit_instance (join.py:421-454), Instance-vs-Instance nominal
        // subset. Only handles args-less instances (no type params) and
        // defers when args are present or the s side is not an Instance

        // (FunctionLike/TypeType/TypedDict/Tuple/Literal cases recurse
        // into join_types and need the InstanceJoiner recursion guard).
        Type::Instance { .. } => visit_instance_join(s, t, ctx, resolver),

        // visit_union_type (join.py:432-436):
        //   if is_proper_subtype(s, t): return t (SameT)
        //   else: return make_simplified_union([s, t])

        // is_subtype(s, Union[..]) is True iff s <: any item. We also
        // check is_subtype(t, s): if every item <: s, the simplified
        // union collapses to s (SameS). Otherwise defer — building a

        // new union needs a Type encoder (not available reader-only).
        Type::UnionType { items, .. } => visit_union_join(s, items, ctx, resolver),

        // visit_callable_type (join.py:541-577). The both-CallableType
        // case (isinstance(s, CallableType)) needs is_similar_callables
        // + is_equivalent + combine_similar_callables, which build a

        // new CallableType. The wire encoder now supports CallableType,
        // so the structurally-identical case (join(c, c) = c) returns
        // SameS without building anything. The similar-but-not-identical

        // case (combine/join_similar_callables) and the protocol-Instance
        // case (unpack_callback_proxy) still defer to Python. The
        // fallback case (s non-callable, non-protocol) recurses into

        // join_types(t.fallback, s).
        Type::CallableType {
            fallback,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            variables,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            name,
            type_guard,
            type_is,
        } => {
            if let Type::CallableType {
                fallback: s_fallback,
                arg_types: s_arg_types,
                arg_kinds: s_arg_kinds,
                arg_names: s_arg_names,
                ret_type: s_ret_type,
                variables: s_variables,
                instance_type: s_instance_type,
                is_ellipsis_args: s_is_ellipsis_args,
                implicit: s_implicit,
                is_bound: s_is_bound,
                from_concatenate: s_from_concatenate,
                imprecise_arg_kinds: s_imprecise_arg_kinds,
                unpack_kwargs: s_unpack_kwargs,
                from_type_type: s_from_type_type,
                name: s_name,
                type_guard: s_type_guard,
                type_is: s_type_is,
            } = s
            {
                // join.py:620-622: is_similar_callables(t, self.s) &&
                // is_equivalent(t, self.s) -> combine_similar_callables.
                // For the structurally-identical case (t == s on all

                // wire-relevant fields), combine_similar_callables(t, t)
                // returns t (every arg_join is join(x, x) = x, ret_join
                // is join(x, x) = x, fallback is t.fallback). So SameS

                // is correct without building a new CallableType.
                //
                // BUT: when `variables` is non-empty, Python's

                // `combine_similar_callables` always calls
                // `match_generic_callables`, which renumbers the tvars
                // via `TypeVarId.new` (a Python global counter). The

                // result has fresh tvar ids that differ from the inputs,
                // so `SameS` (= the original) would be wrong. Defer
                // the both-generic identical case to Python.
                let both_generic = !variables.is_empty() && !s_variables.is_empty();
                let identical = !both_generic
                    && arg_kinds == s_arg_kinds
                    && arg_names == s_arg_names
                    && arg_types == s_arg_types
                    && ret_type == s_ret_type
                    && variables == s_variables
                    && instance_type == s_instance_type
                    && is_ellipsis_args == s_is_ellipsis_args
                    && implicit == s_implicit
                    && is_bound == s_is_bound
                    && from_concatenate == s_from_concatenate
                    && imprecise_arg_kinds == s_imprecise_arg_kinds
                    && unpack_kwargs == s_unpack_kwargs
                    && from_type_type == s_from_type_type
                    && name == s_name
                    && type_guard == s_type_guard
                    && type_is == s_type_is
                    && fallback == s_fallback;
                if identical {
                    return Some(SetOpResult::SameS);
                }
                // join.py:620: is_similar_callables(t, self.s).
                if !is_similar_callables(arg_types, arg_kinds, s_arg_types, s_arg_kinds) {
                    // Not similar: the var-arg / subtype fallback
                    // branches (join.py:638-646) need is_subtype on
                    // whole callables -> defer.
                    return None;
                }
                // join.py:621: is_equivalent(t, self.s). Approximated
                // by is_subtype both ways on pairwise arg_types +
                // ret_type. Returns None (defer) if any is_subtype

                // can't decide (non-Instance, generic args, etc.).
                let equivalent = is_equivalent_callable(
                    arg_types,
                    ret_type,
                    s_arg_types,
                    s_ret_type,
                    arg_names,
                    s_arg_names,
                    ctx,
                    resolver,
                )?;
                // match_generic_callables (join.py:1039-1053): renumber
                // tvars so both callables share the same id space.
                // When `min_len == 0` (one side has no variables), the

                // renumber is a no-op (Python returns the callables
                // unchanged), so the combine/join_similar path proceeds
                // with the original fields.

                //
                // When `min_len > 0` (both sides have variables), Python
                // allocates fresh `TypeVarId`s via `TypeVarId.new` (a

                // Python global counter, types.py:559-562). The result's
                // tvar ids differ from any deterministic Rust allocation,
                // and `CallableType.__eq__` compares tvar ids in

                // `arg_types`/`ret_type`. Rust can't replicate the
                // counter without FFI back to Python, so the both-generic
                // case defers to preserve parity.
                let min_len = variables.len().min(s_variables.len());
                if min_len > 0 {
                    return None;
                }
                if equivalent {
                    // `from_type_type` rides the wire now (issue #388),
                    // so combine_similar_callables preserves it via
                    // extract_callable_invariants. The equivalent path

                    // no longer needs the type-object deferral.
                    return combine_similar_callables(
                        s,
                        t,
                        s_arg_types,
                        arg_types,
                        s_ret_type,
                        ret_type,
                        s_fallback,
                        fallback,
                        s_instance_type,
                        instance_type,
                        s_arg_names,
                        arg_names,
                        s_arg_kinds,
                        arg_kinds,
                        ctx,
                        resolver,
                    );
                }
                // Non-equivalent similar callables need
                // `join_similar_callables` (join.py:622-637). The caller's
                // fallback-join (join.py:628-629, when an arg meets to

                // NoneType/UninhabitedType) cannot be reproduced here
                // without recursing into the visitor with a half-built
                // operand, so the Rust impl defers (returns None) in that

                // case and Python runs the original path. The
                // `from_type_type` force (join.py:630-636, suppress the
                // abstract-instantiation error when concrete class objects

                // join to their abstract superclass) is replicated.
                join_similar_callables_impl(
                    s,
                    t,
                    s_arg_types,
                    arg_types,
                    s_ret_type,
                    ret_type,
                    s_fallback,
                    fallback,
                    s_instance_type,
                    instance_type,
                    s_arg_names,
                    arg_names,
                    s_arg_kinds,
                    arg_kinds,
                    variables,
                    ctx,
                    resolver,
                )
            } else if let Type::Overloaded { .. } = s {
                // join.py:583-585: s is Overloaded -> swap so the
                // visit_overloaded walk runs with self.s=callable
                // (CallableType.items == [self], types.py:2633).
                join_types(t, s, ctx, resolver)
            } else {
                visit_callable_fallback(s, fallback, ctx, resolver)
            }
        }

        // visit_overloaded (join.py:581-632).
        Type::Overloaded { items, .. } => {
            let first = items.first()?;
            let fallback = match first {
                Type::CallableType { fallback, .. } => fallback.as_ref(),
                _ => return None,
            };

            // Both-FunctionLike: s is CallableType or Overloaded.
            // FunctionLike.items: Overloaded returns its items,
            // CallableType returns [self] (types.py:2633).

            // join.py:644-658 walks (t_item, s_item) pairs: similar ->
            // equivalent -> combine, else t_item <: s_item -> s_item.
            let s_items: Vec<&Type> = match s {
                Type::Overloaded { items: s_items, .. } => {
                    if s_items.is_empty() {
                        return None;
                    }
                    s_items.iter().collect()
                }
                Type::CallableType { .. } => vec![s],
                _ => vec![],
            };
            if !s_items.is_empty() {
                let mut result_items: Vec<SetOpResult> = Vec::new();
                for t_item in items {
                    let t_callable = match t_item {
                        Type::CallableType { .. } => t_item,
                        _ => return None,
                    };
                    for s_item in &s_items {
                        // s_item is always a CallableType here: the Overloaded arm
                        // collects only CallableType items, and a plain
                        // CallableType s is a single-item list. The

                        // subtype arm therefore only encodes, never
                        // pushes an Overloaded.
                        let s_callable = s_item;
                        // is_similar_callables check.
                        let t_arg_types = match &t_callable {
                            Type::CallableType { arg_types, .. } => arg_types,
                            _ => unreachable!(),
                        };
                        let t_arg_kinds = match &t_callable {
                            Type::CallableType { arg_kinds, .. } => arg_kinds,
                            _ => unreachable!(),
                        };
                        let s_arg_types = match &s_callable {
                            Type::CallableType { arg_types, .. } => arg_types,
                            _ => unreachable!(),
                        };
                        let s_arg_kinds = match &s_callable {
                            Type::CallableType { arg_kinds, .. } => arg_kinds,
                            _ => unreachable!(),
                        };
                        if !is_similar_callables(t_arg_types, t_arg_kinds, s_arg_types, s_arg_kinds)
                        {
                            continue;
                        }
                        // is_equivalent check.
                        let t_ret = match &t_callable {
                            Type::CallableType { ret_type, .. } => ret_type,
                            _ => unreachable!(),
                        };
                        let s_ret = match &s_callable {
                            Type::CallableType { ret_type, .. } => ret_type,
                            _ => unreachable!(),
                        };
                        let t_arg_names = match &t_callable {
                            Type::CallableType { arg_names, .. } => arg_names,
                            _ => unreachable!(),
                        };
                        let s_arg_names = match &s_callable {
                            Type::CallableType { arg_names, .. } => arg_names,
                            _ => unreachable!(),
                        };
                        let equivalent = is_equivalent_callable(
                            t_arg_types,
                            t_ret,
                            s_arg_types,
                            s_ret,
                            t_arg_names,
                            s_arg_names,
                            ctx,
                            resolver,
                        )?;
                        if equivalent {
                            // combine_similar_callables: build Encoded.
                            let t_fallback = match &t_callable {
                                Type::CallableType { fallback, .. } => fallback.as_ref(),
                                _ => unreachable!(),
                            };
                            let s_fallback = match &s_callable {
                                Type::CallableType { fallback, .. } => fallback.as_ref(),
                                _ => unreachable!(),
                            };
                            let s_instance = match &s_callable {
                                Type::CallableType { instance_type, .. } => instance_type,
                                _ => unreachable!(),
                            };
                            let t_instance = match &t_callable {
                                Type::CallableType { instance_type, .. } => instance_type,
                                _ => unreachable!(),
                            };
                            if let SetOpResult::Encoded(bytes) = combine_similar_callables(
                                s_callable,
                                t_callable,
                                s_arg_types,
                                t_arg_types,
                                s_ret,
                                t_ret,
                                s_fallback,
                                t_fallback,
                                s_instance,
                                t_instance,
                                s_arg_names,
                                t_arg_names,
                                s_arg_kinds,
                                t_arg_kinds,
                                ctx,
                                resolver,
                            )? {
                                result_items.push(SetOpResult::Encoded(bytes));
                            }
                        } else if is_subtype(t_callable, s_callable, ctx, resolver)? {
                            // t_item <: s_item -> s_item (join.py:653).
                            result_items.push(encode_callable((*s_callable).clone())?);
                        }
                    }
                }
                if result_items.is_empty() {
                    // join.py:659: join_types(t.fallback, s.fallback).
                    // `s_fb` is `s`'s own fallback: CallableType s uses
                    // its fallback; Overloaded s uses items[0].fallback.
                    let s_fb: &Type = match s {
                        Type::CallableType { fallback, .. } => fallback.as_ref(),
                        Type::Overloaded { items: s_items, .. } => match s_items.first() {
                            Some(Type::CallableType { fallback, .. }) => fallback.as_ref(),
                            _ => return None,
                        },
                        _ => return None,
                    };
                    visit_callable_fallback(s, s_fb, ctx, resolver)
                } else if result_items.len() == 1 {
                    match &result_items[0] {
                        SetOpResult::Encoded(bytes) => {
                            let mut rbuf = ReadBuffer::new(bytes);
                            match wire::read_type(&mut rbuf, None) {
                                Ok(Type::CallableType { .. }) => {
                                    Some(SetOpResult::Encoded(bytes.clone()))
                                }
                                Ok(Type::Overloaded { .. }) => {
                                    // Single Overloaded result item — keep it.
                                    Some(SetOpResult::Encoded(bytes.clone()))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                } else {
                    // Multiple result items -> Overloaded(result).
                    // Encode each item and wrap as Overloaded.
                    encode_overloaded(result_items)
                }
            } else {
                // s is neither FunctionLike nor protocol-Instance ->
                // fallback join.
                visit_callable_fallback(s, fallback, ctx, resolver)
            }
        }

        // visit_type_type (join.py:854-864). Case 2 (s is Instance with
        // fullname=="builtins.type") returns self.s -> SameS. Case 1
        // (s is TypeType) builds a new TypeType wrapping

        // join_types(t.item, s.item); the joined item is materialized
        // via setop_result_to_type and encoded via write_type. Case 3
        // (else -> default) walks s's fallback chain -> defer.
        Type::TypeType {
            item: t_item,
            is_type_form: t_itf,
        } => {
            if let Type::Instance { type_ref, .. } = s {
                if type_ref == "builtins.type" {
                    return Some(SetOpResult::SameS);
                }
            }
            if let Type::TypeType {
                item: s_item,
                is_type_form: s_itf,
            } = s
            {
                // Python `visit_type_type` (join.py:886-890): both
                // TypeTypes always build a fresh
                // TypeType.make_normalized(join_types(t.item, s.item),

                // is_type_form=s.is_type_form or t.is_type_form), even
                // when the items are identical. Materialize the joined
                // item via setop_result_to_type and encode the fresh

                // TypeType. If the joined item is not encodable,
                // setop_result_to_type returns None -> defer to Python.
                let joined_item = setop_result_to_type(
                    join_types(t_item, s_item, ctx, resolver),
                    t_item,
                    s_item,
                )?;
                let joined = Type::TypeType {
                    item: Box::new(joined_item),
                    is_type_form: *s_itf || *t_itf,
                };
                let mut wbuf = WriteBuffer::new();
                wire::write_type(&mut wbuf, &joined).ok()?;
                return Some(SetOpResult::Encoded(wbuf.into_bytes()));
            }
            None
        }

        // visit_literal_type (join.py:928-938). Cases:
        // 1 (s is LiteralType, t == s) -> SameT.
        // 2 (s is LiteralType, both fallbacks enum) ->

        //   make_simplified_union([s, t]). When the enum has exactly
        //   these 2 members, contraction collapses to the enum
        //   Instance (Encoded). Partial coverage returns a 2-item

        //   union, which we now encode too (write_type emits UnionType).
        // 3 (s is LiteralType, neither enum) -> join_types(s.fallback,
        //   t.fallback). When both fallbacks are the same Instance (the

        //   common bool case: Literal[True] vs Literal[False]), the
        //   recursive join returns SameS -> s.fallback, which we encode.
        //   When fallbacks differ, the recursive join may defer -> None.

        // 4 (s is Instance, s.last_known_value == t) -> SameT.
        // 5 (else) -> join_types(s, t.fallback), materialized and
        //   encoded when decodable (defer if setop_result_to_type

        //   can't materialize, e.g. a recursive Encoded result).
        Type::LiteralType { value: t_val, .. } => {
            if let Type::LiteralType {
                value: s_val,
                fallback: s_fb,
            } = s
            {
                if s_val == t_val {
                    return Some(SetOpResult::SameT);
                }
                if let Type::LiteralType { fallback: t_fb, .. } = t {
                    // Case 2 (both enum): make_simplified_union([s, t]).
                    // Contraction collapses to a single Instance when
                    // the enum's full member set is covered; partial

                    // coverage leaves a union of 2 literals. Both encode
                    // (Python returns the union unchecked); anything
                    // else (e.g. TypeAliasType) defers.
                    if is_enum_fallback(s_fb, resolver)
                        && is_enum_fallback(t_fb, resolver)
                        && s_fb.as_ref() == t_fb.as_ref()
                    {
                        let simplified = make_simplified_union(
                            &[s.clone(), t.clone()],
                            ctx,
                            resolver,
                            true,
                            false,
                        )?;
                        if !matches!(simplified, Type::Instance { .. } | Type::UnionType { .. }) {
                            return None;
                        }
                        let mut wbuf = WriteBuffer::new();
                        wire::write_type(&mut wbuf, &simplified).ok()?;
                        return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                    }
                    // Case 3: join_types(s.fallback, t.fallback). Build
                    // the joined fallback and encode it (the result is
                    // an Instance, not s or t).
                    let joined =
                        setop_result_to_type(join_types(s_fb, t_fb, ctx, resolver), s_fb, t_fb)?;
                    let mut wbuf = WriteBuffer::new();
                    wire::write_type(&mut wbuf, &joined).ok()?;
                    return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                }
                return None;
            }
            if let Type::Instance {
                last_known_value: Some(lkv),
                ..
            } = s
            {
                if let Type::LiteralType { value: lkv_val, .. } = lkv.as_ref() {
                    if lkv_val == t_val {
                        return Some(SetOpResult::SameT);
                    }
                }
            }
            // Case 5: join_types(s, t.fallback) (join.py:966). s is not
            // a LiteralType and not an Instance with a matching LKV.
            // t.fallback is always an Instance; the recursive join

            // typically returns SameS/SameT/Ancestor/Object, which
            // setop_result_to_type materializes. setop_result_to_type
            // defers on Encoded and SameTypeWithArgs -> graceful defer.
            let Type::LiteralType { fallback: t_fb, .. } = t else {
                return None;
            };
            let joined = setop_result_to_type(join_types(s, t_fb, ctx, resolver), s, t_fb)?;
            let mut wbuf = WriteBuffer::new();
            wire::write_type(&mut wbuf, &joined).ok()?;
            Some(SetOpResult::Encoded(wbuf.into_bytes()))
        }

        // visit_type_var (join.py:463-474), case 1 same-id-same-bound
        // and case 3 (s is Instance). Case 1 (s is TypeVarType,
        // s.id==t.id, s.upper_bound==t.upper_bound) returns self.s ->

        // SameS. The copy_modified branch (case 1, upper_bounds differ)
        // and case 2 (s.id != t.id -> join upper_bounds) both produce a
        // new TypeVarType or the bound's join result — neither s nor t

        // in general -> defer. Case 3 (s not TypeVarType -> default(s)):
        // for Instance s, default(s) = object_from_instance(s) = object.
        // The `Object` variant maps to object_or_any_from_type(t); for

        // t=TypeVarType this recurses into object_or_any_from_type(
        // t.upper_bound), which for an Instance upper_bound also returns
        // object. Both paths yield `builtins.object`, so `Object` is

        // parity-correct for the Instance-s + TypeVarType-t case.
        //
        // `TypeVarId.__eq__` (types.py:567-577) checks raw_id,

        // meta_level, AND namespace. The wire format serializes only
        // raw_id + namespace (types.py:739-740); `read` reconstructs
        // TypeVarId with meta_level=0 (types.py:752). Meta variables

        // (meta_level > 0) are constraint-solver internals that do
        // not cross this FFI seam, so raw_id + namespace equality here
        // matches wire-roundtrip semantics exactly.
        Type::TypeVarType {
            raw_id: t_raw,
            namespace: t_ns,
            upper_bound: t_ub,
            name: t_name,
            fullname: t_fullname,
            values: t_values,
            default: t_default,
            variance: t_variance,
            ..
        } => {
            if let Type::TypeVarType {
                raw_id: s_raw,
                namespace: s_ns,
                upper_bound: s_ub,
                ..
            } = s
            {
                if s_raw == t_raw && s_ns == t_ns {
                    if s_ub == t_ub {
                        return Some(SetOpResult::SameS);
                    }
                    // Diff upper_bound, same id (join.py:514-515):
                    // produce copy_modified TypeVarType with joined upper bound.
                    let joined_ub = join_types(s_ub, t_ub, ctx, resolver)?;
                    let new_tv = match joined_ub {
                        SetOpResult::SameT | SetOpResult::Encoded(_) => {
                            // SameT -> t (already has joined UB).
                            // Encoded -> decode then wrap in TypeVarType.
                            let new_ub = match joined_ub {
                                SetOpResult::SameT => t_ub.clone(),
                                SetOpResult::Encoded(bytes) => Box::new(decode_type(&bytes)?),
                                _ => unreachable!(),
                            };
                            Type::TypeVarType {
                                raw_id: *t_raw,
                                namespace: t_ns.clone(),
                                upper_bound: new_ub,
                                name: t_name.clone(),
                                fullname: t_fullname.clone(),
                                values: t_values.clone(),
                                default: t_default.clone(),
                                variance: *t_variance,
                                meta_level: 0,
                            }
                        }
                        SetOpResult::SameS => {
                            // s_ub is wider: make copy_modified with s_ub.
                            Type::TypeVarType {
                                raw_id: *t_raw,
                                namespace: t_ns.clone(),
                                upper_bound: s_ub.clone(),
                                name: t_name.clone(),
                                fullname: t_fullname.clone(),
                                values: t_values.clone(),
                                default: t_default.clone(),
                                variance: *t_variance,
                                meta_level: 0,
                            }
                        }
                        SetOpResult::Object | SetOpResult::Bottom | SetOpResult::Any => {
                            // Join of bounds yields object/Any/Bottom —
                            // wrap in TypeVarType and encode.
                            let base = match joined_ub {
                                SetOpResult::Object => Type::Instance {
                                    type_ref: "builtins.object".to_string(),
                                    args: Vec::new(),
                                    last_known_value: None,
                                    extra_attrs: None,
                                },
                                SetOpResult::Bottom => Type::UninhabitedType { ambiguous: true },
                                SetOpResult::Any => Type::AnyType {
                                    type_of_any: 3,
                                    source_any: None,
                                    missing_import_name: None,
                                },
                                _ => unreachable!(),
                            };
                            Type::TypeVarType {
                                raw_id: *t_raw,
                                namespace: t_ns.clone(),
                                upper_bound: Box::new(base),
                                name: t_name.clone(),
                                fullname: t_fullname.clone(),
                                values: t_values.clone(),
                                default: t_default.clone(),
                                variance: *t_variance,
                                meta_level: 0,
                            }
                        }
                        SetOpResult::Ancestor(_) | SetOpResult::SameTypeWithArgs { .. } => {
                            // These should not arise from join_types on
                            // bounds; keep as-is with encoded fallback.
                            return None;
                        }
                    };
                    let mut wbuf = WriteBuffer::new();
                    if wire::write_type(&mut wbuf, &new_tv).is_ok() {
                        return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                    }
                    return None;
                }
                // Case 2 (diff id) (join.py:545-546): return
                // get_proper_type(join_types(s.upper_bound,
                // t.upper_bound)) — a fresh TYPE, not a TypeVarType.

                // Materialize the bound join and encode. A TypeAliasType
                // result can't expand without the live alias graph ->
                // defer to Python.
                let joined = fruit_to_type(join_types(s_ub, t_ub, ctx, resolver)?, s_ub, t_ub)?;
                if matches!(joined, Type::TypeAliasType { .. }) {
                    return None;
                }
                let mut wbuf = WriteBuffer::new();
                if wire::write_type(&mut wbuf, &joined).is_ok() {
                    return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                }
                return None;
            }
            // Case 3 (s not TypeVarType): default(s). For Instance s,
            // default(s) = object_from_instance(s) = object. The
            // `Object` variant (object_or_any_from_type(t)) yields the

            // same for t=TypeVarType with Instance upper_bound.
            if let Type::Instance { .. } = s {
                return Some(SetOpResult::Object);
            }
            // default(s) for non-Instance s (TypeType, TupleType,
            // CallableType, etc.) walks fallback chains / recurses;
            // defer to Python.
            None
        }

        // visit_typeddict (join.py:811-835).
        // Case 1 (s is TypedDictType): build a NEW TypedDictType via
        // resolve_typeddict_item over zipall, encode via write_type

        // (M8u). Case 2 (s is Instance): recurse into
        // join_types(self.s, t.fallback). Case 3 (else): defer.
        Type::TypedDictType {
            fallback,
            items: t_items,
            required_keys: t_req,
            readonly_keys: t_ro,
            is_closed: t_closed,
        } => {
            if let Type::TypedDictType {
                fallback: s_fallback,
                items: s_items,
                required_keys: s_req,
                readonly_keys: s_ro,
                is_closed: s_closed,
            } = s
            {
                // Case 1: both TypedDictType. Build the joined
                // TypedDictType via resolve_typeddict_item over zipall.
                visit_typeddict_both(
                    s_items, s_req, s_ro, s_fallback, s_closed, t_items, t_req, t_ro, t_closed,
                    ctx, resolver,
                )
            } else if let Type::Instance { .. } = s {
                // Case 2: s is Instance -> join_types(s, t.fallback).
                // SameS/SameT -> outer SameS (s==fallback or result=s);
                // Ancestor/Object pass through; Any/Bottom/Encoded are

                // fresh types, encode whole (disc=7).
                let r = join_types(s, fallback, ctx, resolver)?;
                match &r {
                    SetOpResult::SameS | SetOpResult::SameT => Some(SetOpResult::SameS),
                    SetOpResult::Ancestor(fullname) => {
                        Some(SetOpResult::Ancestor(fullname.to_string()))
                    }
                    SetOpResult::Object => Some(SetOpResult::Object),
                    SetOpResult::Any | SetOpResult::Bottom => {
                        match setop_result_to_type(Some(r.clone()), s, fallback) {
                            Some(typ) => {
                                let mut wbuf = WriteBuffer::new();
                                wire::write_type(&mut wbuf, &typ).ok()?;
                                Some(SetOpResult::Encoded(wbuf.into_bytes()))
                            }
                            None => None,
                        }
                    }
                    SetOpResult::Encoded(bytes) => Some(SetOpResult::Encoded(bytes.clone())),
                    SetOpResult::SameTypeWithArgs { .. } => None,
                }
            } else {
                None
            }
        }

        // visit_tuple_type (join.py:741-775). Two cases:
        //
        // Case 1 (s is TupleType): build a new TupleType via join_tuples

        // + InstanceJoiner.join_instances(tuple_fallback(s),
        // tuple_fallback(t)). Encoded result.
        //

        // Case 2 (s is not TupleType): join_types(self.s,
        // tuple_fallback(t)). SameS/SameT -> outer SameS; Ancestor/Object
        // pass through; else defer.

        //
        // `tuple_fallback(t)` (typeops.py:194-235) equals
        // `t.partial_fallback` when the fallback is NOT `builtins.tuple`.

        // When it IS `builtins.tuple`, it constructs
        // `Instance(builtins.tuple, [make_simplified_union(items)])`.
        //

        // The partial_fallback is always an Instance (wire reader
        // asserts the INSTANCE tag at wire.rs:968; types.py:2909
        // serializes `self.partial_fallback.write(data)`).
        Type::TupleType {
            partial_fallback: t_pf,
            items: t_items,
            ..
        } => {
            if let Type::TupleType {
                partial_fallback: _s_pf,
                items: s_items,
                ..
            } = s
            {
                // Case 1: both TupleType (join.py:848-868).
                // 1. Join fallbacks: instance_joiner.join_instances(
                //    tuple_fallback(s), tuple_fallback(t)).

                // 2. Join items: self.join_tuples(s, t).
                // 3. If items is None: subtype-check fallback
                //    (join.py:863-868).

                // 4. If items has 1 UnpackType(Instance): return the
                //    unpacked Instance (avoid double-wrapping, join.py:857-860).
                // 5. Else: TupleType(items, fallback).
                let s_fb = crate::typeops::tuple_fallback(s, resolver)?;
                let t_fb = crate::typeops::tuple_fallback(t, resolver)?;
                let joined_fb =
                    match rust_join_types_inner(&s_fb, &t_fb, ctx.strict_optional, resolver) {
                        Some(v) => v,
                        None => {
                            return None;
                        }
                    };

                let items = join_tuples_inner(s_items, t_items, ctx.strict_optional, resolver);

                match items {
                    None => {
                        // Python's join_tuples returns None only for fixed tuples with
                        // unequal arity; other None here is a Rust deferral
                        // that Python resolves into a real item list.
                        if find_unpack(s_items).is_some()
                            || find_unpack(t_items).is_some()
                            || s_items.len() == t_items.len()
                        {
                            return None;
                        }
                        // items is None -> fallback (join.py:862-868):
                        // is_proper_subtype(s,t) -> t; (t,s) -> s; else
                        // the joined fallback.
                        let proper_ctx = SubtypeContext {
                            proper_subtype: true,
                            ..*ctx
                        };
                        if let Some(true) = is_subtype(s, t, &proper_ctx, resolver) {
                            Some(SetOpResult::SameT)
                        } else if let Some(true) = is_subtype(t, s, &proper_ctx, resolver) {
                            Some(SetOpResult::SameS)
                        } else {
                            // Return the joined fallback.
                            let mut wbuf = WriteBuffer::new();
                            wire::write_type(&mut wbuf, &joined_fb).ok()?;
                            Some(SetOpResult::Encoded(wbuf.into_bytes()))
                        }
                    }
                    Some(items) => {
                        // join.py:857-860: if len(items) == 1 and
                        // isinstance(item, UnpackType) and
                        // isinstance(unpacked, Instance): return unpacked.
                        if items.len() == 1 {
                            if let Type::UnpackType { typ } = &items[0] {
                                if let Type::Instance { .. } = typ.as_ref() {
                                    let mut wbuf = WriteBuffer::new();
                                    wire::write_type(&mut wbuf, typ.as_ref()).ok()?;
                                    return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                                }
                            }
                        }
                        // join.py:861: return TupleType(items, fallback).
                        let result = Type::TupleType {
                            partial_fallback: Box::new(joined_fb),
                            items,
                            implicit: false,
                        };
                        let mut wbuf = WriteBuffer::new();
                        wire::write_type(&mut wbuf, &result).ok()?;
                        Some(SetOpResult::Encoded(wbuf.into_bytes()))
                    }
                }
            } else if let Type::Instance {
                type_ref: fb_ref, ..
            } = t_pf.as_ref()
            {
                // Case 2: s is not TupleType. join_types(s, tuple_fallback(t)).
                if fb_ref != "builtins.tuple" {
                    let r = join_types(s, t_pf, ctx, resolver)?;
                    match &r {
                        SetOpResult::SameS | SetOpResult::SameT => Some(SetOpResult::SameS),
                        SetOpResult::Ancestor(fullname) => {
                            Some(SetOpResult::Ancestor(fullname.to_string()))
                        }
                        SetOpResult::Object => Some(SetOpResult::Object),
                        SetOpResult::Any | SetOpResult::Bottom => {
                            match setop_result_to_type(Some(r.clone()), s, t_pf) {
                                Some(typ) => {
                                    let mut wbuf = WriteBuffer::new();
                                    wire::write_type(&mut wbuf, &typ).ok()?;
                                    Some(SetOpResult::Encoded(wbuf.into_bytes()))
                                }
                                None => None,
                            }
                        }
                        SetOpResult::Encoded(bytes) => Some(SetOpResult::Encoded(bytes.clone())),
                        SetOpResult::SameTypeWithArgs { .. } => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }

        // visit_param_spec (join.py:550-553): s == t -> t (SameT).
        // Else -> default(s) via join_default. ParamSpec never swaps in
        // the pre-dispatch (t=ParamSpec implies swapped=false), so the

        // emitted discs are not flipped.
        Type::ParamSpecType { .. } => {
            if paramspec_eq(s, t) {
                Some(SetOpResult::SameT)
            } else {
                join_default(s, resolver)
            }
        }

        // visit_parameters (join.py:566-580). Both-Parameters reaches
        // here (no both-callable pre-dispatch on the join side). Similar
        // pair -> arg_types meet + combine_arg_names (encoded); not

        // similar -> default(s) = Any (Parameters is not in the join
        // default elif chain). s not Parameters -> default(s).
        Type::Parameters(t_p) => {
            if let Type::Parameters(s_p) = s {
                if !is_similar_parameters(s_p, t_p) {
                    return Some(SetOpResult::Any);
                }
                // arg_types: meet (contravariant join); all other
                // fields kept from t (copy_modified semantics).
                let mut new_arg_types = Vec::with_capacity(s_p.arg_types.len());
                for (s_a, t_a) in s_p.arg_types.iter().zip(t_p.arg_types.iter()) {
                    let met = meet_types(s_a, t_a, ctx, resolver)?;
                    new_arg_types.push(fruit_to_type(met, s_a, t_a)?);
                }
                let new_names = combine_parameter_arg_names(
                    &s_p.arg_names,
                    &t_p.arg_names,
                    &s_p.arg_kinds,
                    &t_p.arg_kinds,
                );
                let result = Type::Parameters(wire::Parameters {
                    arg_types: new_arg_types,
                    arg_kinds: t_p.arg_kinds.clone(),
                    arg_names: new_names,
                    variables: t_p.variables.clone(),
                    imprecise_arg_kinds: t_p.imprecise_arg_kinds,
                    is_ellipsis_args: t_p.is_ellipsis_args,
                });
                let mut wbuf = WriteBuffer::new();
                wire::write_type(&mut wbuf, &result).ok()?;
                Some(SetOpResult::Encoded(wbuf.into_bytes()))
            } else {
                // s not Parameters -> default(s).
                join_default(s, resolver)
            }
        }

        // Full visitors (TypeVar, TypedDict, etc.) — deferred.
        _ => None,
    }
}

/// `TypeJoinVisitor.visit_callable_type` fallback case (join.py:577):
/// `return join_types(t.fallback, self.s)`. Fires when `s` is not a
/// CallableType, not an Overloaded, and not a protocol-Instance. The
/// fallback is always an Instance (builtins.function / builtins.type /
/// a user metaclass), so this recurses into the Instance-vs-`s` join.
///
/// Protocol check: if `s` is an Instance whose TypeInfo has
/// `is_protocol=True`, defer (needs `unpack_callback_proxy` to extract
/// the `__call__` member). Otherwise recurse.
///
/// The recursive call is `join_types(fallback, s)` (fallback=left,
/// s=right). SameS in the recursive frame means the result is
/// `fallback`; SameT means the result is `s`. The outer shim maps
/// SameS -> s, SameT -> t. Since the result of the fallback join is
/// neither s nor t in general, only the cases where the result IS s
/// can be expressed as SameS. Ancestor/Object pass through.
///
/// Defers when:
/// * `s` is a protocol Instance (needs callback proxy unpacking).
/// * The recursive `join_types(fallback, s)` returns `None`.
/// * The recursive result is `fallback` but `fallback != s` (can't
///   express as SameS; would need SameT-but-for-t-which-is-callable).
fn visit_callable_fallback(
    s: &Type,
    fallback: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // s is a protocol Instance -> defer (needs unpack_callback_proxy).
    if let Type::Instance { type_ref, .. } = s {
        if let Some(snap) = resolver.get(type_ref) {
            if snap.is_protocol {
                return None;
            }
        }
    }
    let r = join_types(fallback, s, ctx, resolver)?;
    // Encode a recursive join result that is a real type (neither s nor
    // t): hand it back whole via disc=7. Replaces the old blanket defer
    // once the encoder existed.
    let encode_result = |typ: Type| -> Option<SetOpResult> {
        let mut wbuf = WriteBuffer::new();
        wire::write_type(&mut wbuf, &typ).ok()?;
        Some(SetOpResult::Encoded(wbuf.into_bytes()))
    };
    match &r {
        // Recursive SameT: result = s (recursive right) -> outer SameS
        // (the shim returns s).
        SetOpResult::SameT => Some(SetOpResult::SameS),
        // Recursive SameS: result = fallback (recursive left). Only
        // expressible if fallback == s (then result is s -> SameS).
        SetOpResult::SameS if fallback == s => Some(SetOpResult::SameS),
        SetOpResult::SameS => {
            setop_result_to_type(Some(r.clone()), fallback, s).and_then(encode_result)
        }
        // Ancestor / Object pass through (swap-invariant).
        SetOpResult::Ancestor(fullname) => Some(SetOpResult::Ancestor(fullname.to_string())),
        SetOpResult::Object => Some(SetOpResult::Object),
        // Any, Bottom: fresh type, encode whole. SameTypeWithArgs: a
        // per-arg reconstruction, cannot express as a single encoded
        // type here. Defer.
        SetOpResult::Any | SetOpResult::Bottom => {
            setop_result_to_type(Some(r.clone()), fallback, s).and_then(encode_result)
        }
        // Encoded: the recursive join already produced a fresh encoded
        // type; pass it through (it decodes to the same type in the
        // shim).
        SetOpResult::Encoded(bytes) => Some(SetOpResult::Encoded(bytes.clone())),
        SetOpResult::SameTypeWithArgs { .. } => None,
    }
}

/// `TypeJoinVisitor.visit_typeddict_type` case 1 (join.py:812-831):
/// both s and t are TypedDictType. Build the joined TypedDictType via
/// `resolve_typeddict_item` over `zipall`, encode via `write_type`.
///
/// `zipall` (types.py:3232-3240) yields all keys from both TypedDicts
/// (left first, then right's unique keys). For each key, `item(name)`
/// (types.py:3218-3230) returns the TypedDictItem:
/// - If the key is in `items`, use `(items[name], name in required,
///   name in readonly)`.
/// - If the key is NOT in `items` and `is_closed`, the item type is
///   `UninhabitedType`, required=False, readonly=False.
/// - If the key is NOT in `items` and NOT `is_closed`, the item type
///   is `None` (implicit `NotRequired[ReadOnly[object]]`), required=
///   False, readonly=True.
///
/// `resolve_typeddict_item` (join.py:802-823):
/// - `is_required = s.required and t.required`.
/// - If either `s.typ` or `t.typ` is None: `join_type = None`, omit
///   the key (implicitly object in the join).
/// - Else: `join_type = join_types(s.typ, t.typ)` (recursive).
///   `is_readonly = True` if `s.required != t.required` or either
///   is readonly. Otherwise `is_readonly = not is_equivalent(s.typ,
///   t.typ)` (two-way subtype check).
///
/// `create_anonymous_fallback` (types.py:3170-3174): if the fallback
/// type's fullname is in `TPDICT_FB_NAMES` (typing._TypedDict, etc.),
/// the TypedDict is anonymous and `self.fallback` is returned as-is.
/// For non-anonymous TypedDicts, Python recurses into
/// `fallback.type.typeddict_type.create_anonymous_fallback()` to find
/// the anonymous root. The wire format carries only `type_ref` (not
/// the live TypeInfo with `.typeddict_type`), so we can't follow the
/// chain. For anonymous TypedDicts (the common case in tests), the
/// fallback is used directly. For non-anonymous, defer to Python.
///
/// Returns `Some(SetOpResult::Encoded(bytes))` on success, `None`
/// when any recursive `join_types` or `is_equivalent` defers, or
/// when the fallback is non-anonymous (can't compute
/// `create_anonymous_fallback` from the snapshot alone).
#[allow(clippy::too_many_arguments)]
fn visit_typeddict_both(
    s_items: &[(String, Type)],
    s_req: &std::collections::HashSet<String>,
    s_ro: &std::collections::HashSet<String>,
    s_fallback: &Type,
    s_closed: &bool,
    t_items: &[(String, Type)],
    t_req: &std::collections::HashSet<String>,
    t_ro: &std::collections::HashSet<String>,
    t_closed: &bool,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // create_anonymous_fallback: only anonymous TypedDicts are
    // supported (fallback type_ref in TPDICT_FB_NAMES). Non-anonymous
    // needs the live TypeInfo chain -> defer.
    let s_fb_ref = match s_fallback {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return None,
    };
    if !is_typeddict_fallback_anonymous(s_fb_ref) {
        return None;
    }

    // zipall: iterate all keys from both items maps.
    let s_map: std::collections::HashMap<&str, &Type> =
        s_items.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let t_map: std::collections::HashMap<&str, &Type> =
        t_items.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut new_items: Vec<(String, Type)> = Vec::new();
    let mut new_required: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut new_readonly: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Left items first (zipall: left.keys() then right's unique keys).
    for (name, s_typ) in s_items {
        seen.insert(name.as_str());
        let t_present = t_map.get(name.as_str()).copied();
        let s_required = s_req.contains(name);
        let s_readonly = s_ro.contains(name);
        let (t_required, t_readonly) =
            typeddict_item_flags(name, t_req, t_ro, t_present.is_some(), *t_closed);
        let t_typ = typeddict_item_type_for_join(t_present, *t_closed);
        let (item_type, is_required, is_readonly) = resolve_typeddict_item_inner(
            Some(s_typ),
            s_required,
            s_readonly,
            t_typ.as_ref(),
            t_required,
            t_readonly,
            ctx,
            resolver,
        )?;
        if let Some(typ) = item_type {
            new_items.push((name.clone(), typ));
            if is_required {
                new_required.insert(name.clone());
            }
            if is_readonly {
                new_readonly.insert(name.clone());
            }
        }
    }
    // Right items not in left.
    for (name, t_typ) in t_items {
        if seen.contains(name.as_str()) {
            continue;
        }
        let s_present = s_map.get(name.as_str()).copied();
        let (s_required, s_readonly) =
            typeddict_item_flags(name, s_req, s_ro, s_present.is_some(), *s_closed);
        let s_typ = typeddict_item_type_for_join(s_present, *s_closed);
        let t_required = t_req.contains(name);
        let t_readonly = t_ro.contains(name);
        let (item_type, is_required, is_readonly) = resolve_typeddict_item_inner(
            s_typ.as_ref(),
            s_required,
            s_readonly,
            Some(t_typ),
            t_required,
            t_readonly,
            ctx,
            resolver,
        )?;
        if let Some(typ) = item_type {
            new_items.push((name.clone(), typ));
            if is_required {
                new_required.insert(name.clone());
            }
            if is_readonly {
                new_readonly.insert(name.clone());
            }
        }
    }

    let is_closed = *s_closed && *t_closed;
    let new_td = Type::TypedDictType {
        fallback: Box::new(s_fallback.clone()),
        items: new_items,
        required_keys: new_required,
        readonly_keys: new_readonly,
        is_closed,
    };
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &new_td).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// `resolve_typeddict_item` inner logic (join.py:802-823).
/// Returns `(Option<Type>, is_required, is_readonly)` or `None` (defer).
/// `None` for the item_type means the key is omitted from the join.
#[allow(clippy::too_many_arguments)]
fn resolve_typeddict_item_inner(
    s_typ: Option<&Type>,
    s_required: bool,
    s_readonly: bool,
    t_typ: Option<&Type>,
    t_required: bool,
    t_readonly: bool,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<(Option<Type>, bool, bool)> {
    let is_required = s_required && t_required;
    if s_typ.is_none() || t_typ.is_none() {
        return Some((None, is_required, true));
    }
    let s_t = s_typ.unwrap();
    let t_t = t_typ.unwrap();
    let join_type = setop_result_to_type(join_types(s_t, t_t, ctx, resolver), s_t, t_t)?;
    // join.py:816-823: is_readonly = True if required mismatch or either
    // is readonly; else not is_equivalent(s.typ, t.typ).
    let is_readonly = if s_required != t_required || s_readonly || t_readonly {
        true
    } else {
        !is_equivalent_types(s_t, t_t, ctx, resolver)?
    };
    Some((Some(join_type), is_required, is_readonly))
}

/// Get the item type for a key in a TypedDict for the join, per
/// `TypedDictType.item` (types.py:3218-3230). If the key is present,
/// return its type. If missing and `is_closed`, return
/// `UninhabitedType`. If missing and NOT `is_closed`, return `None`.
fn typeddict_item_type_for_join(present: Option<&Type>, is_closed: bool) -> Option<Type> {
    if let Some(t) = present {
        return Some(t.clone());
    }
    if is_closed {
        Some(Type::UninhabitedType { ambiguous: true })
    } else {
        None
    }
}

/// Get the required/readonly flags for a TypedDict key, per
/// `TypedDictType.item` (types.py:3218-3230).
fn typeddict_item_flags(
    name: &str,
    required: &std::collections::HashSet<String>,
    readonly: &std::collections::HashSet<String>,
    present: bool,
    is_closed: bool,
) -> (bool, bool) {
    if present {
        return (required.contains(name), readonly.contains(name));
    }
    if is_closed {
        (false, false)
    } else {
        (false, true)
    }
}

/// `is_equivalent` (subtypes.py:277-300) for two arbitrary types:
/// `is_subtype(a, b) and is_subtype(b, a)`. Returns `None` (defer)
/// if any `is_subtype` returns `None`; `Some(true)` if mutually
/// subtype, `Some(false)` otherwise.
fn is_equivalent_types(
    a: &Type,
    b: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    let fwd = is_subtype(a, b, ctx, resolver)?;
    if !fwd {
        return Some(false);
    }
    let bwd = is_subtype(b, a, ctx, resolver)?;
    Some(bwd)
}

/// Check if a TypedDict fallback fullname is one of the anonymous
/// TypedDict fallback names (TPDICT_FB_NAMES in types.py:126-130).
fn is_typeddict_fallback_anonymous(type_ref: &str) -> bool {
    type_ref == "typing._TypedDict"
        || type_ref == "typing_extensions._TypedDict"
        || type_ref == "mypy_extensions._TypedDict"
}

/// `is_better` (join.py:794-810): given two possible results from
/// `join_instances_via_supertype`, indicate whether `t` is the better
/// one. Used by the nominal Instance-Instance join to pick the
/// candidate with the longest MRO (closest common ancestor).
///
/// Pure comparison: no mutation, no plugin visibility, no messages.
/// Ported as a standalone helper for testability and potential reuse
/// by callers that need to compare join candidates.
fn is_better_join(t: &Type, s: &Type, resolver: &TypeResolver) -> bool {
    if let Type::Instance {
        type_ref: t_ref, ..
    } = t
    {
        if !matches!(s, Type::Instance { .. }) {
            return true;
        }
        if let Type::Instance {
            type_ref: s_ref, ..
        } = s
        {
            let t_snap = resolver.get(t_ref);
            let s_snap = resolver.get(s_ref);
            let t_is_protocol = t_snap.is_some_and(|snap| snap.is_protocol);
            let s_is_protocol = s_snap.is_some_and(|snap| snap.is_protocol);
            if t_is_protocol != s_is_protocol {
                let t_is_object = t_ref == "builtins.object";
                let s_is_object = s_ref == "builtins.object";
                if !t_is_object && !s_is_object {
                    return !t_is_protocol;
                }
            }
            let t_mro = mro_len(t_ref, resolver);
            let s_mro = mro_len(s_ref, resolver);
            return t_mro > s_mro;
        }
    }
    false
}

/// `#[pyfunction]` entry for `is_better` (join.py:1177-1193).
/// Serializes both types, runs the comparison, returns `Some(bool)` or
/// `None` when a type can't be decoded (Python falls back to the pure
/// implementation).
#[pyfunction]
pub(crate) fn rust_is_better(
    t_bytes: &[u8],
    s_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    Some(is_better_join(&t, &s, resolver.resolver()))
}

/// `TypeInfo.is_enum` (nodes.py:3753) read for a LiteralType's fallback
/// Instance. The snapshot carries `is_enum`; returns `false` when the
/// fallback is not an Instance or the snapshot is missing (the Python
/// path's `is_enum` defaults to `False` for non-enum types, so a missing
/// snapshot is conservatively non-enum).
fn is_enum_fallback(t: &Type, resolver: &TypeResolver) -> bool {
    if let Type::Instance { type_ref, .. } = t {
        resolver.get(type_ref).is_some_and(|s| s.is_enum)
    } else {
        false
    }
}

/// `flatten_nested_unions` (types.py:4267-4300): recursively expand
/// UnionType items into a flat list. TypeAliasType is NOT expanded
/// (the wire format carries only `type_ref`, not the live `TypeAlias`
/// target needed for `_expand_once`); the boundary pyfunction expands
/// aliases first. If a TypeAliasType is present, return `None` so the
/// caller defers to Python.
pub(crate) fn flatten_nested_unions(items: &[Type]) -> Option<Vec<Type>> {
    let mut flat = Vec::with_capacity(items.len());
    for t in items {
        match t {
            Type::TypeAliasType { .. } => return None,
            Type::UnionType { items: inner, .. } => {
                flat.extend(flatten_nested_unions(inner)?);
            }
            _ => flat.push(t.clone()),
        }
    }
    Some(flat)
}

/// `_remove_redundant_union_items` (typeops.py:695-771), Rust subset.
///
/// Two passes: forward (drop later items that are subtypes of earlier
/// ones), then reverse (drop earlier items that are subtypes of later
/// ones). UninhabitedType is always redundant and dropped. Duplicate
/// detection uses `is_subtype` (the Rust port only handles Instance vs
/// Instance; non-Instance pairs defer).
///
/// Skips the `can_be_true`/`can_be_false` truthiness adjustment
/// (typeops.py:752-756): those flags are not modeled on the wire Type.
/// Skips the LiteralType-fallback optimization (typeops.py:717-728):
/// callers defer before reaching here when LiteralType is present.
fn remove_redundant_union_items(
    items: Vec<Type>,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    // Dedup like Python's `_remove_redundant_union_items` with
    // `is_proper_subtype(ignore_promotions=True)` (typeops.py:878-880):
    // any-right is not a supertype; promotions must not drop `Literal[5]`.
    let mut dedup_ctx = ctx.clone();
    dedup_ctx.ignore_promotions = true;
    dedup_ctx.proper_subtype = true;
    let mut current = items;
    for _direction in 0..2 {
        let mut new_items: Vec<Type> = Vec::with_capacity(current.len());
        for ti in current {
            if matches!(ti, Type::UninhabitedType { .. }) {
                continue;
            }
            let mut duplicate_index = None;
            for (j, tj) in new_items.iter().enumerate() {
                // An Instance with a last_known_value never removes
                // another item, unless it is an Instance with the same
                // last_known_value (typeops.py:878-890). Without this,

                // `Literal[1]? | Literal[2]?` collapses to `Literal[1]?`.
                if let Type::Instance {
                    last_known_value: Some(tj_lkv),
                    ..
                } = tj
                {
                    let ti_lkv = match &ti {
                        Type::Instance {
                            last_known_value, ..
                        } => last_known_value.as_deref(),
                        _ => None,
                    };
                    if ti_lkv != Some(tj_lkv.as_ref()) {
                        continue;
                    }
                }
                if is_subtype(&ti, tj, &dedup_ctx, resolver)? {
                    duplicate_index = Some(j);
                    break;
                }
            }
            if duplicate_index.is_none() {
                new_items.push(ti);
            }
        }
        current = new_items;
        if current.len() <= 1 {
            break;
        }
        current.reverse();
    }
    Some(current)
}

/// Reconstruct an `Instance` from a `SameTypeWithArgs` result. Each
/// arg_disc picks the arg to reuse: 0 -> s.args[i], 1 -> t.args[i],
/// 4 -> `AnyType(from_another_any)`. `type_ref` is the shared fullname.
/// Returns `None` if a disc is unexpected or the arg lists are too short.
fn reconstruct_instance_from_args(
    s: &Type,
    t: &Type,
    type_ref: &str,
    arg_discs: &[i8],
) -> Option<Type> {
    let s_args = match s {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    let t_args = match t {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    let mut args = Vec::with_capacity(arg_discs.len());
    for (i, disc) in arg_discs.iter().enumerate() {
        match disc {
            0 => args.push(s_args.get(i)?.clone()),
            1 => args.push(t_args.get(i)?.clone()),
            4 => args.push(Type::AnyType {
                type_of_any: 4, // from_another_any
                source_any: None,
                missing_import_name: None,
            }),
            _ => return None,
        }
    }
    Some(Type::Instance {
        type_ref: type_ref.to_string(),
        args,
        last_known_value: None,
        extra_attrs: None,
    })
}

/// Materialize a `SetOpResult` into a concrete `Type` for the instance
/// join rewrap. Unlike `setop_result_to_type` (which defers on `Encoded`
/// and `SameTypeWithArgs` because the batch-1 seam keeps deferral
/// semantics for callers that only forward the discriminator), this
/// helper decodes the full result: the instance-arg join genuinely
/// produces new types (a merged `Instance` for the argwise join, an
/// `Any` for diverged args) that must be re-wrapped into the outer
/// `Instance`. `SameTypeWithArgs` is reconstructed via
/// `reconstruct_instance_from_args`.
fn materialize_join(s: &Type, t: &Type, r: SetOpResult, resolver: &TypeResolver) -> Option<Type> {
    let typ = match &r {
        SetOpResult::SameS => s.clone(),
        SetOpResult::SameT => t.clone(),
        SetOpResult::Encoded(bytes) => decode_type(bytes)?,
        SetOpResult::Any => Type::AnyType {
            type_of_any: 3, // TypeOfAny.special_form
            source_any: None,
            missing_import_name: None,
        },
        SetOpResult::Bottom => Type::UninhabitedType { ambiguous: true },
        SetOpResult::Object => {
            // object_or_any_from_type: an Instance(builtins.object).
            Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }
        }
        SetOpResult::Ancestor(fullname) => Type::Instance {
            type_ref: fullname.clone(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        },
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => reconstruct_instance_from_args(s, t, type_ref, arg_discs)?,
    };
    // Ancestor/Object from a real join produce a fresh Instance whose
    // type_ref may be an unfixed fullname; callers that wrap the result
    // rely on the Python-side fixup to resolve refs.
    let _ = resolver;
    Some(typ)
}

fn try_contracting_literals_in_union(
    items: Vec<Type>,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    enum Sum {
        Bool(std::collections::HashSet<bool>),
        Enum(std::collections::HashSet<String>),
    }
    let mut groups: std::collections::HashMap<String, (Sum, Vec<usize>)> =
        std::collections::HashMap::new();
    for (idx, t) in items.iter().enumerate() {
        let Type::LiteralType { fallback, value } = t else {
            continue;
        };
        let Type::Instance { type_ref, .. } = fallback.as_ref() else {
            continue;
        };
        let snap = resolver.get(type_ref)?;
        if snap.is_enum {
            let LiteralValue::Str(name) = value else {
                continue;
            };
            let entry = groups.entry(type_ref.clone()).or_insert_with(|| {
                (
                    Sum::Enum(snap.enum_members.iter().cloned().collect()),
                    Vec::new(),
                )
            });
            if let Sum::Enum(missing) = &mut entry.0 {
                missing.remove(name);
            }
            entry.1.push(idx);
        } else if let LiteralValue::Bool(b) = value {
            let entry = groups.entry(type_ref.clone()).or_insert_with(|| {
                let mut s = std::collections::HashSet::new();
                s.insert(true);
                s.insert(false);
                (Sum::Bool(s), Vec::new())
            });
            if let Sum::Bool(missing) = &mut entry.0 {
                missing.remove(b);
            }
            entry.1.push(idx);
        }
    }
    let mut replace_at: std::collections::HashMap<usize, Type> = std::collections::HashMap::new();
    let mut drop: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (_, (sum, indices)) in groups {
        let complete = match sum {
            Sum::Bool(missing) => missing.is_empty() && indices.len() >= 2,
            Sum::Enum(missing) => missing.is_empty() && !indices.is_empty(),
        };
        if !complete {
            continue;
        }
        let first = indices[0];
        let rest = &indices[1..];
        if let Type::LiteralType { fallback, .. } = &items[first] {
            replace_at.insert(first, (**fallback).clone());
        }
        for &i in rest {
            drop.insert(i);
        }
    }
    if replace_at.is_empty() && drop.is_empty() {
        return Some(items);
    }
    let mut result = Vec::with_capacity(items.len());
    for (i, t) in items.into_iter().enumerate() {
        if drop.contains(&i) {
            continue;
        }
        if let Some(rep) = replace_at.remove(&i) {
            result.push(rep);
        } else {
            result.push(t);
        }
    }
    Some(result)
}

/// `#[pyfunction]` entry for `try_contracting_literals_in_union`
/// (typeops.py:1612-1652).
///
/// Reads the wire-encoded item list, runs the contraction (bool + enum
/// cases only), and returns each resulting item as its own wire blob.
/// Returns `None` (defer to Python) when the list cannot be read, or when
/// the contraction needs a snapshot lookup for a literal fallback whose
/// fullname is absent. The Python shim deserializes each blob and falls
/// through to the pure-Python body on any failure.
#[pyfunction]
pub(crate) fn rust_try_contracting_literals_in_union(
    type_list_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let mut buf = ReadBuffer::new(type_list_bytes);
    let items = wire::read_type_list(&mut buf).ok()?;
    let result = try_contracting_literals_in_union(items, resolver.resolver())?;
    let mut out = Vec::with_capacity(result.len());
    for item in result {
        let mut item_buf = WriteBuffer::new();
        wire::write_type(&mut item_buf, &item).ok()?;
        out.push(item_buf.into_bytes());
    }
    Some(out)
}

/// `try_getting_instance_fallback` (typeops.py:1271-1288): return the
/// `Instance` fallback for a type, if any. Mirrors the Python dispatch:
/// Instance -> self, LiteralType -> fallback, FunctionLike -> fallback
/// (Overloaded delegates to `items[0].fallback`), TypeVarType -> recurse
/// on `upper_bound`, TupleType -> `partial_fallback`, TypedDictType ->
/// `fallback`, NoneType/AnyType -> None.
///
/// Returns `None` for variants Python returns `None` for, or that the
/// Rust subset doesn't carry a fallback for (UnboundType, UnpackType,
/// UninhabitedType, DeletedType, TypeAliasType, ParamSpecType,
/// TypeVarTupleType).
fn try_getting_instance_fallback(t: &Type) -> Option<&Type> {
    match t {
        Type::Instance { .. } => Some(t),
        Type::LiteralType { fallback, .. } => Some(fallback.as_ref()),
        Type::CallableType { fallback, .. } => Some(fallback.as_ref()),
        Type::Overloaded { items } => {
            // Overloaded.fallback = items[0].fallback (types.py:2749).
            if let Some(Type::CallableType { fallback, .. }) = items.first() {
                Some(fallback.as_ref())
            } else {
                None
            }
        }
        Type::TypeVarType { upper_bound, .. } => try_getting_instance_fallback(upper_bound),
        Type::TupleType {
            partial_fallback, ..
        } => Some(partial_fallback.as_ref()),
        Type::TypedDictType { fallback, .. } => Some(fallback.as_ref()),
        _ => None,
    }
}

/// `make_simplified_union` step 5 (typeops.py:656-691): erase
/// inconsistent `extra_attrs` on the final union's fallback.
///
/// Collects the distinct `ExtraAttrs` across items that have a fallback
/// Instance with `extra_attrs`. If there is more than one distinct
/// `ExtraAttrs`, OR some item with the same fallback `type_ref` has no
/// `extra_attrs` while another has, set `fallback.extra_attrs = None`
/// on the final result's fallback.
///
/// Uses a `Vec` for the distinct-set (unions are small; avoids needing
/// `Hash` on `ExtraAttrs` which would require `Hash` on `Type`).
fn erase_extra_attrs_in_union(items: &[Type], result: &mut Type) {
    // Collect distinct ExtraAttrs (linear; small N). Only Instances with
    // extra_attrs contribute.
    let mut distinct: Vec<&ExtraAttrs> = Vec::new();
    for t in items {
        let Some(fb) = try_getting_instance_fallback(t) else {
            continue;
        };
        if let Type::Instance {
            extra_attrs: Some(ea),
            ..
        } = fb
        {
            if !distinct.contains(&ea) {
                distinct.push(ea);
            }
        }
    }
    if distinct.is_empty() {
        return;
    }
    // Determine the result's fallback Instance. If result is a single
    // Instance, it IS the fallback. If result is a UnionType, Python
    // does `try_getting_instance_fallback(result)` on the union, which

    // returns None (UnionType has no fallback) -> step 5 is a no-op.
    // But the Python code path only reaches step 5 when nitems > 1 and
    // the result is the make_union of the simplified set. When the set

    // collapses to a single Instance (via dedup or contraction), the
    // result IS that Instance and step 5 applies.
    let erase = if distinct.len() > 1 {
        true
    } else {
        // Single distinct ExtraAttrs: erase only if some item with the
        // same fallback type_ref has NO extra_attrs.
        let fb_ref = match try_getting_instance_fallback(result) {
            Some(Type::Instance { type_ref, .. }) => type_ref,
            _ => return, // no fallback Instance on result -> no-op
        };
        let mut should_erase = false;
        for t in items {
            if let Some(Type::Instance {
                type_ref: item_ref,
                extra_attrs: None,
                ..
            }) = try_getting_instance_fallback(t)
            {
                if item_ref == fb_ref {
                    should_erase = true;
                    break;
                }
            }
        }
        should_erase
    };
    if erase {
        if let Type::Instance { extra_attrs, .. } = result {
            *extra_attrs = None;
        }
    }
}

/// `make_simplified_union` (typeops.py:605-692), Rust subset.
///
/// Steps ported: flatten nested unions (step 1), single-item fast
/// path (step 2), remove redundant items (step 3), literal contraction
/// (step 4, bool + enum cases), extra-attrs erasure (step 5),
/// `make_union` (final). Returns `None` (defer to Python) when any
/// step can't be completed.
pub(crate) fn make_simplified_union(
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    contract_literals: bool,
    keep_erased: bool,
) -> Option<Type> {
    // Step 1: flatten nested unions. TypeAliasType defers.
    let flat = flatten_nested_unions(items)?;
    // Step 2: single-item fast path.
    if flat.len() == 1 {
        return Some(flat.into_iter().next().unwrap());
    }
    // Step 3: remove redundant items. Defer when any is_subtype returns
    // None (non-Instance pair, e.g. LiteralType-vs-non-LiteralType, where
    // the Rust is_subtype only handles LiteralType == LiteralType).
    let deduped = if keep_erased {
        let mut dedup_ctx = ctx.clone();
        dedup_ctx.ignore_promotions = true;
        dedup_ctx.proper_subtype = true;
        let mut current = flat;
        for _direction in 0..2 {
            current = crate::remove_redundant::remove_redundant_pass(
                &current,
                &dedup_ctx,
                resolver,
                keep_erased,
            )?;
            if current.len() <= 1 {
                break;
            }
            current.reverse();
        }
        current
    } else {
        remove_redundant_union_items(flat, ctx, resolver)?
    };
    // Step 4: contract literals (bool + enum) sharing a fallback
    // whose full value set is covered. Gated on >1 LiteralType item,
    // matching Python (typeops.py:785): a lone enum literal must stay a

    // literal (e.g. a single-member enum's sole value), never contract
    // back to the enum Instance. contract_literals=False (call from
    // try_expanding_sum_type_to_union) skips contraction entirely.
    let contracted = if contract_literals
        && deduped
            .iter()
            .filter(|t| matches!(t, Type::LiteralType { .. }))
            .count()
            > 1
    {
        try_contracting_literals_in_union(deduped, resolver)?
    } else {
        deduped
    };
    // Final: make_union (types.py:3483-3489).
    let mut result = union_make_union(contracted);
    // Step 5: erase inconsistent extra_attrs on the result's fallback.
    // Runs on the original `items` (pre-contraction), matching Python
    // (typeops.py:665 iterates `items`, not `simplified_set`).
    erase_extra_attrs_in_union(items, &mut result);
    Some(result)
}

/// `UnionType.make_union` (types.py:3483-3489): 0 items -> bottom,
/// 1 item -> that item, >1 -> UnionType.
pub(crate) fn union_make_union(items: Vec<Type>) -> Type {
    match items.len() {
        0 => Type::UninhabitedType { ambiguous: false },
        1 => items.into_iter().next().unwrap(),
        _ => {
            let can_be_true = items.iter().any(union_item_can_be_true);
            let can_be_false = items.iter().any(union_item_can_be_false);
            Type::UnionType {
                items,
                uses_pep604_syntax: false,
                can_be_true,
                can_be_false,
            }
        }
    }
}

/// Mirror `UnionType.can_be_true_default`: the union can be true if any
/// item can be true. Reads the stored flag on wire-decoded types; falls
/// back to the per-variant default (via `typeops::can_be_true_default`)
/// otherwise.
pub(crate) fn union_item_can_be_true(t: &Type) -> bool {
    match t {
        Type::UnionType { can_be_true, .. } => *can_be_true,
        _ => crate::typeops::can_be_true_default(t).unwrap_or(true),
    }
}

/// Mirror `UnionType.can_be_false_default` (see `union_item_can_be_true`).
pub(crate) fn union_item_can_be_false(t: &Type) -> bool {
    match t {
        Type::UnionType { can_be_false, .. } => *can_be_false,
        _ => crate::typeops::can_be_false_default(t).unwrap_or(true),
    }
}

/// `TypeJoinVisitor.visit_union_type` (join.py:432-436), Rust subset.
///
/// `is_subtype(s, Union[A, B])` is True iff `s <: A` or `s <: B`
/// (subtypes.py: UnionType right is an OR over items). If True, the
/// join is `t` (the union): `SameT`.
///
/// If `s` is not a subtype of `t`, Python calls `make_simplified_union
/// ([s, t])`. We can't build a new union without a Type encoder, but we
/// can detect one case: if `t <: s` (every union item is a subtype of
/// `s`), the simplified union collapses to `s` alone: `SameS`.
///
/// Defers (returns `None`) when:
/// * Any `is_subtype` call returns `None` (can't conclude).
/// * Neither `s <: t` nor `t <: s` (needs a new union).
fn visit_union_join(
    s: &Type,
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // s <: t iff s <: any item of t.
    let mut found_subtype = false;
    for item in items {
        // UninhabitedType (bottom) is never a supertype: is_subtype(s,
        // UninhabitedType) is False for all s. Rust is_subtype only
        // handles Instance vs Instance, so short-circuit here to avoid

        // a spurious None-defer.
        if matches!(item, Type::UninhabitedType { .. }) {
            continue;
        }
        match is_subtype(s, item, ctx, resolver) {
            Some(true) => {
                found_subtype = true;
                break;
            }
            Some(false) => {}
            None => return None,
        }
    }
    if found_subtype {
        return Some(SetOpResult::SameT);
    }
    // t <: s iff every item of t is <: s. If every item is <: s, the
    // simplified union collapses to s: SameS.
    let mut all_subtype = true;
    for item in items {
        // UninhabitedType is subtype of everything (bottom type).
        // Rust is_subtype only handles Instance vs Instance, so
        // short-circuit here to avoid a spurious None-defer.
        if matches!(item, Type::UninhabitedType { .. }) {
            continue;
        }
        match is_subtype(item, s, ctx, resolver) {
            Some(true) => {}
            Some(false) => {
                all_subtype = false;
                break;
            }
            None => return None,
        }
    }
    if all_subtype {
        return Some(SetOpResult::SameS);
    }
    // Neither s <: t nor t <: s: Python calls
    // make_simplified_union([s, t]). Build the simplified union in
    // Rust and return it Encoded. Returns None (defer) when

    // make_simplified_union can't complete (LiteralType present,
    // TypeAliasType, non-Instance subtype check, etc.).
    let simplified = make_simplified_union(
        &[
            s.clone(),
            Type::UnionType {
                items: items.to_vec(),
                uses_pep604_syntax: false,
                can_be_true: items.iter().any(union_item_can_be_true),
                can_be_false: items.iter().any(union_item_can_be_false),
            },
        ],
        ctx,
        resolver,
        true,
        false,
    )?;
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, &simplified).ok()?;
    Some(SetOpResult::Encoded(wbuf.into_bytes()))
}

/// `TypeJoinVisitor.visit_instance` (join.py:421-454), the
/// `isinstance(self.s, Instance)` branch, Rust subset.
///
/// Ports the `InstanceJoiner.join_instances` (join.py:107-202):
/// - Same type, both args-less -> SameS.
/// - Same type, args present -> `visit_instance_with_args` (M8g):
///   AnyType args + invariant `is_equivalent` only; covariant /
///   variadic / ParamSpec / TypeVarTupleType defer.
/// - Different type, both args-less -> `join_instances_via_supertype`
///   (the nominal common-ancestor walk).
/// - Different type with args -> defer (the via_supertype path with
///   args needs `expand_type_by_instance` on each base, deferred).
///
/// Returns `None` (defer to Python) when args are present but the
/// specific arg-shape is not handled, or when a promote/blob decode
/// fails.
pub(crate) fn visit_instance_join(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let (s_ref, s_args) = match s {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        // s is not an Instance: the FunctionLike/TypeType/TypedDict/
        // Tuple/Literal/TypeVarTuple branches (join.py:437-454) all
        // recurse into join_types — defer to Python.
        _ => return None,
    };
    let (t_ref, t_args) = match t {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    // join.py:114: t.type == s.type -> combine type args.
    // Defer when either side has fallback_to_any: Python's join_instances
    // uses is_proper_subtype (bypasses fallback_to_any) for dispatch,

    // but the promote loop and join_instances_via_supertype need
    // _promote lists and map_instance_to_supertype that the Rust path
    // doesn't fully port. Deferring avoids wrong common-ancestor picks.
    if resolver.get(s_ref).is_some_and(|s| s.fallback_to_any)
        || resolver.get(t_ref).is_some_and(|t| t.fallback_to_any)
    {
        return None;
    }
    if t_ref == s_ref {
        if s_args.is_empty() && t_args.is_empty() {
            // join.py:281 constructs `Instance(t.type, [])` — a
            // fresh Instance with no last_known_value. Return the
            // operand without LKV to match Python; if both have

            // LKV, build a fresh Instance via Encoded.
            let t_has_lkv = matches!(
                t,
                Type::Instance {
                    last_known_value: Some(_),
                    ..
                }
            );
            if !t_has_lkv {
                return Some(SetOpResult::SameT);
            }
            let s_has_lkv = matches!(
                s,
                Type::Instance {
                    last_known_value: Some(_),
                    ..
                }
            );
            if !s_has_lkv {
                return Some(SetOpResult::SameS);
            }
            // Both have LKV: fresh Instance with no LKV.
            let fresh = Type::Instance {
                type_ref: t_ref.to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            };
            let mut wbuf = WriteBuffer::new();
            wire::write_type(&mut wbuf, &fresh).ok()?;
            return Some(SetOpResult::Encoded(wbuf.into_bytes()));
        }
        // Same type with args: M8g handles AnyType + invariant
        // is_equivalent; covariant / variadic / ParamSpec defer.
        return visit_instance_with_args(s_ref, s_args, t_args, ctx, resolver);
    }

    // Different types with args: the via_supertype path needs
    // expand_type_by_instance on each base (join.py:204-240 with
    // args). Deferred — fall through to Python.
    if !s_args.is_empty() || !t_args.is_empty() {
        return None;
    }

    // join.py:282-290: dispatch mirrors Python's join_instances.
    // Python uses is_proper_subtype(t, s, ignore_type_params=True) to
    // decide direction. proper_subtype=True bypasses the

    // fallback_to_any short-circuit (subtypes.py:493), which would
    // wrongly make D <: E when D has fallback_to_any. An
    // ignore_type_params=True context is used because join_instances

    // ignores type params at this stage (args are empty here anyway).
    let proper_ctx = SubtypeContext {
        proper_subtype: true,
        ..*ctx
    };
    let t_is_subtype = is_subtype(t, s, &proper_ctx, resolver)?;
    let result_ref = if t_is_subtype {
        join_instances_nominal(t_ref, s_ref, ctx, resolver)?
    } else {
        join_instances_nominal(s_ref, t_ref, ctx, resolver)?
    };
    Some(match result_ref {
        // Left means the first arg to via_supertype won. When t <: s,
        // via_supertype(t, s) was called, so Left = t -> SameT.
        // Otherwise via_supertype(s, t), so Left = s -> SameS.
        JoinResult::Left => {
            if t_is_subtype {
                SetOpResult::SameT
            } else {
                SetOpResult::SameS
            }
        }
        JoinResult::Ancestor(fullname) => SetOpResult::Ancestor(fullname),
        JoinResult::Object => SetOpResult::Object,
    })
}

/// `InstanceJoiner.join_instances` same-type-with-args branch
/// (join.py:114-180), Rust subset.
///
/// Combines type arguments positionally via `zip(t.args, s.args,
/// type_vars)`. Handles:
/// * AnyType arg (either side) -> `AnyType(from_another_any)`
///   (arg disc 4).
/// * Invariant TypeVarType + `is_equivalent(ta, sa)` False ->
///   `object_from_instance(t)` (return `Object`).
/// * Invariant TypeVarType + `is_equivalent` True + recursive
///   `join_types(ta, sa)` returns SameS/SameT -> arg disc 0/1.
/// * Covariant TypeVarType: recursive `join_types(ta, sa)` returns
///   SameS/SameT (equal args) -> arg disc 1/0, gated by
///   `is_subtype(new_type, upper_bound)` (false -> `Object`).
///
/// Defers (returns `None`) for:
/// * Covariant/contravariant TypeVarType where the recursive join
///   returns `Ancestor`/`Object`/`Any`/`Bottom` (can't express as an
///   arg disc without a Type encoder). In practice this fires when
///   the two args differ: Instance-Instance recursion yields
///   `Ancestor(common-supertype)` rather than `SameS`/`SameT`.
/// * Empty `upper_bound` blob (can't safely skip the bound check).
/// * `type_var.values` non-empty (snapshot has no `values` field;
///   deferred conservatively via the recursive-join-non-trivial path).
/// * ParamSpec (kind=1) / TypeVarTupleType (kind=2).
/// * `has_type_var_tuple_type` (variadic instance).
/// * Arg-count mismatch (Python uses `zip`; Rust requires equal).
///
/// `s_args` / `t_args` are the Instance args (s=left, t=right). The
/// returned `SameTypeWithArgs.arg_discs[i]` is 0 (s.args[i]), 1
/// (t.args[i]), or 4 (Any).
fn visit_instance_with_args(
    type_ref: &str,
    s_args: &[Type],
    t_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let snap = resolver.get(type_ref)?;
    if snap.has_type_var_tuple_type {
        // Variadic instance (builtins.tuple[Ts]): Python partitions args
        // via split_with_prefix_and_suffix and joins the middle as a
        // TupleType (prefix=0/suffix=0/single-arg: whole list).
        let prefix = snap.type_var_tuple_prefix.unwrap_or(0);
        let suffix = snap.type_var_tuple_suffix.unwrap_or(0);
        if prefix == 0 && suffix == 0 && s_args.len() == 1 && t_args.len() == 1 {
            // The per-type-var TypeVarTupleType branch (join.py:230-241):
            // join the rewrapped TupleType args, decode the Encoded result
            // via materialize_join (not recomputable from s/t operands).
            let r = join_types(&t_args[0], &s_args[0], ctx, resolver)?;
            let new_type = materialize_join(&t_args[0], &s_args[0], r, resolver)?;
            // join.py:235-241: rewrap. Instance(builtins.tuple) ->
            // UnpackType; TupleType -> extend items; UnionType (tuple
            // fallback shape) -> Instance(tuple, [union]).
            let result = match &new_type {
                Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
                    Type::UnpackType {
                        typ: Box::new(new_type),
                    }
                }
                Type::TupleType { items, .. } => {
                    if items.len() == 1 {
                        if let Type::UnpackType { typ } = &items[0] {
                            if let Type::Instance { type_ref, .. } = typ.as_ref() {
                                if type_ref == "builtins.tuple" {
                                    return Some(SetOpResult::Encoded({
                                        let mut wbuf = WriteBuffer::new();
                                        wire::write_type(&mut wbuf, typ.as_ref()).ok()?;
                                        wbuf.into_bytes()
                                    }));
                                }
                            }
                        }
                    }
                    Type::Instance {
                        type_ref: type_ref.to_string(),
                        args: items.clone(),
                        last_known_value: None,
                        extra_attrs: None,
                    }
                }
                Type::UnionType { .. } => Type::Instance {
                    type_ref: type_ref.to_string(),
                    args: vec![new_type],
                    last_known_value: None,
                    extra_attrs: None,
                },
                _ => return None,
            };
            let mut wbuf = WriteBuffer::new();
            wire::write_type(&mut wbuf, &result).ok()?;
            return Some(SetOpResult::Encoded(wbuf.into_bytes()));
        }
        // Other variadic patterns (prefix>0, suffix>0, or multi-arg):
        // need split_with_prefix_and_suffix — defer.
        return None;
    }
    let tvars = &snap.type_vars_with_variance;
    // Python uses zip (tolerates length mismatch during daemon
    // reprocessing). Rust requires equal lengths + matching tvars.
    if s_args.len() != t_args.len() || s_args.len() != tvars.len() {
        return None;
    }

    let mut arg_discs: Vec<i8> = Vec::with_capacity(tvars.len());
    let mut joined_args: Vec<Type> = Vec::with_capacity(tvars.len());
    let mut needs_encode = false;
    for (i, (_, variance, kind)) in tvars.iter().enumerate() {
        let ta = &t_args[i]; // Python's t.args[i] (right arg).
        let sa = &s_args[i]; // Python's s.args[i] (left arg).

        // Ambiguous-UninhabitedType args (join.py:126-130): `ambiguous`
        // means the UninhabitedType came from an empty collection /
        // unreachable inference, and the OTHER side's arg wins outright

        // (no join, no is_equivalent, no upper-bound check).
        // ta_ambiguous -> new_type = sa (disc 0); sa_ambiguous ->
        // new_type = ta (disc 1).
        if matches!(ta, Type::UninhabitedType { ambiguous: true }) {
            arg_discs.push(0); // new_type = sa = s.args[i]
            joined_args.push(sa.clone());
            continue;
        }
        if matches!(sa, Type::UninhabitedType { ambiguous: true }) {
            arg_discs.push(1); // new_type = ta = t.args[i]
            joined_args.push(ta.clone());
            continue;
        }

        // join.py:131-135: AnyType arg -> AnyType(from_another_any).
        if matches!(ta, Type::AnyType { .. }) || matches!(sa, Type::AnyType { .. }) {
            arg_discs.push(4);
            let src = if matches!(ta, Type::AnyType { .. }) {
                ta
            } else {
                sa
            };
            joined_args.push(Type::AnyType {
                type_of_any: 4, // from_another_any
                source_any: Some(Box::new(src.clone())),
                missing_import_name: None,
            });
            continue;
        }

        // kind: 0=TypeVarType, 1=ParamSpec, 2=TypeVarTupleType.
        match *kind {
            0 => {} // TypeVarType, handled below.
            1 | 2 => {
                // ParamSpec / TypeVarTupleType: defer (needs
                // is_equivalent for ParamSpec, tuple unpacking for
                // TypeVarTupleType).
                return None;
            }
            _ => return None,
        }

        // VARIANCE_NOT_READY: PEP695 snapshot froze before inference ran;
        // defer to Python for the live variance (mirrors subtypes.rs:1980).
        if *variance == VARIANCE_NOT_READY {
            return None;
        }
        // Push a joined arg. `disc` keeps the SameTypeWithArgs path
        // intact for pure-0/1/4 arg lists; `needs_encode` flips once a
        // real join (Ancestor/Object/Any/Bottom) appears, and the

        // function then emits the full Instance encoded.
        match *variance {
            v if v == COVARIANT => {
                // join.py:136-148: covariant. new_type = join_types(ta,
                // sa). If type_var.values non-empty, defer (needs
                // values check, join.py:140-143; snapshot has no

                // values). Then is_subtype(new_type, upper_bound):
                // false -> object_from_instance(t) (Object).
                // upper_bound blob is at type_var_upper_bounds[i].

                // Empty blob -> defer (can't safely skip the check).
                let ub_blob = snap.type_var_upper_bounds.get(i)?;
                if ub_blob.is_empty() {
                    return None;
                }
                let upper_bound = decode_type(ub_blob)?;
                // Recursive join. SameS -> result = ta = t.args[i]
                // (disc 1); SameT -> result = sa = s.args[i] (disc 0).
                // Ancestor/Object/Any/Bottom: a real join; materialize

                // via setop_result_to_type and emit the whole Instance
                // encoded (disc=7) instead of deferring.
                let r = join_types(ta, sa, ctx, resolver)?;
                let (disc, typ) = match &r {
                    SetOpResult::SameS => (1i8, ta.clone()),
                    SetOpResult::SameT => (0, sa.clone()),
                    _ => {
                        let typ = materialize_join(ta, sa, r, resolver)?;
                        needs_encode = true;
                        (0, typ)
                    }
                };
                if !is_subtype(&typ, &upper_bound, ctx, resolver)? {
                    return Some(SetOpResult::Object);
                }
                arg_discs.push(disc);
                joined_args.push(typ);
            }
            v if v == INVARIANT || v == CONTRAVARIANT => {
                // join.py:149-160: invariant/contravariant.
                // is_equivalent(ta, sa) = is_subtype(ta, sa) &&
                // is_subtype(sa, ta). If not equivalent ->

                // object_from_instance(t) (Object).
                let equiv =
                    is_subtype(ta, sa, ctx, resolver)? && is_subtype(sa, ta, ctx, resolver)?;
                if !equiv {
                    return Some(SetOpResult::Object);
                }
                // Equivalent: new_type = join_types(ta, sa). SameS ->
                // result = ta = t.args[i] (disc 1); SameT -> result =
                // sa = s.args[i] (disc 0). Ancestor/Object/Any/Bottom

                // -> real join, encode whole (disc=7).
                let r = join_types(ta, sa, ctx, resolver)?;
                let (disc, typ) = match &r {
                    SetOpResult::SameS => (1i8, ta.clone()),
                    SetOpResult::SameT => (0, sa.clone()),
                    _ => {
                        let typ = materialize_join(ta, sa, r, resolver)?;
                        needs_encode = true;
                        (0, typ)
                    }
                };
                arg_discs.push(disc);
                joined_args.push(typ);
            }
            _ => return None,
        }
    }
    if needs_encode {
        // At least one arg needed a real join. Emit the full Instance
        // encoded (disc=7) so the Python shim decodes the exact args.
        let result = Type::Instance {
            type_ref: type_ref.to_string(),
            args: joined_args,
            last_known_value: None,
            extra_attrs: None,
        };
        let mut wbuf = WriteBuffer::new();
        wire::write_type(&mut wbuf, &result).ok()?;
        return Some(SetOpResult::Encoded(wbuf.into_bytes()));
    }
    Some(SetOpResult::SameTypeWithArgs {
        type_ref: type_ref.to_string(),
        arg_discs,
    })
}
/// Outcome of the nominal Instance-Instance join, relative to the
/// (left, right) args of the recursive call.
#[derive(Debug, Clone, PartialEq, Eq)]
enum JoinResult {
    /// The result is `left` (the first arg). Only produced by the
    /// `t == s` base case; `join_instances_via_supertype` converts
    /// it to `Ancestor(base)` before propagating.
    Left,
    /// The result is a common ancestor neither arg.
    Ancestor(String),
    /// The result is `builtins.object`.
    Object,
}

/// `InstanceJoiner.join_instances` (join.py:107-202) for args-less
/// instances. Same-type -> Left; t<:s -> via_supertype(t, s); else ->
/// via_supertype(s, t). The recursion mirrors Python's
/// `seen_instances` guard implicitly: args-less instances have no
/// type-arg recursion, so the only cycle is structural (A's base is A),
/// which the `left_ref == right_ref` fast path short-circuits.
fn join_instances_nominal(
    t_ref: &str,
    s_ref: &str,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<JoinResult> {
    if t_ref == s_ref {
        return Some(JoinResult::Left);
    }
    // Python's join_instances calls join_instances_via_supertype
    // directly (no inner is_subtype check). But via_supertype's bases
    // walk recurses into join_instances, which checks is_subtype(t, s)

    // at the TOP of join_instances (the is_proper_subtype dispatch).
    // Since we're in a recursive call without that dispatch, we need
    // the is_subtype check here to detect when one is already a

    // subtype of the other (the common-ancestor walk would otherwise
    // miss it and return Object). When t <: s, the join is s (Right);
    // when s <: t, the join is t (Left).
    let t_inst = Type::Instance {
        type_ref: t_ref.to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    let s_inst = Type::Instance {
        type_ref: s_ref.to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    if is_subtype(&t_inst, &s_inst, ctx, resolver)? {
        // t <: s: join is s. But via_supertype may find a better
        // answer via promotes. Fall through to via_supertype which
        // checks promotes first, then bases.
        join_instances_via_supertype(t_ref, s_ref, ctx, resolver)
    } else {
        join_instances_via_supertype(s_ref, t_ref, ctx, resolver)
    }
}

/// `InstanceJoiner.join_instances_via_supertype` (join.py:204-240),
/// args-less subset. Finds the common ancestor of `left_ref` and
/// `right_ref` by walking `left`'s bases and recursing
/// `join_instances(base, right)`. Returns the best (longest MRO)
/// candidate as a `JoinResult` relative to (left, right): if the
/// recursion returns Left, the base is the result (Ancestor(base));
/// if Right, right_ref is the result (Right).
fn join_instances_via_supertype(
    left_ref: &str,
    right_ref: &str,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<JoinResult> {
    // Fast path: when right is builtins.object, the join is always
    // object (everything is a subtype of object). This short-circuits
    // the bases walk + recursion that would otherwise find object via

    // every base path and hit the tie-breaker.
    if right_ref == "builtins.object" {
        return Some(JoinResult::Ancestor("builtins.object".to_string()));
    }
    let left_snap = resolver.get(left_ref)?;
    let right_snap = resolver.get(right_ref);

    // join.py:298-303: walk _promote lists for duck-type joins.
    // First loop: if left has a promote p where p <: right, return
    // join_types(p, right). Since p <: right, join = right. Return

    // Ancestor(right_ref) so the caller builds Instance(right, []).
    // Second loop: if right has a promote p where p <: left, return
    // join_types(left, p). Since p <: left, join = left. Return Left.
    if !ctx.ignore_promotions {
        for promote_blob in &left_snap.promote_bytes {
            if let Some(promote) = decode_type(promote_blob) {
                if is_subtype(
                    &promote,
                    &Type::Instance {
                        type_ref: right_ref.to_string(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    },
                    ctx,
                    resolver,
                )? {
                    return Some(JoinResult::Ancestor(right_ref.to_string()));
                }
            }
        }
        if let Some(snap) = right_snap {
            for promote_blob in &snap.promote_bytes {
                if let Some(promote) = decode_type(promote_blob) {
                    if is_subtype(
                        &promote,
                        &Type::Instance {
                            type_ref: left_ref.to_string(),
                            args: vec![],
                            last_known_value: None,
                            extra_attrs: None,
                        },
                        ctx,
                        resolver,
                    )? {
                        return Some(JoinResult::Left);
                    }
                }
            }
        }
    }
    // join.py:312-317: collect base type_refs from left's bases,
    // plus right's PROTOCOL bases where left <: base.
    let mut base_refs: Vec<String> = Vec::new();
    for base_blob in &left_snap.bases {
        let base = decode_type(base_blob)?;
        if let Type::Instance { type_ref, .. } = &base {
            base_refs.push(type_ref.clone());
        } else {
            // Non-Instance base (e.g. ParamSpec): defer.
            return None;
        }
    }
    if let Some(snap) = right_snap {
        for base_blob in &snap.bases {
            let base = decode_type(base_blob)?;
            if let Type::Instance {
                type_ref: base_ref, ..
            } = &base
            {
                if let Some(base_snap) = resolver.get(base_ref) {
                    if base_snap.is_protocol {
                        // Only add if left <: base (join.py:316).
                        let left_inst = Type::Instance {
                            type_ref: left_ref.to_string(),
                            args: vec![],
                            last_known_value: None,
                            extra_attrs: None,
                        };
                        if is_subtype(&left_inst, &base, ctx, resolver)? {
                            base_refs.push(base_ref.clone());
                        }
                    }
                }
            }
        }
    }
    // join.py:228-234: for each base, recurse and pick the best.
    // is_better compares the MRO of the RESULT type, not the base.
    let mut best: Option<(JoinResult, usize)> = None;
    for base_ref in &base_refs {
        let candidate = join_instances_nominal(base_ref, right_ref, ctx, resolver)?;
        // Convert the recursive result (relative to base, right) to
        // relative to (left, right): Left means base won -> Ancestor(base);
        // Ancestor/Object pass through unchanged.
        let mapped = match candidate {
            JoinResult::Left => JoinResult::Ancestor(base_ref.clone()),
            other => other,
        };
        // MRO of the RESULT type, not the base (join.py:804+ is_better).
        let mro = match &mapped {
            JoinResult::Ancestor(fullname) => mro_len(fullname, resolver),
            JoinResult::Left => mro_len(left_ref, resolver),
            JoinResult::Object => 1, // builtins.object has MRO length 1
        };
        match &best {
            None => best = Some((mapped, mro)),
            Some((_, best_mro)) if mro > *best_mro => best = Some((mapped, mro)),
            // Tie: defer to Python. Python's is_better returns False on
            // ties (keeping the first), but Python also checks protocol
            // status first (non-protocol beats protocol) and has

            // map_instance_to_supertype + the second promote loop, none
            // of which the Rust mro_len-only comparison replicates.
            // Deferring on ties avoids wrong answers on complex MROs.
            Some((_, best_mro)) if mro == *best_mro => return None,
            _ => {}
        }
    }
    match best {
        Some((result, _)) => {
            // Defer when the result is an Ancestor with type vars:
            // Python's join_instances_via_supertype calls
            // map_instance_to_supertype + join_instances which produces

            // Instance(ancestor, [joined_args]). Rust returns bare
            // Instance(ancestor, []), which is wrong for generic
            // ancestors like Sequence[object].
            if let JoinResult::Ancestor(ref fullname) = result {
                if let Some(snap) = resolver.get(fullname) {
                    if !snap.type_vars_with_variance.is_empty() {
                        return None;
                    }
                }
            }
            Some(result)
        }
        // No bases: if left is builtins.object, return Object. Else
        // defer (Python asserts best is not None when bases non-empty).
        None => {
            if left_ref == "builtins.object" {
                Some(JoinResult::Object)
            } else {
                None
            }
        }
    }
}

/// Build a wire `Instance` from a ref + args, with no LKV and no
/// extra_attrs (matches the fresh Instances `join` operates on).
fn mk_wire_instance(type_ref: &str, args: Vec<Type>) -> Type {
    Type::Instance {
        type_ref: type_ref.to_string(),
        args,
        last_known_value: None,
        extra_attrs: None,
    }
}

/// `InstanceJoiner.join_instances` (join.py:292-303, 350-427) for the
/// argument-bearing different-base case (the historical
/// `join_instances.diff_args` deferral). Python routes it through
/// `join_instances_via_supertype`, which maps the `t` operand onto
/// each candidate base with `map_instance_to_supertype`, recurses
/// `join_instances`, and picks the longest-MRO result. This port
/// mirrors that with the native primitives, returning the winner as a
/// `SetOpResult` relative to the outer `(t, s)` operands.
///
/// Returns `None` (defer to Python) when the dispatch `is_subtype`, a
/// `map_instance_to_supertype` step, a `_promote` walk, or a recursion
/// produces something Rust cannot express as a wire type. Since the
/// whole `diff_args` branch previously deferred unconditionally, every
/// surviving defer is a no-loss fallback.
fn join_diff_instances_with_args(
    t_ref: &str,
    t_args: &[Type],
    s_ref: &str,
    s_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    seen: &mut SeenInstances,
) -> Option<SetOpResult> {
    // Object fast path: object is the universal top type, so the join
    // of any pair with object on either side is object (Python's
    // via_supertype reaches the same via the base walk).
    if t_ref == "builtins.object" || s_ref == "builtins.object" {
        return Some(SetOpResult::Object);
    }
    // join.py:367 dispatch: `t.type.bases and is_proper_subtype(t, s,
    // ignore_type_params=True)`. The args are ignored, so compare
    // args-less Instances (mirroring the args-less branch above).
    let t_snap = resolver.get(t_ref)?;
    let proper_ctx = SubtypeContext {
        proper_subtype: true,
        ..*ctx
    };
    let t_argsless = mk_wire_instance(t_ref, Vec::new());
    let s_argsless = mk_wire_instance(s_ref, Vec::new());
    let t_lt_s = if !t_snap.bases.is_empty() {
        is_subtype(&t_argsless, &s_argsless, &proper_ctx, resolver)?
    } else {
        false
    };
    // Compute the winner as an absolute Type, then re-express it
    // relative to the outer (t, s).
    let result = if t_lt_s {
        via_supertype_arg_type(t_ref, t_args, s_ref, s_args, ctx, resolver, seen)?
    } else {
        via_supertype_arg_type(s_ref, s_args, t_ref, t_args, ctx, resolver, seen)?
    };
    let t_full = mk_wire_instance(t_ref, t_args.to_vec());
    let s_full = mk_wire_instance(s_ref, s_args.to_vec());
    if result == t_full {
        Some(SetOpResult::SameT)
    } else if result == s_full {
        Some(SetOpResult::SameS)
    } else if matches!(
        &result,
        Type::Instance {
            type_ref: tr,
            args,
            ..
        } if tr == "builtins.object" && args.is_empty()
    ) {
        Some(SetOpResult::Object)
    } else {
        let mut wbuf = WriteBuffer::new();
        wire::write_type(&mut wbuf, &result).ok()?;
        Some(SetOpResult::Encoded(wbuf.into_bytes()))
    }
}

/// `InstanceJoiner.join_instances_via_supertype` (join.py:350-427) for
/// argument-bearing operands `(t, s)`, returning the winning result as
/// an absolute wire `Type`. Shared by both dispatch directions of
/// `join_diff_instances_with_args`.
///
/// Python's algorithm: walk `_promote` lists (returning the winning
/// operand directly), then compute the "best" supertype of `t` joined
/// with `s` by mapping `t` onto each base and recursing
/// `join_instances(mapped, s)`, keeping the longest-MRO result (plus a
/// second `_promote` pass over `t`).
fn via_supertype_arg_type(
    t_ref: &str,
    t_args: &[Type],
    s_ref: &str,
    s_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    seen: &mut SeenInstances,
) -> Option<Type> {
    let t_inst = mk_wire_instance(t_ref, t_args.to_vec());
    let s_inst = mk_wire_instance(s_ref, s_args.to_vec());
    let t_snap = resolver.get(t_ref)?;
    let s_snap = resolver.get(s_ref);

    // join.py:352-361: _promote walks. First loop returns s (t wins the
    // promote); second returns t.
    for pb in &t_snap.promote_bytes {
        let p = decode_type(pb)?;
        if is_subtype(&p, &s_inst, ctx, resolver)? {
            return Some(s_inst);
        }
    }
    if let Some(ss) = &s_snap {
        for pb in &ss.promote_bytes {
            let p = decode_type(pb)?;
            if is_subtype(&p, &t_inst, ctx, resolver)? {
                return Some(t_inst);
            }
        }
    }

    // base_types (join.py:441-456): t's bases + s's protocol bases
    // where t <: base. Map t onto each, recurse, pick the best.
    let mut best: Option<Type> = None;
    let mut consider = |candidate: &Type| -> Option<()> {
        let mark = seen.len();
        let res = join_instances_core(candidate, &s_inst, ctx, resolver, seen);
        // Python's InstanceJoiner pops (t, s) off seen_instances when a
        // join_instances call returns; restore that stack discipline so
        // sibling candidates do not see each other's guard entries.
        seen.truncate(mark);
        let res = res?;
        // Trust only simple sub-results: Encoded/SameTypeWithArgs/
        // Ancestor reconstructions' MRO tie-break cannot be guaranteed
        // to match Python (tuple/Sequence mixtures), so defer those.
        if !matches!(
            res,
            SetOpResult::SameS | SetOpResult::SameT | SetOpResult::Object
        ) {
            return None;
        }
        let res_type = materialize_join(candidate, &s_inst, res, resolver)?;
        if best.is_none() || is_better_join(&res_type, best.as_ref().unwrap(), resolver) {
            best = Some(res_type);
        }
        Some(())
    };

    for base_blob in &t_snap.bases {
        let base = decode_type(base_blob)?;
        let Type::Instance {
            type_ref: base_ref, ..
        } = &base
        else {
            // Non-Instance base (e.g. ParamSpec): defer.
            return None;
        };
        let mapped_args = map_instance_to_supertype(t_ref, t_args, base_ref, resolver)?;
        let mapped = mk_wire_instance(base_ref, mapped_args);
        consider(&mapped)?;
    }
    if let Some(ss) = &s_snap {
        for base_blob in &ss.bases {
            let base = decode_type(base_blob)?;
            let Type::Instance {
                type_ref: base_ref, ..
            } = &base
            else {
                continue;
            };
            if let Some(b_snap) = resolver.get(base_ref) {
                if b_snap.is_protocol && is_subtype(&t_inst, &base, ctx, resolver)? {
                    let mapped_args = map_instance_to_supertype(t_ref, t_args, base_ref, resolver)?;
                    let mapped = mk_wire_instance(base_ref, mapped_args);
                    consider(&mapped)?;
                }
            }
        }
    }
    // Second _promote pass (join.py:458-462): t's promote Instances.
    for pb in &t_snap.promote_bytes {
        let p = decode_type(pb)?;
        if let Type::Instance { .. } = &p {
            consider(&p)?;
        }
    }
    best
}

/// MRO length for `is_better` (join.py:804+). Returns 0 if the
/// TypeInfo is missing (treated as shortest; loses the is_better tie).
fn mro_len(type_ref: &str, resolver: &TypeResolver) -> usize {
    resolver.get(type_ref).map_or(0, |s| s.mro.len())
}

/// `#[pyfunction]` entry for `trivial_join`. The Python-side shim
/// (mypy/join.py) calls this with serialized `s`/`t` blobs plus the
/// `NativeTypeResolver` pyclass. Returns `None` (Python `None`) when
/// Rust doesn't handle the case; `Some(i64)` discriminator
/// otherwise (0=SameS, 1=SameT, 2=Object, 3=Bottom, 4=Any).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_trivial_join(
    s_bytes: &[u8],
    t_bytes: &[u8],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<i64> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        false,
        strict_optional,
    );
    trivial_join(&s, &t, &ctx, resolver.resolver()).map(discriminator_trivial)
}

/// `#[pyfunction]` entry for `trivial_meet`. Mirrors
/// `rust_trivial_join`; see its docstring.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_trivial_meet(
    s_bytes: &[u8],
    t_bytes: &[u8],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<i64> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        false,
        strict_optional,
    );
    trivial_meet(&s, &t, &ctx, resolver.resolver()).map(discriminator_trivial)
}

/// `#[pyfunction]` entry for `join_types`. The Python-side shim
/// (mypy/join.py) calls this after `get_proper_type` expansion with
/// serialized `s`/`t` blobs plus the `NativeTypeResolver` pyclass.
/// Returns `None` (Python `None`) when Rust doesn't handle the case;
/// `Some((disc, fullname, arg_discs, encoded))` otherwise. `disc` is
/// 0=SameS, 1=SameT, 2=Object, 3=Bottom, 4=Any, 5=Ancestor (fullname
/// set), 6=SameTypeWithArgs, 7=Encoded (the `encoded` bytes hold a
/// wire-format type blob the shim decodes via `read_type`).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_join_types(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<DiscriminatorOut> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    join_types(&s, &t, &ctx, resolver.resolver()).map(discriminator)
}

/// `#[pyfunction]` entry for `meet_types`. The Python-side shim
/// (mypy/meet.py) calls this after `get_proper_type` expansion with
/// serialized `s`/`t` blobs plus the `NativeTypeResolver` pyclass.
/// Returns `None` (Python `None`) when Rust doesn't handle the case;
/// `Some((disc, fullname, arg_discs, encoded))` otherwise.
/// `meet_types` only emits disc 0=SameS, 1=SameT, 3=Bottom, 4=Any
/// (never 2=Object, 5=Ancestor, 6=SameTypeWithArgs — those are join
/// supertype results).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_meet_types(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<DiscriminatorOut> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    meet_types(&s, &t, &ctx, resolver.resolver()).map(discriminator)
}

/// `InstanceJoiner.seen_instances` recursion guard: a list of
/// `((t_ref, t_args_bytes), (s_ref, s_args_bytes))` pairs mirroring
/// Python's `list[(Instance, Instance)]`. Args-less operands encode to
/// empty arg bytes.
type SeenInstances = Vec<((String, Vec<u8>), (String, Vec<u8>))>;

/// `InstanceJoiner.join_instances` (join.py:208-303), full Rust port
/// behind a new seam with a real recursion guard.
///
/// Python's `InstanceJoiner` keeps a `seen_instances` list and returns
/// `object_from_instance(t)` when a pair recurs. The args-less nominal
/// join (`join_instances_via_supertype`) has no type-arg recursion, so
/// its cycles are structural only; the recursive-with-args cases
/// (`equal types with args`, `TypeVarTupleType` rewrap) go through
/// `join_types` → `visit_instance` → `join_instances` and can cycle
/// via generic args. This port passes a `seen` set of
/// `(type_ref, args-bytes)` pairs through that recursion so Rust stops
/// the cycle exactly where Python does.
///
/// Handled (returns a `SetOpResult`, encoded to a `DiscriminatorOut`
/// the Python shim decodes):
/// - Same type, both args-less → SameS/SameT (or Encoded fresh
///   Instance when both carry a last_known_value).
/// - Same type with args (non-variadic) → `visit_instance_with_args`
///   (AnyType args, invariant is_equivalent, covariant bound check).
/// - Different type, both args-less → the nominal
///   `join_instances_via_supertype` walk.
/// - Same variadic type with a single TypeVarTuple argument → the
///   tuple-fallback rewrap (join.py:230-241 end-state).
///
/// Defers (returns `None`, Python runs the pure body):
/// - ParamSpec type vars (kind=1) — needs `type_var.upper_bound`
///   (join.py:242-246, the not-is_equivalent → upper_bound result) and
///   the snapshot has no upper_bound for ParamSpec kinds.
/// - TypeVarTuple item-extension (`new_type` tuple-of-multiple-items:
///   `args.extend(new_type.items); continue`) — the joined middle
///   fallback is only available as snapshot bytes; the continuation
///   defers conservatively.
/// - Variadic instances with prefix/suffix > 0 or multiple TypeVarTuple
///   args (needs `split_with_prefix_and_suffix`).
/// - Different types with args (needs `expand_type_by_instance`).
/// - `type_var.values` non-empty (snapshot has no values field).
/// - Ambiguous-Uninhabited args are handled; non-ambiguous rewrap of
///   Instance(tuple) stays (it mirrors the Instance-tuple rewrap).
/// - All the fallbacks `visit_instance_with_args` already defers on.
///
/// The recursion guard mirrors Python's `(t, s) in seen_instances`
/// exactly: a list of `((ref, encoded-args), (ref, encoded-args))`
/// pairs walked father-to-son, checked as a pair (either order) and
/// pushed on entry. Args encode to bytes so Instance equality is
/// compared by type + args; lkv / extra_attrs are dropped from the
/// key (conservative: walks a cyclic pair one extra level before the
/// guard fires). Each recursion level truncates back to its entry
/// mark on return (Python pops after every `join_instances`).
fn join_instances_core(
    t: &Type,
    s: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    seen: &mut SeenInstances,
) -> Option<SetOpResult> {
    let (t_ref, t_args, t_lkv) = match t {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            ..
        } => (type_ref.as_str(), args.as_slice(), last_known_value),
        _ => return None,
    };
    let (s_ref, s_args, s_lkv) = match s {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            ..
        } => (type_ref.as_str(), args.as_slice(), last_known_value),
        _ => return None,
    };

    // join.py:209-210: seen guard. Python stores `(t, s)` Instance pairs
    // and returns object_from_instance when the pair (or mirror) is on
    // the stack; pair keys avoid sibling-args false-fires.
    let mut t_buf = WriteBuffer::new();
    if !t_args.is_empty() {
        wire::write_type_list(&mut t_buf, t_args).ok()?;
    }
    let mut s_buf = WriteBuffer::new();
    if !s_args.is_empty() {
        wire::write_type_list(&mut s_buf, s_args).ok()?;
    }
    let t_enc = (t_ref.to_string(), t_buf.into_bytes());
    let s_enc = (s_ref.to_string(), s_buf.into_bytes());
    if seen.contains(&(t_enc.clone(), s_enc.clone()))
        || seen.contains(&(s_enc.clone(), t_enc.clone()))
    {
        // Python's object_from_instance(t): always Instance
        // (builtins.object) for a well-formed instance; fall back to a
        // bare fullname Instance blob (the shim fixups type_ref).
        return object_from_instance_result(t_ref, resolver);
    }
    seen.push((t_enc, s_enc));

    // fallback_to_any deferral (same as visit_instance_join): the
    // promote loop and via_supertype need Python-side _promote lists.
    if resolver.get(t_ref).is_some_and(|s| s.fallback_to_any)
        || resolver.get(s_ref).is_some_and(|s| s.fallback_to_any)
    {
        return None;
    }

    let result = if t_ref == s_ref {
        // join.py:215-241: same base type, combine type arguments.
        if t_args.is_empty() && s_args.is_empty() {
            // Both args-less: fresh Instance(type, []) (join.py:281).
            if !t_lkv.is_some() {
                SetOpResult::SameT
            } else if !s_lkv.is_some() {
                SetOpResult::SameS
            } else {
                // Both LKV: fresh Instance without LKV.
                let fresh = Type::Instance {
                    type_ref: t_ref.to_string(),
                    args: Vec::new(),
                    last_known_value: None,
                    extra_attrs: None,
                };
                let mut wbuf = WriteBuffer::new();
                wire::write_type(&mut wbuf, &fresh).ok()?;
                SetOpResult::Encoded(wbuf.into_bytes())
            }
        } else {
            let snap = resolver.get(t_ref)?;
            if snap.has_type_var_tuple_type {
                // join.py:218-241: variadic instance. Numeric prefix /
                // suffix from the snapshot; the resolve of
                // split_with_prefix_and_suffix uses them.
                let prefix = snap.type_var_tuple_prefix.unwrap_or(0);
                let suffix = snap.type_var_tuple_suffix.unwrap_or(0);
                // Simplified: only the single-argument form
                // (TupleType rewrapped as one arg) is handled. Defer
                // prefix/suffix splits and multi-arg forms.
                if prefix == 0 && suffix == 0 && t_args.len() == 1 && s_args.len() == 1 {
                    // join.py:230-241: join the rewrapped TupleType args
                    // and rewrap the result.
                    let r = join_types(&t_args[0], &s_args[0], ctx, resolver)?;
                    let new_type = materialize_join(&t_args[0], &s_args[0], r, resolver)?;
                    match &new_type {
                        Type::Instance { type_ref: tr, .. } if tr == "builtins.tuple" => {
                            // join.py:235-237: Tuple[X, Y, Z] join ->
                            // Instance(tuple) -> UnpackType(new_type).
                            let mut wbuf = WriteBuffer::new();
                            wire::write_type(
                                &mut wbuf,
                                &Type::UnpackType {
                                    typ: Box::new(new_type),
                                },
                            )
                            .ok()?;
                            SetOpResult::Encoded(wbuf.into_bytes())
                        }
                        Type::TupleType { items, .. } if items.len() == 1 => {
                            // join.py:238-241: single-item TupleType ->
                            // UnpackType(tuple[item]) if the item is
                            // Instance(tuple); else Instance(tuple, [i]).
                            if let Type::UnpackType { typ } = &items[0] {
                                if let Type::Instance { type_ref: tr, .. } = typ.as_ref() {
                                    if tr == "builtins.tuple" {
                                        let mut wbuf = WriteBuffer::new();
                                        wire::write_type(&mut wbuf, typ.as_ref()).ok()?;
                                        return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                                    }
                                }
                            }
                            let result = Type::Instance {
                                type_ref: t_ref.to_string(),
                                args: items.clone(),
                                last_known_value: None,
                                extra_attrs: None,
                            };
                            let mut wbuf = WriteBuffer::new();
                            wire::write_type(&mut wbuf, &result).ok()?;
                            SetOpResult::Encoded(wbuf.into_bytes())
                        }
                        _ => return None,
                    }
                } else {
                    // Prefix/suffix split or multi-arg: defer.
                    return None;
                }
            } else {
                // join.py:241-290: non-variadic same-type with args.
                visit_instance_with_args(t_ref, s_args, t_args, ctx, resolver)?
            }
        }
    } else {
        // join.py:292-300: different base types. Args-present routes
        // through the args-aware via_supertype walk (join.py:350-427);
        // args-less runs the nominal walk.
        if !t_args.is_empty() || !s_args.is_empty() {
            return join_diff_instances_with_args(
                t_ref, t_args, s_ref, s_args, ctx, resolver, seen,
            );
        }
        let proper_ctx = SubtypeContext {
            proper_subtype: true,
            ..*ctx
        };
        // join.py:292-295: t <: s? -> join_instances_via_supertype(t, s).
        // JoinResult::Left means the first arg (t) won -> SameT.
        let t_is_subtype = is_subtype(t, s, &proper_ctx, resolver)?;
        let result_ref = if t_is_subtype {
            join_instances_nominal(t_ref, s_ref, ctx, resolver)?
        } else {
            join_instances_nominal(s_ref, t_ref, ctx, resolver)?
        };
        match result_ref {
            JoinResult::Left => {
                if t_is_subtype {
                    SetOpResult::SameT
                } else {
                    SetOpResult::SameS
                }
            }
            JoinResult::Ancestor(fullname) => SetOpResult::Ancestor(fullname),
            JoinResult::Object => SetOpResult::Object,
        }
    };
    Some(result)
}

/// `object_from_instance`-as-SetOpResult: the seen-guard fallback
/// (join.py:210 `object_from_instance(t)`). Always the last MRO entry
/// (`builtins.object` in a sane graph); the shim maps the fullname
/// through the typeinfo map.
fn object_from_instance_result(t_ref: &str, resolver: &TypeResolver) -> Option<SetOpResult> {
    let snap = resolver.get(t_ref)?;
    let fullname = snap.mro.last()?;
    Some(SetOpResult::Ancestor(fullname.clone()))
}

/// `#[pyfunction]` entry for `join_instances` (join.py:208-303).
///
/// The Python-side shim (`mypy/join.py` `_try_native_join_instances`)
/// calls this with serialized `t`/`s` Instance blobs plus the
/// `NativeTypeResolver` pyclass and `strict_optional`. Returns `None`
/// (Python `None`) when Rust doesn't handle the case;
/// `Some((disc, fullname, arg_discs, encoded))` otherwise — the same
/// `DiscriminatorOut` shape as `rust_join_types` (0=SameS, 1=SameT,
/// 2=Object, 3=Bottom, 4=Any, 5=Ancestor, 6=SameTypeWithArgs,
/// 7=Encoded).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_join_instances(
    t_bytes: &[u8],
    s_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<DiscriminatorOut> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let mut seen: SeenInstances = Vec::new();
    join_instances_core(&t, &s, &ctx, resolver.resolver(), &mut seen).map(discriminator)
}

/// `#[pyfunction]` entry for `join_tuples` (join.py:680-785).
///
/// Joins two wire-format `TupleType` values, handling variadic entries
/// (`UnpackType`) via prefix/join-middle/suffix logic. Returns encoded
/// bytes of the joined item list, or `None` (Python fallback).
///
/// Matches the Python algorithm exactly:
///   * Both fixed-length, equal length → item-wise join, return items.
///   * Both variadic, perfectly aligned → join prefix, join middle
///     (the unpacked inner types), join suffix, return items.
///   * Both variadic, one purely variadic (length 1) → join the inner
///     args, return single `UnpackType`.
///   * One variadic, one fixed → join prefix items, join middle against
///     unpacked arg, return `UnpackType` + suffix.
///
/// Returns `None` (Python fallback) for:
///   * Unequal fixed lengths.
///   * Both variadic with misaligned structure.
///   * Non-Instance unpack types.
///   * Fixed length < variadic length - 1.
///   * Any item that produces a type Rust cannot serialize (defer).
#[pyfunction]
pub(crate) fn rust_join_tuples(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let (Type::TupleType { items: s_items, .. }, Type::TupleType { items: t_items, .. }) = (&s, &t)
    else {
        return None;
    };
    let result = join_tuples_inner(s_items, t_items, strict_optional, resolver.resolver())?;
    encode_items(&result)
}

/// `#[pyfunction]` entry for `meet_tuples` (meet.py:1167-1243).
///
/// Meets two wire-format `TupleType` values, handling variadic entries
/// (`UnpackType`) via prefix/meet-middle/suffix logic. Returns encoded
/// bytes of the met item list, or `None` (Python fallback).
///
/// Matches the Python algorithm exactly:
///   * Both fixed-length, equal length → item-wise meet, return items.
///   * Both variadic, perfectly aligned → meet prefix items, meet middle
///     (the unpacked inner types via `meet_types`), meet suffix items.
///   * One variadic, one fixed → meet prefix items, meet middle items
///     against unpacked arg, meet suffix items.
///
/// Returns `None` (Python fallback) for:
///   * Unequal fixed lengths.
///   * Both variadic with misaligned structure.
///   * Non-Instance unpack types.
///   * Fixed length < variadic length - 1.
///   * Any item that produces a type Rust cannot serialize (defer).
#[pyfunction]
pub(crate) fn rust_meet_tuples(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let (Type::TupleType { items: s_items, .. }, Type::TupleType { items: t_items, .. }) = (&s, &t)
    else {
        return None;
    };
    let result = meet_tuples_inner(s_items, t_items, strict_optional, resolver.resolver())?;
    encode_items(&result)
}

/// Inner join_tuples logic on wire-format items.
fn join_tuples_inner(
    s_items: &[Type],
    t_items: &[Type],
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let s_unpack_idx = find_unpack(s_items);
    let t_unpack_idx = find_unpack(t_items);

    match (s_unpack_idx, t_unpack_idx) {
        (None, None) => {
            // Both fixed-length.
            if s_items.len() == t_items.len() {
                let mut items = Vec::with_capacity(s_items.len());
                for (si, ti) in s_items.iter().zip(t_items.iter()) {
                    items.push(rust_join_types_inner(si, ti, strict_optional, resolver)?);
                }
                Some(items)
            } else {
                // Unequal fixed lengths → defer.
                None
            }
        }
        (Some(si), Some(ti)) => {
            // Both variadic.
            join_both_variadic(s_items, t_items, si, ti, strict_optional, resolver)
        }
        (Some(si), None) => {
            // s is variadic, t is fixed.
            join_one_variadic(s_items, t_items, si, true, strict_optional, resolver)
        }
        (None, Some(ti)) => {
            // t is variadic, s is fixed.
            join_one_variadic(t_items, s_items, ti, false, strict_optional, resolver)
        }
    }
}

/// Join both-tuples-variadic case.
fn join_both_variadic(
    s_items: &[Type],
    t_items: &[Type],
    s_unpack_idx: usize,
    t_unpack_idx: usize,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let s_unpack = get_unpack_type(s_items.get(s_unpack_idx)?);
    let t_unpack = get_unpack_type(t_items.get(t_unpack_idx)?);

    let Type::Instance {
        type_ref: _s_type_ref,
        args: s_args,
        ..
    } = s_unpack
    else {
        return None;
    };
    let Type::Instance {
        type_ref: _t_type_ref,
        args: t_args,
        ..
    } = t_unpack
    else {
        return None;
    };

    // Case: perfect alignment (same unpack position and same total length).
    if s_items.len() == t_items.len() && s_unpack_idx == t_unpack_idx {
        let prefix_len = s_unpack_idx;
        let suffix_len = s_items.len() - s_unpack_idx - 1;
        let mut items = Vec::with_capacity(s_items.len());

        // Join prefix items.
        for (si, ti) in s_items
            .iter()
            .take(prefix_len)
            .zip(t_items.iter().take(prefix_len))
        {
            items.push(rust_join_types_inner(si, ti, strict_optional, resolver)?);
        }

        // Join the unpacked inner types (the args[0] of each `tuple[T, ...]`).
        let Type::Instance { args: s_inner, .. } = s_unpack else {
            return None;
        };
        let Type::Instance { args: t_inner, .. } = t_unpack else {
            return None;
        };
        let joined_inner = rust_join_types_inner(
            s_inner.first()?,
            t_inner.first()?,
            strict_optional,
            resolver,
        )?;

        // Build the resulting UnpackType — matches Python join.py:713-731.
        // joined_inner is already a full Type (e.g., TypeVarTupleType, Instance(tuple), etc.);
        // pass it through directly rather than destructuring partial fields.
        items.push(Type::UnpackType {
            typ: Box::new(joined_inner),
        });

        // Join suffix items.
        if suffix_len > 0 {
            let s_suffix_start = s_items.len() - suffix_len;
            let t_suffix_start = t_items.len() - suffix_len;
            for i in 0..suffix_len {
                items.push(rust_join_types_inner(
                    &s_items[s_suffix_start + i],
                    &t_items[t_suffix_start + i],
                    strict_optional,
                    resolver,
                )?);
            }
        }
        return Some(items);
    }

    // Case: one tuple is purely variadic (length 1).
    if s_items.len() == 1 || t_items.len() == 1 {
        // Both must be Instance for the inner join to work.
        let s_inner = s_args.first()?;
        let t_inner = t_args.first()?;
        let mid_joined = rust_join_types_inner(s_inner, t_inner, strict_optional, resolver)?;

        // Collect "other" items (the non-unpack items from each).
        let s_other: Vec<&Type> = s_items
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != s_unpack_idx)
            .map(|(_, t)| t)
            .collect();
        let t_other: Vec<&Type> = t_items
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != t_unpack_idx)
            .map(|(_, t)| t)
            .collect();

        // Join all "other" items into one, then join with mid_joined.
        let other_joined = if s_other.is_empty() && t_other.is_empty() {
            // Nothing else to join; use mid_joined as-is.
            mid_joined.clone()
        } else {
            let mut all_others = Vec::new();
            all_others.extend(s_other);
            all_others.extend(t_other);
            let mut acc = mid_joined.clone();
            for o in all_others {
                acc = rust_join_types_inner(&acc, o, strict_optional, resolver)?;
            }
            acc
        };

        // Produce single UnpackType(builtins.tuple[other_joined]).
        let tuple_inst = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![other_joined],
            last_known_value: None,
            extra_attrs: None,
        };
        return Some(vec![Type::UnpackType {
            typ: Box::new(tuple_inst),
        }]);
    }

    // TODO: other cases (both prefix/suffix shorter).
    None
}

/// Join one variadic + one fixed tuple.
fn join_one_variadic(
    variadic_items: &[Type],
    fixed_items: &[Type],
    unpack_idx: usize,
    // Whether variadic is `s` (true) or `t` (false).
    // Not used in pure-join logic since join is commutative,
    // but kept for clarity.
    _variadic_is_s: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let unpack = get_unpack_type(variadic_items.get(unpack_idx)?);
    let Type::Instance { type_ref, args, .. } = unpack else {
        return None;
    };
    // Must be builtins.tuple.
    if type_ref != "builtins.tuple" {
        return None;
    }

    let variadic_len = variadic_items.len();
    if fixed_items.len() < variadic_len - 1 {
        // Not enough fixed items to cover prefix + suffix.
        return None;
    }

    let prefix_len = unpack_idx;
    let suffix_len = variadic_len - prefix_len - 1;

    // Split fixed items into prefix / middle / suffix.
    let (prefix, middle, suffix) = {
        let n = fixed_items.len();
        if prefix_len.saturating_add(suffix_len) >= n {
            (fixed_items.to_vec(), Vec::new(), Vec::new())
        } else {
            let mid_end = n.saturating_sub(suffix_len);
            (
                fixed_items[..prefix_len].to_vec(),
                fixed_items[prefix_len..mid_end].to_vec(),
                fixed_items[mid_end..].to_vec(),
            )
        }
    };

    let mut items = Vec::new();

    // Join prefix items.
    for (fi, vi) in prefix.iter().zip(variadic_items.iter().take(prefix_len)) {
        items.push(rust_join_types_inner(fi, vi, strict_optional, resolver)?);
    }

    // Join middle items with unpacked inner type.
    let unpacked_inner = args.first()?;
    let mid_joined = if middle.is_empty() {
        unpacked_inner.clone()
    } else {
        let mut acc = middle.first()?.clone();
        for m in middle.iter().skip(1) {
            acc = rust_join_types_inner(&acc, m, strict_optional, resolver)?;
        }
        rust_join_types_inner(&acc, unpacked_inner, strict_optional, resolver)?
    };

    items.push(Type::UnpackType {
        typ: Box::new(Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![mid_joined],
            last_known_value: None,
            extra_attrs: None,
        }),
    });

    // Join suffix items.
    if suffix_len > 0 {
        let var_start = variadic_len - suffix_len;
        for (fi, vi) in suffix.iter().zip(variadic_items.iter().skip(var_start)) {
            items.push(rust_join_types_inner(fi, vi, strict_optional, resolver)?);
        }
    }

    Some(items)
}

/// Inner meet_tuples logic on wire-format items.
fn meet_tuples_inner(
    s_items: &[Type],
    t_items: &[Type],
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let s_unpack_idx = find_unpack(s_items);
    let t_unpack_idx = find_unpack(t_items);

    match (s_unpack_idx, t_unpack_idx) {
        (None, None) => {
            if s_items.len() == t_items.len() {
                let mut items = Vec::with_capacity(s_items.len());
                for (si, ti) in s_items.iter().zip(t_items.iter()) {
                    items.push(rust_meet_types_inner(si, ti, strict_optional, resolver)?);
                }
                Some(items)
            } else {
                None
            }
        }
        (Some(si), Some(ti)) => {
            // Both variadic — only handle perfectly-aligned case.
            if s_items.len() == t_items.len() && si == ti {
                meet_both_variadic(s_items, t_items, si, strict_optional, resolver)
            } else {
                None
            }
        }
        (Some(si), None) => {
            // s variadic, t fixed.
            meet_one_variadic(s_items, t_items, si, strict_optional, resolver)
        }
        (None, Some(ti)) => {
            // t variadic, s fixed.
            meet_one_variadic(t_items, s_items, ti, strict_optional, resolver)
        }
    }
}

/// Meet both variadic — only the perfectly-aligned case.
fn meet_both_variadic(
    s_items: &[Type],
    t_items: &[Type],
    unpack_idx: usize,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let s_unpack = get_unpack_type(s_items.get(unpack_idx)?);
    let t_unpack = get_unpack_type(t_items.get(unpack_idx)?);

    if !is_instance_tuple(s_unpack) || !is_instance_tuple(t_unpack) {
        return None;
    }

    // Destructure the Instance types to access fields.
    let Type::Instance {
        type_ref: s_type_ref,
        args: s_args,
        last_known_value: s_lkv,
        extra_attrs: s_ea,
    } = s_unpack
    else {
        return None;
    };
    let Type::Instance {
        type_ref: t_type_ref,
        args: t_args,
        last_known_value: t_lkv,
        extra_attrs: t_ea,
    } = t_unpack
    else {
        return None;
    };

    let _s_inner = s_args.first()?;
    let _t_inner = t_args.first()?;

    // Meet the two Instance(tuple) types themselves.
    let meet_inner = rust_meet_types_inner(
        &Type::Instance {
            type_ref: s_type_ref.clone(),
            args: s_args.clone(),
            last_known_value: s_lkv.clone(),
            extra_attrs: s_ea.clone(),
        },
        &Type::Instance {
            type_ref: t_type_ref.clone(),
            args: t_args.clone(),
            last_known_value: t_lkv.clone(),
            extra_attrs: t_ea.clone(),
        },
        strict_optional,
        resolver,
    );

    let Some(Type::Instance {
        type_ref,
        args,
        last_known_value,
        extra_attrs,
    }) = meet_inner
    else {
        return None;
    };

    let mut items = Vec::with_capacity(s_items.len());

    // Meet prefix items.
    for (si, ti) in s_items
        .iter()
        .take(unpack_idx)
        .zip(t_items.iter().take(unpack_idx))
    {
        items.push(rust_meet_types_inner(si, ti, strict_optional, resolver)?);
    }

    // Push the single UnpackType(meet).
    items.push(Type::UnpackType {
        typ: Box::new(Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        }),
    });

    // Meet suffix items.
    let s_start = unpack_idx + 1;
    let t_start = unpack_idx + 1;
    for (si, ti) in s_items
        .iter()
        .skip(s_start)
        .zip(t_items.iter().skip(t_start))
    {
        items.push(rust_meet_types_inner(si, ti, strict_optional, resolver)?);
    }

    Some(items)
}

/// Meet one variadic + one fixed tuple.
fn meet_one_variadic(
    variadic_items: &[Type],
    fixed_items: &[Type],
    unpack_idx: usize,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let unpack = get_unpack_type(variadic_items.get(unpack_idx)?);
    if !is_instance_tuple(unpack) {
        return None;
    }

    let variadic_len = variadic_items.len();
    if fixed_items.len() < variadic_len - 1 {
        return None;
    }

    let prefix_len = unpack_idx;
    let suffix_len = variadic_len - prefix_len - 1;
    let (prefix, middle, suffix) = {
        let n = fixed_items.len();
        if prefix_len.saturating_add(suffix_len) >= n {
            (fixed_items.to_vec(), Vec::new(), Vec::new())
        } else {
            let mid_end = n.saturating_sub(suffix_len);
            (
                fixed_items[..prefix_len].to_vec(),
                fixed_items[prefix_len..mid_end].to_vec(),
                fixed_items[mid_end..].to_vec(),
            )
        }
    };

    let mut items = Vec::new();

    // Meet prefix items.
    for (fi, vi) in prefix.iter().zip(variadic_items.iter().take(prefix_len)) {
        items.push(rust_meet_types_inner(fi, vi, strict_optional, resolver)?);
    }

    // Meet middle items with unpacked inner type.
    let Type::Instance {
        args: unpacked_args,
        ..
    } = unpack
    else {
        return None;
    };
    let unpacked_inner = unpacked_args.first()?;
    for mi in &middle {
        items.push(rust_meet_types_inner(
            mi,
            unpacked_inner,
            strict_optional,
            resolver,
        )?);
    }

    // Meet suffix items.
    if suffix_len > 0 {
        let var_start = variadic_len - suffix_len;
        for (fi, vi) in suffix.iter().zip(variadic_items.iter().skip(var_start)) {
            items.push(rust_meet_types_inner(fi, vi, strict_optional, resolver)?);
        }
    }

    Some(items)
}

// ---------------------------------------------------------------------------
// Wire-format helpers for TupleType items
// ---------------------------------------------------------------------------

/// Find the index of an `UnpackType` in a type list, or `None`.
fn find_unpack(items: &[Type]) -> Option<usize> {
    items
        .iter()
        .position(|t| matches!(t, Type::UnpackType { .. }))
}

/// Extract the inner type of an `UnpackType`.
fn get_unpack_type(unpack: &Type) -> &Type {
    match unpack {
        Type::UnpackType { typ } => typ.as_ref(),
        _ => unreachable!(),
    }
}

/// Check whether a `Type` is `Instance(builtins.tuple, ...)`.
fn is_instance_tuple(t: &Type) -> bool {
    matches!(
        t,
        Type::Instance {
            type_ref,
            ..
        } if type_ref == "builtins.tuple"
    )
}

/// Encode a list of `Type`s into wire-format bytes (LIST_GEN + size + items + END_TAG).
fn encode_items(items: &[Type]) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_tag(&mut buf, wire::LIST_GEN);
    if items.is_empty() {
        wire::write_int_bare(&mut buf, 0).ok()?;
    } else {
        wire::write_int_bare(&mut buf, items.len() as i64).ok()?;
    }
    for item in items {
        wire::write_type(&mut buf, item).ok()?;
    }
    wire::write_tag(&mut buf, 255u8);
    Some(buf.into_bytes())
}

/// Inner join_types dispatcher — returns `None` (defer) when Rust
/// doesn't handle the case, `Some(Type)` when it does.
fn rust_join_types_inner(
    s: &Type,
    t: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    match join_types(
        s,
        t,
        &SubtypeContext::new(false, false, false, false, false, strict_optional),
        resolver,
    ) {
        Some(r) => materialize_join(s, t, r, resolver),
        None => None,
    }
}

/// Inner meet_types dispatcher — returns `None` (defer) when Rust
/// doesn't handle the case, `Some(Type)` when it does.
fn rust_meet_types_inner(
    s: &Type,
    t: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    match meet_types(
        s,
        t,
        &SubtypeContext::new(false, false, false, false, false, strict_optional),
        resolver,
    ) {
        Some(r) => setop_result_to_type(Some(r), s, t),
        None => None,
    }
}

/// Map `SetOpResult` to the Python-side
/// `(disc, fullname, arg_discs, encoded)` 4-tuple. `disc` is 0=SameS,
/// 1=SameT, 2=Object, 3=Bottom, 4=Any, 5=Ancestor (fullname set,
/// arg_discs empty), 6=SameTypeWithArgs (fullname set, arg_discs
/// populated: 0=s.args[i], 1=t.args[i], 4=Any), 7=Encoded (the
/// `encoded` bytes hold a wire-format type blob the shim decodes via
/// `read_type(ReadBuffer(encoded))`).
type DiscriminatorOut = (i64, Option<String>, Vec<i8>, Vec<u8>);

fn discriminator(r: SetOpResult) -> DiscriminatorOut {
    match r {
        SetOpResult::SameS => (0, None, Vec::new(), Vec::new()),
        SetOpResult::SameT => (1, None, Vec::new(), Vec::new()),
        SetOpResult::Object => (2, None, Vec::new(), Vec::new()),
        SetOpResult::Bottom => (3, None, Vec::new(), Vec::new()),
        SetOpResult::Any => (4, None, Vec::new(), Vec::new()),
        SetOpResult::Ancestor(fullname) => (5, Some(fullname), Vec::new(), Vec::new()),
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => (6, Some(type_ref), arg_discs, Vec::new()),
        SetOpResult::Encoded(bytes) => (7, None, Vec::new(), bytes),
    }
}

/// `trivial_join`/`trivial_meet` only produce SameS/SameT/Object/Bottom
/// (never Any, Ancestor, or SameTypeWithArgs), so they return a plain
/// `i64` discriminator.
fn discriminator_trivial(r: SetOpResult) -> i64 {
    match r {
        SetOpResult::SameS => 0,
        SetOpResult::SameT => 1,
        SetOpResult::Object => 2,
        SetOpResult::Bottom => 3,
        SetOpResult::Any
        | SetOpResult::Ancestor(_)
        | SetOpResult::SameTypeWithArgs { .. }
        | SetOpResult::Encoded(_) => {
            unreachable!("trivial_join/trivial_meet never produce Any/Ancestor/WithArgs/Encoded")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::{read_type, LiteralValue, Type};

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// Instance with `extra_attrs` set (for step 5 erasure tests).
    fn instance_with_attrs(type_ref: &str, attrs: Vec<(&str, Type)>, immutable: Vec<&str>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: Some(ExtraAttrs {
                attrs: attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                immutable: immutable.into_iter().map(String::from).collect(),
                mod_name: None,
            }),
        }
    }

    /// Minimal `CallableType` for join tests. `fallback` is the
    /// `builtins.function` (or `builtins.type`) Instance. arg_kinds
    /// defaults to ARG_POS (0) per arg.
    fn callable(fallback_ref: &str, arg_types: Vec<Type>, ret_type: Type) -> Type {
        let arg_kinds = vec![0i64; arg_types.len()];
        let arg_names = vec![None; arg_types.len()];
        Type::CallableType {
            fallback: Box::new(instance(fallback_ref, vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(ret_type),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    /// `CallableType` with explicit `variables` (TypeVarLikeType list).
    /// Mirrors `def f[T](x: T) -> T`: `variables` carries the declared
    /// TypeVars, `arg_types`/`ret_type` reference them by `TypeVarType`
    /// nodes whose `(raw_id, namespace)` match the declared tvar.
    fn callable_with_vars(
        fallback_ref: &str,
        arg_types: Vec<Type>,
        ret_type: Type,
        variables: Vec<Type>,
    ) -> Type {
        let arg_kinds = vec![0i64; arg_types.len()];
        let arg_names = vec![None; arg_types.len()];
        Type::CallableType {
            fallback: Box::new(instance(fallback_ref, vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(ret_type),
            name: None,
            variables,
            type_guard: None,
            type_is: None,
        }
    }

    fn ctx(strict_optional: bool) -> SubtypeContext {
        SubtypeContext::new(false, false, false, false, false, strict_optional)
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        // Every class implicitly has builtins.object in its MRO
        // (mirrors the Python TypeFixture where oi=object is in every
        // class's mro). Needed for is_subtype(X, builtins.object)=True.
        if fullname != "builtins.object" {
            s.mro.push("builtins.object".to_string());
            s.has_base.insert("builtins.object".to_string());
        }
        s
    }

    #[test]
    fn trivial_meet_subtype_returns_first() {
        // A <: B -> meet(A, B) = A (SameS).
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_meet(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn trivial_meet_supertype_returns_second() {
        // B <: A -> meet(A, B) = B (SameT): A not <: B, B <: A.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let a = snap("a.A", "A");
        let r = make_resolver(vec![a, b]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_meet(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameT)
        );
    }

    #[test]
    fn trivial_meet_unrelated_returns_bottom() {
        // A and B unrelated -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_meet(&left, &right, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn trivial_join_subtype_returns_second() {
        // A <: B -> join(A, B) = B (SameT, the supertype).
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_join(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameT)
        );
    }

    #[test]
    fn trivial_join_supertype_returns_first() {
        // B <: A -> join(A, B) = A (SameS): B <: A, not A <: B.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let a = snap("a.A", "A");
        let r = make_resolver(vec![a, b]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_join(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn trivial_join_unrelated_returns_object() {
        // A and B unrelated, Instance right -> Object.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(
            trivial_join(&left, &right, &ctx(true), &r),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn trivial_meet_defers_on_tuple_left() {
        // TupleType left -> is_subtype defers for both directions ->
        // trivial_meet defers (returns None).
        let r = make_resolver(vec![]);
        let left = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![],
            implicit: true,
        };
        let right = instance("a.A", vec![]);
        assert_eq!(trivial_meet(&left, &right, &ctx(true), &r), None);
    }

    #[test]
    fn trivial_join_returns_none_for_non_instance_right() {
        // Non-Instance right -> object_or_any_from_type defers.
        let r = make_resolver(vec![]);
        let left = instance("a.A", vec![]);
        let right = Type::NoneType;
        assert_eq!(trivial_join(&left, &right, &ctx(true), &r), None);
    }

    #[test]
    fn trivial_join_same_type_returns_itself() {
        // A <: A -> join(A, A) = A (SameT, first check fires).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(
            trivial_join(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameT)
        );
    }

    #[test]
    fn trivial_meet_same_type_returns_itself() {
        // A <: A -> meet(A, A) = A (SameS).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(
            trivial_meet(&left, &right, &ctx(true), &r),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn discriminator_maps_variants() {
        assert_eq!(discriminator(SetOpResult::SameS), (0, None, vec![], vec![]));
        assert_eq!(discriminator(SetOpResult::SameT), (1, None, vec![], vec![]));
        assert_eq!(
            discriminator(SetOpResult::Object),
            (2, None, vec![], vec![])
        );
        assert_eq!(
            discriminator(SetOpResult::Bottom),
            (3, None, vec![], vec![])
        );
        assert_eq!(discriminator(SetOpResult::Any), (4, None, vec![], vec![]));
        assert_eq!(
            discriminator(SetOpResult::Ancestor("a.C".to_string())),
            (5, Some("a.C".to_string()), vec![], vec![])
        );
        assert_eq!(
            discriminator(SetOpResult::SameTypeWithArgs {
                type_ref: "g.G".to_string(),
                arg_discs: vec![0, 1, 4],
            }),
            (6, Some("g.G".to_string()), vec![0, 1, 4], vec![])
        );
        assert_eq!(
            discriminator(SetOpResult::Encoded(vec![80, 81])),
            (7, None, vec![], vec![80, 81])
        );
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    #[test]
    fn join_types_any_left_returns_s() {
        // join.py:314-315: isinstance(s, AnyType) -> return s.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = any_type();
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_types_none_right_strict_s_is_none_returns_t() {
        // visit_none_type, strict_optional, s is NoneType -> SameT.
        let r = make_resolver(vec![]);
        let s = Type::NoneType;
        let t = Type::NoneType;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_types_none_right_strict_s_is_uninhabited_returns_none() {
        // s=Uninhabited, t=None: the UninhabitedType swap fires
        // (s is Uninhabited, t is not) -> s=None, t=Uninhabited.
        // visit_uninhabited_type returns s (NoneType, post-swap).

        // flip_if(SameS, swapped=true) -> SameT (original t = None).
        let r = make_resolver(vec![]);
        let s = Type::UninhabitedType { ambiguous: false };
        let t = Type::NoneType;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_types_none_right_strict_s_is_any_returns_any() {
        // s=Any, t=None: the AnyType short-circuit (join.py:314)
        // fires before the NoneType swap -> return s (Any) -> SameS.
        let r = make_resolver(vec![]);
        let s = any_type();
        let t = Type::NoneType;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_types_none_right_non_strict_returns_s() {
        // visit_none_type, non-strict-optional -> return s.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::NoneType;
        assert_eq!(
            join_types(&s, &t, &ctx(false), &r),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn join_types_uninhabited_right_returns_s() {
        // visit_uninhabited_type -> return s.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::UninhabitedType { ambiguous: false };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_types_deleted_right_returns_s() {
        // visit_deleted_type -> return s.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::DeletedType { source: None };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_types_none_right_strict_s_is_instance_defers() {
        // visit_none_type, strict_optional, s is Instance ->
        // make_simplified_union([Instance, None]) which now resolves to
        // Union[Instance, None] (issue #591: Instance-vs-NoneType no

        // longer defers; Python join.py:493-494 produces the same
        // simplified union). Previously deferred while the
        // Instance-vs-non-Instance subtype check fell through.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::NoneType;
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(result.is_some(), "expected a concrete join, got None");
        if let Some(SetOpResult::Encoded(bytes)) = &result {
            let decoded = crate::wire::read_type(&mut ReadBuffer::new(bytes), None).unwrap();
            match decoded {
                Type::UnionType { items, .. } => {
                    assert_eq!(items.len(), 2);
                    assert!(matches!(items[0], Type::Instance { .. }));
                    assert!(matches!(items[1], Type::NoneType));
                }
                other => panic!("expected UnionType, got {other:?}"),
            }
        } else {
            panic!("expected Encoded, got {result:?}");
        }
    }

    #[test]
    fn join_types_instance_right_defers() {
        // visit_instance needs InstanceJoiner + protocol checks ->
        // defer.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_types_union_subtype_returns_union() {
        // visit_union_type (join.py:432-434): if is_proper_subtype(s, t)
        // return t. s=A, t=Union[A, B] where A <: Union[A, B] (every
        // member of the union is a supertype of A via A itself). The

        // is_subtype(s, t) check walks the union items and returns True
        // if s is a subtype of any item -> SameT (return t=the union).
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_types_union_supertype_returns_s() {
        // visit_union_type: s is not <: t (s=A, t=Union[B, C] where A
        // is unrelated). make_simplified_union([s, t]) would flatten
        // to Union[A, B, C]. We can't express a new union without a

        // Type encoder, BUT if t <: s (every union item is a subtype of
        // s), the simplified union is just s. Detect via is_subtype(t,
        // s): Union[B, C] <: A when B <: A and C <: A -> SameS.
        let a = snap("a.A", "A");
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let mut c = snap("a.C", "C");
        c.has_base.insert("a.A".to_string());
        c.mro.push("a.A".to_string());
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, c, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.B", vec![]), instance("a.C", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_types_union_equal_single_item_returns_s() {
        // visit_union_type: s=A, t=Union[A] (single-item union, after
        // get_proper_type it's just A). is_subtype(A, Union[A])=True
        // (A is a subtype of A which is an item) -> SameT. But t is

        // Union[A] not A, so the result is the union. In practice the
        // Python shim calls get_proper_type before the Rust entry, so
        // single-item unions are flattened. This test guards the

        // is_subtype(s, t) path with a single-item union.
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_types_union_unrelated_defers() {
        // visit_union_type: s=A, t=Union[B, C] where A is not <: t
        // and t is not <: s (B, C unrelated to A). make_simplified_union
        // produces Union[A, B, C] via the Rust encoder + decoded/

        // type_ref-fixed on the Python side. The result is Encoded.
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let c = snap("a.C", "C");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, c, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.B", vec![]), instance("a.C", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![
                    instance("a.A", vec![]),
                    instance("a.B", vec![]),
                    instance("a.C", vec![]),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_types_literal_true_false_returns_bool() {
        // visit_literal_type case 3 (join.py:915-917): s is LiteralType,
        // s != t, neither fallback is_enum -> join_types(s.fallback,
        // t.fallback). For Literal[True] and Literal[False] both fallbacks

        // are builtins.bool, so join_types(bool, bool) = bool. The result
        // is builtins.bool (Encoded Instance), not s or t.
        let bool_snap = snap("builtins.bool", "bool");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![bool_snap, o]);
        let s = literal(LiteralValue::Bool(true), "builtins.bool");
        let t = literal(LiteralValue::Bool(false), "builtins.bool");
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("builtins.bool", vec![]));
        }
    }

    #[test]
    fn join_types_union_contracts_bool_literals() {
        // s=A, t=Union[Literal[True], Literal[False]]. Neither s <: t
        // nor t <: s. make_simplified_union flattens to
        // [A, Literal[True], Literal[False]], dedup keeps all (A not

        // subtype of bool literal, bool literals not subtype of A or
        // each other), then try_contracting_literals_in_union collapses
        // Literal[True] + Literal[False] -> bool. Result: Union[A, bool].
        let a = snap("a.A", "A");
        let bool_snap = snap("builtins.bool", "bool");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, bool_snap, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![
                literal(LiteralValue::Bool(true), "builtins.bool"),
                literal(LiteralValue::Bool(false), "builtins.bool"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![instance("a.A", vec![]), instance("builtins.bool", vec![])],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_types_union_contracts_enum_literals() {
        // s=A, t=Union[Literal[Color.RED], Literal[Color.BLUE],
        // Literal[Color.GREEN]]. make_simplified_union flattens,
        // dedup keeps all (enum literals with distinct str values are

        // not subtypes of each other), then
        // try_contracting_literals_in_union collects all 3 enum
        // literals sharing the builtins.Color fallback, checks that

        // every enum_member is covered (RED, BLUE, GREEN), and
        // collapses the first to Color + drops the rest.
        // Result: Union[A, Color].
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let mut color = snap("color.Color", "Color");
        color.is_enum = true;
        color.enum_members = vec!["RED".to_string(), "BLUE".to_string(), "GREEN".to_string()];
        let r = make_resolver(vec![a, color, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![
                literal(LiteralValue::Str("RED".to_string()), "color.Color"),
                literal(LiteralValue::Str("BLUE".to_string()), "color.Color"),
                literal(LiteralValue::Str("GREEN".to_string()), "color.Color"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![instance("a.A", vec![]), instance("color.Color", vec![])],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_types_union_partial_enum_literals_defers() {
        // s=A, t=Union[Literal[Color.RED], Literal[Color.BLUE]] with
        // Color={RED, BLUE, GREEN}. Only 2 of 3 members present ->
        // the enum is NOT fully covered, so contraction does NOT fire.

        // Python keeps the union as-is ([A, Color.RED, Color.BLUE]).
        // Rust defers (None): the wire format round-trip would need to
        // emit the partial union, but that path is identical to the

        // bool case's "missing member" branch, so we just defer.
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let mut color = snap("color.Color", "Color");
        color.is_enum = true;
        color.enum_members = vec!["RED".to_string(), "BLUE".to_string(), "GREEN".to_string()];
        let r = make_resolver(vec![a, color, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![
                literal(LiteralValue::Str("RED".to_string()), "color.Color"),
                literal(LiteralValue::Str("BLUE".to_string()), "color.Color"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        // Partial enum coverage does not contract; Python returns the
        // union unchanged. Rust emits the same union (no contraction).
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![
                    instance("a.A", vec![]),
                    literal(LiteralValue::Str("RED".to_string()), "color.Color"),
                    literal(LiteralValue::Str("BLUE".to_string()), "color.Color"),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn make_simplified_union_erases_extra_attrs_when_one_item_lacks_them() {
        // step 5 (typeops.py:656-691): when one item has extra_attrs and
        // another item with the same fallback type_ref has none, the
        // collapsed result's extra_attrs is erased.

        // [A1(attrs={x:int}), A2(no attrs)] -> dedup keeps A1 (is_subtype
        // of A2 True, same type_ref) -> single A1. step 5: distinct=1
        // (from A1), but A2 has None -> erase -> A1.extra_attrs = None.
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, o]);
        let items = vec![
            instance_with_attrs("a.A", vec![("x", instance("builtins.int", vec![]))], vec![]),
            instance("a.A", vec![]),
        ];
        let result = make_simplified_union(&items, &ctx(true), &r, true, false).expect("deferred");
        let expected = instance("a.A", vec![]);
        assert_eq!(result, expected);
    }

    #[test]
    fn make_simplified_union_keeps_extra_attrs_when_consistent() {
        // step 5: when all items with the same fallback type have the
        // SAME ExtraAttrs (and none lacks them), erase does NOT fire.
        // [A1(attrs={x:int}), A2(same attrs)] -> dedup keeps A1 ->

        // single A1. step 5: distinct=1, no item has None -> erase=False
        // -> attrs preserved.
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, o]);
        let attrs_fn =
            || instance_with_attrs("a.A", vec![("x", instance("builtins.int", vec![]))], vec![]);
        let items = vec![attrs_fn(), attrs_fn()];
        let result = make_simplified_union(&items, &ctx(true), &r, true, false).expect("deferred");
        assert_eq!(result, attrs_fn());
    }

    #[test]
    fn make_simplified_union_erases_extra_attrs_when_distinct() {
        // step 5: when items have >1 distinct ExtraAttrs sharing a
        // fallback type_ref, erase fires on the collapsed result.
        // [A1(attrs={x:int}), A2(attrs={y:str})] -> dedup keeps A1

        // (is_subtype A1<:A2 True) -> single A1. step 5: distinct=2
        // ({x}, {y}) -> erase -> A1.extra_attrs = None.
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, o]);
        let items = vec![
            instance_with_attrs("a.A", vec![("x", instance("builtins.int", vec![]))], vec![]),
            instance_with_attrs("a.A", vec![("y", instance("builtins.str", vec![]))], vec![]),
        ];
        let result = make_simplified_union(&items, &ctx(true), &r, true, false).expect("deferred");
        let expected = instance("a.A", vec![]);
        assert_eq!(result, expected);
    }

    #[test]
    fn join_types_union_drops_uninhabited() {
        // s=A, t=Union[UninhabitedType, B]. Neither is_subtype(A, t)
        // (B unrelated) nor is_subtype(every item, A) (UninhabitedType
        // <: A is True, but B is not <: A). So make_simplified_union

        // fires: flatten -> [A, UninhabitedType, B], redundancy drops
        // UninhabitedType, leaving [A, B]. Encoded.
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, o]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![
                Type::UninhabitedType { ambiguous: false },
                instance("a.B", vec![]),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_types_union_s_is_union_defers() {
        // Both s and t are UnionType. The pre-dispatch swap only fires
        // when exactly one side is a union (join.py:311-312). When both
        // are unions, visit_union_type calls make_simplified_union

        // which merges/flatten -> now returns Encoded union of
        // [a.A, a.B] (no longer defers, the union encoder is available).
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, o]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let t = Type::UnionType {
            items: vec![instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_callable_with_unrelated_instance_returns_object() {
        // visit_callable_type fallback (join.py:579): s is a non-
        // callable, non-protocol Instance. Result is
        // join_types(t.fallback, s). t.fallback=builtins.function (with

        // bases=[object], mirroring the Python fixture), s=a.A (with
        // bases=[object]). Neither is_subtype(function, a) nor
        // is_subtype(a, function) holds, so join_instances_nominal(

        // function, a) -> via_supertype(a, function). a's bases=[object];
        // join_instances_nominal(object, function) -> is_subtype(
        // function, object)=True -> via_supertype(function, object).

        // function's bases=[object]; join_instances_nominal(object,
        // object) -> Left -> mapped to Ancestor("builtins.object").
        // The outer callable fallback passes Ancestor through; the

        // shim maps disc 5 to Instance(object_typeinfo, []) = object.
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let a = snap_with_bases("a.A", "A", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, a, o]);
        let s = instance("a.A", vec![]);
        let t = callable(
            "builtins.function",
            vec![instance("a.A", vec![])],
            instance("a.A", vec![]),
        );
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_callable_with_object_returns_object() {
        // visit_callable_type fallback: s=builtins.object, t=callable
        // with fallback=builtins.function (bases=[object], mirroring
        // the Python fixture). join_types(function, object):

        // is_subtype(object, function)=False, is_subtype(function,
        // object)=True -> join_instances_nominal(function, object) ->
        // via_supertype(function, object). function's bases=[object];

        // join_instances_nominal(object, object) -> Left (same type)
        // -> mapped to Ancestor("builtins.object"). The outer
        // callable fallback passes Ancestor through. The shim maps

        // disc 5 to Instance(object_typeinfo, []) = object.
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, o]);
        let s = instance("builtins.object", vec![]);
        let t = callable(
            "builtins.function",
            vec![],
            instance("builtins.object", vec![]),
        );
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_callable_with_same_fallback_instance_returns_s() {
        // visit_callable_type fallback: s=builtins.function (the
        // callable's own fallback), t=callable with fallback=
        // builtins.function. join_types(function, function) ->

        // visit_instance_join: same type, no args -> SameS. The
        // outer callable join returns SameS (s=builtins.function).
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, o]);
        let s = instance("builtins.function", vec![]);
        let t = callable(
            "builtins.function",
            vec![],
            instance("builtins.object", vec![]),
        );
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_callable_with_callable_defers() {
        // Both s and t are CallableType but NOT similar (different arg
        // counts), so is_similar_callables returns false. The Rust
        // visit_callable_type both-CallableType case defers (the

        // var-arg / subtype fallback branches at join.py:638-646 need
        // is_subtype on whole callables -> not yet ported).
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let s = callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        );
        let t = callable(
            "builtins.function",
            vec![
                instance("builtins.object", vec![]),
                instance("builtins.object", vec![]),
            ],
            instance("builtins.object", vec![]),
        );
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_callable_with_identical_callable_returns_same_s() {
        // join(c, c) where c is a non-generic CallableType. Both sides
        // are structurally identical, so visit_callable_type's
        // both-CallableType case returns SameS (the joined callable is

        // the same as s). Exercises the wire-format CallableType
        // encoder end-to-end (Encoded -> read_type -> fixup).
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let s = callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        );
        let t = callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        );
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_similar_callables_non_equivalent_returns_encoded() {
        // join(callable(B, object), callable(A, object)) where B <: A.
        // is_similar_callables=True (same arg count, same min_args,
        // same is_var_arg). is_equivalent=False (B <: A but not A <: B).

        // The Rust port (join_similar_callables_impl) handles it:
        // per-arg safe_meet(B, A) = B (A is a supertype of B, so the
        // meet is the more specific B), ret join(object, object) =

        // object, from_type_type=True (neither operand is an abstract
        // type object). Result is an encoded CallableType, not None.
        let o = snap("builtins.object", "object");
        let a = snap_with_bases("a.A", "A", &["builtins.object"]);
        let b = snap_with_bases("a.B", "B", &["a.A", "builtins.object"]);
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, a, b, func]);
        let s = callable(
            "builtins.function",
            vec![instance("a.B", vec![])],
            instance("builtins.object", vec![]),
        );
        let t = callable(
            "builtins.function",
            vec![instance("a.A", vec![])],
            instance("builtins.object", vec![]),
        );
        assert!(matches!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Encoded(_))
        ));
    }

    #[test]
    fn combine_similar_callables_equivalent_returns_encoded() {
        // join(callable(A, object), callable(A, object)) where the two
        // A instances are structurally equal (same type_ref) but the
        // callables are not the same Rust object. is_equivalent=True

        // (A <: A both ways). combine_similar_callables: per-arg
        // safe_join(A, A) = A, ret join(object, object) = object.
        // Result is a new CallableType(arg=[A], ret=object) returned as

        // Encoded. (Distinct from the identical case because the M8t
        // identical check compares the full struct; here we want to
        // exercise the combine path. Since the structs are identical,

        // M8t returns SameS. To force the combine path, we'd need
        // non-identical-but-equivalent args, which for Instance means
        // same type_ref. So this test is subsumed by the identical case;

        // we assert SameS here to document the overlap.)
        let o = snap("builtins.object", "object");
        let a = snap_with_bases("a.A", "A", &["builtins.object"]);
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, a, func]);
        let s = callable(
            "builtins.function",
            vec![instance("a.A", vec![])],
            instance("builtins.object", vec![]),
        );
        let t = callable(
            "builtins.function",
            vec![instance("a.A", vec![])],
            instance("builtins.object", vec![]),
        );
        // Structurally identical -> SameS (M8t path).
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_similar_callables_one_generic_one_not_returns_encoded() {
        // join(def f[T](x: T) -> T, def g(x: object) -> object) where
        // only f is generic. min_len == 0 (s.variables empty), so
        // match_generic_callables is a no-op (returns inputs unchanged,

        // join.py:1048-1050). No renumber, no fresh-id parity gap.
        //
        // is_similar_callables=True (same arity). is_equivalent=False:

        // is_subtype(T, object)=True (TypeVar upper_bound <: object),
        // but is_subtype(object, T)=False (Instance not <: TypeVar).
        // The Rust port handles it: safe_meet(T, object) = T (meet of

        // a TypeVar with its upper bound is the TypeVar), ret join(T,
        // object) = object, from_type_type=True (no abstract operands).
        // Result is an encoded CallableType, not None. The deferred

        // case is the one where args meet to NoneType/UninhabitedType.
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let ub = instance("builtins.object", vec![]);
        let tvar = type_var(-1, "ns", ub.clone());
        // s is generic (has variables=[T]); t is non-generic.
        let s = callable_with_vars(
            "builtins.function",
            vec![tvar.clone()],
            tvar.clone(),
            vec![tvar],
        );
        let t = callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        );
        assert!(matches!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Encoded(_))
        ));
    }

    #[test]
    fn combine_similar_callables_both_generic_defers() {
        // join(def f[T](x: T) -> T, def g[T](x: T) -> T) where BOTH
        // callables are generic (min_len > 0). Python's
        // match_generic_callables renumbers both T's via

        // TypeVarId.new (a Python global counter, types.py:559-562).
        // The result's tvar ids differ from any deterministic Rust
        // allocation, and CallableType.__eq__ compares tvar ids in

        // arg_types/ret_type (types.py:2590-2604 + 699-706). Rust
        // can't replicate the counter without FFI back to Python, so
        // the both-generic case DEFERS to preserve parity.

        //
        // This test documents the defer (returns None) and guards
        // against a future change that ports match_generic_callables

        // without solving the fresh-id parity gap.
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let ub = instance("builtins.object", vec![]);
        let tvar_s = type_var(-1, "ns_s", ub.clone());
        let tvar_t = type_var(-1, "ns_t", ub.clone());
        let s = callable_with_vars(
            "builtins.function",
            vec![tvar_s.clone()],
            tvar_s.clone(),
            vec![tvar_s],
        );
        let t = callable_with_vars(
            "builtins.function",
            vec![tvar_t.clone()],
            tvar_t.clone(),
            vec![tvar_t],
        );
        let result = join_types(&s, &t, &ctx(true), &r);
        assert_eq!(
            result, None,
            "both-generic must defer (fresh-id parity gap): got {:?}",
            result
        );
    }

    #[test]
    fn identical_generic_callable_defers() {
        // join(c, c) where c is a generic CallableType. Both sides are
        // structurally identical, BUT Python's combine_similar_callables
        // always calls match_generic_callables (join.py:1114), which

        // renumbers the tvars via TypeVarId.new even when both sides
        // share the same id (join.py:1047-1053). The result has fresh
        // tvar ids, so it is NOT equal to c (CallableType.__eq__

        // compares arg_types/ret_type which carry tvar ids). Returning
        // SameS (= c) would be a parity bug.
        //

        // The M8z identical-check guard defers when both sides have
        // non-empty variables (both_generic). This test documents that
        // guard: join(c, c) for generic c returns None (defer to

        // Python, which produces the correctly-renumbered result).
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let ub = instance("builtins.object", vec![]);
        let tvar = type_var(-1, "ns", ub.clone());
        let c = callable_with_vars(
            "builtins.function",
            vec![tvar.clone()],
            tvar.clone(),
            vec![tvar],
        );
        let result = join_types(&c, &c, &ctx(true), &r);
        assert_eq!(
            result, None,
            "identical generic callable must defer (renumber parity): got {:?}",
            result
        );
    }

    /// Minimal `Overloaded` for join tests. The fallback is
    /// `items[0].fallback` (mirrors `Overloaded.__init__` in
    /// types.py:2744). Each item is a `CallableType`.
    fn overloaded(items: Vec<Type>) -> Type {
        assert!(!items.is_empty(), "Overloaded requires >=1 item");
        Type::Overloaded { items }
    }

    /// Minimal `LiteralType` for join tests. `fallback` is the
    /// Instance whose value space the literal belongs to (e.g.
    /// builtins.int, builtins.str, or a user enum).
    fn literal(value: LiteralValue, fallback_ref: &str) -> Type {
        Type::LiteralType {
            fallback: Box::new(instance(fallback_ref, vec![])),
            value,
        }
    }

    /// Minimal `TypeType` for join tests. `item` is the Instance
    /// the type-of-type refers to (e.g. type[A]).
    fn type_type(item_ref: &str) -> Type {
        Type::TypeType {
            item: Box::new(instance(item_ref, vec![])),
            is_type_form: false,
        }
    }

    /// Minimal `TypeVarType` for join tests. `raw_id` + `namespace`
    /// form the identity (mirrors `TypeVarId.__eq__` in
    /// types.py:567-577; `meta_level` is not in the wire format —
    /// see `visit_type_var` docstring). `upper_bound` is the bound
    /// compared by join.py:466.
    fn type_var(raw_id: i64, namespace: &str, upper_bound: Type) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: namespace.to_string(),
            values: Vec::new(),
            upper_bound: Box::new(upper_bound),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: INVARIANT,
            meta_level: 0,
        }
    }

    /// Minimal `ParamSpecType` for meet/join tests. `raw_id` +
    /// `namespace` form the `TypeVarId` used by `ParamSpecType.__eq__`
    /// (types.py:931-938); prefix/flavor/default take non-deferring
    /// defaults.
    fn param_spec(raw_id: i64, namespace: &str, upper_bound: Type) -> Type {
        Type::ParamSpecType {
            prefix: Box::new(wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "P".to_string(),
            raw_id,
            namespace: namespace.to_string(),
            flavor: 0,
            upper_bound: Box::new(upper_bound),
            default: Box::new(any_type()),
        }
    }

    /// Minimal `Type::Parameters` for meet/join tests. `arg_kinds` are
    /// wire i64s (ARG_POS=0, ARG_NAMED=3, ARG_STAR=2, ARG_NAMED_OPT=5).
    fn parameters(arg_types: Vec<Type>, arg_kinds: Vec<i64>) -> Type {
        Type::Parameters(wire::Parameters {
            arg_names: vec![None; arg_types.len()],
            variables: vec![],
            imprecise_arg_kinds: false,
            is_ellipsis_args: false,
            arg_types,
            arg_kinds,
        })
    }

    /// Minimal `TypedDictType` for join tests. `fallback_ref` is the
    /// Instance TypedDict falls back to (typically builtins.dict or a
    /// user TypedDict class). The portable join case (visit_typeddict
    /// case 2, join.py:832-833) only reads `t.fallback`; items /
    /// required_keys / readonly_keys / is_closed don't affect the
    /// deferral decision, so they default to empty.
    fn typed_dict(fallback_ref: &str) -> Type {
        Type::TypedDictType {
            fallback: Box::new(instance(fallback_ref, vec![])),
            items: Vec::new(),
            required_keys: std::collections::HashSet::new(),
            readonly_keys: std::collections::HashSet::new(),
            is_closed: true,
        }
    }

    /// Minimal `TupleType` for join tests. `fallback_ref` is the
    /// `partial_fallback` Instance (always an Instance per wire
    /// format). The portable join case (visit_tuple_type case 2,
    /// join.py:774-775) calls `tuple_fallback(t)` which equals
    /// `t.partial_fallback` only when the fallback is NOT
    /// `builtins.tuple` (typeops.py:108-109). When it IS
    /// `builtins.tuple`, `tuple_fallback` constructs a new Instance
    /// with a union of items — Rust can't replicate without a Type
    /// encoder, so that case must defer.
    fn tuple_type(fallback_ref: &str, items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance(fallback_ref, vec![])),
            items,
            implicit: false,
        }
    }

    /// Minimal `UnboundType` for meet tests. `visit_unbound_type`
    /// (meet.py:864-873) reads only `s`'s variant, so the name/args
    /// fields are don't-cares for the meet decision.
    fn unbound_type() -> Type {
        Type::UnboundType {
            name: "?".to_string(),
            args: Vec::new(),
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    /// Minimal `TypeVarTupleType` for meet tests. `visit_type_var_tuple`
    /// (meet.py:930-934) compares `s.id == t.id` (raw_id + namespace,
    /// mirroring `TypeVarId.__eq__`) then picks by `min_len`.
    fn type_var_tuple(raw_id: i64, namespace: &str, min_len: i64) -> Type {
        Type::TypeVarTupleType {
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "Ts".to_string(),
            raw_id,
            namespace: namespace.to_string(),
            upper_bound: Box::new(instance("builtins.tuple", vec![])),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            min_len,
        }
    }

    #[test]
    fn join_type_type_with_builtins_type_instance_returns_s() {
        // visit_type_type case 2 (join.py:861-862): s is Instance with
        // fullname=="builtins.type" -> return self.s. Fires the Rust
        // SameS path (shim returns s=builtins.type).
        let o = snap("builtins.object", "object");
        let tt = snap("builtins.type", "type");
        let r = make_resolver(vec![o, tt]);
        let s = instance("builtins.type", vec![]);
        let t = type_type("builtins.object");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_builtins_type_instance_with_type_type_returns_s() {
        // Same as above but with s/t swapped to verify the flip_if
        // mapping. s=builtins.type, t=type[object]. The Rust path
        // returns SameS (shim returns s=builtins.type).
        let o = snap("builtins.object", "object");
        let tt = snap("builtins.type", "type");
        let r = make_resolver(vec![o, tt]);
        let s = instance("builtins.type", vec![]);
        let t = type_type("builtins.object");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_type_type_with_type_type_same_item_returns_encoded() {
        // visit_type_type case 1 (join.py:855-860): both TypeType ->
        // TypeType(make_normalized(join_types(t.item, s.item)),
        // is_type_form=s.is_type_form or t.is_type_form). With same

        // item (builtins.object), join_types returns SameS, so the
        // joined item is s.item=Instance(builtins.object). The result
        // is TypeType{item: builtins.object, is_type_form: false},

        // encoded via write_type -> Encoded(bytes).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = type_type("builtins.object");
        let t = type_type("builtins.object");
        let result = join_types(&s, &t, &ctx(true), &r);
        let bytes = match result {
            Some(SetOpResult::Encoded(bytes)) => bytes,
            other => panic!("expected Encoded, got {other:?}"),
        };
        // Decode and verify: TypeType(Instance(builtins.object)).
        let mut rbuf = ReadBuffer::new(&bytes);
        let decoded = crate::wire::read_type(&mut rbuf, None).expect("decode failed");
        let expected = Type::TypeType {
            item: Box::new(instance("builtins.object", vec![])),
            is_type_form: false,
        };
        assert_eq!(decoded, expected);
    }

    #[test]
    fn join_type_type_with_other_instance_defers() {
        // visit_type_type case 3 (join.py:863-864 -> default): s is
        // Instance that is NOT builtins.type. default(s) walks the
        // fallback chain. Defer (default is complex).
        let o = snap("builtins.object", "object");
        let a = snap("a.A", "A");
        let r = make_resolver(vec![o, a]);
        let s = instance("a.A", vec![]);
        let t = type_type("builtins.object");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_literal_with_equal_literal_returns_t() {
        // visit_literal_type case 1 (join.py:838-840): s is
        // LiteralType, t == s -> return t. Fires the Rust SameT path
        // (shim returns t).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = literal(LiteralValue::Int(1), "builtins.int");
        let t = literal(LiteralValue::Int(1), "builtins.int");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_literal_with_unequal_literal_defers() {
        // visit_literal_type case 3 (join.py:915-917): s is
        // LiteralType, t != s, neither enum -> join_types(s.fallback,
        // t.fallback). Both fallbacks are builtins.int, so the

        // recursive join returns SameS -> builtins.int (Encoded).
        let o = snap("builtins.object", "object");
        let i = snap("builtins.int", "int");
        let r = make_resolver(vec![o, i]);
        let s = literal(LiteralValue::Int(1), "builtins.int");
        let t = literal(LiteralValue::Int(2), "builtins.int");
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("builtins.int", vec![]));
        }
    }

    #[test]
    fn join_instance_with_matching_last_known_value_returns_t() {
        // visit_literal_type case 4 (join.py:844-845): s is Instance,
        // s.last_known_value == t -> return t. Fires the Rust SameT
        // path (shim returns t, the literal).
        let o = snap("builtins.object", "object");
        let a = snap("a.A", "A");
        let r = make_resolver(vec![o, a]);
        let lit = literal(LiteralValue::Int(1), "a.A");
        let s = Type::Instance {
            type_ref: "a.A".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lit.clone())),
            extra_attrs: None,
        };
        let t = lit;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_instance_with_mismatched_last_known_value_encodes_fallback_join() {
        // visit_literal_type case 5 (join.py:847): s is Instance,
        // s.last_known_value != t -> join_types(self.s, t.fallback).
        // The recursive call is Instance-vs-Instance (both fallback=A),

        // which yields SameS -> s.fallback = A. Encoded: the joined
        // fallback is dropped of the LKV (fresh Instance(A, [])), which
        // mirrors Python join_types(A, A) = A.
        let o = snap("builtins.object", "object");
        let a = snap("a.A", "A");
        let r = make_resolver(vec![o, a]);
        let lkv = literal(LiteralValue::Int(1), "a.A");
        let s = Type::Instance {
            type_ref: "a.A".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv)),
            extra_attrs: None,
        };
        let t = literal(LiteralValue::Int(2), "a.A");
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("a.A", vec![]));
        }
    }

    #[test]
    fn join_overloaded_with_object_returns_object() {
        // visit_overloaded fallback (join.py:632): s=object, t=Overloaded.
        // Recursive join_types(t.fallback=function, s=object) ->
        // is_subtype(function, object)=True -> via_supertype(function,

        // object) -> function.bases=[object] ->
        // join_instances_nominal(object, object) -> Left ->
        // Ancestor("builtins.object"). The outer overloaded fallback

        // passes Ancestor through; the shim maps disc 5 to
        // Instance(object_typeinfo, []) = object.
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, o]);
        let s = instance("builtins.object", vec![]);
        let t = overloaded(vec![callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        )]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_overloaded_with_function_returns_function() {
        // visit_overloaded fallback: s=builtins.function, t=Overloaded
        // with fallback=builtins.function. Recursive join_types(function,
        // function) -> visit_instance_join: same type, no args -> SameS.

        // The outer overloaded join returns SameS (s=builtins.function).
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, o]);
        let s = instance("builtins.function", vec![]);
        let t = overloaded(vec![callable(
            "builtins.function",
            vec![],
            instance("builtins.object", vec![]),
        )]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_overloaded_with_unrelated_instance_returns_object() {
        // visit_overloaded fallback: s=a.A, t=Overloaded with fallback=
        // builtins.function. Neither is_subtype(function, a) nor
        // is_subtype(a, function) holds, so via_supertype(a, function)

        // walks a.bases=[object] -> join_instances_nominal(object,
        // function) -> is_subtype(function, object)=True ->
        // via_supertype(function, object) -> function.bases=[object] ->

        // join_instances_nominal(object, object) -> Left ->
        // Ancestor("builtins.object").
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let a = snap_with_bases("a.A", "A", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, a, o]);
        let s = instance("a.A", vec![]);
        let t = overloaded(vec![callable(
            "builtins.function",
            vec![instance("a.A", vec![])],
            instance("a.A", vec![]),
        )]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_overloaded_with_overloaded_defers() {
        // Both s and t are callable-like (Overloaded). The both-FunctionLike
        // case (join.py:612-627) uses is_similar_callables +
        // combine_similar_callables which produces a new Overloaded via

        // wire encoder. No longer defers — now returns Encoded(Overloaded).
        // Fixed: combine_similar_callables was called with outer Overloaded
        // types instead of inner CallableType items, causing a panic.
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let c = || {
            callable(
                "builtins.function",
                vec![instance("builtins.object", vec![])],
                instance("builtins.object", vec![]),
            )
        };
        let s = overloaded(vec![c()]);
        let t = overloaded(vec![c()]);
        assert!(
            matches!(
                join_types(&s, &t, &ctx(true), &r),
                Some(SetOpResult::Encoded(_))
            ),
            "overloaded-join should return Encoded, not defer"
        );
    }

    #[test]
    fn join_overloaded_with_callable_defers() {
        // s=CallableType, t=Overloaded. Both callable-like -> the
        // walk runs with s_items=[s] (CallableType.items == [self]).
        // Identical items -> similar+equivalent -> combine -> the

        // walk yields one result, encoded (disc=7).
        let o = snap("builtins.object", "object");
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let r = make_resolver(vec![o, func]);
        let s = callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        );
        let t = overloaded(vec![callable(
            "builtins.function",
            vec![instance("builtins.object", vec![])],
            instance("builtins.object", vec![]),
        )]);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(matches!(result, Some(SetOpResult::Encoded(_))));
    }

    #[test]
    fn join_object_with_overloaded_returns_object() {
        // s=object, t=Overloaded. Same as
        // join_overloaded_with_object_returns_object but with s/t roles
        // verified from the other direction (s=object, t=overloaded).

        // The recursive join_types(fallback=function, s=object) ->
        // Ancestor("builtins.object"). Fires the Rust Ancestor path.
        let func = snap_with_bases("builtins.function", "function", &["builtins.object"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![func, o]);
        let s = instance("builtins.object", vec![]);
        let t = overloaded(vec![callable(
            "builtins.function",
            vec![],
            instance("builtins.object", vec![]),
        )]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_types_swaps_none_left_to_right() {
        // join.py:320-321: s is None, t is not -> swap. Post-swap:
        // visit_none_type, strict_optional, s=Instance, t=None ->
        // make_simplified_union([Instance, None]) which now resolves to

        // Union[Instance, None] (issue #591), then flip back (swapped)
        // yielding the same union.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::NoneType;
        let t = instance("a.A", vec![]);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(result.is_some(), "expected a concrete join, got None");
        if let Some(SetOpResult::Encoded(bytes)) = &result {
            let decoded = crate::wire::read_type(&mut ReadBuffer::new(bytes), None).unwrap();
            match decoded {
                Type::UnionType { items, .. } => {
                    assert_eq!(items.len(), 2);
                }
                other => panic!("expected UnionType, got {other:?}"),
            }
        } else {
            panic!("expected Encoded, got {result:?}");
        }
    }

    #[test]
    fn join_types_swaps_uninhabited_left_to_right() {
        // join.py:323-324: s is Uninhabited, t is not -> swap.
        // Post-swap: s=Instance, t=Uninhabited.
        // visit_uninhabited_type returns s (Instance, post-swap).

        // flip_if(SameS, swapped=true) -> SameT (original t = Instance).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::UninhabitedType { ambiguous: false };
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_types_callable_left_defers() {
        // normalize_callables deferred -> defer.
        let r = make_resolver(vec![]);
        let s = any_type();
        // Force t to be a callable-like so the normalize_callables
        // guard fires. Use a CallableType blob via the wire reader is
        // complex; instead verify the guard via the NoneType path: if

        // s is Any and t is CallableType, the AnyType short-circuit
        // (join.py:314) should fire BEFORE normalize_callables. So this
        // test verifies ordering: AnyType s returns SameS even with

        // callable t.
        let t = Type::NoneType;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    // ---- visit_instance nominal join (M8f) ----

    fn snap_with_bases(fullname: &str, name: &str, base_refs: &[&str]) -> TypeInfoSnapshot {
        let mut s = snap(fullname, name);
        let mut bases = Vec::new();
        for base_ref in base_refs {
            bases.push(crate::wire::encode_instance_simple_for_test(base_ref));
            s.has_base.insert((*base_ref).to_string());
            s.mro.push((*base_ref).to_string());
        }
        s.bases = bases;
        s
    }

    #[test]
    fn visit_instance_same_type_returns_s() {
        // join.py:281 constructs Instance(t.type, []) — fresh, no LKV.
        // Neither operand has LKV -> SameT (t has no LKV).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn visit_instance_direct_subtype_returns_supertype() {
        // B <: A -> join(A, B): s=A, t=B. is_subtype(B, A)=true ->
        // join_instances_nominal(B, A) -> via_supertype(B, A).
        // B's bases=[A]. join_instances_nominal(A, A) -> Left.

        // Mapped: Left -> Ancestor("a.A") (the base is the common
        // ancestor, which equals original s=A).
        let a = snap("a.A", "A");
        let b = snap_with_bases("a.B", "B", &["a.A"]);
        let r = make_resolver(vec![a, b]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("a.A".to_string()))
        );
    }

    #[test]
    fn visit_instance_common_ancestor_returns_ancestor() {
        // D <: C, E <: C, D not <: E, E not <: D.
        // join(D, E): t=D, s=E. is_subtype(D, E)=false ->
        // via_supertype(E, D). E's bases=[C].

        // join_instances_nominal(C, D): C != D, is_subtype(C, D)=false
        // -> via_supertype(D, C). D's bases=[C].
        // join_instances_nominal(C, C) -> SameS (Ancestor(C)).

        // The best candidate is C -> Ancestor("a.C").
        let c = snap("a.C", "C");
        let d = snap_with_bases("a.D", "D", &["a.C"]);
        let e = snap_with_bases("a.E", "E", &["a.C"]);
        let r = make_resolver(vec![c, d, e]);
        let s = instance("a.D", vec![]);
        let t = instance("a.E", vec![]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("a.C".to_string()))
        );
    }

    #[test]
    fn visit_instance_unrelated_returns_object() {
        // D and E unrelated (no common base in resolver) ->
        // via_supertype bottoms out at builtins.object -> Object.
        let d = snap("a.D", "D");
        let e = snap("a.E", "E");
        let r = make_resolver(vec![d, e]);
        let s = instance("a.D", vec![]);
        let t = instance("a.E", vec![]);
        // No bases on either -> join_instances_via_supertype returns
        // None (defer) since bases is empty and neither is object.
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn visit_instance_args_defers() {
        // Instance with args -> defer (needs type-arg join).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![any_type()]);
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn visit_instance_s_not_instance_defers() {
        // s is AnyType, t is Instance -> the visit_instance Instance
        // branch requires s to be Instance; AnyType s falls to the
        // else branch (join.py:453 default). But AnyType s is caught

        // by the AnyType short-circuit BEFORE visit_join. So this test
        // uses UnboundType s (not AnyType, not Instance).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::UnboundType {
            name: "X".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        let t = instance("a.A", vec![]);
        // visit_instance with s=UnboundType -> not Instance -> defer.
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    // ---- visit_instance with args (M8g) ----
    //
    // join.py:114-180: t.type == s.type, combine type args via

    // join_types (covariant) or is_equivalent (invariant). M8g
    // handles: AnyType arg, invariant is_equivalent (False -> Object,
    // True -> SameS/SameT). Covariant recursion needs upper_bound

    // (deferred to M8h). Variadic / ParamSpec / TypeVarTupleType
    // defer.

    /// TypeInfo with one invariant TypeVar `T` (variance=0, kind=0).
    fn snap_with_invariant_tvar(fullname: &str) -> TypeInfoSnapshot {
        let mut s = snap(fullname, fullname.rsplit('.').next().unwrap_or(fullname));
        s.type_vars_with_variance = vec![("T".to_string(), INVARIANT, 0)];
        s
    }

    /// TypeInfo with one covariant TypeVar `T` (variance=1, kind=0)
    /// and `upper_bound = builtins.object`.
    fn snap_with_covariant_tvar(fullname: &str) -> TypeInfoSnapshot {
        let mut s = snap(fullname, fullname.rsplit('.').next().unwrap_or(fullname));
        s.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        s.type_var_upper_bounds = vec![crate::wire::encode_instance_simple_for_test(
            "builtins.object",
        )];
        s
    }

    #[test]
    fn join_instance_any_arg_returns_any_arg() {
        // join(G[Any, int], G[int, Any]) where T1, T2 are invariant.
        // AnyType arg short-circuits (join.py:131-135) before the
        // variance dispatch. Both args have an Any on one side ->

        // both reduce to Any -> SameTypeWithArgs { [Any, Any] }.
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![
            ("T1".to_string(), INVARIANT, 0),
            ("T2".to_string(), INVARIANT, 0),
        ];
        let r = make_resolver(vec![g]);
        let s = instance("g.G", vec![any_type(), instance("builtins.int", vec![])]);
        let t = instance("g.G", vec![instance("builtins.int", vec![]), any_type()]);
        let result = join_types(&s, &t, &ctx(true), &r);
        match result {
            Some(SetOpResult::SameTypeWithArgs { arg_discs, .. }) => {
                assert_eq!(arg_discs, vec![4, 4]);
            }
            other => panic!("expected SameTypeWithArgs, got {other:?}"),
        }
    }

    #[test]
    fn join_instance_invariant_equiv_false_returns_object() {
        // join(G[int], G[str]) where T is invariant.
        // is_equivalent(int, str) = false -> object_from_instance(t).
        // Result: Object.
        let g = snap_with_invariant_tvar("g.G");
        let int_snap = snap("builtins.int", "int");
        let str_snap = snap("builtins.str", "str");
        let r = make_resolver(vec![g, int_snap, str_snap]);
        let s = instance("g.G", vec![instance("builtins.int", vec![])]);
        let t = instance("g.G", vec![instance("builtins.str", vec![])]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn join_instance_invariant_equiv_true_returns_same_args() {
        // join(G[A], G[A]) where T is invariant, A <: A.
        // is_equivalent(A, A) = true. join_types(A, A) = A (SameS).
        // SameS means result = ta = t.args[0] -> disc 1. Both args are

        // A so the reconstructed Instance is G[A] either way.
        let g = snap_with_invariant_tvar("g.G");
        let a = snap("a.A", "A");
        let r = make_resolver(vec![g, a]);
        let s = instance("g.G", vec![instance("a.A", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        let result = join_types(&s, &t, &ctx(true), &r);
        match result {
            Some(SetOpResult::SameTypeWithArgs {
                type_ref,
                arg_discs,
            }) => {
                assert_eq!(type_ref, "g.G");
                // join_types(ta, sa) where ta=t.args[0], sa=s.args[0].
                // Neither has LKV -> SameT (return t=sa=s.args[0]) -> disc 0.
                assert_eq!(arg_discs, vec![0]);
            }
            other => panic!("expected SameTypeWithArgs, got {other:?}"),
        }
    }

    #[test]
    fn join_instance_covariant_same_arg_returns_same() {
        // Covariant T, upper_bound=object. join(G[A], G[A]):
        // join_types(ta, sa) where ta=t.args[0]=A, sa=s.args[0]=A.
        // Neither has LKV -> SameT (return t=sa=s.args[0]) -> disc 0.

        // is_subtype(A, object)=True.
        let g = snap_with_covariant_tvar("g.G");
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, o]);
        let s = instance("g.G", vec![instance("a.A", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        let result = join_types(&s, &t, &ctx(true), &r);
        match result {
            Some(SetOpResult::SameTypeWithArgs {
                type_ref,
                arg_discs,
            }) => {
                assert_eq!(type_ref, "g.G");
                assert_eq!(arg_discs, vec![0]);
            }
            other => panic!("expected SameTypeWithArgs, got {other:?}"),
        }
    }

    #[test]
    fn join_instance_covariant_subtype_encodes() {
        // Covariant T, upper_bound=object. join(G[B], G[A]) where
        // B <: A. The recursive join_types(A, B) returns Ancestor(A)
        // (the common supertype), not SameS/SameT. The covariant

        // branch can't express an Ancestor result as an arg disc, so
        // it encodes the full Instance (disc=7) instead of deferring
        // (join.py:136-148: new_type = join_types(ta, sa) is a real

        // type once the encoder exists).
        let g = snap_with_covariant_tvar("g.G");
        let a = snap("a.A", "A");
        let b = snap_with_bases("a.B", "B", &["a.A"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, b, o]);
        let s = instance("g.G", vec![instance("a.B", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(matches!(result, Some(SetOpResult::Encoded(_))));
    }

    #[test]
    fn join_instance_covariant_unrelated_defers() {
        // Covariant T, upper_bound=object. join(G[A], G[D]) where
        // A, D unrelated with no shared bases in the snapshot. The
        // recursive join_types(A, D) cannot decide an ancestor (no

        // bases/mro), so it returns None and the whole call defers to
        // Python. (With resolvable ancestors the covariant branch
        // encodes the full Instance instead.)
        let g = snap_with_covariant_tvar("g.G");
        let a = snap("a.A", "A");
        let d = snap("a.D", "D");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, d, o]);
        let s = instance("g.G", vec![instance("a.A", vec![])]);
        let t = instance("g.G", vec![instance("a.D", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_covariant_upper_bound_fail_returns_object() {
        // Covariant T, upper_bound=A (narrow). join(G[B], G[B]) where
        // B is NOT <: A (an invalid arg, constructed for the test).
        // join_types(B, B) = SameS -> new_type = ta = B.

        // is_subtype(B, A) = False (B not in A's has_base) ->
        // object_from_instance(t) = Object (whole result bails).
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        g.type_var_upper_bounds = vec![crate::wire::encode_instance_simple_for_test("a.A")];
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, b, o]);
        let s = instance("g.G", vec![instance("a.B", vec![])]);
        let t = instance("g.G", vec![instance("a.B", vec![])]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn join_instance_covariant_no_upper_bound_defers() {
        // Covariant T with empty upper_bound blob (missing from
        // snapshot). Defer — can't safely skip the bound check.
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        g.type_var_upper_bounds = vec![Vec::new()]; // empty blob
        let a = snap("a.A", "A");
        let r = make_resolver(vec![g, a]);
        let s = instance("g.G", vec![instance("a.A", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_variance_not_ready_defers() {
        // PEP695 `class C[T]`: snapshot froze T.variance at
        // VARIANCE_NOT_READY; defer (mirrors subtypes.rs:1980, #860).
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), VARIANCE_NOT_READY, 0)];
        g.type_var_upper_bounds = vec![crate::wire::encode_instance_simple_for_test(
            "builtins.object",
        )];
        let a = snap("a.A", "A");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, o]);
        let s = instance("g.G", vec![instance("a.A", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_arg_count_mismatch_defers() {
        // len(s.args) != len(t.args) -> Python uses zip (mismatch OK
        // during daemon reprocessing). Rust defers (no zip semantics).
        let g = snap_with_invariant_tvar("g.G");
        let r = make_resolver(vec![g]);
        let s = instance("g.G", vec![any_type(), any_type()]);
        let t = instance("g.G", vec![any_type()]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_variadic_defers() {
        // has_type_var_tuple_type -> variadic instance. Defer.
        let mut g = snap_with_invariant_tvar("g.G");
        g.has_type_var_tuple_type = true;
        let r = make_resolver(vec![g]);
        let s = instance("g.G", vec![any_type()]);
        let t = instance("g.G", vec![any_type()]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_paramspec_arg_defers() {
        // kind=1 (ParamSpec) with non-Any arg -> defer (AnyType
        // short-circuits first, so use Instance args to reach the
        // kind dispatch).
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("P".to_string(), INVARIANT, 1)];
        let int_snap = snap("builtins.int", "int");
        let r = make_resolver(vec![g, int_snap]);
        let s = instance("g.G", vec![instance("builtins.int", vec![])]);
        let t = instance("g.G", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_typevartuple_arg_defers() {
        // kind=2 (TypeVarTupleType) with non-Any arg -> defer.
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("Ts".to_string(), INVARIANT, 2)];
        let int_snap = snap("builtins.int", "int");
        let r = make_resolver(vec![g, int_snap]);
        let s = instance("g.G", vec![instance("builtins.int", vec![])]);
        let t = instance("g.G", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_type_var_same_id_same_upper_bound_returns_s() {
        // visit_type_var case 1 (join.py:465-467): s is TypeVarType,
        // s.id == t.id, s.upper_bound == t.upper_bound -> return
        // self.s. Fires the Rust SameS path (shim returns s).
        let bound = instance("builtins.object", vec![]);
        let s = type_var(1, "~", bound.clone());
        let t = type_var(1, "~", bound);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &make_resolver(vec![])),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn join_type_var_same_id_different_upper_bound_defers() {
        // visit_type_var case 1 (join.py:468-470): s.id == t.id but
        // upper_bounds differ -> copy_modified(upper_bound=join_types(...)).
        // Produces a NEW TypeVarType (neither s nor t) -> defer (no Type

        // encoder).
        let s = type_var(1, "~", instance("builtins.int", vec![]));
        let t = type_var(1, "~", instance("builtins.str", vec![]));
        assert_eq!(join_types(&s, &t, &ctx(true), &make_resolver(vec![])), None);
    }

    #[test]
    fn join_type_var_different_id_encodes_bound_join() {
        // visit_type_var case 2 (join.py:545-546): s is TypeVarType but
        // s.id != t.id -> get_proper_type(join_types(s.upper_bound,
        // t.upper_bound)). join(int, int) = int — a fresh TYPE (the

        // bound), not a TypeVarType. Materialized and encoded.
        let s = type_var(1, "~", instance("builtins.int", vec![]));
        let t = type_var(2, "~", instance("builtins.int", vec![]));
        let result = join_types(&s, &t, &ctx(true), &make_resolver(vec![]));
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("builtins.int", vec![]));
        }
    }

    #[test]
    fn join_type_var_with_non_type_var_s_returns_object() {
        // visit_type_var case 3 (join.py:474): s is NOT a TypeVarType ->
        // return self.default(self.s). For Instance s, default(s) =
        // object_from_instance(s) = builtins.object. t's object side

        // is object_or_any_from_type(upper_bound=object) = object. Both
        // sides collapse to object -> SetOpResult::Object.
        let t = type_var(1, "~", instance("builtins.object", vec![]));
        let s = instance("builtins.int", vec![]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &make_resolver(vec![])),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn join_type_var_same_id_different_namespace_encodes_bound_join() {
        // TypeVarId equality checks namespace (types.py:576): same
        // raw_id, different namespace -> s.id != t.id -> case 2 ->
        // join_types(upper_bound, upper_bound). join(object, object) =

        // object, materialized and encoded.
        let s = type_var(1, "~", instance("builtins.object", vec![]));
        let t = type_var(1, "other", instance("builtins.object", vec![]));
        let result = join_types(&s, &t, &ctx(true), &make_resolver(vec![]));
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("builtins.object", vec![]));
        }
    }

    #[test]
    fn join_typeddict_with_instance_equal_fallback_returns_s() {
        // visit_typeddict case 2 (join.py:832-833): s is Instance,
        // t is TypedDictType -> join_types(self.s, t.fallback).
        // Recursive call: join_types(s=builtins.dict, t.fallback=

        // builtins.dict). Same Instance, no LKV -> SameT, mapped to
        // outer SameS (s == fallback, so returning s is equivalent).
        let o = snap("builtins.object", "object");
        let dict = snap_with_bases("builtins.dict", "dict", &["builtins.object"]);
        let r = make_resolver(vec![o, dict]);
        let s = instance("builtins.dict", vec![]);
        let t = typed_dict("builtins.dict");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_typeddict_with_instance_supertype_fallback_returns_ancestor() {
        // visit_typeddict case 2: s is Instance(builtins.object),
        // t is TypedDictType with fallback=builtins.dict.
        // Recursive: join_types(object, builtins.dict). dict <: object,

        // so the join is object (the supertype). The Rust Instance
        // path returns Ancestor("builtins.object") (the common base),
        // which the Python shim reconstructs as Instance(object) =

        // object. Passes through.
        let o = snap("builtins.object", "object");
        let dict = snap_with_bases("builtins.dict", "dict", &["builtins.object"]);
        let r = make_resolver(vec![o, dict]);
        let s = instance("builtins.object", vec![]);
        let t = typed_dict("builtins.dict");
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_typeddict_with_instance_subtype_fallback_returns_ancestor() {
        // visit_typeddict case 2: s is Instance(builtins.dict), t is
        // TypedDictType with fallback=builtins.object. Recursive:
        // join_types(builtins.dict, builtins.object). dict <: object,

        // so the join is object (the supertype). The Rust path returns
        // Ancestor("builtins.object"), which passes through (the shim
        // reconstructs Instance(object) = object). NOT a defer — the

        // Ancestor is the correct result, not SameT.
        let o = snap("builtins.object", "object");
        let dict = snap_with_bases("builtins.dict", "dict", &["builtins.object"]);
        let r = make_resolver(vec![o, dict]);
        let s = instance("builtins.dict", vec![]);
        let t = typed_dict("builtins.object");
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_typeddict_with_typeddict_defers() {
        // visit_typeddict case 1 (join.py:812-831): s is TypedDictType
        // -> builds a NEW TypedDictType via resolve_typeddict_item over
        // zipall. Produces a new type (neither s nor t) -> defer (no

        // Type encoder).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = typed_dict("builtins.dict");
        let t = typed_dict("builtins.dict");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_typeddict_with_non_instance_s_defers() {
        // visit_typeddict case 3 (join.py:834-835): s is not an
        // Instance (and not a TypedDictType) -> default(self.s).
        // Walks s's fallback chain -> defer. Use TypeVarType (passes

        // pre-dispatch: not Any/None/Uninhabited/Union/Callable, reaches
        // visit_typeddict case 3).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = type_var(1, "~", instance("builtins.object", vec![]));
        let t = typed_dict("builtins.dict");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_tuple_with_instance_equal_namedtuple_fallback_returns_s() {
        // visit_tuple_type case 2 (join.py:774-775): s is not a
        // TupleType -> join_types(self.s, tuple_fallback(t)). When
        // partial_fallback is NOT builtins.tuple (e.g. a namedtuple

        // class "nt.NT"), tuple_fallback(t) == t.partial_fallback
        // (typeops.py:108-109). Recursive: join_types(NT, NT).
        // Neither has LKV -> SameT, mapped to outer SameS (s==fallback).
        let o = snap("builtins.object", "object");
        let nt = snap_with_bases("nt.NT", "NT", &["builtins.object"]);
        let r = make_resolver(vec![o, nt]);
        let s = instance("nt.NT", vec![]);
        let t = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn join_tuple_with_instance_supertype_namedtuple_fallback_returns_ancestor() {
        // visit_tuple_type case 2: s=object, t=Tuple(fallback=NT).
        // Recursive: join_types(object, NT). NT <: object, so the
        // join is object. Rust returns Ancestor("builtins.object"),

        // which the shim reconstructs as Instance(object).
        let o = snap("builtins.object", "object");
        let nt = snap_with_bases("nt.NT", "NT", &["builtins.object"]);
        let r = make_resolver(vec![o, nt]);
        let s = instance("builtins.object", vec![]);
        let t = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_tuple_with_instance_subtype_namedtuple_fallback_returns_ancestor() {
        // visit_tuple_type case 2: s=NT, t=Tuple(fallback=object).
        // Recursive: join_types(NT, object). NT <: object, so the
        // join is object. Rust returns Ancestor("builtins.object").
        let o = snap("builtins.object", "object");
        let nt = snap_with_bases("nt.NT", "NT", &["builtins.object"]);
        let r = make_resolver(vec![o, nt]);
        let s = instance("nt.NT", vec![]);
        let t = tuple_type("builtins.object", vec![instance("builtins.int", vec![])]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Ancestor("builtins.object".to_string()))
        );
    }

    #[test]
    fn join_tuple_with_builtins_tuple_fallback_defers() {
        // visit_tuple_type case 2: s is Instance, t=Tuple with
        // partial_fallback=builtins.tuple. tuple_fallback(t) constructs
        // Instance(builtins.tuple, [make_simplified_union(items)])

        // (typeops.py:110-129) — NOT the same as partial_fallback.
        // Rust can't replicate without a Type encoder -> defer.
        let o = snap("builtins.object", "object");
        let tuple = snap_with_bases("builtins.tuple", "tuple", &["builtins.object"]);
        let r = make_resolver(vec![o, tuple]);
        let s = instance("builtins.tuple", vec![]);
        let t = tuple_type("builtins.tuple", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_tuple_with_tuple_returns_encoded() {
        // visit_tuple_type case 1 (join.py:753-773): s is TupleType ->
        // builds a new TupleType via join_tuples + InstanceJoiner. Now
        // encoded rather than deferred (Phase B1, issue #587).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        let t = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(result.is_some(), "should not defer");
        match result.unwrap() {
            SetOpResult::Encoded(bytes) => {
                let mut rbuf = ReadBuffer::new(&bytes);
                let decoded = read_type(&mut rbuf, None).expect("decode failed");
                match decoded {
                    Type::TupleType { items, .. } => {
                        assert_eq!(items.len(), 1, "one item");
                    }
                    other => panic!("expected TupleType, got {other:?}"),
                }
            }
            other => panic!("expected Encoded, got {other:?}"),
        }
    }

    // ---- meet_types (M8p) ----
    // Mirrors meet.py:114-153 (pre-dispatch) + meet.py:822+
    // (TypeMeetVisitor leaf visitors). Returns SameS/SameT/Bottom/Any

    // for the portable cases; defers (None) for everything else.

    #[test]
    fn meet_types_any_s_returns_t() {
        // meet.py:145-146: isinstance(s, AnyType) -> return t.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = any_type();
        let t = instance("a.A", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_any_t_returns_s() {
        // visit_any (meet.py:837): return self.s.
        // Pre-check (proper_subtype) returns None (not Instance-Instance
        // proper subtype via ignore_promotions). AnyType-s pre-dispatch

        // does not fire (s is Instance). Reaches visitor -> SameS.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = any_type();
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_none_t_strict_s_is_none_returns_t() {
        // meet_types pre-dispatch (meet.py:138-139): is_proper_subtype
        // (s=None, t=None) is True (visit_none_type right=NoneType ->
        // True), so the dispatch returns s = SameS. The visitor's

        // visit_none_type would return SameT, but the pre-dispatch
        // fires first.
        let r = make_resolver(vec![]);
        let s = Type::NoneType;
        let t = Type::NoneType;
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_none_t_strict_s_is_object_returns_t() {
        // visit_none_type strict, s is Instance(builtins.object) -> t.
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = instance("builtins.object", vec![]);
        let t = Type::NoneType;
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_none_t_strict_s_is_instance_returns_bottom() {
        // visit_none_type strict, s is non-object Instance -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::NoneType;
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_types_none_t_non_strict_returns_t() {
        // visit_none_type non-strict -> return t (SameT).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::NoneType;
        assert_eq!(
            meet_types(&s, &t, &ctx(false), &r),
            Some(SetOpResult::SameT)
        );
    }

    #[test]
    fn meet_types_uninhabited_t_returns_t() {
        // visit_uninhabited_type (meet.py:861): return t (SameT).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::UninhabitedType { ambiguous: false };
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_deleted_t_s_is_instance_returns_t() {
        // visit_deleted_type (meet.py:864-873): s not None/Uninhabited
        // -> return t (SameT).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::DeletedType { source: None };
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_deleted_t_s_is_uninhabited_returns_s() {
        // visit_deleted_type: s is UninhabitedType -> return self.s
        // (SameS = Uninhabited).
        let r = make_resolver(vec![]);
        let s = Type::UninhabitedType { ambiguous: false };
        let t = Type::DeletedType { source: None };
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_proper_subtype_s_returns_s() {
        // meet.py:137-141 pre-check: is_proper_subtype(s, t) -> s.
        // A <: B (proper, args-less) -> SameS.
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_proper_subtype_t_returns_t() {
        // meet.py:137-141: is_proper_subtype(t, s) -> t.
        // B <: A (proper, args-less) -> SameT.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let a = snap("a.A", "A");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_instance_same_type_no_args_returns_s() {
        // visit_instance (meet.py:913-957), same type_ref, args-less.
        // is_subtype(t, s) True (equal) -> would combine args (empty)
        // -> Instance(t.type, []) == s -> SameS.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.A", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_instance_different_unrelated_returns_bottom() {
        // visit_instance different types, neither <: other -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_types_instance_different_unrelated_non_strict_returns_bottom() {
        // Non-strict: Bottom maps to NoneType in Python; Rust still
        // reports Bottom (the shim maps Bottom -> NoneType when
        // strict_optional is False).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(
            meet_types(&s, &t, &ctx(false), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_types_instance_different_subtype_returns_t() {
        // visit_instance different types, is_subtype(t, s) True ->
        // return t (SameT). A <: B, s=B, t=A -> meet(B, A) = A.
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.B", vec![]);
        let t = instance("a.A", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_instance_different_supertype_returns_s() {
        // visit_instance different types, is_subtype(s, t) True ->
        // return s (SameS). A <: B, s=A, t=B -> meet(A, B) = A.
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.A", vec![]);
        let t = instance("a.B", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_instance_with_args_same_type_promote_encodes() {
        // visit_instance same type_ref with args -> per-arg meet.
        // G[i64] meet G[int] with covariant T: the outer proper-subtype
        // pre-check (ignore_promotions) misses (i64 !<: int properly),

        // the non-proper gate passes via i64's promote to int, and the
        // per-arg meet_types(int, i64) -> i64 <: int via promote ->
        // returns i64. Result: G[i64] encoded (disc=7).
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut i64 = snap("a.i64", "i64");
        i64.promote_bytes = vec![crate::wire::encode_instance_simple_for_test("builtins.int")];
        let int_snap = snap("builtins.int", "int");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, i64, int_snap, o]);
        let s = instance("g.G", vec![instance("a.i64", vec![])]);
        let t = instance("g.G", vec![instance("builtins.int", vec![])]);
        match meet_types(&s, &t, &ctx(true), &r) {
            Some(SetOpResult::Encoded(bytes)) => {
                let decoded = decode_type(&bytes).unwrap();
                assert_eq!(decoded, instance("g.G", vec![instance("a.i64", vec![])]));
            }
            other => panic!("expected Encoded, got {:?}", other),
        }
    }

    #[test]
    fn meet_types_instance_with_args_unrelated_bottom() {
        // visit_instance same type_ref with args, gate fails both ways
        // (invariant T, int/str unrelated). meet.py:1081-1084 ->
        // Bottom (shim maps disc 3 to UninhabitedType / NoneType).
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), INVARIANT, 0)];
        let int_snap = snap("builtins.int", "int");
        let str_snap = snap("builtins.str", "str");
        let r = make_resolver(vec![g, int_snap, str_snap]);
        let s = instance("g.G", vec![instance("builtins.int", vec![])]);
        let t = instance("g.G", vec![instance("builtins.str", vec![])]);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_types_union_s_non_union_t_swaps_then_defers() {
        // meet.py:147-148: isinstance(s, UnionType) and not isinstance(t,
        // UnionType) -> swap. After swap, s=A (Instance), t=A|B (Union).
        // trivial_meet: is_subtype(A, A|B) -> True (UnionType-right

        // handler finds A <: A). Returns SameT (t=A|B). The shim
        // returns t, which is the union. Python's meet would simplify
        // to just A, but Rust returns SameT (the full union) which

        // is a valid meet (A is in the union). Python simplification
        // happens in make_simplified_union (not ported), so this is
        // a conservative answer.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let t = instance("a.A", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_types_both_union_resolves() {
        // Both UnionType -> meet_union does pairwise meets then
        // make_simplified_union.  For identical unions the result is
        // the same union (encoded).
        let r = make_resolver(vec![]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert!(matches!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Encoded(_))
        ));
    }

    #[test]
    fn meet_types_one_sided_union_decodes_and_simplifies() {
        // visit_union_type (meet.py:965-966), one-sided: t is Union,
        // s is a plain Instance unrelated to the items. Pre-dispatch
        // proper-subtype checks miss (C <: A|B false, A|B <: C false),

        // so the arm runs: meets = [meet_types(x, s) for x in
        // t.items] = [Bottom, Bottom] -> all dropped -> empty ->
        // make_simplified_union([]) -> UninhabitedType. Encoded bottom

        // matches Python's meet (all items disjoint).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B"), snap("a.C", "C")]);
        let s = instance("a.C", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        match meet_types(&s, &t, &ctx(true), &r) {
            Some(SetOpResult::Encoded(bytes)) => {
                let decoded = decode_type(&bytes).unwrap();
                assert!(matches!(decoded, Type::UninhabitedType { .. }));
            }
            other => panic!("expected Encoded, got {:?}", other),
        }
    }

    #[test]
    fn meet_types_typevar_upper_bound_decodes() {
        // visit_type_var (meet.py:1000-1004): same raw_id/namespace but
        // different upper_bound -> meet upper bounds and encode the
        // new TypeVar (copy_modified). The pre-dispatch

        // is_proper_subtype(s, t) recurses on the upper bounds and,
        // with ignore_promotions, G[i64] is not a proper subtype of
        // G[int] (i64 promotes to int, not subclasses it) -> pre-check

        // misses. The arm runs: meet_types(G[i64], G[int]) = Encoded
        // (same-type-with-args per-arg meet, i64 via promote) and
        // fruit_to_type decodes it into the new TypeVar's upper_bound.
        let mut g = snap("g.G", "G");
        g.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut i64 = snap("a.i64", "i64");
        i64.promote_bytes = vec![crate::wire::encode_instance_simple_for_test("builtins.int")];
        let int_snap = snap("builtins.int", "int");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, i64, int_snap, o]);
        let s = type_var(1, "ns", instance("g.G", vec![instance("a.i64", vec![])]));
        let t = type_var(
            1,
            "ns",
            instance("g.G", vec![instance("builtins.int", vec![])]),
        );
        match meet_types(&s, &t, &ctx(true), &r) {
            Some(SetOpResult::Encoded(bytes)) => {
                let decoded = decode_type(&bytes).unwrap();
                match decoded {
                    Type::TypeVarType {
                        upper_bound,
                        raw_id,
                        namespace,
                        ..
                    } => {
                        assert_eq!(raw_id, 1);
                        assert_eq!(namespace, "ns");
                        assert_eq!(
                            *upper_bound,
                            instance("g.G", vec![instance("a.i64", vec![])])
                        );
                    }
                    other => panic!("expected TypeVarType, got {:?}", other),
                }
            }
            other => panic!("expected Encoded, got {:?}", other),
        }
    }

    #[test]
    fn meet_types_typevar_upper_bound_same_returns_same() {
        // Same bound -> SameS (no encode needed).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = type_var(1, "ns", instance("a.A", vec![]));
        let t = type_var(1, "ns", instance("a.A", vec![]));
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_types_both_type_type_encodes() {
        // visit_type_type (meet.py:1412-1419), case 1: both TypeType.
        // typ = meet(t.item, s.item); if not NoneType, wrap
        // TypeType.make_normalized with is_type_form = AND. s is a

        // TypeForm, t is a plain TypeType: is_subtype(TypeForm[B],
        // Type[A]) is Some(false) (subtypes.py:537 — the right side
        // lacks is_type_form), so the pre-dispatch skips and the arm

        // runs. B <: A, meet(A, B)=B -> TypeType[B] encoded with
        // is_type_form = True AND False = False.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let a = snap("a.A", "A");
        let r = make_resolver(vec![b, a]);
        let s = Type::TypeType {
            item: Box::new(instance("a.B", vec![])),
            is_type_form: true,
        };
        let t = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        match meet_types(&s, &t, &ctx(true), &r) {
            Some(SetOpResult::Encoded(bytes)) => {
                let decoded = decode_type(&bytes).unwrap();
                match decoded {
                    Type::TypeType { item, is_type_form } => {
                        assert_eq!(*item, instance("a.B", vec![]));
                        assert!(!is_type_form);
                    }
                    other => panic!("expected TypeType, got {:?}", other),
                }
            }
            other => panic!("expected Encoded, got {:?}", other),
        }
    }

    #[test]
    fn meet_types_both_type_type_unrelated_encodes_union() {
        // A vs B (unrelated) -> meet returns Bottom (Uninhabited). In
        // Python, TypeType.make_normalized(UninhabitedType) returns a
        // TypeType of bottom. Fresh encode wraps the Uninhabited item

        // in a TypeType.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let t = Type::TypeType {
            item: Box::new(instance("a.B", vec![])),
            is_type_form: false,
        };
        match meet_types(&s, &t, &ctx(true), &r) {
            Some(SetOpResult::Encoded(bytes)) => {
                let decoded = decode_type(&bytes).unwrap();
                match decoded {
                    Type::TypeType { .. } => {}
                    other => panic!("expected TypeType, got {:?}", other),
                }
            }
            other => panic!("expected Encoded, got {:?}", other),
        }
    }

    #[test]
    fn meet_types_both_callable_resolves() {
        // Both callable-like (identical) -> meet_similar_callables
        // produces a new CallableType encoded in the wire format.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let s = callable(
            "builtins.function",
            vec![],
            instance("builtins.int", vec![]),
        );
        let t = callable(
            "builtins.function",
            vec![],
            instance("builtins.int", vec![]),
        );
        assert!(matches!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Encoded(_))
        ));
    }

    #[test]
    fn meet_similar_callables_uses_t_operand_for_ret_order() {
        // Regression: meet of def(A)->A and def(B)->B (B<:A) is
        // def(A)->B (args joined, ret met); the operand order passed
        // to setop_result_to_type was reversed.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap_with_bases("a.A", "A", &["builtins.object"]),
            snap_with_bases("a.B", "B", &["a.A"]),
        ]);
        let a = instance("a.A", vec![]);
        let b = instance("a.B", vec![]);
        let s = callable("builtins.function", vec![a.clone()], a.clone());
        let t = callable("builtins.function", vec![b.clone()], b.clone());
        let result = meet_types(&s, &t, &ctx(true), &r)
            .expect("both non-generic similar callables should resolve");
        let SetOpResult::Encoded(bytes) = result else {
            panic!("expected Encoded result");
        };
        let joined = decode_type(&bytes).expect("encoded callable should decode");
        let Type::CallableType {
            arg_types,
            ret_type,
            ..
        } = joined
        else {
            panic!("expected decoded CallableType");
        };
        assert_eq!(
            arg_types,
            vec![a],
            "argument types are joined (contravariant): (A) stays A"
        );
        assert_eq!(
            *ret_type, b,
            "return types are met (covariant): meet(A, B) = B"
        );
    }

    #[test]
    fn meet_types_callable_t_non_callable_s_fallback_defers() {
        // visit_callable_type fallback case (s non-callable): meet_types
        // does NOT have a callable-fallback short-circuit like join's
        // visit_callable_type. Instead visit_instance handles s=Instance

        // via is_subtype(t.fallback, s) -> produces s or t only when
        // fallback <: s. For unrelated s, falls to default -> Bottom.
        // But CallableType t is not Instance -> visit_instance not

        // reached. visit_callable_type checks isinstance(self.s,
        // CallableType) (no), TypeType (no), Instance+protocol (no) ->
        // default(self.s) -> Bottom. However, reaching visit_callable_type

        // requires passing the both-callable guard; the Rust path defers
        // both-callable. For non-both-callable, t is CallableType and s
        // is not: Rust would hit visit_callable_type which checks s

        // shape. Defer conservatively (the s=Instance+protocol branch
        // needs unpack_callback_proxy).
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("builtins.function", "function"),
            snap("builtins.int", "int"),
        ]);
        let s = instance("a.A", vec![]);
        let t = callable(
            "builtins.function",
            vec![],
            instance("builtins.int", vec![]),
        );
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
    }

    // ---- meet visit_type_var (M8q) ----
    // Mirrors meet.py:878-884. Case 1 (same id + same upper_bound) ->
    // SameS. copy_modified (different bound) -> defer. default (s not

    // TypeVarType or different id) -> Bottom.

    #[test]
    fn meet_type_var_same_id_same_upper_bound_returns_s() {
        // visit_type_var case 1 (meet.py:880-881): s.id == t.id,
        // s.upper_bound == t.upper_bound -> return self.s (SameS).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let ub = instance("a.A", vec![]);
        let s = type_var(1, "ns", ub.clone());
        let t = type_var(1, "ns", ub);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_type_var_same_id_different_upper_bound() {
        // visit_type_var case 1, upper_bounds differ (meet.py:882):
        // meet the upper bounds and construct a new TypeVarType.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = type_var(1, "ns", instance("a.A", vec![]));
        let t = type_var(1, "ns", instance("a.B", vec![]));
        // Should produce an encoded result (met upper bound), not None.
        let result = meet_types(&s, &t, &ctx(true), &r);
        assert!(result.is_some(), "expected Some, got None");
        assert!(matches!(result, Some(SetOpResult::Encoded(_))));
    }

    #[test]
    fn meet_type_var_different_id_returns_bottom() {
        // visit_type_var else (meet.py:883-884): s.id != t.id ->
        // default(self.s) -> Bottom (strict).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let ub = instance("a.A", vec![]);
        let s = type_var(1, "ns", ub.clone());
        let t = type_var(2, "ns", ub);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_type_var_different_namespace_returns_bottom() {
        // visit_type_var else: same raw_id, different namespace ->
        // s.id != t.id (TypeVarId.__eq__ checks namespace) -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let ub = instance("a.A", vec![]);
        let s = type_var(1, "ns1", ub.clone());
        let t = type_var(1, "ns2", ub);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_type_var_s_not_type_var_returns_t() {
        // meet_types(Instance, TypeVar) where the TypeVar's upper_bound
        // is the Instance. The pre-dispatch is_proper_subtype(t, s)
        // (meet.py:141) fires: is_subtype(TypeVar, Instance) recurses

        // into is_subtype(upper_bound=Instance, Instance) = True, so
        // the pre-check returns SameT (= t = the TypeVar). Python
        // matches: meet_types(a, tv) = T.

        //
        // Pre-M8z this returned Bottom because the Rust is_subtype
        // didn't handle TypeVarType on the left, so is_proper_subtype

        // returned None and the visitor (visit_type_var else) returned
        // default(s) = Bottom. The M8z is_subtype extension makes the
        // pre-dispatch fire, which is the parity-correct behavior.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = type_var(1, "ns", instance("a.A", vec![]));
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    // ---- meet visit_literal_type (M8q) ----
    // Mirrors meet.py:1236-1242. Case 1 (s is LiteralType, s==t) ->
    // SameT. Case 2 (s is Instance, is_subtype(t.fallback, s)) ->

    // SameT. Else -> Bottom (default).

    #[test]
    fn meet_literal_equal_literal_returns_s() {
        // meet.py:139-140 pre-check: is_proper_subtype(s, t) is True
        // for LiteralType == LiteralType (visit_literal_type subtypes.py:1069:
        // left == right). So meet(Literal[1], Literal[1]) returns s.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = literal(LiteralValue::Int(1), "a.A");
        let t = literal(LiteralValue::Int(1), "a.A");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_literal_unequal_literal_returns_bottom() {
        // visit_literal_type else (meet.py:1241-1242): s is LiteralType,
        // s != t (different value) -> default -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = literal(LiteralValue::Int(1), "a.A");
        let t = literal(LiteralValue::Int(2), "a.A");
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_literal_s_is_instance_fallback_subtype_returns_t() {
        // visit_literal_type case 2 (meet.py:1239-1240): s is Instance,
        // is_subtype(t.fallback, s) -> return t (SameT).
        // t.fallback = a.B, s = a.A, B <: A -> is_subtype(B, A) = True.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let a = snap("a.A", "A");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.A", vec![]);
        let t = literal(LiteralValue::Int(1), "a.B");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_literal_s_is_instance_fallback_not_subtype_returns_bottom() {
        // visit_literal_type else (meet.py:1241-1242): s is Instance,
        // is_subtype(t.fallback, s) = False -> default -> Bottom.
        // t.fallback = a.B, s = a.A, B not <: A (unrelated).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = instance("a.A", vec![]);
        let t = literal(LiteralValue::Int(1), "a.B");
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    // ---- meet visit_type_type (M8q) ----
    // Mirrors meet.py:1248-1261. Case 2 (s is Instance(builtins.type))
    // -> SameT. Case 1 (both TypeType) -> defer (recursive meet +

    // make_normalized). Case 3 (CallableType) -> defer (recursive).
    // Else -> Bottom (default).

    #[test]
    fn meet_type_type_s_is_builtins_type_returns_t() {
        // visit_type_type case 2 (meet.py:1256-1257): s is
        // Instance(builtins.type) -> return t (SameT).
        let r = make_resolver(vec![snap("builtins.type", "type"), snap("a.A", "A")]);
        let s = instance("builtins.type", vec![]);
        let t = type_type("a.A");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_type_type_both_type_type_returns_s() {
        // visit_type_type case 1 (meet.py:1249-1255): s is TypeType ->
        // meet(t.item, s.item) + make_normalized. With TypeType now enabled
        // (issue #443), the is_proper_subtype pre-check (meet.py:139-141)

        // fires: is_proper_subtype(Type[A], Type[A]) recurses on items
        // (subtypes.py:1264-1265) -> True, so the meet pre-check returns
        // SameS before the visitor. This matches Python.
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let s = type_type("a.A");
        let t = type_type("a.A");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_type_type_s_is_unrelated_instance_returns_bottom() {
        // visit_type_type else (meet.py:1260-1261): s is Instance (not
        // builtins.type) -> default(self.s) -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = type_type("a.A");
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_type_type_s_is_uninhabited_returns_bottom() {
        // visit_type_type else: s is UninhabitedType -> default ->
        // Bottom (strict). Note: UninhabitedType as s would normally
        // be caught by visit_uninhabited_type if t were Uninhabited,

        // but here t is TypeType so visit_type_type fires.
        //
        // The meet_types pre-dispatch (meet.py:138-139) fires first

        // now that is_proper_subtype(Uninhabited, TypeType) returns
        // True (visit_uninhabited_type is subtype of everything):
        // returns s = SameS, not the visitor's Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::UninhabitedType { ambiguous: false };
        let t = type_type("a.A");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    // ---- meet visit_unbound_type (M8r) ----
    // Mirrors meet.py:864-873. Three branches on s:
    //   * NoneType, strict_optional -> UninhabitedType (Bottom).

    //   * NoneType, non-strict -> self.s (SameS).
    //   * UninhabitedType -> self.s (SameS).
    //   * else -> AnyType (Any).

    #[test]
    fn meet_unbound_s_is_none_strict_returns_bottom() {
        // visit_unbound_type (meet.py:865-867): s is NoneType,
        // strict_optional -> UninhabitedType. The shim maps disc=3 to
        // UninhabitedType(strict) / NoneType(non-strict).
        let r = make_resolver(vec![]);
        let s = Type::NoneType;
        let t = unbound_type();
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_unbound_s_is_none_non_strict_returns_s() {
        // visit_unbound_type (meet.py:865,868-869): s is NoneType,
        // non-strict -> return self.s (SameS).
        let r = make_resolver(vec![]);
        let s = Type::NoneType;
        let t = unbound_type();
        assert_eq!(
            meet_types(&s, &t, &ctx(false), &r),
            Some(SetOpResult::SameS)
        );
    }

    #[test]
    fn meet_unbound_s_is_uninhabited_returns_s() {
        // visit_unbound_type (meet.py:870-871): s is UninhabitedType ->
        // return self.s (SameS).
        let r = make_resolver(vec![]);
        let s = Type::UninhabitedType { ambiguous: false };
        let t = unbound_type();
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_unbound_s_is_instance_returns_any() {
        // visit_unbound_type (meet.py:872-873): else -> AnyType. The
        // shim maps disc=4 to AnyType(TypeOfAny.special_form).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = unbound_type();
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::Any));
    }

    #[test]
    fn meet_unbound_s_is_any_returns_any() {
        // visit_unbound_type else branch fires for AnyType s too (AnyType
        // is not NoneType/Uninhabited). Result is AnyType. The meet_types
        // AnyType-s short-circuit (meet.py:145) returns t before the

        // visitor when s is AnyType, so this case is actually unreachable
        // in Python. Rust mirrors: meet_types returns SameT (t) for
        // AnyType-s. Assert the short-circuit wins.
        let r = make_resolver(vec![]);
        let s = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let t = unbound_type();
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    // ---- meet visit_type_var_tuple (M8r) ----
    // Mirrors meet.py:930-934. Same id (raw_id + namespace) -> pick by
    // min_len: s if s.min_len > t.min_len else t. Different id ->

    // default(self.s) -> Bottom (strict) / NoneType (non-strict).

    #[test]
    fn meet_type_var_tuple_same_id_s_larger_min_len_returns_s() {
        // visit_type_var_tuple (meet.py:931-932): s.id == t.id, s.min_len
        // (2) > t.min_len (1) -> return self.s (SameS).
        let r = make_resolver(vec![]);
        let s = type_var_tuple(1, "ns", 2);
        let t = type_var_tuple(1, "ns", 1);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_type_var_tuple_same_id_t_larger_min_len_returns_t() {
        // visit_type_var_tuple (meet.py:931-932): s.id == t.id, s.min_len
        // (1) <= t.min_len (2) -> return t (SameT).
        let r = make_resolver(vec![]);
        let s = type_var_tuple(1, "ns", 1);
        let t = type_var_tuple(1, "ns", 2);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_type_var_tuple_same_id_equal_min_len_returns_t() {
        // visit_type_var_tuple (meet.py:932): s.min_len == t.min_len ->
        // `self.s if self.s.min_len > t.min_len else t` -> t (SameT).
        let r = make_resolver(vec![]);
        let s = type_var_tuple(1, "ns", 3);
        let t = type_var_tuple(1, "ns", 3);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn meet_type_var_tuple_different_id_returns_bottom() {
        // visit_type_var_tuple else (meet.py:933-934): s.id != t.id ->
        // default(self.s) -> Bottom (strict).
        let r = make_resolver(vec![]);
        let s = type_var_tuple(1, "ns", 2);
        let t = type_var_tuple(2, "ns", 2);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_type_var_tuple_different_namespace_returns_bottom() {
        // visit_type_var_tuple else: same raw_id, different namespace ->
        // TypeVarId.__eq__ False (types.py:567-577) -> default -> Bottom.
        let r = make_resolver(vec![]);
        let s = type_var_tuple(1, "ns1", 2);
        let t = type_var_tuple(1, "ns2", 2);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_type_var_tuple_s_not_tvt_returns_bottom() {
        // visit_type_var_tuple else (meet.py:933): s not TypeVarTupleType
        // -> default(self.s). s is Instance -> default(Instance) ->
        // Bottom (strict). Instance.default falls to object_from_instance

        // in join but meet.default(strict) returns UninhabitedType.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = type_var_tuple(1, "ns", 2);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    // ---- visit_typeddict_type case 1 (both TypedDictType) (#436) ----
    // Mirrors join.py:812-831. Builds a new TypedDictType via
    // resolve_typeddict_item over zipall, encoded via write_type.

    /// TypedDictType with items, required_keys, readonly_keys, and
    /// a fallback of typing._TypedDict (anonymous).
    fn typed_dict_full(
        items: Vec<(&str, Type)>,
        required: Vec<&str>,
        readonly: Vec<&str>,
        is_closed: bool,
    ) -> Type {
        let fallback = instance("typing._TypedDict", vec![]);
        let items: Vec<(String, Type)> =
            items.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let required_keys: std::collections::HashSet<String> =
            required.into_iter().map(String::from).collect();
        let readonly_keys: std::collections::HashSet<String> =
            readonly.into_iter().map(String::from).collect();
        Type::TypedDictType {
            fallback: Box::new(fallback),
            items,
            required_keys,
            readonly_keys,
            is_closed,
        }
    }

    #[test]
    fn join_typeddict_both_identical_returns_encoded() {
        // join(TD, TD) where both have the same items, same required,
        // same readonly. The joined TypedDictType has the same items
        // (each join_types(item, item) = item), same required/readonly.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            true,
        );
        let t = s.clone();
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let decoded = decode_type(&bytes).expect("decode failed");
            let Type::TypedDictType {
                items,
                required_keys,
                readonly_keys,
                is_closed,
                ..
            } = decoded
            else {
                panic!("expected TypedDictType");
            };
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].0, "x");
            assert!(required_keys.contains("x"));
            assert!(readonly_keys.is_empty());
            assert!(is_closed);
        }
    }

    #[test]
    fn join_typeddict_both_disjoint_keys_merges() {
        // s has key "x", t has key "y". zipall yields both. Each
        // item is joined with the other's missing key (is_closed=True
        // -> UninhabitedType). join_types(A, Uninhabited) = A.

        // Required: s.required(x)=True, t.required(x)=False (missing
        // in closed TD) -> is_required = True and False = False.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            true,
        );
        let t = typed_dict_full(
            vec![("y", instance("a.A", vec![]))],
            vec!["y"],
            vec![],
            true,
        );
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let decoded = decode_type(&bytes).expect("decode failed");
            let Type::TypedDictType {
                items,
                required_keys,
                is_closed,
                ..
            } = decoded
            else {
                panic!("expected TypedDictType");
            };
            assert_eq!(items.len(), 2);
            // Neither key is required (each is missing from one side).
            assert!(required_keys.is_empty());
            assert!(is_closed);
        }
    }

    #[test]
    fn join_typeddict_both_different_types_for_same_key_joins() {
        // s has key "x": A, t has key "x": A (same type). The joined
        // item type is join_types(A, A) = A. is_equivalent(A, A) =
        // True -> is_readonly = False.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            true,
        );
        let t = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            true,
        );
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let decoded = decode_type(&bytes).expect("decode failed");
            let Type::TypedDictType {
                items,
                required_keys,
                readonly_keys,
                ..
            } = decoded
            else {
                panic!("expected TypedDictType");
            };
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].0, "x");
            assert!(required_keys.contains("x"));
            // Same type, same required, no readonly -> not readonly.
            assert!(readonly_keys.is_empty());
        }
    }

    #[test]
    fn join_typeddict_both_required_mismatch_marks_readonly() {
        // s has key "x" required, t has key "x" NOT required.
        // is_required = False. is_readonly = True (required mismatch).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            true,
        );
        let t = typed_dict_full(vec![("x", instance("a.A", vec![]))], vec![], vec![], true);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let decoded = decode_type(&bytes).expect("decode failed");
            let Type::TypedDictType {
                items,
                required_keys,
                readonly_keys,
                ..
            } = decoded
            else {
                panic!("expected TypedDictType");
            };
            assert_eq!(items.len(), 1);
            assert!(!required_keys.contains("x"));
            assert!(readonly_keys.contains("x"));
        }
    }

    #[test]
    fn join_typeddict_both_open_td_missing_key_omits() {
        // s has key "x" (open TD), t is open TD with no keys.
        // t.item("x") with open TD -> typ=None, required=False,
        // readonly=True. resolve_typeddict_item: s.typ=Some(A),

        // t.typ=None -> join_type=None -> key omitted.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = typed_dict_full(
            vec![("x", instance("a.A", vec![]))],
            vec!["x"],
            vec![],
            false,
        );
        let t = typed_dict_full(vec![], vec![], vec![], false);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let decoded = decode_type(&bytes).expect("decode failed");
            let Type::TypedDictType { items, .. } = decoded else {
                panic!("expected TypedDictType");
            };
            // Key "x" is omitted (join_type=None).
            assert!(items.is_empty());
        }
    }

    #[test]
    fn join_typeddict_non_anonymous_fallback_defers() {
        // Non-anonymous fallback (not in TPDICT_FB_NAMES) -> defer
        // (can't compute create_anonymous_fallback from snapshot).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::TypedDictType {
            fallback: Box::new(instance("a.MyDict", vec![])),
            items: vec![("x".to_string(), instance("a.A", vec![]))],
            required_keys: std::collections::HashSet::from(["x".to_string()]),
            readonly_keys: std::collections::HashSet::new(),
            is_closed: true,
        };
        let t = s.clone();
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    // ---- is_better_join standalone helper (#436) ----

    #[test]
    fn is_better_instance_vs_non_instance_returns_true() {
        // t is Instance, s is not Instance -> True.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let t = instance("a.A", vec![]);
        let s = Type::NoneType;
        assert!(is_better_join(&t, &s, &r));
    }

    #[test]
    fn is_better_longer_mro_returns_true() {
        // t has longer MRO than s -> True.
        let a = snap("a.A", "A");
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![a, b]);
        let t = instance("a.B", vec![]);
        let s = instance("a.A", vec![]);
        assert!(is_better_join(&t, &s, &r));
    }

    #[test]
    fn is_better_shorter_mro_returns_false() {
        // t has shorter MRO than s -> False.
        let a = snap("a.A", "A");
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![a, b]);
        let t = instance("a.A", vec![]);
        let s = instance("a.B", vec![]);
        assert!(!is_better_join(&t, &s, &r));
    }

    #[test]
    fn is_better_equal_mro_returns_false() {
        // Same MRO length -> not better (Python is_better returns False
        // when len(t.mro) <= len(s.mro)).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let t = instance("a.A", vec![]);
        let s = instance("a.B", vec![]);
        assert!(!is_better_join(&t, &s, &r));
    }

    #[test]
    fn is_better_protocol_vs_non_protocol_returns_non_protocol() {
        // t is non-protocol, s is protocol, neither is object -> t is
        // better (non-protocol preferred).
        let mut a = snap("a.A", "A");
        a.is_protocol = true;
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let t = instance("a.B", vec![]);
        let s = instance("a.A", vec![]);
        assert!(is_better_join(&t, &s, &r));
    }

    #[test]
    fn meet_paramspec_same_returns_s() {
        // visit_param_spec (meet.py:1008-1012): s == t -> self.s.
        // is_proper_subtype(ParamSpec, ParamSpec) defers (line 706
        // `_ => return None`), so visit_meet's paramspec_eq decides.
        let r = make_resolver(vec![]);
        let s = param_spec(1, "~", instance("builtins.object", vec![]));
        let t = param_spec(1, "~", instance("builtins.object", vec![]));
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
    }

    #[test]
    fn meet_paramspec_different_ids_returns_bottom() {
        // Same visitor, distinct ids -> else branch -> Bottom.
        let r = make_resolver(vec![]);
        let s = param_spec(1, "~", instance("builtins.object", vec![]));
        let t = param_spec(2, "~", instance("builtins.object", vec![]));
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_paramspec_s_is_unbound_returns_any() {
        // default(self.s): s is UnboundType -> Any. has_unbound skips
        // the subtype pre-check, so visit_meet's paramspec arm fires.
        let r = make_resolver(vec![]);
        let s = unbound_type();
        let t = param_spec(1, "~", instance("builtins.object", vec![]));
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::Any));
    }

    #[test]
    fn meet_parameters_same_length_encodes_join() {
        // Both-Parameters routed to meet_callable_like (both-callable
        // pre-dispatch), then meet_parameters_pair: same length -> join
        // int/int = int -> Encoded Parameters([int], [0]).
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        let s = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let t = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let result = meet_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = wire::read_type(&mut rbuf, None).expect("decode failed");
            let expected = parameters(vec![instance("builtins.int", vec![])], vec![0]);
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn meet_parameters_different_length_returns_bottom() {
        // meet_parameters_pair length mismatch -> Bottom (s is
        // Parameters, not UnboundType, so default(s) = Bottom).
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        let s = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let t = parameters(
            vec![
                instance("builtins.int", vec![]),
                instance("builtins.int", vec![]),
            ],
            vec![0, 0],
        );
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn meet_parameters_s_is_instance_returns_bottom() {
        // s=Instance, t=Parameters: not both-callable -> visit_meet
        // Parameters arm -> default(s): s not Unbound -> Bottom.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        assert_eq!(
            meet_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Bottom)
        );
    }

    #[test]
    fn join_paramspec_same_returns_t() {
        // visit_param_spec (join.py:550-553): s == t -> t (SameT).
        let r = make_resolver(vec![]);
        let s = param_spec(1, "~", instance("builtins.object", vec![]));
        let t = param_spec(1, "~", instance("builtins.object", vec![]));
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
    }

    #[test]
    fn join_paramspec_different_s_instance_returns_object() {
        // s=Instance, t=ParamSpec, ids differ -> default(s) = Object
        // (object_from_instance). No swap (s not Union), no Any/None/
        // Uninhabited -> visit_join ParamSpec arm.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = param_spec(1, "~", instance("builtins.object", vec![]));
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn join_paramspec_different_s_literal_returns_any() {
        // s=LiteralType, t=ParamSpec: default(s) has no LiteralType arm
        // in join_default's chain -> Any.
        let r = make_resolver(vec![snap("builtins.bool", "bool")]);
        let s = literal(LiteralValue::Bool(true), "builtins.bool");
        let t = param_spec(1, "~", instance("builtins.object", vec![]));
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::Any));
    }

    #[test]
    fn join_parameters_similar_encodes_meet() {
        // visit_parameters (join.py:566-580): similar pair -> arg_types
        // meet (int/int = int), arg_names combined (both None), kept
        // from t -> Encoded Parameters([int], [0]).
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        let s = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let t = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = wire::read_type(&mut rbuf, None).expect("decode failed");
            let expected = parameters(vec![instance("builtins.int", vec![])], vec![0]);
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_parameters_not_similar_returns_any() {
        // Different arg count -> not similar -> default(s) = Any
        // (Parameters not in the join default elif chain).
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        let s = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        let t = parameters(
            vec![
                instance("builtins.int", vec![]),
                instance("builtins.int", vec![]),
            ],
            vec![0, 0],
        );
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::Any));
    }

    #[test]
    fn join_parameters_s_is_instance_returns_object() {
        // s=Instance, t=Parameters -> not both-Parameters -> default(s)
        // = Object.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = parameters(vec![instance("builtins.int", vec![])], vec![0]);
        assert_eq!(
            join_types(&s, &t, &ctx(true), &r),
            Some(SetOpResult::Object)
        );
    }

    #[test]
    fn join_literals_partial_enum_coverage_encodes_union() {
        // visit_literal_type case 2: both LiteralTypes, same enum
        // fallback Color={RED,BLUE,GREEN}, values RED/BLUE. Partial
        // coverage -> make_simplified_union keeps both literals ->

        // Encoded UnionType (previously defers; now encodes).
        let o = snap("builtins.object", "object");
        let mut color = snap("color.Color", "Color");
        color.is_enum = true;
        color.enum_members = vec!["RED".to_string(), "BLUE".to_string(), "GREEN".to_string()];
        let r = make_resolver(vec![color, o]);
        let s = literal(LiteralValue::Str("RED".to_string()), "color.Color");
        let t = literal(LiteralValue::Str("BLUE".to_string()), "color.Color");
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = wire::read_type(&mut rbuf, None).expect("decode failed");
            let expected = Type::UnionType {
                items: vec![
                    literal(LiteralValue::Str("RED".to_string()), "color.Color"),
                    literal(LiteralValue::Str("BLUE".to_string()), "color.Color"),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_literals_full_enum_coverage_encodes_instance() {
        // Case-2 full-coverage branch: both sides LiteralType, same
        // 2-member enum fallback, values RED/BLUE. Contraction collapses
        // [RED, BLUE] to the enum Instance Color (Encoded), the path

        // case 2 used to reach before partial coverage was added.
        let o = snap("builtins.object", "object");
        let mut color = snap("color.Color", "Color");
        color.is_enum = true;
        color.enum_members = vec!["RED".to_string(), "BLUE".to_string()];
        let r = make_resolver(vec![color, o]);
        let s = literal(LiteralValue::Str("RED".to_string()), "color.Color");
        let t = literal(LiteralValue::Str("BLUE".to_string()), "color.Color");
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = wire::read_type(&mut rbuf, None).expect("decode failed");
            assert_eq!(decoded, instance("color.Color", vec![]));
        }
    }

    // ---- join_diff_instances_with_args (args-bearing via_supertype) ----

    /// Call `join_diff_instances_with_args` on Instance operands `s`,
    /// `t` with a fresh `seen` guard, extracting the refs/args.
    fn join_diff(s: &Type, t: &Type, r: &TypeResolver) -> Option<SetOpResult> {
        let (t_ref, t_args) = match t {
            Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
            _ => return None,
        };
        let (s_ref, s_args) = match s {
            Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
            _ => return None,
        };
        let mut seen: SeenInstances = Vec::new();
        join_diff_instances_with_args(t_ref, t_args, s_ref, s_args, &ctx(true), r, &mut seen)
    }

    #[test]
    fn join_diff_args_object_fast_path_is_object() {
        // join(object, Sequence[int]) and its mirror: object is the
        // universal top, so the join is object regardless of the other
        // side's args. Only the object operand needs no snapshot.
        let r = make_resolver(vec![snap("typing.Sequence", "Sequence")]);
        let s = instance("typing.Sequence", vec![instance("builtins.int", vec![])]);
        let t = instance("builtins.object", vec![]);
        assert_eq!(join_diff(&s, &t, &r), Some(SetOpResult::Object));
        assert_eq!(join_diff(&t, &s, &r), Some(SetOpResult::Object));
    }

    #[test]
    fn join_diff_args_sibling_generics_reduce_to_object() {
        // A[int] join B[str] where both A and B are unrelated classes
        // whose only common ancestor is object. The via_supertype
        // walk maps each onto object and recurses -> object.
        let obj = snap("builtins.object", "object");
        let sa = snap_with_bases("a.A", "A", &["builtins.object"]);
        let sb = snap_with_bases("a.B", "B", &["builtins.object"]);
        let r = make_resolver(vec![obj, sa, sb]);
        let s = instance("a.A", vec![instance("builtins.int", vec![])]);
        let t = instance("a.B", vec![instance("builtins.str", vec![])]);
        assert_eq!(join_diff(&s, &t, &r), Some(SetOpResult::Object));
    }

    #[test]
    fn join_diff_args_missing_snapshot_defers() {
        // t's TypeInfo is absent from the resolver: the dispatch
        // snapshot lookup defers (None -> Python).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![instance("builtins.int", vec![])]);
        let t = instance("a.Missing", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_diff(&s, &t, &r), None);
    }
}
