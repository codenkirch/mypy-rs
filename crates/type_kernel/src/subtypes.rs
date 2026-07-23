//! Stage 3c (M8b): nominal-instance `is_subtype` on the Rust `Type` enum.
//!
//! Ports the `visit_instance` `isinstance(right, Instance)` branch of
//! `mypy.subtypes.SubtypeVisitor` (subtypes.py:531-626) plus the shared
//! `_is_subtype` worker entry (subtypes.py:295-376) for the subset each
//! handles. Returns `None` (fall through to Python) for every variant the
//! nominal path does not cover: `TypeAliasType`, `TypeVarTupleType`
//! variadic, protocol right, `find_member` path, `TupleType` right,
//! `TypeType` right, `LiteralType` right with lkv, `FunctionLike` right,
//! `PartialType` left, and the generic `map_instance_to_supertype` path
//! (which needs `expand_type_by_instance`, deferred to M8c).
//!
//! The strangler-fig contract mirrors `erase::erase_type`
//! (erasetype.py:80-86): `None` means "Rust doesn't handle this, let
//! Python decide". No production code calls this until
//! `Options.native_type_kernel` is on AND `mypy/subtypes.py` dispatches
//! to it (the shim is added in this same milestone).

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type};

/// Variance constants mirroring `mypy.nodes` (nodes.py:3146).
pub(crate) const INVARIANT: i64 = 0;
pub(crate) const COVARIANT: i64 = 1;
pub(crate) const CONTRAVARIANT: i64 = 2;
pub(crate) const VARIANCE_NOT_READY: i64 = 3;

/// Mirrors `mypy.subtypes.SubtypeContext` (subtypes.py:90-122). Only the
/// flags the nominal-instance path reads are carried; the rest stay
/// Python-side (the shim passes them through unchanged when Rust
/// returns `None`).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct SubtypeContext {
    pub ignore_type_params: bool,
    pub ignore_declared_variance: bool,
    pub always_covariant: bool,
    pub ignore_promotions: bool,
    pub proper_subtype: bool,
    pub strict_optional: bool,
}

impl SubtypeContext {
    pub(crate) fn new(
        ignore_type_params: bool,
        ignore_declared_variance: bool,
        always_covariant: bool,
        ignore_promotions: bool,
        proper_subtype: bool,
        strict_optional: bool,
    ) -> Self {
        Self {
            ignore_type_params,
            ignore_declared_variance,
            always_covariant,
            ignore_promotions,
            proper_subtype,
            strict_optional,
        }
    }
}

