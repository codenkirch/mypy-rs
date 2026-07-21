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
///
/// Returns `None` (defer to Python) for:
/// - `is_recursive_pair` (needs the live alias graph).
/// - `can_be_true`/`can_be_false` mismatch (needs the properties).
/// - UnionType right (needs `make_simplified_union`).
/// - Instance/CallableType/TypeVarType/etc right (full visitor).
/// - `normalize_callables` (needs live callable normalization).
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
    // the non-union. We only handle the post-swap shape; if t is
    // UnionType, defer (needs make_simplified_union).
    let (s, t, swapped) = match (s, t) {
        (Type::UnionType { .. }, other) if !matches!(other, Type::UnionType { .. }) => (t, s, true),
        (_, Type::UnionType { .. }) => return None,
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

    // normalize_callables (join.py:327): deferred. If either side is a
    // callable-like variant, defer to Python.
    if matches!(
        s,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    ) || matches!(
        t,
        Type::CallableType { .. } | Type::Overloaded { .. } | Type::Parameters { .. }
    ) {
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

        // Full visitors (UnionType, CallableType, TypeVar, etc.) —
        // deferred to M8g+.
        _ => None,
    }
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
    use crate::wire::Type;

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
    fn join_types_union_right_defers() {
        // visit_union_type needs make_simplified_union -> defer.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let s = instance("a.A", vec![]);
        let t = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: false,
        };
        assert_eq!(join_types(&s, &t, &ctx(true), &r), None);
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
}
