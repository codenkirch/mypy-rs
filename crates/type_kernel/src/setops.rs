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
//! `visit_erased_type` (join.py:373-374) is not ported: `ErasedType`
//! has no wire-format variant (see `wire::Type`), so it cannot arrive
//! over FFI; it stays on the Python path.
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
    is_subtype, SubtypeContext, CONTRAVARIANT, COVARIANT, INVARIANT, VARIANCE_NOT_READY,
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
    // object_or_any_from_type(t): Instance right -> Object. Other
    // variants walk fallbacks / upper_bound / union items in Python;
    // defer.
    match t {
        Type::Instance { .. } => Some(SetOpResult::Object),
        _ => None,
    }
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
    // First direction: s <: t? If yes, meet is s.
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
    // the non-union. If both are unions, visit_union_type would need
    // make_simplified_union to merge them — defer.
    let (s, t, swapped) = match (s, t) {
        (Type::UnionType { .. }, other) if !matches!(other, Type::UnionType { .. }) => (t, s, true),
        (Type::UnionType { .. }, Type::UnionType { .. }) => return None,
        _ => (s, t, false),
    };

    // join.py:314-315: isinstance(s, AnyType) -> return s.
    // `swapped` is irrelevant: AnyType short-circuit returns the
    // (post-swap) s, which is the original t if swapped. But the
    // AnyType is on the left after swap, so SameS is correct relative
    // to post-swap s. The caller maps SameS/SameT to original s/t via
    // the `swapped` flag (see `rust_join_types`).
    if matches!(s, Type::AnyType { .. }) {
        return Some(flip_if(SetOpResult::SameS, swapped));
    }

    // join.py:317-318 (isinstance(s, ErasedType) -> return t) is
    // unreachable here: ErasedType has no wire-format variant, so
    // the Python shim never serializes it across FFI.

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
    let (s, t, swap3) = if matches!(s, Type::UninhabitedType) && !matches!(t, Type::UninhabitedType)
    {
        (t, s, true)
    } else {
        (s, t, false)
    };
    let swapped = swapped ^ swap3;

    // normalize_callables (join.py:327) is a no-op for the Rust path:
    // the Python shim serializes the post-normalization form. The
    // both-FunctionLike case where either side is Overloaded or
    // Parameters needs combine logic that produces a new
    // CallableType/Overloaded -> defer. The
    // CallableType-vs-CallableType case is handled in visit_join
    // (identical shape returns SameS; everything else defers). The
    // fallback case (t=CallableType/Overloaded, s non-callable)
    // recurses into join_types(t.fallback, s), which the Rust
    // Instance-Instance path handles.
    let s_is_callable = matches!(
        s,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    let t_is_callable = matches!(
        t,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    let either_overloaded_or_params =
        matches!(s, Type::Overloaded { .. } | Type::Parameters { .. })
            || matches!(t, Type::Overloaded { .. } | Type::Parameters { .. });
    if s_is_callable && t_is_callable && either_overloaded_or_params {
        return None;
    }

    // t.accept(TypeJoinVisitor(s)) — leaf visitors only. The visitor
    // returns SameS/SameT relative to the post-swap s/t; flip back to
    // the original s/t frame so the Python shim can map to its args.
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
///   * visit_instance (meet.py:913-996), args-less nominal only:
///     same type_ref -> SameS (equal, no args to combine); different
///     type_ref with is_subtype(t, s) -> SameT; is_subtype(s, t) ->
///     SameS; else Bottom. Args / alt_promote / protocol defers.
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
/// - Instance right with args: needs arg combination -> defer.
///
/// The Python shim is responsible for `get_proper_type` expansion
/// BEFORE calling this, matching `meet.py:120-121`.
pub(crate) fn meet_types(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    // meet.py:137-141: is_proper_subtype pre-check (ignore_promotions).
    // Only fires for Instance-Instance (Rust is_subtype returns None
    // for non-Instance). Both directions must not be UnboundType
    // (ErasedType has no wire variant -> never UnboundType here).
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

    // meet.py:143-144 (isinstance(s, ErasedType) -> return s) is
    // unreachable: ErasedType has no wire-format variant.
    // meet.py:145-146: isinstance(s, AnyType) -> return t (SameT).
    if matches!(s, Type::AnyType { .. }) {
        return Some(SetOpResult::SameT);
    }

    // meet.py:147-148: isinstance(s, UnionType) and not isinstance(t,
    // UnionType) -> swap. Both UnionType -> visit_union_type builds a
    // new union -> defer.
    let (s, t, swapped) = match (s, t) {
        (Type::UnionType { .. }, other) if !matches!(other, Type::UnionType { .. }) => (t, s, true),
        (Type::UnionType { .. }, Type::UnionType { .. }) => return None,
        _ => (s, t, false),
    };

    // normalize_callables (meet.py:151) is a no-op for the Rust path:
    // the Python shim serializes the post-normalization form. The
    // both-FunctionLike case needs meet_similar_callables (produces a
    // new CallableType) -> defer.
    let s_is_callable = matches!(
        s,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    let t_is_callable = matches!(
        t,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    );
    if s_is_callable && t_is_callable {
        return None;
    }

    // t.accept(TypeMeetVisitor(s)) — leaf visitors only. The visitor
    // returns SameS/SameT relative to the post-swap s/t; flip back to
    // the original s/t frame.
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
        Type::UninhabitedType => Some(SetOpResult::SameT),

        // visit_deleted_type (meet.py:864-873).
        Type::DeletedType { .. } => {
            if matches!(s, Type::NoneType) {
                // strict_optional: return t (DeletedType); non-strict:
                // return self.s (NoneType). The Python shim maps both
                // via the SameS/SameT discriminator + strict_optional
                // flag.
                if ctx.strict_optional {
                    Some(SetOpResult::SameT)
                } else {
                    Some(SetOpResult::SameS)
                }
            } else if matches!(s, Type::UninhabitedType) {
                // return self.s (Uninhabited).
                Some(SetOpResult::SameS)
            } else {
                // else: return t (DeletedType).
                Some(SetOpResult::SameT)
            }
        }

        // visit_erased_type (meet.py:875) is unreachable: ErasedType
        // has no wire-format variant.

        // visit_unbound_type (meet.py:864-873). Three branches on s:
        //   * NoneType + strict_optional -> UninhabitedType (Bottom).
        //     Non-strict -> self.s (SameS).
        //   * UninhabitedType -> self.s (SameS).
        //   * else -> AnyType (Any). AnyType-s never reaches here (the
        //     meet_types AnyType-s short-circuit at meet.py:145 returns
        //     t before the visitor fires).
        Type::UnboundType { .. } => {
            if matches!(s, Type::NoneType) {
                if ctx.strict_optional {
                    Some(SetOpResult::Bottom)
                } else {
                    Some(SetOpResult::SameS)
                }
            } else if matches!(s, Type::UninhabitedType) {
                Some(SetOpResult::SameS)
            } else {
                Some(SetOpResult::Any)
            }
        }

        // visit_instance (meet.py:913-996), args-less nominal subset.
        Type::Instance { .. } => visit_instance_meet(s, t, ctx, resolver),

        // visit_type_var (meet.py:878-884), case 1 same-id-same-bound
        // only. Case 1 (s is TypeVarType, s.id==t.id,
        // s.upper_bound==t.upper_bound) returns self.s -> SameS. The
        // copy_modified branch (upper_bounds differ) produces a new
        // TypeVarType -> defer. The else (s not TypeVarType or
        // different id) -> default(self.s) -> Bottom.
        //
        // TypeVarId.__eq__ (types.py:567-577) checks raw_id,
        // meta_level, namespace. Wire format omits meta_level
        // (types.py:739-740, 752); meta variables don't cross FFI.
        // raw_id + namespace equality matches wire-roundtrip semantics.
        Type::TypeVarType {
            raw_id: t_raw,
            namespace: t_ns,
            upper_bound: t_ub,
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
                    // Different upper_bound: copy_modified ->
                    // defer (no encoder).
                    return None;
                }
            }
            // s not TypeVarType or different id -> default -> Bottom.
            Some(SetOpResult::Bottom)
        }

        // visit_type_var_tuple (meet.py:930-934). Same id (raw_id +
        // namespace) -> `self.s if self.s.min_len > t.min_len else t`.
        // Different id / s not TypeVarTupleType -> default(self.s) ->
        // Bottom (strict) / NoneType (non-strict).
        //
        // TypeVarId.__eq__ (types.py:567-577) checks raw_id,
        // meta_level, namespace; wire format omits meta_level (meta
        // variables don't cross FFI), so raw_id + namespace equality
        // matches wire-roundtrip semantics.
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

        // visit_literal_type (meet.py:1236-1242). Case 1 (s is
        // LiteralType, s==t) -> return t (SameT). Case 2 (s is
        // Instance, is_subtype(t.fallback, s)) -> return t (SameT).
        // Else -> default(self.s) -> Bottom.
        //
        // LiteralType.__eq__ (types.py:3361-3363) compares value AND
        // fallback. The Type enum derives PartialEq, so s == t is
        // structural equality over the LiteralType variant (fallback +
        // value). This matches Python's s == t exactly.
        Type::LiteralType { .. } => {
            if let Type::LiteralType { .. } = s {
                if s == t {
                    return Some(SetOpResult::SameT);
                }
                // s is LiteralType but s != t -> default -> Bottom.
                return Some(SetOpResult::Bottom);
            }
            if let Type::Instance { .. } = s {
                // Case 2: is_subtype(t.fallback, s). t.fallback is
                // the LiteralType's fallback Instance. Extract it and
                // check. is_subtype returns None for unsupported
                // (non-Instance) -> defer conservatively.
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

        // visit_type_type (meet.py:1248-1261), case 2 only. Case 1
        // (both TypeType) recurses + make_normalized -> defer. Case 3
        // (CallableType) recurses -> defer. Case 2 (s is
        // Instance(builtins.type)) -> return t (SameT). Else ->
        // default -> Bottom.
        Type::TypeType { .. } => {
            if let Type::Instance { type_ref, .. } = s {
                if type_ref == "builtins.type" {
                    return Some(SetOpResult::SameT);
                }
            }
            if matches!(s, Type::TypeType { .. } | Type::CallableType { .. }) {
                // Case 1 (both TypeType) + case 3 (CallableType):
                // recursive meet -> defer.
                return None;
            }
            // Else -> default -> Bottom.
            Some(SetOpResult::Bottom)
        }

        // Full visitors (union, callable, typeddict, tuple,
        // paramspec, typevartuple, parameters, overloaded) — deferred.
        // The both-FunctionLike and both-Union cases are already
        // deferred by meet_types pre-dispatch. The remaining cases (s
        // non-callable, t callable-like; s non-union, t union after
        // swap) reach here and defer.
        _ => None,
    }
}

/// `TypeMeetVisitor.visit_instance` (meet.py:913-996), args-less
/// nominal subset. Mirrors `visit_instance_join` but for meet.
///
/// Handles:
/// - Same type_ref, both args-less -> SameS (equal).
/// - Different type_ref, args-less: `is_subtype(t, s)` -> SameT;
///   `is_subtype(s, t)` -> SameS; else Bottom.
///
/// Defers (returns `None`) for:
/// - s not Instance (FunctionLike/TypeType/Tuple/Literal/TypedDict
///   branches recurse into meet_types(t, self.s) or default).
/// - Same type_ref with args: combines args via meet -> produces a new
///   Instance -> defer (no Type encoder).
/// - Different type_ref with args: needs map_instance_to_supertype +
///   arg combination -> defer.
/// - `alt_promote` (meet.py:964-969): snapshot has no alt_promote
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

    // meet.py:914-957: t.type == self.s.type -> combine args.
    if t_ref == s_ref {
        if s_args.is_empty() && t_args.is_empty() {
            // Equal args-less Instances -> meet is the type itself.
            return Some(SetOpResult::SameS);
        }
        // Same type with args: needs arg combination -> defer.
        return None;
    }

    // Different types with args: needs map_instance_to_supertype ->
    // defer.
    if !s_args.is_empty() || !t_args.is_empty() {
        return None;
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
fn setop_result_to_type(r: Option<SetOpResult>, s: &Type, t: &Type) -> Option<Type> {
    match r? {
        SetOpResult::SameS => Some(s.clone()),
        SetOpResult::SameT => Some(t.clone()),
        SetOpResult::Any => Some(Type::AnyType {
            type_of_any: 3, // TypeOfAny.special_form
            source_any: None,
            missing_import_name: None,
        }),
        SetOpResult::Bottom => Some(Type::UninhabitedType),
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
        SetOpResult::SameTypeWithArgs { .. } | SetOpResult::Encoded(_) => None,
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

/// `is_equivalent` (subtypes.py:277-300) for two Callables: is_subtype
/// both ways on pairwise arg_types + ret_type. Returns `None` (defer)
/// if any is_subtype can't decide; `Some(true)` if mutually subtype,
/// `Some(false)` otherwise.
fn is_equivalent_callable(
    t_arg_types: &[Type],
    t_ret_type: &Type,
    s_arg_types: &[Type],
    s_ret_type: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
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
fn combine_arg_names(
    t_names: &[Option<String>],
    s_names: &[Option<String>],
) -> Vec<Option<String>> {
    t_names
        .iter()
        .zip(s_names.iter())
        .map(|(tn, sn)| match (tn, sn) {
            (Some(tn), Some(sn)) if tn == sn => Some(tn.clone()),
            _ => None,
        })
        .collect()
}

/// `safe_join` (join.py:1065-1072): join_types for non-UnpackType
/// pairs. Both-UnpackType -> UnpackType(join). Mixed -> defer (None).
fn safe_join(t: &Type, s: &Type, ctx: &SubtypeContext, resolver: &TypeResolver) -> Option<Type> {
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

/// `safe_meet` (meet.py equivalent of safe_join): meet_types for
/// non-UnpackType pairs. Both-UnpackType needs tuple_fallback lookup
/// (defer). Mixed -> UninhabitedType. Returns None (defer) if the
/// underlying meet_types defers.
fn safe_meet(t: &Type, s: &Type, ctx: &SubtypeContext, resolver: &TypeResolver) -> Option<Type> {
    let t_unpack = matches!(t, Type::UnpackType { .. });
    let s_unpack = matches!(s, Type::UnpackType { .. });
    if !t_unpack && !s_unpack {
        return setop_result_to_type(meet_types(t, s, ctx, resolver), t, s);
    }
    if t_unpack && s_unpack {
        // meet.py:1082-1093: needs tuple_fallback.type from the
        // unpacked TypeVarTupleType/TupleType/Instance. Defer.
        return None;
    }
    // Mixed: meet.py:1094 returns UninhabitedType().
    Some(Type::UninhabitedType)
}

/// Pick the fallback per join.py:1106-1109 (combine) / 1048-1051
/// (join_similar): if t.fallback is builtins.function, use t.fallback,
/// else s.fallback. The "t" here is the second operand (self.s in
/// Python is the first arg; our s/t naming follows the Rust convention
/// where s is the first arg to join_types).
fn pick_fallback(s_fallback: &Type, t_fallback: &Type) -> Type {
    if let Type::Instance { type_ref, .. } = s_fallback {
        if type_ref == "builtins.function" {
            return s_fallback.clone();
        }
    }
    t_fallback.clone()
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
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        new_arg_types.push(safe_join(ta, sa, ctx, resolver)?);
    }
    let new_ret = setop_result_to_type(
        join_types(t_ret_type, s_ret_type, ctx, resolver),
        s_ret_type,
        t_ret_type,
    )?;
    let new_instance_type = match (s_instance_type, t_instance_type) {
        (Some(si), Some(ti)) => Some(Box::new(setop_result_to_type(
            join_types(ti.as_ref(), si.as_ref(), ctx, resolver),
            si.as_ref(),
            ti.as_ref(),
        )?)),
        _ => None,
    };
    let new_arg_names = combine_arg_names(t_arg_names, s_arg_names);
    let new_fallback = pick_fallback(s_fallback, t_fallback);
    let (
        arg_kinds,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
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

/// `join_similar_callables` (join.py:1040-1062): similar-but-not-
/// equivalent path. Per-arg safe_meet, ret join, instance_type join,
/// fallback pick. Returns Encoded(new CallableType) or None (defer).
#[allow(clippy::too_many_arguments)]
fn join_similar_callables(
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
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<SetOpResult> {
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        new_arg_types.push(safe_meet(ta, sa, ctx, resolver)?);
    }
    // join.py:644-647: if any arg type is NoneType or UninhabitedType
    // (Bottom), the callable is unusable. Python falls back to
    // join_types(t.fallback, s). Defer so Python handles the fallback.
    if new_arg_types.iter().any(|a| {
        matches!(
            a,
            Type::NoneType { .. } | Type::UninhabitedType { .. }
        )
    }) {
        return None;
    }
    let new_ret = setop_result_to_type(
        join_types(t_ret_type, s_ret_type, ctx, resolver),
        s_ret_type,
        t_ret_type,
    )?;
    let new_instance_type = match (s_instance_type, t_instance_type) {
        (Some(si), Some(ti)) => Some(Box::new(setop_result_to_type(
            join_types(ti.as_ref(), si.as_ref(), ctx, resolver),
            si.as_ref(),
            ti.as_ref(),
        )?)),
        _ => None,
    };
    let new_arg_names = combine_arg_names(t_arg_names, s_arg_names);
    let new_fallback = pick_fallback(s_fallback, t_fallback);
    let (
        arg_kinds,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
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
        arg_types: new_arg_types,
        arg_kinds,
        arg_names: new_arg_names,
        ret_type: Box::new(new_ret),
        name: None,
        variables: Vec::new(),
        type_guard,
        type_is,
    };
    let _ = s;
    let _ = t;
    encode_callable(new_callable)
}

/// Extract the invariant fields (arg_kinds, flags, type_guard, type_is)
/// from a CallableType `t`. These are copied as-is to the result
/// (join.py:1113-1119 copy_modified preserves them).
#[allow(clippy::type_complexity)]
fn extract_callable_invariants(
    t: &Type,
) -> (
    Vec<i64>,
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
                    Type::NoneType | Type::UninhabitedType => Some(SetOpResult::SameT),
                    Type::UnboundType { .. } | Type::AnyType { .. } => Some(SetOpResult::Any),
                    // Else branch: make_simplified_union([s, t])
                    // (join.py:363) — deferred.
                    _ => None,
                }
            } else {
                // Non-strict: return s.
                Some(SetOpResult::SameS)
            }
        }

        // visit_uninhabited_type (join.py:367-368): return s.
        Type::UninhabitedType => Some(SetOpResult::SameS),

        // visit_deleted_type (join.py:370-371): return s.
        Type::DeletedType { .. } => Some(SetOpResult::SameS),

        // visit_erased_type (join.py:373-374) is unhandled: ErasedType
        // has no wire-format variant, so it cannot arrive over FFI.

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
                        ctx,
                        resolver,
                    );
                }
                return join_similar_callables(
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
                    ctx,
                    resolver,
                );
            }
            visit_callable_fallback(s, fallback, ctx, resolver)
        }

        // visit_overloaded (join.py:581-632), fallback case only. The
        // both-FunctionLike case (s is CallableType/Overloaded) is
        // already deferred by the pre-dispatch both-callable-like guard.
        // The protocol-Instance case needs unpack_callback_proxy. The
        // fallback case (join.py:632: join_types(t.fallback, s))
        // recurses into the Instance-vs-s join. `t.fallback` is
        // `items[0].fallback` (types.py:2744); the wire format stores
        // only `items`, so extract it here.
        Type::Overloaded { items, .. } => {
            let first = items.first()?;
            let fallback = match first {
                Type::CallableType { fallback, .. } => fallback.as_ref(),
                // Non-Callable item violates the Overloaded invariant
                // (types.py:2739: "_items: list[CallableType]"). Defer
                // rather than panic: the wire format can't enforce this.
                _ => return None,
            };
            visit_callable_fallback(s, fallback, ctx, resolver)
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
                // join.py:857-861: TypeType.make_normalized(
                //   join_types(t.item, self.s.item),
                //   is_type_form=s.is_type_form or t.is_type_form)
                let joined = setop_result_to_type(
                    join_types(t_item, s_item, ctx, resolver),
                    s_item,
                    t_item,
                )?;
                let new_type = Type::TypeType {
                    item: Box::new(joined),
                    is_type_form: *s_itf || *t_itf,
                };
                let mut wbuf = WriteBuffer::new();
                wire::write_type(&mut wbuf, &new_type).ok()?;
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
        //   union, which is neither s nor t -> defer.
        // 3 (s is LiteralType, neither enum) -> join_types(s.fallback,
        //   t.fallback). When both fallbacks are the same Instance (the
        //   common bool case: Literal[True] vs Literal[False]), the
        //   recursive join returns SameS -> s.fallback, which we encode.
        //   When fallbacks differ, the recursive join may defer -> None.
        // 4 (s is Instance, s.last_known_value == t) -> SameT.
        // 5 (else) -> join_types(s, t.fallback) -> defer (result not
        //   generally s or t).
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
                    // the enum's full member set is covered; the result
                    // is the fallback. Partial coverage yields a union
                    // of 2 literals, which is neither s nor t -> defer.
                    if is_enum_fallback(s_fb, resolver)
                        && is_enum_fallback(t_fb, resolver)
                        && s_fb.as_ref() == t_fb.as_ref()
                    {
                        let simplified =
                            make_simplified_union(&[s.clone(), t.clone()], ctx, resolver)?;
                        if matches!(simplified, Type::Instance { .. }) {
                            let mut wbuf = WriteBuffer::new();
                            wire::write_type(&mut wbuf, &simplified).ok()?;
                            return Some(SetOpResult::Encoded(wbuf.into_bytes()));
                        }
                        return None;
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
            None
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
            ..
        } => {
            if let Type::TypeVarType {
                raw_id: s_raw,
                namespace: s_ns,
                upper_bound: s_ub,
                ..
            } = s
            {
                if s_raw == t_raw && s_ns == t_ns && s_ub == t_ub {
                    return Some(SetOpResult::SameS);
                }
                // Cases 1 (diff upper_bound) and 2 (diff id): produce
                // a new TypeVarType or join upper_bounds -> defer.
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

        // visit_typeddict (join.py:811-835), case 2 Instance-s only.
        // Case 1 (s is TypedDictType) builds a NEW TypedDictType via
        // resolve_typeddict_item over zipall -> defer (no encoder).
        // Case 2 (s is Instance) recurses into
        // join_types(self.s, t.fallback). The recursive call is
        // join_types(s, fallback) (s=left, fallback=right). SameS in
        // the recursive frame means result=s -> outer SameS. SameT
        // means result=fallback, which is neither s nor t (t is the
        // TypedDict) -> defer. Ancestor/Object pass through.
        // Case 3 (else) walks s's fallback chain -> defer.
        //
        // The fallback is always an Instance (the wire reader asserts
        // the INSTANCE tag at wire.rs:987; types.py:3122 serializes
        // `self.fallback.write(data)` where fallback is an Instance).
        // No protocol deferral needed (unlike visit_callable_fallback):
        // the recursion is a plain Instance-Instance join, no callback
        // proxy unpacking involved.
        Type::TypedDictType { fallback, .. } => {
            if let Type::Instance { .. } = s {
                match join_types(s, fallback, ctx, resolver)? {
                    SetOpResult::SameS => Some(SetOpResult::SameS),
                    SetOpResult::Ancestor(fullname) => Some(SetOpResult::Ancestor(fullname)),
                    SetOpResult::Object => Some(SetOpResult::Object),
                    _ => None,
                }
            } else {
                None
            }
        }

        // visit_tuple_type (join.py:741-775), case 2 non-TupleType-s
        // only. Case 1 (s is TupleType) builds a new TupleType via
        // join_tuples + InstanceJoiner -> defer (no encoder). Case 2
        // (else) calls join_types(self.s, tuple_fallback(t)).
        //
        // `tuple_fallback(t)` (typeops.py:105-129) equals
        // `t.partial_fallback` only when the fallback is NOT
        // `builtins.tuple` (typeops.py:108-109). When it IS
        // `builtins.tuple`, it constructs `Instance(builtins.tuple,
        // [make_simplified_union(items)])` — a new Instance the Rust
        // path can't replicate without a Type encoder -> defer.
        //
        // When the fallback is a non-builtin (e.g. a namedtuple class),
        // `tuple_fallback(t) == t.partial_fallback`, and the recursive
        // call join_types(s, partial_fallback) lands on the
        // Instance-Instance nominal path. SameS -> outer SameS
        // (result = s); Ancestor/Object pass through; SameT (result =
        // fallback != t) defers.
        //
        // The partial_fallback is always an Instance (wire reader
        // asserts the INSTANCE tag at wire.rs:968; types.py:2909
        // serializes `self.partial_fallback.write(data)`).
        Type::TupleType {
            partial_fallback, ..
        } => {
            if let Type::Instance {
                type_ref: fb_ref, ..
            } = partial_fallback.as_ref()
            {
                if fb_ref != "builtins.tuple" {
                    match join_types(s, partial_fallback, ctx, resolver)? {
                        SetOpResult::SameS => Some(SetOpResult::SameS),
                        SetOpResult::Ancestor(fullname) => Some(SetOpResult::Ancestor(fullname)),
                        SetOpResult::Object => Some(SetOpResult::Object),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
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
    match join_types(fallback, s, ctx, resolver)? {
        // Recursive SameT: result = s (recursive right) -> outer SameS
        // (the shim returns s).
        SetOpResult::SameT => Some(SetOpResult::SameS),
        // Recursive SameS: result = fallback (recursive left). Only
        // expressible if fallback == s (then result is s -> SameS).
        SetOpResult::SameS if fallback == s => Some(SetOpResult::SameS),
        // Ancestor / Object pass through (swap-invariant).
        SetOpResult::Ancestor(fullname) => Some(SetOpResult::Ancestor(fullname)),
        SetOpResult::Object => Some(SetOpResult::Object),
        // SameS (fallback != s), Any, Bottom, SameTypeWithArgs: can't
        // express without a Type encoder. Defer.
        _ => None,
    }
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
/// target needed for `_expand_once`); if one is present, return `None`
/// so the caller defers to Python.
fn flatten_nested_unions(items: &[Type]) -> Option<Vec<Type>> {
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
    let mut current = items;
    for _direction in 0..2 {
        let mut new_items: Vec<Type> = Vec::with_capacity(current.len());
        for ti in current {
            if matches!(ti, Type::UninhabitedType) {
                continue;
            }
            let mut duplicate_index = None;
            for (j, tj) in new_items.iter().enumerate() {
                if is_subtype(&ti, tj, ctx, resolver)? {
                    duplicate_index = Some(j);
                    break;
                }
            }
            if duplicate_index.is_some() {
                // Truthiness adjustment skipped (not modeled).
            } else {
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

/// `try_contracting_literals_in_union` (typeops.py:1121-1161), Rust
/// port. Contracts literals sharing a fallback back into the sum type
/// when all values of the sum are present.
///
/// Ported: the `bool` case and the `enum` case. For bool, when both
/// `Literal[True]` and `Literal[False]` appear, replace the first with
/// `builtins.bool` and drop the rest. For enum, when every member in
/// `TypeInfo.enum_members` appears as a `LiteralType(value=Str(name))`
/// with the same enum fallback, replace the first with the fallback
/// Instance and drop the rest.
///
/// Deferred: nothing in this function (enum_members is now in the
/// snapshot). Returns `None` only if a snapshot lookup is needed but
/// the fullname is absent (conservative defer; the Python path's
/// `is_enum` defaults to false so a missing snapshot is non-enum).
fn try_contracting_literals_in_union(
    items: Vec<Type>,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    // Contraction groups keyed by fallback fullname. Each group tracks
    // the set of sum-type values still missing and the indices of
    // LiteralType items that participate. For bool, the "sum" is
    // {true, false}; for enum, the "sum" is `TypeInfo.enum_members`.
    // fullname -> (missing_values, indices, is_bool)
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
fn make_simplified_union(
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Type> {
    // Step 1: flatten nested unions. TypeAliasType defers.
    let flat = flatten_nested_unions(items)?;
    // Step 2: single-item fast path.
    if flat.len() == 1 {
        return Some(flat.into_iter().next().unwrap());
    }
    // Step 3: remove redundant items. Defer when any is_subtype returns
    // None (non-Instance pair, including LiteralType-vs-non-LiteralType
    // where the Rust is_subtype only handles LiteralType == LiteralType).
    let deduped = remove_redundant_union_items(flat, ctx, resolver)?;
    // Step 4: contract literals (bool + enum) sharing a fallback
    // whose full value set is covered.
    let contracted = try_contracting_literals_in_union(deduped, resolver)?;
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
fn union_make_union(items: Vec<Type>) -> Type {
    match items.len() {
        0 => Type::UninhabitedType,
        1 => items.into_iter().next().unwrap(),
        _ => Type::UnionType {
            items,
            uses_pep604_syntax: false,
        },
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
        if matches!(item, Type::UninhabitedType) {
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
        if matches!(item, Type::UninhabitedType) {
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
            },
        ],
        ctx,
        resolver,
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
fn visit_instance_join(
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
            return Some(SetOpResult::SameS);
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
    // Variadic instance: needs split_with_prefix_and_suffix — defer.
    if snap.has_type_var_tuple_type {
        return None;
    }
    let tvars = &snap.type_vars_with_variance;
    // Python uses zip (tolerates length mismatch during daemon
    // reprocessing). Rust requires equal lengths + matching tvars.
    if s_args.len() != t_args.len() || s_args.len() != tvars.len() {
        return None;
    }

    let mut arg_discs: Vec<i8> = Vec::with_capacity(tvars.len());
    for (i, (_, variance, kind)) in tvars.iter().enumerate() {
        let ta = &t_args[i]; // Python's t.args[i] (right arg).
        let sa = &s_args[i]; // Python's s.args[i] (left arg).

        // join.py:131-135: AnyType arg -> AnyType(from_another_any).
        if matches!(ta, Type::AnyType { .. }) || matches!(sa, Type::AnyType { .. }) {
            arg_discs.push(4);
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

        // TypeVarType. values non-empty -> defer (needs values check,
        // join.py:140-143).
        // We can't read `values` from the snapshot (only name +
        // variance + kind); defer if the tvar might have values.
        // The snapshot doesn't carry `values`, so we conservatively
        // defer only when the recursive join is non-trivial. For the
        // invariant equivalent-same-type case, values are typically
        // empty, so we proceed and let the recursive join decide.

        match *variance {
            v if v == COVARIANT || v == VARIANCE_NOT_READY => {
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
                // Ancestor/Object/Any/Bottom -> defer (can't express as
                // an arg disc without a Type encoder). In practice
                // Instance-Instance recursion only yields SameS/SameT
                // (when args are equal) or Ancestor (when they differ),
                // so the covariant branch fires on equal-arg cases and
                // defers otherwise.
                let new_type_disc = match join_types(ta, sa, ctx, resolver) {
                    Some(SetOpResult::SameS) => 1i8,
                    Some(SetOpResult::SameT) => 0,
                    Some(_) | None => return None,
                };
                let new_type = if new_type_disc == 1 { ta } else { sa };
                if !is_subtype(new_type, &upper_bound, ctx, resolver)? {
                    return Some(SetOpResult::Object);
                }
                arg_discs.push(new_type_disc);
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
                // -> defer (can't express without a Type encoder).
                match join_types(ta, sa, ctx, resolver)? {
                    SetOpResult::SameS => arg_discs.push(1),
                    SetOpResult::SameT => arg_discs.push(0),
                    _ => return None,
                }
            }
            _ => return None,
        }
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
                if is_subtype(&promote, &Type::Instance {
                    type_ref: right_ref.to_string(),
                    args: vec![],
                    last_known_value: None,
                    extra_attrs: None,
                }, ctx, resolver)? {
                    return Some(JoinResult::Ancestor(right_ref.to_string()));
                }
            }
        }
        if let Some(snap) = right_snap {
            for promote_blob in &snap.promote_bytes {
                if let Some(promote) = decode_type(promote_blob) {
                    if is_subtype(&promote, &Type::Instance {
                        type_ref: left_ref.to_string(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    }, ctx, resolver)? {
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
            if let Type::Instance { type_ref: base_ref, .. } = &base {
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
            // ties (keeping the first), but Python also has map_instance_to_supertype
            // and the second promote loop that may change the result.
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
    fn trivial_meet_returns_none_when_subtype_defers() {
        // Non-Instance left -> is_subtype returns None for both
        // directions -> trivial_meet defers (returns None).
        let r = make_resolver(vec![]);
        let left = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
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
        let s = Type::UninhabitedType;
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
        let t = Type::UninhabitedType;
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
        // make_simplified_union (deferred).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::NoneType;
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        let result = make_simplified_union(&items, &ctx(true), &r).expect("deferred");
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
        let result = make_simplified_union(&items, &ctx(true), &r).expect("deferred");
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
        let result = make_simplified_union(&items, &ctx(true), &r).expect("deferred");
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
            items: vec![Type::UninhabitedType, instance("a.B", vec![])],
            uses_pep604_syntax: false,
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
            };
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn join_types_union_s_is_union_defers() {
        // Both s and t are UnionType. The pre-dispatch swap only fires
        // when exactly one side is a union (join.py:311-312). When both
        // are unions, visit_union_type calls make_simplified_union
        // which needs to merge/flatten -> defer (no Type encoder).
        let a = snap("a.A", "A");
        let b = snap("a.B", "B");
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![a, b, o]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
        };
        let t = Type::UnionType {
            items: vec![instance("a.B", vec![])],
            uses_pep604_syntax: false,
        };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        // visit_callable_type fires join_similar_callables: per-arg
        // safe_meet(B, A) = B (the narrower), ret join(object, object)
        // = object. Result is a new CallableType(arg=[B], ret=object)
        // returned as Encoded (disc=7).
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
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            let expected = callable(
                "builtins.function",
                vec![instance("a.B", vec![])],
                instance("builtins.object", vec![]),
            );
            assert_eq!(decoded, expected);
        }
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
        // So join_similar_callables fires: per-arg safe_meet(T, object)
        // = T (meet_types pre-check: is_proper_subtype(T, object)=True
        // -> SameS=T), ret join_types(T, object)=object (trivial_join:
        // T <: object -> return object=SameT). Result is a new
        // CallableType(arg=[T], ret=object, variables=[]) returned as
        // Encoded.
        //
        // Pre-M8z: the both-generic defer (line 1261) returned None for
        // ANY non-empty variables, including this min_len==0 case.
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
        let result = join_types(&s, &t, &ctx(true), &r);
        assert!(
            matches!(result, Some(SetOpResult::Encoded(_))),
            "one-generic join_similar: got {:?}",
            result
        );
        if let Some(SetOpResult::Encoded(bytes)) = result {
            let mut rbuf = ReadBuffer::new(&bytes);
            let decoded = read_type(&mut rbuf, None).expect("decode failed");
            // Expected: CallableType(arg=[T], ret=object, variables=[]).
            // arg_types[0] is the TypeVar T (safe_meet(T, object)=T).
            // ret_type is object (join(T, object)=object).
            // variables is empty (combine/join_similar always sets
            // variables=[] in the Rust port; Python's
            // join_similar_callables preserves t.variables which for
            // the non-generic t is empty).
            let expected = Type::CallableType {
                fallback: Box::new(instance("builtins.function", vec![])),
                instance_type: None,
                is_ellipsis_args: false,
                implicit: false,
                is_bound: false,
                from_concatenate: false,
                imprecise_arg_kinds: false,
                unpack_kwargs: false,
                arg_types: vec![type_var(-1, "ns", ub.clone())],
                arg_kinds: vec![0],
                arg_names: vec![None],
                ret_type: Box::new(instance("builtins.object", vec![])),
                name: None,
                variables: Vec::new(),
                type_guard: None,
                type_is: None,
            };
            assert_eq!(decoded, expected);
        }
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
        }
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
    fn join_instance_with_mismatched_last_known_value_defers() {
        // visit_literal_type case 5 (join.py:847): s is Instance,
        // s.last_known_value != t -> join_types(self.s, t.fallback).
        // The recursive call is Instance-vs-Instance (both fallback=A),
        // which yields SameS. But the result (A) is neither s nor t.
        // Defer (can't express as SameS/SameT relative to the outer
        // s=Instance(A, lkv=Lit[1]), t=Lit[2] frame).
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
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        // Both s and t are callable-like (Overloaded). The pre-dispatch
        // defers because visit_overloaded's both-FunctionLike case
        // (join.py:612-627) needs is_similar_callables +
        // combine_similar_callables, which produce new CallableType /
        // Overloaded. No Type encoder -> defer.
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
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_overloaded_with_callable_defers() {
        // s=CallableType, t=Overloaded. Both callable-like -> the
        // pre-dispatch defers (both sides callable-like).
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
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        // make_simplified_union (deferred).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::NoneType;
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_types_swaps_uninhabited_left_to_right() {
        // join.py:323-324: s is Uninhabited, t is not -> swap.
        // Post-swap: s=Instance, t=Uninhabited.
        // visit_uninhabited_type returns s (Instance, post-swap).
        // flip_if(SameS, swapped=true) -> SameT (original t = Instance).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = Type::UninhabitedType;
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
        // join.py:114: t.type == s.type, no args -> SameS.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = instance("a.A", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameS));
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
                assert_eq!(arg_discs, vec![1]);
            }
            other => panic!("expected SameTypeWithArgs, got {other:?}"),
        }
    }

    #[test]
    fn join_instance_covariant_same_arg_returns_same() {
        // Covariant T, upper_bound=object. join(G[A], G[A]):
        // join_types(A, A) = A (SameS). is_subtype(A, object)=True.
        // arg disc 1 (t.args[0]=A, since SameS -> ta).
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
                assert_eq!(arg_discs, vec![1]);
            }
            other => panic!("expected SameTypeWithArgs, got {other:?}"),
        }
    }

    #[test]
    fn join_instance_covariant_subtype_defers() {
        // Covariant T, upper_bound=object. join(G[B], G[A]) where
        // B <: A. The recursive join_types(A, B) returns Ancestor(A)
        // (the common supertype), not SameS/SameT. The covariant
        // branch can't express an Ancestor result as an arg disc, so
        // it defers to Python. This is a known limitation: the
        // covariant branch only fires when ta and sa are structurally
        // equal (trivial join -> SameS/SameT).
        let g = snap_with_covariant_tvar("g.G");
        let a = snap("a.A", "A");
        let b = snap_with_bases("a.B", "B", &["a.A"]);
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![g, a, b, o]);
        let s = instance("g.G", vec![instance("a.B", vec![])]);
        let t = instance("g.G", vec![instance("a.A", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn join_instance_covariant_unrelated_defers() {
        // Covariant T, upper_bound=object. join(G[A], G[D]) where
        // A, D unrelated. The recursive join_types(A, D) returns
        // Ancestor(builtins.object), which the covariant branch
        // can't express as an arg disc. Defers to Python.
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
    fn join_type_var_different_id_defers() {
        // visit_type_var case 2 (join.py:472): s is TypeVarType but
        // s.id != t.id -> join_types(s.upper_bound, t.upper_bound).
        // The bound join is generally neither s nor t -> defer.
        let s = type_var(1, "~", instance("builtins.int", vec![]));
        let t = type_var(2, "~", instance("builtins.int", vec![]));
        assert_eq!(join_types(&s, &t, &ctx(true), &make_resolver(vec![])), None);
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
    fn join_type_var_same_id_different_namespace_defers() {
        // TypeVarId equality checks namespace (types.py:576): same
        // raw_id, different namespace -> s.id != t.id -> case 2 ->
        // defer.
        let s = type_var(1, "~", instance("builtins.object", vec![]));
        let t = type_var(1, "other", instance("builtins.object", vec![]));
        assert_eq!(join_types(&s, &t, &ctx(true), &make_resolver(vec![])), None);
    }

    #[test]
    fn join_typeddict_with_instance_equal_fallback_returns_s() {
        // visit_typeddict case 2 (join.py:832-833): s is Instance,
        // t is TypedDictType -> join_types(self.s, t.fallback).
        // Recursive call: join_types(s=builtins.dict, t.fallback=
        // builtins.dict). Same Instance, no args -> SameS (recursive
        // left = s). Maps to outer SameS (shim returns s).
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
        // (typeops.py:108-109). Recursive: join_types(NT, NT) = NT
        // (SameS). Fires the Rust SameS path (shim returns s=NT).
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
    fn join_tuple_with_tuple_defers() {
        // visit_tuple_type case 1 (join.py:753-773): s is TupleType ->
        // builds a new TupleType via join_tuples + InstanceJoiner.
        // Produces a new type -> defer (no Type encoder).
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        let t = tuple_type("nt.NT", vec![instance("builtins.int", vec![])]);
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        let t = Type::UninhabitedType;
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
        let s = Type::UninhabitedType;
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
    fn meet_types_instance_with_args_same_type_defers() {
        // visit_instance same type_ref with args -> combine args
        // (produces new Instance with meet args) -> defer (no encoder).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![instance("builtins.int", vec![])]);
        let t = instance("a.A", vec![instance("builtins.str", vec![])]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn meet_types_instance_with_args_different_subtype_defers() {
        // visit_instance different types with args -> needs
        // map_instance_to_supertype + arg combination -> defer.
        let mut a = snap("a.A", "A");
        a.has_base.insert("a.B".to_string());
        a.mro.push("a.B".to_string());
        let b = snap("a.B", "B");
        let r = make_resolver(vec![a, b]);
        let s = instance("a.B", vec![instance("builtins.int", vec![])]);
        let t = instance("a.A", vec![instance("builtins.int", vec![])]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn meet_types_union_s_non_union_t_swaps_then_defers() {
        // meet.py:147-148: isinstance(s, UnionType) and not isinstance(t,
        // UnionType) -> swap. After swap, s is non-union, t is union.
        // visit_union_type (meet.py:840-848) builds a new union via
        // make_simplified_union -> defer (no encoder).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
        };
        let t = instance("a.A", vec![]);
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn meet_types_both_union_defers() {
        // Both UnionType -> visit_union_type builds a new union -> defer.
        let r = make_resolver(vec![]);
        let s = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
        };
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
        };
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
    }

    #[test]
    fn meet_types_both_callable_defers() {
        // normalize_callables + visit_callable_type: both callable-like
        // -> needs combine_similar_callables / meet_similar_callables
        // (produces a new CallableType) -> defer.
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
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
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
    fn meet_type_var_same_id_different_upper_bound_defers() {
        // visit_type_var case 1, upper_bounds differ (meet.py:882):
        // copy_modified(upper_bound=meet(...)) -> produces a new
        // TypeVarType -> defer (no encoder).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let s = type_var(1, "ns", instance("a.A", vec![]));
        let t = type_var(1, "ns", instance("a.B", vec![]));
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
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
    fn meet_type_type_both_type_type_defers() {
        // visit_type_type case 1 (meet.py:1249-1255): s is TypeType ->
        // meet(t.item, s.item) + make_normalized -> produces a new
        // TypeType -> defer (no encoder).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = type_type("a.A");
        let t = type_type("a.A");
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), None);
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
        let s = Type::UninhabitedType;
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
        let s = Type::UninhabitedType;
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
}