/// Entry point mirroring `mypy.subtypes._is_subtype` for the nominal path.
///
/// Returns `Some(bool)` when Rust decided the check; `None` when the
/// variant is not handled (Python falls through). The Python shim is
/// responsible for `get_proper_type` expansion, the `AnyType`/`UnboundType`/
/// `ErasedType` right short-circuit (subtypes.py:306-313), the `UnionType`
/// right dispatch (subtypes.py:317-364), the `TypeVarType`-with-values
/// right (subtypes.py:366-374), and the `assuming` recursion guard
/// (subtypes.py:167-189) BEFORE calling this.
#[allow(dead_code)]
pub(crate) fn is_subtype(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // visit_uninhabited_type (subtypes.py:555-556): UninhabitedType is
    // a subtype of everything (bottom type). Fires before any right-side
    // dispatch because the Python visitor's `accept` lands on
    // `visit_uninhabited_type` regardless of `self.right`.
    if matches!(left, Type::UninhabitedType) {
        return Some(true);
    }
    // visit_deleted_type (subtypes.py:564): DeletedType is a subtype of
    // everything (same rationale as UninhabitedType).
    if matches!(left, Type::DeletedType { .. }) {
        return Some(true);
    }
    // visit_none_type (subtypes.py:539-554). The shim already passes
    // state.strict_optional as ctx.strict_optional.
    if matches!(left, Type::NoneType) {
        if !ctx.strict_optional {
            // subtypes.py:553-554: when strict_optional is off, None
            // is a subtype of everything.
            return Some(true);
        }
        return match right {
            // subtypes.py:539-541: right is NoneType or builtins.object
            // -> True.
            Type::NoneType => Some(true),
            Type::Instance { type_ref, .. } if type_ref == "builtins.object" => Some(true),
            Type::Instance { type_ref, .. } => {
                // subtypes.py:543-549: right is a protocol Instance.
                // When all protocol_members are __hash__/__str__ (or
                // members is empty), Python returns True; else False.
                // Non-protocol Instance -> False (subtypes.py:551).
                let snap = resolver.get(type_ref)?;
                if snap.is_protocol {
                    let ok = snap.protocol_members.is_empty()
                        || snap
                            .protocol_members
                            .iter()
                            .all(|m| m == "__hash__" || m == "__str__");
                    Some(ok)
                } else {
                    Some(false)
                }
            }
            // Any other right (CallableType, TupleType, UnionType, etc.)
            // falls to the `return False` at subtypes.py:551.
            _ => Some(false),
        };
    }
    // visit_literal_type (subtypes.py:1068-1072): when both sides are
    // LiteralType, subtype is structural equality. Needed by the
    // `_remove_redundant_union_items` dedup pass for unions like
    // [Literal[True], Literal[False]] (neither is a subtype of the
    // other, so dedup keeps both before literal contraction collapses
    // them to `bool`).
    if let (Type::LiteralType { .. }, Type::LiteralType { .. }) = (left, right) {
        return Some(left == right);
    }
    // visit_literal_type else-branch (subtypes.py:1072):
    // is_subtype(LiteralType, Instance) = is_subtype(lit.fallback, right).
    if let Type::LiteralType { fallback, .. } = left {
        if let Type::Instance { .. } = right {
            return is_subtype(fallback, right, ctx, resolver);
        }
    }
    // visit_instance vs LiteralType right (subtypes.py:724-728): only
    // fires when left.last_known_value is Some, recursing into
    // is_subtype(left.last_known_value, right). When lkv is None,
    // Instance is NOT a subtype of LiteralType (falls to else: False).
    if let Type::LiteralType { .. } = right {
        if let Type::Instance {
            last_known_value: Some(lkv),
            ..
        } = left
        {
            return is_subtype(lkv.as_ref(), right, ctx, resolver);
        }
        if let Type::Instance {
            last_known_value: None,
            ..
        } = left
        {
            return Some(false);
        }
    }
    // visit_type_var (subtypes.py:735-748), fast path only. When both
    // sides are TypeVarType with the same id (raw_id + namespace, per
    // TypeVarId.__eq__ types.py:567-577; meta_level is not in the wire
    // format) and the same upper_bound, Python returns True. The
    // values-with-upper_bound and upper_bound-recursion branches
    // produce results that need a deeper walker; defer those.
    //
    // This fast path is what makes is_equivalent_callable return
    // Some(true) for `def f[T](x: T) -> T` vs `def g[T](x: T) -> T`
    // after match_generic_callables renumbers both T's to the same id.
    if let Type::TypeVarType {
        raw_id: l_raw,
        namespace: l_ns,
        upper_bound: l_ub,
        values: l_values,
        ..
    } = left
    {
        if let Type::TypeVarType {
            raw_id: r_raw,
            namespace: r_ns,
            upper_bound: r_ub,
            ..
        } = right
        {
            if l_raw == r_raw && l_ns == r_ns {
                if l_ub == r_ub {
                    return Some(true);
                }
                // Different upper_bound: Python recurses into
                // _is_subtype(left.upper_bound, right.upper_bound) or
                // returns True for self-types. Defer (no is_self on the
                // wire; the recursive call may hit unsupported variants).
                return None;
            }
            // Different id: Python checks `left.values` then falls back
            // to `_is_subtype(left.upper_bound, right)`. The values
            // branch needs a UnionType join; defer.
            if !l_values.is_empty() {
                return None;
            }
            return is_subtype(l_ub.as_ref(), right, ctx, resolver);
        }
        // right not TypeVarType: Python checks `left.values` then
        // `_is_subtype(left.upper_bound, right)`. The values branch
        // needs a UnionType; defer when non-empty.
        if !l_values.is_empty() {
            return None;
        }
        return is_subtype(l_ub.as_ref(), right, ctx, resolver);
    }
    // visit_instance (subtypes.py:567-710) when right is TypeVarType:
    // Python falls through to `return False` (line 710) since right is
    // not Instance/TupleType/TypeVarTupleType/TypeType. Mirror that
    // for the common case (left=Instance, right=TypeVarType). The
    // protocol/TypeType branches are not reachable here (right is
    // TypeVarType, not those).
    if let Type::Instance { .. } = left {
        if let Type::TypeVarType { .. } = right {
            return Some(false);
        }
    }
    let (left_ref, left_args) = match left {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    let (right_ref, right_args) = match right {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    visit_instance_nominal(
        left_ref, left_args, right, right_ref, right_args, ctx, resolver,
    )
}

/// The `visit_instance` `isinstance(right, Instance)` branch
/// `expand_type_by_instance` (expandtype.py:85-115), Rust subset.
///
/// Substitutes TypeVarType nodes in `typ` whose `namespace` matches
/// `left_ref` and whose `raw_id` is a 1-based position into `left_args`.
/// Mirrors the Python env build:
///   `variables[binder.id] = arg` for each (defn.type_vars[i], args[i]).
///
/// Class type vars have `raw_id = i+1` and `namespace = class.fullname`,
/// so we match `(namespace == left_ref, raw_id == i+1)` and substitute
/// `left_args[i]`. Returns `None` for Type variants the subset walker
/// does not handle (CallableType, ParamSpec, UnpackType, etc. inside
/// the tree); the caller falls through to Python for those.
fn expand_type_by_instance(typ: &Type, left_ref: &str, left_args: &[Type]) -> Option<Type> {
    match typ {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let mut new_args = Vec::with_capacity(args.len());
            for arg in args {
                new_args.push(expand_type_by_instance(arg, left_ref, left_args)?);
            }
            // builtins.tuple normalization (expandtype.py:228-237) is
            // deferred; the common nominal case doesn't need it.
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }
        Type::TypeVarType {
            raw_id, namespace, ..
        } => {
            // Match class type vars: namespace == left_ref, raw_id is
            // 1-based position into defn.type_vars (== left_args).
            if namespace == left_ref && *raw_id >= 1 {
                let idx = (*raw_id - 1) as usize;
                if idx < left_args.len() {
                    // Python clears last_known_value on Instance
                    // replacements (expandtype.py:246-249); we clone as-is
                    // since lkv handling is the LiteralType path (M8c+).
                    return Some(left_args[idx].clone());
                }
            }
            // Unmatched TypeVar: namespace mismatch or raw_id out of
            // range. Python leaves it as-is and visit_typevar (not
            // ported) handles it. Return None to fall through.
            None
        }
        Type::UnionType {
            items,
            uses_pep604_syntax,
        } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_by_instance(item, left_ref, left_args)?);
            }
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
            })
        }
        Type::NoneType | Type::UninhabitedType => Some(typ.clone()),
        Type::AnyType { .. } | Type::DeletedType { .. } | Type::LiteralType { .. } => {
            Some(typ.clone())
        }
        // Unsupported variants in the tree: fall through to Python.
        _ => None,
    }
}

