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
    fn new(
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
/// (subtypes.py:531-626), minus the protocol/variadic/find_member/
/// tuple-type/type_type/literal/callable paths (all return `None`).
///
/// Handles:
/// - `fallback_to_any` short-circuit (subtypes.py:493-498).
/// - the promote loop (subtypes.py:536-542).
/// - `alt_promote` equality (subtypes.py:546-547).
/// - nominal `has_base` check + `check_type_parameter` over
///   `type_vars_with_variance` (subtypes.py:554-626) for the
///   non-generic-substitution cases.
///
/// Returns `None` (fall through) when the path needs
/// `map_instance_to_supertype` with arg substitution (right has type
/// vars and left.type != right.type), since that requires
/// `expand_type_by_instance` (deferred to M8c).
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
    let is_named_tuple_right = right_snap.is_some_and(|s| s.is_named_tuple)
        && left_snap.is_some_and(|s| {
            s.mro
                .iter()
                .any(|m| resolver.get(m).is_some_and(|n| n.is_named_tuple))
        });
    let nominal_applies =
        (has_base || is_object || is_named_tuple_right) && !ctx.ignore_declared_variance;
    if !nominal_applies {
        // Nominal branch skipped. If right is a protocol, defer to the
        // Python protocol-implementation path (M8c). Otherwise Python
        // records a negative cache entry and returns False
        // (subtypes.py:627-635).
        if right_is_protocol {
            return None;
        }
        return Some(false);
    }

    let right_snap = right_snap?;

    // Map left to right's type. Fast path: left.type == right.type (no
    // substitution needed). Slow path (map_instance_to_supertype with
    // arg substitution) returns None for the generic case.
    let mapped_args: Vec<Type> = if left_ref == right_ref {
        left_args.to_vec()
    } else if right_snap.type_vars_with_variance.is_empty() {
        // right has no type vars: map_instance_to_supertype returns
        // Instance(right, []) (no args to substitute).
        Vec::new()
    } else {
        // Generic substitution path. Needs expand_type_by_instance,
        // which is a TypeVar-substitution walk over the base Instance
        // blob. Deferred to M8c.
        return None;
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
    for (i, (_tvar_name, variance, _kind)) in right_tvars.iter().enumerate() {
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
        if !check_type_parameter(lefta, righta, effective_variance, ctx, resolver) {
            nominal = false;
            break;
        }
    }
    Some(nominal)
}

/// `check_type_parameter` (subtypes.py:379-410), Rust subset.
///
/// COVARIANT / VARIANCE_NOT_READY: `is_subtype(left, right)`.
/// CONTRAVARIANT: `is_subtype(right, left)`.
/// INVARIANT (non-proper): `is_equivalent(left, right)` — needs
/// `is_same_type`, which is a two-way subtype check; we recurse
/// `is_subtype(left, right) && is_subtype(right, left)` for the
/// non-proper case (Python's `is_equivalent` does exactly this).
///
/// `proper_subtype` + INVARIANT returns `true` conservatively so the
/// caller's `nominal` flag isn't falsely lowered; the Python path will
/// re-check via `is_same_type` (its `ignore_promotions` plumbing is
/// deferred to M8c).
fn check_type_parameter(
    left: &Type,
    right: &Type,
    variance: i64,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> bool {
    match variance {
        COVARIANT | VARIANCE_NOT_READY => is_subtype(left, right, ctx, resolver).unwrap_or(false),
        CONTRAVARIANT => is_subtype(right, left, ctx, resolver).unwrap_or(false),
        _ => {
            if ctx.proper_subtype {
                true
            } else {
                is_subtype(left, right, ctx, resolver).unwrap_or(false)
                    && is_subtype(right, left, ctx, resolver).unwrap_or(false)
            }
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
    fn generic_substitution_returns_none() {
        // right has type_vars_with_variance and left.type != right.type:
        // needs expand_type_by_instance, returns None.
        let mut base = snap("a.Gen", "Gen");
        base.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut derived = snap("a.Sub", "Sub");
        derived.has_base.insert("a.Gen".to_string());
        derived.mro.push("a.Gen".to_string());
        let r = make_resolver(vec![base, derived]);
        let left = instance("a.Sub", vec![]);
        let right = instance("a.Gen", vec![any_type()]);
        // Generic substitution path -> None.
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
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
}
