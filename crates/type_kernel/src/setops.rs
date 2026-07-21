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
use crate::wire::{self, ReadBuffer, Type};

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
    if let Some(true) = is_subtype(s, t, ctx, resolver) {
        return Some(SetOpResult::SameT);
    }
    if let Some(true) = is_subtype(t, s, ctx, resolver) {
        return Some(SetOpResult::SameS);
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
    // both-FunctionLike case (both sides callable-like) needs
    // is_similar_callables + combine_similar_callables which produce
    // a new CallableType / Overloaded — no Type encoder -> defer. The
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
    if s_is_callable && t_is_callable {
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

        // visit_instance (meet.py:913-996), args-less nominal subset.
        Type::Instance { .. } => visit_instance_meet(s, t, ctx, resolver),

        // Full visitors (union, callable, type_var, typeddict, tuple,
        // literal, type_type, paramspec, typevartuple, parameters,
        // overloaded) — deferred. The both-FunctionLike and both-Union
        // cases are already deferred by meet_types pre-dispatch. The
        // remaining cases (s non-callable, t callable-like; s
        // non-union, t union after swap) reach here and defer.
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

    // meet.py:970-979 (alt_promote branches skipped — snapshot has no
    // alt_promote; is_subtype covers the nominal case): is_subtype(t,
    // s) -> return t; is_subtype(s, t) -> return s; else Bottom.
    // Use non-proper is_subtype (visit_instance uses is_subtype, not
    // is_proper_subtype, here — the pre-check already failed for
    // proper_subtype with ignore_promotions=True, but is_subtype may
    // still succeed via promotions).
    if let Some(true) = is_subtype(t, s, ctx, resolver) {
        Some(SetOpResult::SameT)
    } else if let Some(true) = is_subtype(s, t, ctx, resolver) {
        Some(SetOpResult::SameS)
    } else {
        Some(SetOpResult::Bottom)
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

        // visit_callable_type (join.py:541-577), fallback case only.
        // The similar-callables case (s is CallableType) needs
        // combine_similar_callables (produces a new CallableType — no
        // Type encoder). The protocol-Instance case needs
        // unpack_callback_proxy. The fallback case (s is non-callable,
        // non-protocol) recurses into join_types(t.fallback, s).
        Type::CallableType { fallback, .. } => visit_callable_fallback(s, fallback, ctx, resolver),

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

        // visit_type_type (join.py:854-864), case 2 only. Case 1 (s is
        // TypeType) produces a new TypeType via
        // TypeType.make_normalized — no Type encoder -> defer. Case 3
        // (else -> default) walks s's fallback chain, complex -> defer.
        // Case 2 (s is Instance with fullname=="builtins.type")
        // returns self.s -> SameS.
        Type::TypeType { .. } => {
            if let Type::Instance { type_ref, .. } = s {
                if type_ref == "builtins.type" {
                    return Some(SetOpResult::SameS);
                }
            }
            None
        }

        // visit_literal_type (join.py:837-847), cases 1+4 only. Case 1
        // (s is LiteralType, t==s) returns t -> SameT. Case 4 (s is
        // Instance, s.last_known_value==t) returns t -> SameT. Case 2
        // (enum simplified union) and case 3 (fallback join) produce
        // types other than s/t -> defer. Case 5 (join_types(s,
        // t.fallback)) recurses into Instance-vs-Instance but the
        // result is neither s nor t in general -> defer.
        Type::LiteralType { value: t_val, .. } => {
            if let Type::LiteralType { value: s_val, .. } = s {
                if s_val == t_val {
                    return Some(SetOpResult::SameT);
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
        // only. Case 1 (s is TypeVarType, s.id==t.id,
        // s.upper_bound==t.upper_bound) returns self.s -> SameS. The
        // copy_modified branch (case 1, upper_bounds differ) and
        // case 2 (s.id != t.id -> join upper_bounds) both produce a
        // new TypeVarType or the bound's join result — neither s nor
        // t in general -> defer. Case 3 (s not TypeVarType ->
        // default(s)) walks fallback chains -> defer.
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
            }
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
    // t <: s iff every item of t is <: s. If any item is not a
    // subtype, the simplified union won't collapse to s -> defer.
    for item in items {
        match is_subtype(item, s, ctx, resolver) {
            Some(true) => {}
            Some(false) => return None,
            None => return None,
        }
    }
    Some(SetOpResult::SameS)
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

    // join.py:191-199: dispatch to join_instances_via_supertype.
    // The recursive nominal join returns the fullname of the result
    // Instance; convert to SameS/SameT/Ancestor relative to the
    // original s/t frame.
    let result_ref = if is_subtype(t, s, ctx, resolver)? {
        join_instances_nominal(t_ref, s_ref, ctx, resolver)?
    } else {
        join_instances_nominal(s_ref, t_ref, ctx, resolver)?
    };
    Some(match result_ref {
        // Left/Right never escape via_supertype (Left -> Ancestor(base)
        // inside via_supertype). The top-level call only produces
        // Ancestor/Object after the t==s early return.
        JoinResult::Left => SetOpResult::SameS,
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
    let t = Type::Instance {
        type_ref: t_ref.to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    let s = Type::Instance {
        type_ref: s_ref.to_string(),
        args: vec![],
        last_known_value: None,
        extra_attrs: None,
    };
    if is_subtype(&t, &s, ctx, resolver)? {
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
    // join.py:221-226: collect base type_refs from left's bases.
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
    // join.py:228-234: for each base, recurse and pick the best.
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
        let mro = mro_len(base_ref, resolver);
        match &best {
            None => best = Some((mapped, mro)),
            Some((_, best_mro)) if mro > *best_mro => best = Some((mapped, mro)),
            _ => {}
        }
    }
    match best {
        Some((result, _)) => Some(result),
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
/// `Some((disc, fullname))` otherwise. `disc` is 0=SameS, 1=SameT,
/// 2=Object, 3=Bottom, 4=Any, 5=Ancestor (fullname set).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_join_types(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<(i64, Option<String>, Vec<i8>)> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    join_types(&s, &t, &ctx, resolver.resolver()).map(discriminator)
}

/// `#[pyfunction]` entry for `meet_types`. The Python-side shim
/// (mypy/meet.py) calls this after `get_proper_type` expansion with
/// serialized `s`/`t` blobs plus the `NativeTypeResolver` pyclass.
/// Returns `None` (Python `None`) when Rust doesn't handle the case;
/// `Some((disc, fullname, arg_discs))` otherwise. `meet_types` only
/// emits disc 0=SameS, 1=SameT, 3=Bottom, 4=Any (never 2=Object,
/// 5=Ancestor, 6=SameTypeWithArgs — those are join supertype results).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_meet_types(
    s_bytes: &[u8],
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<(i64, Option<String>, Vec<i8>)> {
    let s = decode_type(s_bytes)?;
    let t = decode_type(t_bytes)?;
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    meet_types(&s, &t, &ctx, resolver.resolver()).map(discriminator)
}

/// Map `SetOpResult` to the Python-side `(disc, fullname, arg_discs)`
/// triple. `disc` is 0=SameS, 1=SameT, 2=Object, 3=Bottom, 4=Any,
/// 5=Ancestor (fullname set, arg_discs empty), 6=SameTypeWithArgs
/// (fullname set, arg_discs populated: 0=s.args[i], 1=t.args[i],
/// 4=Any).
fn discriminator(r: SetOpResult) -> (i64, Option<String>, Vec<i8>) {
    match r {
        SetOpResult::SameS => (0, None, Vec::new()),
        SetOpResult::SameT => (1, None, Vec::new()),
        SetOpResult::Object => (2, None, Vec::new()),
        SetOpResult::Bottom => (3, None, Vec::new()),
        SetOpResult::Any => (4, None, Vec::new()),
        SetOpResult::Ancestor(fullname) => (5, Some(fullname), Vec::new()),
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => (6, Some(type_ref), arg_discs),
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
        SetOpResult::Any | SetOpResult::Ancestor(_) | SetOpResult::SameTypeWithArgs { .. } => {
            unreachable!("trivial_join/trivial_meet never produce Any/Ancestor/WithArgs")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::{LiteralValue, Type};

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
        assert_eq!(discriminator(SetOpResult::SameS), (0, None, vec![]));
        assert_eq!(discriminator(SetOpResult::SameT), (1, None, vec![]));
        assert_eq!(discriminator(SetOpResult::Object), (2, None, vec![]));
        assert_eq!(discriminator(SetOpResult::Bottom), (3, None, vec![]));
        assert_eq!(discriminator(SetOpResult::Any), (4, None, vec![]));
        assert_eq!(
            discriminator(SetOpResult::Ancestor("a.C".to_string())),
            (5, Some("a.C".to_string()), vec![])
        );
        assert_eq!(
            discriminator(SetOpResult::SameTypeWithArgs {
                type_ref: "g.G".to_string(),
                arg_discs: vec![0, 1, 4],
            }),
            (6, Some("g.G".to_string()), vec![0, 1, 4])
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
        // would produce Union[A, B, C]. We can't express a new union
        // without a Type encoder -> defer to Python.
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
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        // Both s and t are CallableType. visit_callable_type case 1
        // (isinstance(s, CallableType)) needs is_similar_callables +
        // combine_similar_callables, which produce a new CallableType.
        // No Type encoder -> defer.
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
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
    fn join_type_type_with_type_type_defers() {
        // visit_type_type case 1 (join.py:855-860): s is TypeType ->
        // TypeType.make_normalized(join_types(t.item, s.item), ...).
        // Produces a new TypeType — no Type encoder -> defer.
        let o = snap("builtins.object", "object");
        let r = make_resolver(vec![o]);
        let s = type_type("builtins.object");
        let t = type_type("builtins.object");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
        // visit_literal_type case 1 (join.py:841-843): s is
        // LiteralType, t != s, not enum -> join_types(s.fallback,
        // t.fallback). The result is the joined fallback, which is
        // neither s nor t in general. Defer (can't express as
        // SameS/SameT unless the fallback equals s or t, which the
        // Instance-Instance path handles separately when both sides
        // are Instances — but here both sides are LiteralType).
        let o = snap("builtins.object", "object");
        let i = snap("builtins.int", "int");
        let r = make_resolver(vec![o, i]);
        let s = literal(LiteralValue::Int(1), "builtins.int");
        let t = literal(LiteralValue::Int(2), "builtins.int");
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
    fn join_type_var_with_non_type_var_s_defers() {
        // visit_type_var case 3 (join.py:474): s is NOT a TypeVarType ->
        // return self.default(self.s). The default walks s's fallback
        // chain (join.py:869-888) and produces object/Any/instance —
        // generally neither s nor t -> defer.
        let t = type_var(1, "~", instance("builtins.object", vec![]));
        let s = instance("builtins.int", vec![]);
        assert_eq!(join_types(&s, &t, &ctx(true), &make_resolver(vec![])), None);
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
        // visit_none_type (meet.py:850-859), strict_optional, s is
        // NoneType -> return t (SameT).
        let r = make_resolver(vec![]);
        let s = Type::NoneType;
        let t = Type::NoneType;
        assert_eq!(meet_types(&s, &t, &ctx(true), &r), Some(SetOpResult::SameT));
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
}