/// `map_instance_to_supertype` (maptype.py:8-23), Rust subset.
///
/// Walks `class_derivation_paths` (maptype.py:46-67) over the snapshot's
/// `bases` blobs, mapping `left` up to `right_ref`'s frame step by step
/// via `expand_type_by_instance`. Returns the mapped args (to compare
/// against `right_args` in `check_type_parameter`).
///
/// Handles direct bases (path length 1) and multi-level paths. Returns
/// `None` when any step hits an unsupported Type variant in
/// `expand_type_by_instance`, or when no derivation path is found (the
/// snapshot may be stale mid-build; Python handles the Any fallback).
pub(crate) fn map_instance_to_supertype(
    left_ref: &str,
    left_args: &[Type],
    right_ref: &str,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let _left_snap = resolver.get(left_ref)?;
    // Fast path: left.type == right.type (maptype.py:15-17).
    if left_ref == right_ref {
        return Some(left_args.to_vec());
    }
    // Walk class_derivation_paths via the snapshot's bases blobs.
    // Each base is a serialized Instance; decode and recurse.
    map_derivation_path(left_ref, left_args, right_ref, resolver)
}

/// Recursive step of `map_instance_to_supertypes` (maptype.py:26-43).
/// Finds a base whose type_ref == right_ref (direct) or recurses through
/// a base whose own bases lead to right_ref (multi-level path).
fn map_derivation_path(
    left_ref: &str,
    left_args: &[Type],
    right_ref: &str,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let left_snap = resolver.get(left_ref)?;
    // Variadic left: expand_type_by_instance would need the
    // split_with_prefix_and_suffix logic to substitute the TypeVarTuple
    // middle. Not ported; defer to Python. Also guards mid-path bases
    // that are variadic even when the original left isn't.
    if left_snap.has_type_var_tuple_type {
        return None;
    }
    for base_blob in &left_snap.bases {
        let base = decode_type(base_blob)?;
        if let Type::Instance {
            type_ref: base_ref,
            args: _base_args,
            ..
        } = &base
        {
            if base_ref == right_ref {
                // Direct base: expand base's args by left's frame.
                let expanded = expand_type_by_instance(&base, left_ref, left_args)?;
                if let Type::Instance { args, .. } = expanded {
                    return Some(args);
                }
                return None;
            }
            // Multi-level: recurse through this base. First map left to
            // this base's frame, then continue from there.
            let mapped = expand_type_by_instance(&base, left_ref, left_args)?;
            if let Type::Instance {
                type_ref: mid_ref,
                args: mid_args,
                ..
            } = mapped
            {
                if let Some(result) = map_derivation_path(&mid_ref, &mid_args, right_ref, resolver)
                {
                    return Some(result);
                }
            }
        }
    }
    None
}

