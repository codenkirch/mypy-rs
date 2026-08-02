//! Stage 4b constraint solver: `solve_one` (solve.py:263-329).
//!
//! Ports the single-variable best-type computation: given lower and
//! upper bound iterables, solve for the candidate type. Pure fold over
//! `join_types`/`meet_types`/`is_subtype`, materializing each `SetOpResult`
//! to a concrete `Type` so the next fold step can continue in Rust.
//!
//! The Python shim (`mypy/solve.py` `solve_one`) is responsible for
//! `get_proper_type` expansion and the ambiguous-`UninhabitedType`
//! upper-bound filter (the `ambiguous` flag is not on the wire), so the
//! Rust entry receives already-processed bound lists.

use pyo3::prelude::*;

use crate::setops::{self, SetOpResult};
use crate::subtypes::{self, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// Wrap `wire::read_type` into an `Option`, mirroring `setops::decode_type`.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// Wire-encode a `Type` into a fresh byte buffer.
fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// `(kind, bytes)`:
/// * `kind=0` solved; `bytes` holds the encoded candidate Type.
/// * `kind=1` no solution or no lower bound; `bytes` is empty (candidate
///   is the folded upper bound, or Python's `None` when `bytes` is
///   absent).
/// * `kind=2` ambiguous `UninhabitedType` (no bounds at all);
///   `bytes` empty. The shim returns `UninhabitedType(ambiguous=True)`
///   mirroring solve.py:276-281.
/// * Any-absorption defers to Python entirely (the `from_another_any`
///   source identity lives there), so no Any flag is on the wire.
type SolveOut = (i64, Option<Vec<u8>>);

/// `solve_one` (solve.py:263-329), Rust subset.
///
/// Returns `None` (defer to Python) when any `join_types`, `meet_types`,
/// `is_subtype`, or `UnionType`-construction step is not handled by the
/// Rust kernel. The pure mechanics (fold + selection) match Python:
/// * `UnionType.make_union(lowers)` (infer_unions) with
///   `UnionType.__init__` flattening.
/// * `join_type_list` sorted by `_join_sorted_key` (non-infer_unions).
/// * `meet_types` fold over uppers.
/// * Any-absorption (any side is `AnyType`), deferred to Python.
/// * `is_subtype(bottom, top)` selection (bottom wins when it is a
///   subtype of top; else no solution).
#[allow(dead_code)]
pub(crate) fn solve_one_inner(
    lowers: &[Type],
    uppers: &[Type],
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Option<SolveOut> {
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);

    if uppers.is_empty() && lowers.is_empty() {
        return Some((2, None));
    }

    // bottom: type_state.infer_unions -> UnionType.make_union(lowers);
    // else join of lowers sorted by _join_sorted_key. (solve.py's extra
    // `if sorted_lowers` is always True after the empty-check; see tests.)
    let bottom: Option<Type> = if lowers.is_empty() {
        None
    } else if infer_unions {
        // UnionType.make_union = make_simplified_union (flatten +
        // dedupe + literal contraction + primitive union), which already
        // returns Option (defers internally).
        setops::make_simplified_union(lowers, &ctx, resolver)
    } else {
        // join_type_list preserves sorted_lowers[0] even for an AnyType
        // (mypy TypeAlias). Wire types are never alias shadows, so a
        // plain fold matches.
        let mut sorted = lowers.to_vec();
        sorted.sort_by_key(join_sorted_key);
        let mut joined: Option<Type> = None;
        for t in &sorted {
            joined = Some(match joined {
                None => t.clone(),
                Some(prev) => materialize_join(&prev, t, &ctx, resolver)?,
            });
        }
        joined
    };

    // top: meet of uppers (meet_types fold).
    let mut top: Option<Type> = None;
    for target in uppers {
        top = Some(match top {
            None => target.clone(),
            Some(prev) => materialize_meet(&prev, target, &ctx, resolver)?,
        });
    }

    let p_top = top.as_ref();
    let p_bottom = bottom.as_ref();
    if matches!(p_top, Some(Type::AnyType { .. })) || matches!(p_bottom, Some(Type::AnyType { .. }))
    {
        // Any-absorption defers to Python: the source AnyType identity
        // (from_another_any) is a live object the wire cannot preserve;
        // the shim computes `AnyType(from_another_any, source_any)`.
        return None;
    }

    match (bottom, top) {
        (None, Some(top_t)) => Some((1, encode_type(&top_t))),
        (None, None) => Some((1, None)),
        (Some(bottom_t), None) => {
            let bytes = encode_type(&bottom_t)?;
            Some((0, Some(bytes)))
        }
        (Some(bottom_t), Some(top_t)) => {
            let ok = subtypes::is_subtype(&bottom_t, &top_t, &ctx, resolver)?;
            if ok {
                let bytes = encode_type(&bottom_t)?;
                Some((0, Some(bytes)))
            } else {
                // Not a subtype: solve_one returns None (unbound). The
                // kind=1/no-blob signal maps to exactly that, so no defer
                // is needed (the Python re-run would compute the same).
                Some((1, None))
            }
        }
    }
}

/// `_join_sorted_key` (solve.py:251-261): UnionType=-2, NoneType=-1,
/// Overloaded=1, else 0. Mirror exactly (solve_one sorts lowers by this
/// before joining, since joins are non-associative).
fn join_sorted_key(t: &Type) -> i64 {
    match t {
        Type::UnionType { .. } => -2,
        Type::NoneType => -1,
        Type::Overloaded { .. } => 1,
        _ => 0,
    }
}

/// Materialize a `SetOpResult` from `join_types` into a concrete `Type`.
/// Needs the `type_ref -> TypeInfo` resolver for `Object`, `Ancestor`,
/// and `SameTypeWithArgs` (mirroring the Python shim's
/// `_native_join_typeinfo_map`). `Encoded` decode is internal to the
/// wire format, no live TypeInfo needed.
fn materialize_join(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &crate::typeinfo::TypeResolver,
) -> Option<Type> {
    let r = setops::join_types(s, t, ctx, resolver)?;
    materialize_setop(r, s, t, resolver)
}

/// Materialize a `SetOpResult` from `meet_types` into a concrete `Type`.
/// `meet_types` only emits SameS/SameT/Bottom/Any (never Object,
/// Ancestor, SameTypeWithArgs).
fn materialize_meet(
    s: &Type,
    t: &Type,
    ctx: &SubtypeContext,
    resolver: &crate::typeinfo::TypeResolver,
) -> Option<Type> {
    let r = setops::meet_types(s, t, ctx, resolver)?;
    match r {
        SetOpResult::SameS => Some(s.clone()),
        SetOpResult::SameT => Some(t.clone()),
        SetOpResult::Bottom => Some(Type::UninhabitedType { ambiguous: true }),
        SetOpResult::Any => Some(any_type()),
        _ => None, // meet never produces these; defer if it ever does.
    }
}

/// Materialize a generic `SetOpResult` (join path) into a `Type`.
fn materialize_setop(
    r: SetOpResult,
    s: &Type,
    t: &Type,
    resolver: &crate::typeinfo::TypeResolver,
) -> Option<Type> {
    match r {
        SetOpResult::SameS => Some(s.clone()),
        SetOpResult::SameT => Some(t.clone()),
        SetOpResult::Object => Some(object_or_any_from_type(t, resolver)),
        SetOpResult::Bottom => Some(Type::UninhabitedType { ambiguous: true }),
        SetOpResult::Any => Some(any_type()),
        SetOpResult::Ancestor(fullname) => {
            resolver.get(&fullname)?;
            Some(Type::Instance {
                type_ref: fullname,
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            })
        }
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => {
            let s_args = instance_args(s);
            let t_args = instance_args(t);
            let mut new_args = Vec::with_capacity(arg_discs.len());
            for (i, ad) in arg_discs.iter().enumerate() {
                match ad {
                    0 => new_args.push(s_args.get(i)?.clone()),
                    1 => new_args.push(t_args.get(i)?.clone()),
                    4 => {
                        // AnyType(from_another_any, source): pick the
                        // AnyType side (join.py:131-135).
                        let src = match s_args.get(i) {
                            Some(a) if matches!(a, Type::AnyType { .. }) => a.clone(),
                            _ => t_args.get(i)?.clone(),
                        };
                        new_args.push(any_type_from(src));
                    }
                    _ => return None,
                }
            }
            resolver.get(&type_ref)?;
            Some(Type::Instance {
                type_ref,
                args: new_args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
        SetOpResult::Encoded(bytes) => decode_type(&bytes),
    }
}

/// Extract `Instance` args for SameTypeWithArgs reconstruction.
fn instance_args(t: &Type) -> Vec<Type> {
    match t {
        Type::Instance { args, .. } => args.clone(),
        _ => Vec::new(),
    }
}

/// `object_or_any_from_type` (join.py:262-276), Instance-only subset.
fn object_or_any_from_type(t: &Type, resolver: &crate::typeinfo::TypeResolver) -> Type {
    if matches!(t, Type::AnyType { .. }) {
        any_type()
    } else if let Type::CallableType { fallback, .. } = t {
        // CallableType fallback is an Instance.
        if matches!(fallback.as_ref(), Type::AnyType { .. }) {
            any_type()
        } else {
            fallback.as_ref().clone()
        }
    } else if resolver.get("builtins.object").is_some() {
        Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    } else {
        any_type()
    }
}

/// AnyType with `type_of_any=0` (special_form).
fn any_type() -> Type {
    Type::AnyType {
        type_of_any: 0,
        source_any: None,
        missing_import_name: None,
    }
}

/// AnyType with `source_any` set (from_another_any).
fn any_type_from(source: Type) -> Type {
    Type::AnyType {
        type_of_any: 1,
        source_any: Some(Box::new(source)),
        missing_import_name: None,
    }
}

/// `#[pyfunction]` entry for `solve_one`. The Python-side shim
/// (mypy/solve.py) calls this after `get_proper_type` expansion and the
/// ambiguous-upper filter, with serialized `lowers`/`uppers` blob lists,
/// `infer_unions`, and `strict_optional`. Returns `None` (Python `None`)
/// when Rust doesn't handle the case; `Some((kind, bytes))` otherwise
/// (see `SolveOut`).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_solve_one(
    lowers: Vec<Vec<u8>>,
    uppers: Vec<Vec<u8>>,
    infer_unions: bool,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> Option<(i64, Option<Vec<u8>>)> {
    let mut lowers_types: Vec<Type> = Vec::with_capacity(lowers.len());
    for b in &lowers {
        lowers_types.push(decode_type(b)?);
    }
    let mut uppers_types: Vec<Type> = Vec::with_capacity(uppers.len());
    for b in &uppers {
        uppers_types.push(decode_type(b)?);
    }
    // TypeVar-carrying bounds are identity-bearing (their assignment is
    // semantically meaningful, e.g. upper_bounds). Defer to Python so the
    // candidate is chosen there, matching the full mypy solve.
    if lowers_types
        .iter()
        .chain(uppers_types.iter())
        .any(crate::visitor::has_type_vars_inner)
    {
        return None;
    }
    solve_one_inner(
        &lowers_types,
        &uppers_types,
        infer_unions,
        strict_optional,
        resolver.resolver(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> crate::typeinfo::TypeResolver {
        let mut r = crate::typeinfo::TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    /// Test snapshot with `fullname` as both fullname and name, with
    /// optional extra bases (added to mro + has_base).
    fn snap_with_bases(fullname: &str, bases: &[&str]) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        for b in bases {
            s.mro.push(b.to_string());
            s.has_base.insert(b.to_string());
        }
        if fullname != "builtins.object" && !s.has_base.contains("builtins.object") {
            s.mro.push("builtins.object".to_string());
            s.has_base.insert("builtins.object".to_string());
        }
        s
    }

    fn snap(fullname: &str) -> TypeInfoSnapshot {
        snap_with_bases(fullname, &[])
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// decode a `SolveOut`'s bytes with a fresh resolver (mirrors the
    /// Python shim's `read_type` + type_ref resolution).
    fn out_bytes(out: &SolveOut) -> Option<Vec<u8>> {
        out.1.clone()
    }

    #[test]
    fn no_bounds_returns_ambiguous() {
        let r = make_resolver(vec![]);
        let out = solve_one_inner(&[], &[], false, true, &r).unwrap();
        assert_eq!(out.0, 2);
        assert!(out.1.is_none());
    }

    #[test]
    fn lower_only_returns_lower() {
        let r = make_resolver(vec![]);
        let lo = instance("a.A", vec![]);
        let out = solve_one_inner(std::slice::from_ref(&lo), &[], false, true, &r).unwrap();
        assert_eq!(out.0, 0);
        let bytes = out_bytes(&out).unwrap();
        assert_eq!(decode_type(&bytes).unwrap(), lo);
    }

    #[test]
    fn upper_only_returns_upper() {
        let r = make_resolver(vec![]);
        let up = instance("a.A", vec![]);
        let out = solve_one_inner(&[], std::slice::from_ref(&up), false, true, &r).unwrap();
        assert_eq!(out.0, 1);
        let bytes = out.1.unwrap();
        assert_eq!(decode_type(&bytes).unwrap(), up);
    }

    #[test]
    fn subtype_selects_bottom() {
        // A <: B -> candidate = A (bottom). A's snapshot must declare
        // B in mro/has_base so is_subtype(A, B) resolves.
        let r = make_resolver(vec![snap_with_bases("a.A", &["a.B"]), snap("a.B")]);
        let lo = instance("a.A", vec![]);
        let up = instance("a.B", vec![]);
        let out = solve_one_inner(
            std::slice::from_ref(&lo),
            std::slice::from_ref(&up),
            false,
            true,
            &r,
        )
        .unwrap();
        assert_eq!(out.0, 0);
        let bytes = out_bytes(&out).unwrap();
        assert_eq!(decode_type(&bytes).unwrap(), lo);
    }

    #[test]
    fn non_subtype_returns_none_with_bounds() {
        // A and B unrelated -> candidate = None (kind=1, no bytes).
        let r = make_resolver(vec![snap("a.A"), snap("a.B")]);
        let lo = instance("a.A", vec![]);
        let up = instance("a.B", vec![]);
        let out = solve_one_inner(&[lo], &[up], false, true, &r).unwrap();
        assert_eq!(out.0, 1);
        assert!(out.1.is_none());
    }

    #[test]
    fn join_fold_merges_lowers() {
        // join(int, str) = object (unrelated, Instance right -> Object).
        // int/str each have builtins.object as a base so the
        // via_supertype walk resolves.
        let mut int_snap = snap("builtins.int");
        int_snap
            .bases
            .push(crate::wire::encode_instance_simple_for_test(
                "builtins.object",
            ));
        let mut str_snap = snap("builtins.str");
        str_snap
            .bases
            .push(crate::wire::encode_instance_simple_for_test(
                "builtins.object",
            ));
        let r = make_resolver(vec![snap("builtins.object"), int_snap, str_snap]);
        let lo_int = instance("builtins.int", vec![]);
        let lo_str = instance("builtins.str", vec![]);
        let out = solve_one_inner(&[lo_int, lo_str], &[], false, true, &r).unwrap();
        assert_eq!(out.0, 0);
        let bytes = out_bytes(&out).unwrap();
        assert_eq!(
            decode_type(&bytes).unwrap(),
            instance("builtins.object", vec![])
        );
    }
}
