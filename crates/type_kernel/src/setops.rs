//! Stage 3c (M8d): `trivial_join` / `trivial_meet` on the Rust `Type` enum.
//!
//! Ports the subtype-only fallbacks `mypy.join.trivial_join`
//! (join.py:198-205) and `mypy.meet.trivial_meet` (meet.py:62-72).
//! Both reduce the set-theoretic op to `is_subtype` + a branch on
//! which side is wider:
//!
//! * `trivial_join(s, t)`: if `s <: t` return `t`; if `t <: s` return
//!   `s`; else `object_or_any_from_type(t)`.
//! * `trivial_meet(s, t)`: if `s <: t` return `s`; if `t <: s` return
//!   `t`; else `bottom` (strict_optional ? `UninhabitedType` :
//!   `NoneType`).
//!
//! These are the fallback paths the full `join_types`/`meet_types`
//! visitors hit for recursive pairs and protocols (meet.py:77-80,
//! join.py:252-257). Porting them first reuses the M8b/M8c `is_subtype`
//! primitive and unblocks the parity suite for the trivial cases
//! before the full visitors land.
//!
//! Rather than serialize the result Type on the Rust side (the wire
//! format is reader-only, no Rust writer), we return a small
//! `SetOpResult` discriminator. Python maps it back to the live Type:
//! `SameS` -> `s`, `SameT` -> `t`, `Object` -> `object_from_instance(t)`,
//! `Bottom` -> `UninhabitedType` / `NoneType` (strict_optional).
//!
//! The strangler-fig contract mirrors `erase::erase_type`
//! (erasetype.py:80-86): `None` means "Rust doesn't handle this, let
//! Python decide". No production code calls this until
//! `Options.native_type_kernel` is on AND `mypy/join.py` / `mypy/meet.py`
//! dispatch to it (the shims are added in this same milestone).

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type};

use crate::subtypes::{is_subtype, SubtypeContext};

/// Discriminator for `trivial_join` / `trivial_meet` results.
///
/// Python maps each variant to a live `Type`:
/// * `SameS` -> the `s` argument (unchanged).
/// * `SameT` -> the `t` argument (unchanged).
/// * `Object` -> `object_or_any_from_type(t)` (Instance right only;
///   non-Instance right defers with `None`).
/// * `Bottom` -> `UninhabitedType` (strict_optional) or `NoneType`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum SetOpResult {
    SameS,
    SameT,
    Object,
    Bottom,
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

/// `#[pyfunction]` entry for `trivial_join`. The Python-side shim
/// (mypy/join.py) calls this with serialized `s`/`t` blobs plus the
/// `NativeTypeResolver` pyclass. Returns `None` (Python `None`) when
/// Rust doesn't handle the case; `Some(i64)` discriminator
/// otherwise (0=SameS, 1=SameT, 2=Object, 3=Bottom).
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
    trivial_join(&s, &t, &ctx, resolver.resolver()).map(discriminator)
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
    trivial_meet(&s, &t, &ctx, resolver.resolver()).map(discriminator)
}

/// Map `SetOpResult` to the Python-side discriminator integer.
fn discriminator(r: SetOpResult) -> i64 {
    match r {
        SetOpResult::SameS => 0,
        SetOpResult::SameT => 1,
        SetOpResult::Object => 2,
        SetOpResult::Bottom => 3,
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
        assert_eq!(discriminator(SetOpResult::SameS), 0);
        assert_eq!(discriminator(SetOpResult::SameT), 1);
        assert_eq!(discriminator(SetOpResult::Object), 2);
        assert_eq!(discriminator(SetOpResult::Bottom), 3);
    }
}