fn visit_instance_nominal(
    left_ref: &str,
    left_args: &[Type],
    right: &Type,
    right_ref: &str,
    right_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    let left_snap = resolver.get(left_ref);
    let right_snap = resolver.get(right_ref);

    // If left's TypeInfo is not in the resolver, it may be a synthesized
    // type (e.g. ad-hoc intersection from isinstance narrowing) whose
    // MRO and bases are only available on the live Python TypeInfo.
    // Defer rather than returning a wrong Some(false).
    if left_snap.is_none() {
        return None;
    }

    // fallback_to_any short-circuit (subtypes.py:493-498): a class with
    // dynamic bases is a subtype of everything except None. We only
    // detect NoneType by tag; right is Instance here, so it never fires.
    if let Some(snap) = &left_snap {
        if snap.fallback_to_any && !ctx.proper_subtype {
            // Python handled NoneType before us (right is Instance here).
            return Some(true);
        }
    }

    // promote loop (subtypes.py:536-542): walk left.type.mro, check each
    // base's _promote against right. Skip on ignore_promotions or when
    // right is a protocol (snapshot missing means "assume not protocol").
    let right_not_protocol = right_snap.is_none_or(|s| !s.is_protocol);
    if !ctx.ignore_promotions && right_not_protocol {
        if let Some(snap) = &left_snap {
            for base_fullname in &snap.mro {
                if let Some(base_snap) = resolver.get(base_fullname) {
                    for promote_blob in &base_snap.promote_bytes {
                        if let Some(promote) = decode_type(promote_blob) {
                            if is_subtype(&promote, right, ctx, resolver) == Some(true) {
                                return Some(true);
                            }
                        }
                    }
                }
            }
            // alt_promote (subtypes.py:546-547): left.type.alt_promote
            // whose target type is right.type.
            if let Some(alt) = &snap.alt_promote_fullname {
                if alt == right_ref {
                    return Some(true);
                }
            }
        }
    }

    // Nominal check (subtypes.py:554-561). NamedTuple special case and
    // builtins.object fast-path mirror the Python condition.
    let has_base = left_snap.is_some_and(|s| s.has_base(right_ref));
    let is_object = right_ref == "builtins.object";
    let right_is_protocol = right_snap.is_some_and(|s| s.is_protocol);
    // Python's NamedTuple clause (subtypes.py:632-635) fires only when
    // `rname in TYPED_NAMEDTUPLE_NAMES` (right is typing.NamedTuple or
    // typing_extensions.NamedTuple literally) AND some class in left's
    // mro is a NamedTuple. The snapshot's `is_named_tuple` flag is True
    // for ANY NamedTuple subclass (e.g. __main__.A), not just the
    // typing.NamedTuple base, so checking `right_snap.is_named_tuple`
    // would wrongly apply the nominal branch to two unrelated
    // NamedTuples (e.g. is_subtype(A, B) -> Some(true)). Rust can't read
    // `rname in TYPED_NAMEDTUPLE_NAMES` from the snapshot alone without
    // also special-casing the two base fullnames; defer the whole
    // NamedTuple-right case so Python's exact condition decides.
    // Python's NamedTuple clause (subtypes.py:632-637) fires when right
    // is literally typing.NamedTuple or typing_extensions.NamedTuple (the
    // only two names in TYPED_NAMEDTUPLE_NAMES) AND some class in left's
    // mro is_named_tuple. Checking right_snap.is_named_tuple would be
    // wrong because that flag is True for ANY NamedTuple subclass.
    let is_named_tuple_right = matches!(
        right_ref,
        "typing.NamedTuple" | "typing_extensions.NamedTuple"
    ) && left_snap.is_some_and(|s| {
        s.mro
            .iter()
            .any(|m| resolver.get(m).is_some_and(|n| n.is_named_tuple))
    });
    let nominal_applies =
        (has_base || is_object || is_named_tuple_right) && !ctx.ignore_declared_variance;
    if !nominal_applies {
        // Nominal branch skipped. If right is a protocol, defer to the
        // Python protocol-implementation path (M8c). Otherwise Python
        // records a negative cache entry and returns False.
        if right_is_protocol {
            return None;
        }
        return Some(false);
    }

    let right_snap = right_snap?;

    // Variadic right (subtypes.py:644-670): Python takes a special path
    // using split_with_prefix_and_suffix to splice the TypeVarTuple
    // middle into left/right args. Not ported; defer to Python.
    if right_snap.has_type_var_tuple_type {
        return None;
    }
    // Variadic left when left != right: map_instance_to_supertype would
    // need the same split logic to substitute the variadic tvar. Defer.
    if left_ref != right_ref && left_snap.is_some_and(|s| s.has_type_var_tuple_type) {
        return None;
    }

    // Map left to right's type. Fast path: left.type == right.type (no
    // substitution needed). Slow path calls map_instance_to_supertype
    // to walk the bases blobs and substitute TypeVars.
    let mapped_args: Vec<Type> = if left_ref == right_ref {
        left_args.to_vec()
    } else if right_snap.type_vars_with_variance.is_empty() {
        // right has no type vars: map_instance_to_supertype returns
        // Instance(right, []) (no args to substitute).
        Vec::new()
    } else {
        // Generic substitution path: map_instance_to_supertype walks
        // class_derivation_paths over the snapshot's bases blobs,
        // substituting TypeVars via expand_type_by_instance. Returns
        // None when an unsupported Type variant is in the tree (e.g.
        // UnpackType, ParamSpec), in which case Python falls through.
        map_instance_to_supertype(left_ref, left_args, right_ref, resolver)?
    };

    if ctx.ignore_type_params {
        return Some(true);
    }

    // check_type_parameter over (lefta, righta, tvar) triples
    // (subtypes.py:598-621). VARIANCE_NOT_READY returns None (Python
    // handles infer_class_variances; mutating live defn, deferred).
    let right_tvars = &right_snap.type_vars_with_variance;
    if mapped_args.len() != right_args.len() || mapped_args.len() != right_tvars.len() {
        // Arity mismatch. Python would assert; we fall through rather
        // than panic, since the snapshot may be stale mid-build.
        return None;
    }
    let mut nominal = true;
    for (i, (_tvar_name, variance, kind)) in right_tvars.iter().enumerate() {
        // ParamSpec (kind=1) / TypeVarTuple (kind=2): Python's else
        // branch (subtypes.py:691-696) treats them as COVARIANT, but
        // the arg shapes (CallableType with ParamSpec prefix, TupleType
        // for variadic middle) hit unsupported variants in the
        // recursive is_subtype. Defer to Python.
        if *kind != 0 {
            return None;
        }
        let lefta = &mapped_args[i];
        let righta = &right_args[i];
        let effective_variance = if ctx.always_covariant && *variance == INVARIANT {
            COVARIANT
        } else {
            *variance
        };
        if *variance == VARIANCE_NOT_READY {
            // infer_class_variances mutates live defn.type_vars; the
            // snapshot can't mirror that without re-reading. Fall through.
            return None;
        }
        match check_type_parameter(lefta, righta, effective_variance, ctx, resolver) {
            Some(true) => {}
            Some(false) => {
                nominal = false;
                break;
            }
            // Recursive is_subtype hit an unsupported variant. Don't
            // assume not-subtype (would give wrong answers); defer.
            None => return None,
        }
    }
    Some(nominal)
}

/// `check_type_parameter` (subtypes.py:379-410), Rust subset.
///
/// Returns `Some(bool)` when Rust decided; `None` when a recursive
/// `is_subtype` hit an unsupported variant. Propagating `None` (rather
/// than swallowing it as `false` via `unwrap_or`) prevents wrong
/// answers: if Rust can't decide `is_subtype(lefta, righta)`, the whole
/// `visit_instance_nominal` must defer to Python, not assume not-subtype.
///
/// COVARIANT / VARIANCE_NOT_READY: `is_subtype(left, right)`.
/// CONTRAVARIANT: `is_subtype(right, left)`.
/// INVARIANT: `is_equivalent(left, right)` — a two-way subtype check
/// (both `is_subtype(left, right)` and `is_subtype(right, left)` must
/// hold). This mirrors Python's `is_equivalent` / `is_same_type` for
/// both proper and non-proper subtype checks. The `proper_subtype` flag
/// flows through `ctx.proper_subtype` into the recursive `is_subtype`
/// calls, so the two-way check respects properness at every depth.
fn check_type_parameter(
    left: &Type,
    right: &Type,
    variance: i64,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    match variance {
        COVARIANT | VARIANCE_NOT_READY => is_subtype(left, right, ctx, resolver),
        CONTRAVARIANT => is_subtype(right, left, ctx, resolver),
        _ => {
            let fwd = is_subtype(left, right, ctx, resolver)?;
            let bwd = is_subtype(right, left, ctx, resolver)?;
            Some(fwd && bwd)
        }
    }
}

/// Decode a wire-format `Type` blob via `wire::read_type`. Returns
/// `None` on any read failure (truncated input, unknown tag).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `#[pyfunction]` entry: the Python-side shim calls this with the
/// serialized `left` and `right` Type blobs plus the
/// `NativeTypeResolver` pyclass. Returns `None` (Python `None`) when
/// Rust doesn't handle the case; `Some(bool)` otherwise.
///
/// The shim in `mypy/subtypes.py` is responsible for `get_proper_type`,
/// the `AnyType`/`UnboundType`/`ErasedType` right short-circuit, the
/// `UnionType` right dispatch, the `TypeVarType`-with-values right,
/// and the `assuming` recursion guard BEFORE calling this.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_is_subtype(
    left_bytes: &[u8],
    right_bytes: &[u8],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let ctx = SubtypeContext::new(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
    );
    is_subtype(&left, &right, &ctx, resolver.resolver())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

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

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn ctx_nominal() -> SubtypeContext {
        SubtypeContext::new(false, false, false, false, false, true)
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        // Real TypeInfo always has its own fullname in mro and has_base.
        // Tests that need a different mro should overwrite these fields.
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s
    }

    #[test]
    fn same_instance_no_args_is_subtype() {
        // A <: A when both have no args (subtypes.py:554 has_base + map
        // to self + no type_params to check).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn instance_is_subtype_of_object() {
        // Any Instance is a subtype of builtins.object (subtypes.py:556).
        // object has no type vars, so the non-generic path applies.
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        let right = instance("builtins.object", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn derived_is_subtype_of_base_when_has_base() {
        // a.B has_base("a.A") -> B <: A.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![snap("a.A", "A"), b]);
        let left = instance("a.B", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn unrelated_instances_are_not_subtypes() {
        // a.A does not has_base("a.B") -> not a subtype, not object.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn fallback_to_any_short_circuits_non_proper() {
        // fallback_to_any=True, non-proper -> True (subtypes.py:493-498).
        let mut base = snap("a.AnyBase", "AnyBase");
        base.fallback_to_any = true;
        let r = make_resolver(vec![base, snap("a.Other", "Other")]);
        let left = instance("a.AnyBase", vec![]);
        let right = instance("a.Other", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn fallback_to_any_does_not_short_circuit_proper() {
        // proper_subtype=True: the fallback_to_any branch is skipped
        // (subtypes.py:493 `not self.proper_subtype`). a.AnyBase is not
        // a nominal base of a.Other and a.Other is not a protocol, so
        // Python records a negative cache and returns False
        // (subtypes.py:634-635).
        let mut base = snap("a.AnyBase", "AnyBase");
        base.fallback_to_any = true;
        let r = make_resolver(vec![base, snap("a.Other", "Other")]);
        let left = instance("a.AnyBase", vec![]);
        let right = instance("a.Other", vec![]);
        let ctx = SubtypeContext::new(false, false, false, false, true, true);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), Some(false));
    }

    #[test]
    fn alt_promote_matches_right() {
        // left.alt_promote_fullname == right.type_ref -> True
        // (subtypes.py:546-547).
        let mut s = snap("builtins.int", "int");
        s.alt_promote_fullname = Some("builtins.something".to_string());
        let r = make_resolver(vec![s, snap("builtins.something", "something")]);
        let left = instance("builtins.int", vec![]);
        let right = instance("builtins.something", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn generic_substitution_without_bases_returns_none() {
        // right has type_vars_with_variance and left.type != right.type,
        // but left has no bases blobs (snapshot not populated). The
        // map_instance_to_supertype walker returns None, falling through.
        let mut base = snap("a.Gen", "Gen");
        base.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut derived = snap("a.Sub", "Sub");
        derived.has_base.insert("a.Gen".to_string());
        derived.mro.push("a.Gen".to_string());
        let r = make_resolver(vec![base, derived]);
        let left = instance("a.Sub", vec![]);
        let right = instance("a.Gen", vec![any_type()]);
        // No bases blobs -> map_instance_to_supertype returns None.
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn expand_type_by_instance_substitutes_typevar() {
        // Instance[TypeVarType(T`1, ns="a.Sub")] with left = a.Sub[A]
        // -> Instance[A]. The TypeVar's (namespace, raw_id) matches
        // (left_ref, 1), so it's replaced by left_args[0].
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Sub.T".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
        };
        let base = instance("a.Gen", vec![tvar]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&base, "a.Sub", &[left_arg.clone()]);
        assert_eq!(expanded, Some(instance("a.Gen", vec![left_arg])));
    }

    #[test]
    fn expand_type_by_instance_no_match_returns_none() {
        // TypeVar with a different namespace is not substituted. Python
        // leaves it as-is, but visit_typevar (the unmatched-tvar path)
        // is not ported, so Rust returns None to fall through.
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Other.T".to_string(),
            raw_id: 1,
            namespace: "a.Other".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
        };
        let base = instance("a.Gen", vec![tvar]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&base, "a.Sub", &[left_arg]);
        assert_eq!(expanded, None);
    }

    #[test]
    fn expand_type_by_instance_recurses_into_instance_args() {
        // Instance[a.Gen[Instance[a.Gen[T]]]] with left = a.Sub[A]:
        // both outer and inner TypeVars get substituted.
        let tvar = || Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Sub.T".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
        };
        let inner = instance("a.Gen", vec![tvar()]);
        let outer = instance("a.Gen", vec![inner]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&outer, "a.Sub", &[left_arg.clone()]);
        let expected = instance("a.Gen", vec![instance("a.Gen", vec![left_arg])]);
        assert_eq!(expanded, Some(expected));
    }

    #[test]
    fn expand_type_by_instance_passthrough_for_leaf_types() {
        // NoneType, AnyType, LiteralType pass through unchanged.
        assert_eq!(
            expand_type_by_instance(&Type::NoneType, "a.Sub", &[]),
            Some(Type::NoneType)
        );
        let any = any_type();
        assert_eq!(expand_type_by_instance(&any, "a.Sub", &[]), Some(any));
    }

    #[test]
    fn expand_type_by_instance_returns_none_for_unsupported() {
        // TupleType inside the tree is not handled by the subset walker.
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![any_type()],
            implicit: false,
        };
        let expanded = expand_type_by_instance(&t, "a.Sub", &[any_type()]);
        assert_eq!(expanded, None);
    }

    #[test]
    fn same_type_covariant_args_subtype() {
        // A[A] <: A[A] when A's T is covariant. Args are Instance so the
        // Rust recurse handles them (AnyType right is handled Python-side
        // by the shim before calling into Rust).
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let r = make_resolver(vec![gen, snap("a.A", "A")]);
        let arg = instance("a.A", vec![]);
        let left = instance("a.Gen", vec![arg.clone()]);
        let right = instance("a.Gen", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn non_instance_returns_none() {
        // UnionType right, AnyType left: not the nominal path.
        let r = make_resolver(vec![]);
        let left = any_type();
        let right = Type::UnionType {
            items: vec![instance("a.A", vec![])],
            uses_pep604_syntax: true,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn variance_not_ready_returns_none() {
        // When a tvar has VARIANCE_NOT_READY, Python calls
        // infer_class_variances (mutates live defn); we return None.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), VARIANCE_NOT_READY, 0)];
        let r = make_resolver(vec![gen]);
        let left = instance("a.Gen", vec![any_type()]);
        let right = instance("a.Gen", vec![any_type()]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn ignore_type_params_short_circuits_to_true() {
        // When ignore_type_params is set, nominal base -> True.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![snap("a.A", "A"), b]);
        let ctx = SubtypeContext::new(true, false, false, false, false, true);
        let left = instance("a.B", vec![any_type()]);
        let right = instance("a.A", vec![any_type()]);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), Some(true));
    }

    #[test]
    fn invariant_args_equivalent_when_same() {
        // A[A] <: A[A] when A's T is invariant: is_equivalent =
        // is_subtype both ways, each direction is A <: A = True.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), INVARIANT, 0)];
        let r = make_resolver(vec![gen, snap("a.A", "A")]);
        let arg = instance("a.A", vec![]);
        let left = instance("a.Gen", vec![arg.clone()]);
        let right = instance("a.Gen", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    // ---- visit_none_type / visit_uninhabited_type / visit_deleted_type (M8aa) ----

    fn ctx_strict_optional(strict_optional: bool) -> SubtypeContext {
        SubtypeContext::new(false, false, false, false, false, strict_optional)
    }

    #[test]
    fn none_subtype_of_none_strict_optional() {
        // visit_none_type (subtypes.py:539-541): right is NoneType -> True.
        let r = make_resolver(vec![]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &Type::NoneType,
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_subtype_of_object_strict_optional() {
        // visit_none_type (subtypes.py:539-541): right is builtins.object
        // -> True (is_named_instance check).
        let r = make_resolver(vec![snap("builtins.object", "object")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("builtins.object", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_not_subtype_of_instance_strict_optional() {
        // visit_none_type (subtypes.py:551): strict_optional + right is
        // Instance (non-protocol) -> False. Protocol detection needs the
        // snapshot's is_protocol field; this test uses a non-protocol
        // Instance, so we return False.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(false)
        );
    }

    #[test]
    fn none_subtype_of_anything_when_optional_disabled() {
        // visit_none_type (subtypes.py:553-554): strict_optional=False
        // -> True for any right.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("a.A", vec![]),
                &ctx_strict_optional(false),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_subtype_of_protocol_with_hashable_members() {
        // visit_none_type (subtypes.py:543-549): right is a protocol
        // Instance. When all protocol_members are __hash__/__str__
        // (or members is empty), Python returns True.
        let mut proto = snap("typing.Hashable", "Hashable");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__hash__".to_string()];
        let r = make_resolver(vec![proto]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("typing.Hashable", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_not_subtype_of_protocol_with_other_members() {
        // visit_none_type (subtypes.py:543-549): right is a protocol
        // Instance but members include something other than
        // __hash__/__str__ -> False.
        let mut proto = snap("typing.Iterable", "Iterable");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__iter__".to_string()];
        let r = make_resolver(vec![proto]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("typing.Iterable", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(false)
        );
    }

    #[test]
    fn uninhabited_subtype_of_anything() {
        // visit_uninhabited_type (subtypes.py:555-556): UninhabitedType
        // is a subtype of everything.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::UninhabitedType,
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn deleted_subtype_of_anything() {
        // visit_deleted_type (subtypes.py:564): DeletedType is a
        // subtype of everything.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::DeletedType { source: None },
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }
}
