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

use std::collections::{BTreeSet, HashMap, HashSet};

use pyo3::prelude::*;

use crate::constraints::Constraint;
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
/// * `kind=0` solved; `bytes` holds the encoded candidate Type (`None`
///   bytes = the Python body's final `None`).
/// * `kind=1` no lower bound; `bytes` holds the folded upper bound (or
///   no solution/`None` when `bytes` is absent).
/// * `kind=2` ambiguous `UninhabitedType` (no bounds at all);
///   `bytes` empty. The shim returns `UninhabitedType(ambiguous=True)`
///   mirroring solve.py:276-281.
/// * `kind=3` AnyType absorption (top or bottom is `AnyType`); `bytes`
///   holds the encoded `AnyType(from_another_any, source_any=<Any>)`.
///   `source_any` is the Any side itself (solve.py:604-607).
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
/// * Any-absorption (solve.py:604-607): either bound is a proper
///   `AnyType` -> `AnyType(TypeOfAny.from_another_any, source_any=<the
///   Any side>`).
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
        // UnionType.make_union(lowers) (solve.py:591) is the RAW
        // constructor; make_simplified_union would re-simplify e.g.
        // `T :> A | B` (B <: A) to `A` while Python keeps A | B.
        Some(match lowers {
            [single] => single.clone(),
            _ => Type::UnionType {
                items: lowers.to_vec(),
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            },
        })
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

    // Any-absorption (solve.py:604-607): `source_any` is the AnyType
    // side itself (one of top/bottom is a real AnyType; the fold
    // results carry Any positions from the input, never fold-produced).
    let p_top = top.as_ref();
    let p_bottom = bottom.as_ref();
    if matches!(p_bottom, Some(Type::AnyType { .. })) || matches!(p_top, Some(Type::AnyType { .. }))
    {
        let source = if matches!(p_top, Some(Type::AnyType { .. })) {
            top.clone()?
        } else {
            bottom.clone()?
        };
        let missing_import_name = match &source {
            Type::AnyType {
                missing_import_name,
                ..
            } => missing_import_name.clone(),
            _ => None,
        };
        let any = Type::AnyType {
            type_of_any: 7, // TypeOfAny.from_another_any
            source_any: Some(Box::new(source)),
            missing_import_name,
        };
        return Some((3, encode_type(&any)));
    }

    match (bottom, top) {
        (None, Some(top_t)) => Some((0, Some(encode_type(&top_t)?))),
        (None, None) => Some((0, None)),
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
                // kind=0/no-blob signal maps to exactly that, so no defer
                // is needed (the Python re-run would compute the same).
                Some((0, None))
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

/// AnyType materialized by meet/join folds: `TypeOfAny.special_form`
/// (6), matching the Anys Python's meet/join emit (meet.py:200-204,
/// join.py:131-135). A raw 0 would be an enum-invalid value that the
/// wire-safety probe (`wire_unsafe_solution`) rejects, forcing a defer.
fn any_type() -> Type {
    Type::AnyType {
        type_of_any: 6,
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
/// (mypy/solve.py) calls this after the ambiguous-upper filter, with
/// serialized `lowers`/`uppers` blob lists, `infer_unions`, and
/// `strict_optional`. Returns `None` (Python `None`) when Rust doesn't
/// handle the case; `Some((kind, bytes))` otherwise (see `SolveOut`).
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

/// `is_trivial_bound` (solve.py:651-655): is `tp` a trivial bound, i.e. the
/// wide `builtins.object` top (or `builtins.tuple` when `allow_tuple` is
/// set, recursing into the singleton type argument). Returns `None` either
/// on decode failure or when an alias needs expansion, which the shim maps
/// to the Python fallback.
#[pyfunction]
pub(crate) fn rust_is_trivial_bound(t_bytes: &[u8], allow_tuple: bool) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_trivial_bound_inner(&t, allow_tuple)
}

fn is_trivial_bound_inner(t: &Type, allow_tuple: bool) -> Option<bool> {
    match t {
        Type::TypeAliasType { args, .. } => {
            let arg = args.first()?;
            is_trivial_bound_inner(arg, allow_tuple)
        }
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
            if !allow_tuple {
                return Some(false);
            }
            let arg = args.first()?;
            is_trivial_bound_inner(arg, allow_tuple)
        }
        Type::Instance { type_ref, .. } => Some(type_ref == "builtins.object"),
        _ => Some(false),
    }
}

/// PyO3 connector for `find_linear` (solve.py:657-671): decode a wire
/// `Constraint` (origin_type_var, op, target) and run the linearity check.
/// Returns `(is_linear, tv_id)` where `tv_id` is `(raw_id, meta_level,
/// namespace)` or `None`. On decode failure, `None` defers to Python.
#[allow(clippy::type_complexity)]
#[pyfunction]
pub(crate) fn rust_find_linear(c_bytes: &[u8]) -> Option<(bool, Option<(i64, i64, String)>)> {
    let mut buf = ReadBuffer::new(c_bytes);
    let c = crate::constraints::Constraint::read(&mut buf).ok()?;
    Some(find_linear(&c))
}

// ---------------------------------------------------------------------------
// solve_with_dependent (solve.py:234-292) Rust subset.
// ---------------------------------------------------------------------------

/// Type variable id = (raw_id, meta_level, namespace), mirroring
/// `mypy.types.TypeVarId` and `expandtype::EnvKey` (the wire carries the
/// same triple, so the two are interchangeable).
type TvId = (i64, i64, String);

/// Bound sets and reachability graph produced by `transitive_closure`.
type BoundSets = (
    HashSet<(TvId, TvId)>,
    HashMap<TvId, Vec<Type>>,
    HashMap<TvId, Vec<Type>>,
);

/// `solve_with_dependent_native` outcome: `Some` = both blobs, `None` = empty
/// solutions.
type NativeDependentOut = Option<(Option<Vec<u8>>, Option<Vec<u8>>)>;

/// PyO3-facing `(num_solved, sol_blob, free_blob)`; `None` defers to Python.
type SolveDependentOut = Option<(i64, Option<Vec<u8>>, Option<Vec<u8>>)>;

/// `to_raw_id(x)` — resolve a `Type`'s TypeVar id (raw_id, meta_level,
/// namespace). ParamSpec and TypeVarTuple are stored with `meta_level 0`.
fn tv_id(t: &Type) -> Option<TvId> {
    match t {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        _ => None,
    }
}

/// Collect the type variable ids inside `typ`, mirroring
/// `has_type_vars_inner`'s exact recursion (the three TypeVar variants are
/// leaves; alias targets are skipped).
fn collect_type_var_ids(typ: &Type, out: &mut Vec<TvId>) {
    match typ {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            if let Some(id) = tv_id(typ) {
                out.push(id);
            }
        }
        Type::UnboundType { args, .. } => {
            for a in args {
                collect_type_var_ids(a, out);
            }
        }
        Type::UnpackType { typ } => collect_type_var_ids(typ, out),
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            for a in args {
                collect_type_var_ids(a, out);
            }
            if let Some(v) = last_known_value {
                collect_type_var_ids(v, out);
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            ..
        } => {
            for a in arg_types {
                collect_type_var_ids(a, out);
            }
            collect_type_var_ids(ret_type, out);
            for v in variables {
                collect_type_var_ids(v, out);
            }
            if let Some(i) = instance_type {
                collect_type_var_ids(i, out);
            }
        }
        Type::Overloaded { items } => {
            for i in items {
                collect_type_var_ids(i, out);
            }
        }
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            for i in items {
                collect_type_var_ids(i, out);
            }
            collect_type_var_ids(partial_fallback, out);
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            for (_, t) in items {
                collect_type_var_ids(t, out);
            }
            collect_type_var_ids(fallback, out);
        }
        Type::LiteralType { fallback, .. } => collect_type_var_ids(fallback, out),
        Type::UnionType { items, .. } => {
            for i in items {
                collect_type_var_ids(i, out);
            }
        }
        Type::TypeType { item, .. } => collect_type_var_ids(item, out),
        Type::AnyType {
            source_any: Some(s),
            ..
        } => collect_type_var_ids(s, out),
        Type::AnyType { .. } => {}
        Type::TypeAliasType { args, .. } => {
            for a in args {
                collect_type_var_ids(a, out);
            }
        }
        _ => {}
    }
}

/// `get_vars(target, vars)` (solve.py:705-707): ids in `target` that are in
/// `vars`, as a set.
fn get_vars(target: &Type, vars: &HashSet<TvId>) -> HashSet<TvId> {
    let mut all = Vec::new();
    collect_type_var_ids(target, &mut all);
    all.into_iter().filter(|id| vars.contains(id)).collect()
}

/// Push `t` if not already present (bounds are sets, ordered irrelevant).
fn push_unique(vec: &mut Vec<Type>, t: Type) {
    if !vec.contains(&t) {
        vec.push(t);
    }
}

/// `find_linear` (solve.py:534-548).
fn find_linear(c: &Constraint) -> (bool, Option<TvId>) {
    if matches!(&c.origin_type_var, Type::TypeVarType { .. }) {
        if let Type::TypeVarType { .. } = &c.target {
            return (true, tv_id(&c.target));
        }
    }
    if matches!(&c.origin_type_var, Type::ParamSpecType { .. }) {
        if let Type::ParamSpecType { prefix, .. } = &c.target {
            if prefix.arg_types.is_empty() {
                return (true, tv_id(&c.target));
            }
        }
    }
    if let Type::TypeVarTupleType { .. } = &c.origin_type_var {
        if let Type::TupleType { items, .. } = &c.target {
            if items.len() == 1 {
                let item = &items[0];
                if let Type::UnpackType { typ } = item {
                    if matches!(typ.as_ref(), Type::TypeVarTupleType { .. }) {
                        return (true, tv_id(typ));
                    }
                }
            }
        }
    }
    (false, None)
}

/// `add_secondary_constraints` (solve.py:625-636). Both union sides skipped
/// (not a defer). `Err(())` = Rust cannot infer these secondary constraints.
fn add_secondary_constraints(
    remaining: &mut Vec<Constraint>,
    lower: &Type,
    upper: &Type,
    resolver: &crate::typeinfo::TypeResolver,
    strict_optional: bool,
) -> Result<(), ()> {
    if matches!(upper, Type::UnionType { .. }) && matches!(lower, Type::UnionType { .. }) {
        return Ok(());
    }
    // No alias resolver is threaded through solve's secondary-constraint
    // path; an empty resolver means a TypeAliasType operand defers exactly
    // as it did before (the old top-level check returned None on an alias).
    let no_aliases = crate::aliases::TypeAliasResolver::new();
    let sub = crate::constraints::infer_constraints_full_inner(
        lower,
        upper,
        crate::constraints::SUBTYPE_OF,
        resolver,
        &no_aliases,
        strict_optional,
    )
    .ok_or(())?;
    for c in sub {
        if !remaining.contains(&c) {
            remaining.push(c);
        }
    }
    let sup = crate::constraints::infer_constraints_full_inner(
        upper,
        lower,
        crate::constraints::SUPERTYPE_OF,
        resolver,
        &no_aliases,
        strict_optional,
    )
    .ok_or(())?;
    for c in sup {
        if !remaining.contains(&c) {
            remaining.push(c);
        }
    }
    Ok(())
}

/// `transitive_closure` (solve.py:551-622).
fn transitive_closure(
    tvars: &[TvId],
    constraints: &[Constraint],
    resolver: &crate::typeinfo::TypeResolver,
    strict_optional: bool,
) -> Result<BoundSets, ()> {
    let tvars_set: HashSet<TvId> = tvars.iter().cloned().collect();
    let mut uppers: HashMap<TvId, Vec<Type>> = HashMap::new();
    let mut lowers: HashMap<TvId, Vec<Type>> = HashMap::new();
    let mut graph: HashSet<(TvId, TvId)> =
        tvars.iter().cloned().map(|tv| (tv.clone(), tv)).collect();

    let mut remaining: Vec<Constraint> = constraints.to_vec();
    while let Some(c) = remaining.pop() {
        let (is_linear, target_id) = find_linear(&c);
        if is_linear && target_id.as_ref().is_some_and(|t| tvars_set.contains(t)) {
            let target_id = target_id.unwrap();
            let origin_id = tv_id(&c.origin_type_var).ok_or(())?;
            let (lower, upper) = if c.op == crate::constraints::SUBTYPE_OF {
                (origin_id, target_id)
            } else {
                (target_id, origin_id)
            };
            if graph.contains(&(lower.clone(), upper.clone())) {
                continue;
            }
            // graph |= {(l,u) for l in tvars for u in tvars if (l,lower) in graph and
            // (upper,u) in graph}
            let mut new_pairs = Vec::new();
            for l in tvars {
                for u in tvars {
                    if graph.contains(&(l.clone(), lower.clone()))
                        && graph.contains(&(upper.clone(), u.clone()))
                    {
                        new_pairs.push((l.clone(), u.clone()));
                    }
                }
            }
            graph.extend(new_pairs);
            // for u in tvars: if (upper,u) in graph: lowers[u] |= lowers[lower]
            let lower_bounds = lowers.get(&lower).cloned().unwrap_or_default();
            for u in tvars {
                if graph.contains(&(upper.clone(), u.clone())) {
                    for b in &lower_bounds {
                        let entry = lowers.entry(u.clone()).or_default();
                        push_unique(entry, b.clone());
                    }
                }
            }
            // for l in tvars: if (l,lower) in graph: uppers[l] |= uppers[upper]
            let upper_bounds = uppers.get(&upper).cloned().unwrap_or_default();
            for l in tvars {
                if graph.contains(&(l.clone(), lower.clone())) {
                    for b in &upper_bounds {
                        let entry = uppers.entry(l.clone()).or_default();
                        push_unique(entry, b.clone());
                    }
                }
            }
            // for lt in lowers[lower]: for ut in uppers[upper]: add_secondary
            for lt in &lower_bounds {
                for ut in &upper_bounds {
                    add_secondary_constraints(&mut remaining, lt, ut, resolver, strict_optional)?;
                }
            }
        } else if c.op == crate::constraints::SUBTYPE_OF {
            let cv = tv_id(&c.origin_type_var).ok_or(())?;
            if uppers
                .get(&cv)
                .is_some_and(|bounds| bounds.contains(&c.target))
            {
                continue;
            }
            for l in tvars {
                if graph.contains(&(l.clone(), cv.clone())) {
                    let entry = uppers.entry(l.clone()).or_default();
                    push_unique(entry, c.target.clone());
                }
            }
            for lt in lowers.get(&cv).cloned().unwrap_or_default() {
                add_secondary_constraints(
                    &mut remaining,
                    &lt,
                    &c.target,
                    resolver,
                    strict_optional,
                )?;
            }
        } else {
            // c.op == SUPERTYPE_OF
            let cv = tv_id(&c.origin_type_var).ok_or(())?;
            if lowers
                .get(&cv)
                .is_some_and(|bounds| bounds.contains(&c.target))
            {
                continue;
            }
            for u in tvars {
                if graph.contains(&(cv.clone(), u.clone())) {
                    let entry = lowers.entry(u.clone()).or_default();
                    push_unique(entry, c.target.clone());
                }
            }
            for ut in uppers.get(&cv).cloned().unwrap_or_default() {
                add_secondary_constraints(
                    &mut remaining,
                    &c.target,
                    &ut,
                    resolver,
                    strict_optional,
                )?;
            }
        }
    }
    Ok((graph, lowers, uppers))
}

/// `compute_dependencies` (solve.py:639-660).
fn compute_dependencies(
    tvars: &[TvId],
    graph: &HashSet<(TvId, TvId)>,
    lowers: &HashMap<TvId, Vec<Type>>,
    uppers: &HashMap<TvId, Vec<Type>>,
) -> HashMap<TvId, Vec<TvId>> {
    let tvars_set: HashSet<TvId> = tvars.iter().cloned().collect();
    let mut res: HashMap<TvId, Vec<TvId>> = HashMap::new();
    for tv in tvars {
        let mut deps: HashSet<TvId> = HashSet::new();
        if let Some(bs) = lowers.get(tv) {
            for lt in bs {
                deps.extend(get_vars(lt, &tvars_set));
            }
        }
        if let Some(bs) = uppers.get(tv) {
            for ut in bs {
                deps.extend(get_vars(ut, &tvars_set));
            }
        }
        for other in tvars {
            if other == tv {
                continue;
            }
            if graph.contains(&(tv.clone(), other.clone()))
                || graph.contains(&(other.clone(), tv.clone()))
            {
                deps.insert(other.clone());
            }
        }
        res.insert(tv.clone(), deps.into_iter().collect());
    }
    res
}

/// `check_linear` (solve.py:663-673).
fn check_linear(
    scc: &BTreeSet<TvId>,
    lowers: &HashMap<TvId, Vec<Type>>,
    uppers: &HashMap<TvId, Vec<Type>>,
) -> bool {
    // `get_vars` needs a HashSet; collect the SCC ids once.
    let vars_set: HashSet<TvId> = scc.iter().cloned().collect();
    for tv in scc {
        if let Some(bs) = lowers.get(tv) {
            for lt in bs {
                if !get_vars(lt, &vars_set).is_empty() {
                    return false;
                }
            }
        }
        if let Some(bs) = uppers.get(tv) {
            for ut in bs {
                if !get_vars(ut, &vars_set).is_empty() {
                    return false;
                }
            }
        }
    }
    true
}

/// Tarjan's strongly connected components, mirroring the iterative
/// recipe in `graph_utils.py:11-54` (`index[v] = len(stack)`, with the
/// boundary-pop loop comparing against `boundaries[-1]`). Edges missing
/// from the map are empty (KeyError would be equivalent).
fn strongly_connected_components(
    vertices: HashSet<TvId>,
    edges: &HashMap<TvId, Vec<TvId>>,
) -> Vec<BTreeSet<TvId>> {
    let mut identified: HashSet<TvId> = HashSet::new();
    let mut stack: Vec<TvId> = Vec::new();
    let mut index: HashMap<TvId, usize> = HashMap::new();
    let mut boundaries: Vec<usize> = Vec::new();
    let mut result: Vec<BTreeSet<TvId>> = Vec::new();

    fn dfs(
        v: &TvId,
        index: &mut HashMap<TvId, usize>,
        boundaries: &mut Vec<usize>,
        stack: &mut Vec<TvId>,
        identified: &mut HashSet<TvId>,
        edges: &HashMap<TvId, Vec<TvId>>,
        result: &mut Vec<BTreeSet<TvId>>,
    ) {
        let idx = stack.len();
        index.insert(v.clone(), idx);
        stack.push(v.clone());
        boundaries.push(idx);

        let edge_list: Vec<TvId> = edges.get(v).cloned().unwrap_or_default();
        for w in &edge_list {
            if !index.contains_key(w) {
                dfs(w, index, boundaries, stack, identified, edges, result);
            } else if !identified.contains(w) {
                while index[w] < *boundaries.last().unwrap() {
                    boundaries.pop();
                }
            }
        }

        if *boundaries.last().unwrap() == index[v] {
            boundaries.pop();
            let mut scc: BTreeSet<TvId> = BTreeSet::new();
            while let Some(x) = stack.pop() {
                identified.insert(x.clone());
                let is_v = x == *v;
                scc.insert(x);
                if is_v {
                    break;
                }
            }
            result.push(scc);
        }
    }

    let vlist: Vec<TvId> = vertices.into_iter().collect();
    for v in vlist {
        if !index.contains_key(&v) {
            dfs(
                &v,
                &mut index,
                &mut boundaries,
                &mut stack,
                &mut identified,
                edges,
                &mut result,
            );
        }
    }
    result
}

/// `prepare_sccs` (graph_utils.py:57-72).
fn prepare_sccs(
    sccs: Vec<BTreeSet<TvId>>,
    edges: &HashMap<TvId, Vec<TvId>>,
) -> HashMap<BTreeSet<TvId>, BTreeSet<BTreeSet<TvId>>> {
    let mut sccsmap: HashMap<TvId, BTreeSet<TvId>> = HashMap::new();
    for scc in &sccs {
        for v in scc {
            sccsmap.insert(v.clone(), scc.clone());
        }
    }
    let mut data: HashMap<BTreeSet<TvId>, BTreeSet<BTreeSet<TvId>>> = HashMap::new();
    for scc in &sccs {
        let mut deps: BTreeSet<BTreeSet<TvId>> = BTreeSet::new();
        for v in scc {
            if let Some(es) = edges.get(v) {
                for x in es {
                    if let Some(s) = sccsmap.get(x) {
                        deps.insert(s.clone());
                    }
                }
            }
        }
        data.insert(scc.clone(), deps);
    }
    data
}

/// Kahn topological sort over a DAG of SCCs (graph_utils.py:75-161).
/// Returns one batch per level; each batch is the set of SCCs ready at
/// that point. `Err(())` on a cycle (Python asserts; should never fire
/// for real input).
fn topsort(
    data: &HashMap<BTreeSet<TvId>, BTreeSet<BTreeSet<TvId>>>,
) -> Result<Vec<Vec<BTreeSet<TvId>>>, ()> {
    // Remove self-deps (Python mutates `deps.discard(item)` in place).
    let mut deps_map: HashMap<BTreeSet<TvId>, BTreeSet<BTreeSet<TvId>>> = HashMap::new();
    for (item, deps) in data {
        let mut d = deps.clone();
        d.remove(item);
        deps_map.insert(item.clone(), d);
    }
    let mut rev: HashMap<BTreeSet<TvId>, Vec<BTreeSet<TvId>>> = HashMap::new();
    let mut in_degree: HashMap<BTreeSet<TvId>, usize> = HashMap::new();
    let mut ready: BTreeSet<BTreeSet<TvId>> = BTreeSet::new();

    for (item, deps) in &deps_map {
        let deg = deps.len();
        in_degree.insert(item.clone(), deg);
        if deg == 0 {
            ready.insert(item.clone());
        }
        rev.entry(item.clone()).or_default();
        for dep in deps {
            rev.entry(dep.clone()).or_default().push(item.clone());
            if !data.contains_key(dep) {
                // Orphan: appears as dependency but has no entry in data.
                in_degree.entry(dep.clone()).or_insert(0);
                ready.insert(dep.clone());
            }
        }
    }

    let mut remaining: usize = in_degree.len() - ready.len();
    let mut levels: Vec<Vec<BTreeSet<TvId>>> = Vec::new();

    loop {
        if ready.is_empty() {
            if remaining != 0 {
                return Err(());
            }
            break;
        }
        let level: Vec<BTreeSet<TvId>> = ready.iter().cloned().collect();
        levels.push(level);
        let mut new_ready: BTreeSet<BTreeSet<TvId>> = BTreeSet::new();
        for item in &ready {
            if let Some(dependents) = rev.get(item) {
                for dependent in dependents {
                    let nd = in_degree.get_mut(dependent).unwrap();
                    *nd -= 1;
                    if *nd == 0 {
                        new_ready.insert(dependent.clone());
                    }
                }
            }
        }
        remaining -= new_ready.len();
        ready = new_ready;
    }
    Ok(levels)
}

/// `choose_free` (solve.py:477-525), singleton fast path only. Larger SCCs
/// defer to Python (meet folding is order-sensitive and can leak Anys).
fn choose_free_single(scc: &BTreeSet<TvId>) -> Option<TvId> {
    if scc.len() == 1 {
        Some(scc.iter().next().unwrap().clone())
    } else {
        None
    }
}

/// Check a candidate `Type` for structures that cannot round-trip the wire
/// without breaking identity or leaking illegal Any places.
fn wire_unsafe_solution(typ: &Type) -> bool {
    match typ {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            true
        }
        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name: _,
        } => {
            // type_of_any 0 is illegal on the wire (TypeOfAny starts at 1);
            // defer so Python rebuilds a valid Any. Also defer Anys that
            // own a source (from_another_any identity cannot survive).
            *type_of_any == 0 || (*type_of_any == 1 && source_any.is_some())
        }
        Type::UnionType { items, .. } => items.iter().any(wire_unsafe_solution),
        Type::Instance { args, .. } => args.iter().any(wire_unsafe_solution),
        _ => false,
    }
}

/// `solve_one` (solve.py:367-474), single-variable best-type computation
/// via `solve_one_inner`, plus identity-preserving defer checks. Returns
/// `Ok(Some(Type))` solved, `Ok(None)` no solution, or `Err(())` defer.
fn solve_one_for_dependent(
    lowers: &[Type],
    uppers: &[Type],
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<Option<Type>, ()> {
    // Filter ambiguous UninhabitedType uppers (solve.py:372-378).
    let filtered_uppers: Vec<Type> = uppers
        .iter()
        .filter(|u| !matches!(u, Type::UninhabitedType { ambiguous: true }))
        .cloned()
        .collect();

    // No bounds after the filter: ambiguous Never (solve.py:381-385).
    if lowers.is_empty() && filtered_uppers.is_empty() {
        return Ok(Some(Type::UninhabitedType { ambiguous: true }));
    }

    // Single-bound no-op solves return the bound itself: Python's raw
    // UnionType.make_union(lowers) / single-upper top (solve.py:587-610)
    // mirror solve_one_inner exactly, so no special-casing is needed.

    // Identity-bearing bounds: solving while a bound still holds a live
    // TypeVar would leak it into a candidate.
    if lowers
        .iter()
        .chain(filtered_uppers.iter())
        .any(crate::visitor::has_type_vars_inner)
    {
        return Err(());
    }

    let out = match solve_one_inner(
        lowers,
        &filtered_uppers,
        infer_unions,
        strict_optional,
        resolver,
    ) {
        Some(o) => o,
        None => return Err(()),
    };
    match out {
        (0, Some(bytes)) | (1, Some(bytes)) | (3, Some(bytes)) => {
            let typ = decode_type(&bytes).ok_or(())?;
            if wire_unsafe_solution(&typ) {
                return Err(());
            }
            Ok(Some(typ))
        }
        // No-blob kinds: no solution (Python returns None).
        (0, None) | (1, None) => Ok(None),
        // kind=2: ambiguous Never.
        _ => Ok(Some(Type::UninhabitedType { ambiguous: true })),
    }
}

/// `solve_iteratively` (solve.py:295-352). Solves one batch and returns
/// the per-var solutions. Returns `Err(())` to defer.
fn solve_iteratively_native(
    batch: &[TvId],
    graph: &mut HashSet<(TvId, TvId)>,
    lowers: &mut HashMap<TvId, Vec<Type>>,
    uppers: &mut HashMap<TvId, Vec<Type>>,
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<Vec<(TvId, Option<Type>)>, ()> {
    let mut solutions: Vec<(TvId, Option<Type>)> = Vec::new();
    // s_batch ordered by raw_id only (solve.py:314 sorts by `.raw_id`);
    // select the first var that has any bound.
    let mut s_batch: Vec<TvId> = batch.to_vec();
    loop {
        s_batch.sort_by_key(|(raw, _, _)| *raw);
        let mut solvable: Option<usize> = None;
        for (i, tv) in s_batch.iter().enumerate() {
            let has = lowers.get(tv).is_some_and(|b| !b.is_empty())
                || uppers.get(tv).is_some_and(|b| !b.is_empty());
            if has {
                solvable = Some(i);
                break;
            }
        }
        let Some(idx) = solvable else { break };
        let solvable_tv = s_batch.remove(idx);

        let lo = lowers.get(&solvable_tv).cloned().unwrap_or_default();
        let up = uppers.get(&solvable_tv).cloned().unwrap_or_default();
        let result = solve_one_for_dependent(&lo, &up, infer_unions, strict_optional, resolver)?;
        solutions.push((solvable_tv.clone(), result.clone()));
        let Some(result) = result else {
            continue;
        };

        // Move the solved var's graph edges into bounds (solve.py:334-342).
        let edges: Vec<(TvId, TvId)> = graph.clone().into_iter().collect();
        for (l, u) in edges {
            if l == u {
                continue;
            }
            if l == solvable_tv {
                lowers.entry(u.clone()).or_default().push(result.clone());
                graph.remove(&(l.clone(), u.clone()));
            }
            if u == solvable_tv {
                uppers.entry(l.clone()).or_default().push(result.clone());
                graph.remove(&(l.clone(), u.clone()));
            }
        }
    }

    // Expand (transitive) bounds after the batch (solve.py:347-351).
    let subs: HashMap<TvId, Type> = solutions
        .iter()
        .filter_map(|(tv, s)| s.clone().map(|s| (tv.clone(), s)))
        .collect();
    for tv in lowers.keys().cloned().collect::<Vec<_>>() {
        let bs = lowers.remove(&tv).unwrap_or_default();
        let mut new_bs = Vec::new();
        for lt in bs {
            let e = crate::expandtype::expand_type_inner(&lt, &subs, strict_optional).ok_or(())?;
            push_unique(&mut new_bs, e);
        }
        lowers.insert(tv, new_bs);
    }
    for tv in uppers.keys().cloned().collect::<Vec<_>>() {
        let bs = uppers.remove(&tv).unwrap_or_default();
        let mut new_bs = Vec::new();
        for ut in bs {
            let e = crate::expandtype::expand_type_inner(&ut, &subs, strict_optional).ok_or(())?;
            push_unique(&mut new_bs, e);
        }
        uppers.insert(tv, new_bs);
    }
    Ok(solutions)
}

/// `solve_with_dependent` (solve.py:234-292), Rust subset.
///
/// `Err(())` = defer to Python. `Ok(Some((None, None)))` = empty
/// solutions `({}, [])`. `Ok(Some((Some(sol_blob), Some(free_blob))))`
/// = native solution.
fn solve_with_dependent_native(
    vars: &[Type],
    constraints: &[Constraint],
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<NativeDependentOut, ()> {
    let tvars: Vec<TvId> = vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()?;
    let (mut graph, mut lowers, mut uppers) =
        transitive_closure(&tvars, constraints, resolver, strict_optional)?;
    let dmap = compute_dependencies(&tvars, &graph, &lowers, &uppers);

    let vertices: HashSet<TvId> = tvars.iter().cloned().collect();
    let sccs = strongly_connected_components(vertices, &dmap);
    if !sccs.iter().all(|s| check_linear(s, &lowers, &uppers)) {
        // Python returns the literal `({}, [])` (solve.py:253-254).
        return Ok(Some((None, None)));
    }
    let data = prepare_sccs(sccs, &dmap);
    let raw_batches = match topsort(&data) {
        Ok(b) => b,
        Err(()) => return Err(()),
    };

    // Free-variable pass over the first (leaf) batch only (solve.py:257-269).
    let mut free_vars: Vec<TvId> = Vec::new();
    let mut free_solutions: HashMap<TvId, Type> = HashMap::new();
    if let Some(first) = raw_batches.first() {
        for scc in first {
            let all_empty = scc.iter().all(|tv| {
                lowers.get(tv).is_none_or(|b| b.is_empty())
                    && uppers.get(tv).is_none_or(|b| b.is_empty())
            });
            if all_empty {
                match choose_free_single(scc) {
                    Some(id) => {
                        let orig = vars
                            .iter()
                            .find(|t| tv_id(t) == Some(id.clone()))
                            .cloned()
                            .ok_or(())?;
                        free_vars.push(id.clone());
                        free_solutions.insert(id.clone(), orig);
                    }
                    None => return Err(()),
                }
            }
        }
    }

    // Update lowers/uppers with free vars (solve.py:271-277).
    for (l, u) in graph.clone() {
        if free_vars.contains(&l) {
            lowers
                .entry(u.clone())
                .or_default()
                .push(free_solutions.get(&l).unwrap().clone());
        }
        if free_vars.contains(&u) {
            uppers
                .entry(l.clone())
                .or_default()
                .push(free_solutions.get(&u).unwrap().clone());
        }
    }

    // Flatten SCC batches (solve.py:279-286).
    let mut solutions: Vec<(TvId, Option<Type>)> = Vec::new();
    for level in &raw_batches {
        let flat: Vec<TvId> = level.iter().flat_map(|s| s.iter().cloned()).collect();
        match solve_iteratively_native(
            &flat,
            &mut graph,
            &mut lowers,
            &mut uppers,
            infer_unions,
            strict_optional,
            resolver,
        ) {
            Ok(res) => solutions.extend(res),
            Err(()) => return Err(()),
        }
    }
    let sol_blob = encode_solutions_blob(&solutions)?;
    let free_blob = encode_free_vars_blob(&free_vars)?;
    Ok(Some((Some(sol_blob), Some(free_blob))))
}

/// `encode_solutions_blob`: `(raw, meta, ns, has_sol, type?)...`. Uses
/// tagged `write_int`/`write_str` (no `write_bool`).
fn encode_solutions_blob(solutions: &[(TvId, Option<Type>)]) -> Result<Vec<u8>, ()> {
    let mut buf = WriteBuffer::new();
    wire::write_int(&mut buf, solutions.len() as i64).map_err(|_| ())?;
    for (tv, sol) in solutions {
        let (raw, meta, ns) = tv;
        wire::write_int(&mut buf, *raw).map_err(|_| ())?;
        wire::write_int(&mut buf, *meta).map_err(|_| ())?;
        wire::write_str(&mut buf, ns).map_err(|_| ())?;
        match sol {
            Some(t) => {
                wire::write_int(&mut buf, 1).map_err(|_| ())?;
                wire::write_type(&mut buf, t).map_err(|_| ())?;
            }
            None => wire::write_int(&mut buf, 0).map_err(|_| ())?,
        }
    }
    Ok(buf.into_bytes())
}

/// `encode_free_vars_blob`: `(raw, meta, ns, 0)...` (is_modified=0).
fn encode_free_vars_blob(free_vars: &[TvId]) -> Result<Vec<u8>, ()> {
    let mut buf = WriteBuffer::new();
    wire::write_int(&mut buf, free_vars.len() as i64).map_err(|_| ())?;
    for (raw, meta, ns) in free_vars {
        wire::write_int(&mut buf, *raw).map_err(|_| ())?;
        wire::write_int(&mut buf, *meta).map_err(|_| ())?;
        wire::write_str(&mut buf, ns).map_err(|_| ())?;
        wire::write_int(&mut buf, 0).map_err(|_| ())?;
    }
    Ok(buf.into_bytes())
}

/// `#[pyfunction]` entry for `solve_with_dependent`.
///
/// `vars` are serialized `TypeVarLikeType` blobs (their ids are the
/// variables to solve, including `extra_tvars`); `constraints` are
/// serialized `Constraint` blobs. Returns `None` to defer the whole call
/// to Python; otherwise `(num_solved, sol_blob, free_blob)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_solve_dependent(
    vars: Vec<Vec<u8>>,
    constraints: Vec<Vec<u8>>,
    infer_unions: bool,
    strict_optional: bool,
    resolver: &NativeTypeResolver,
) -> SolveDependentOut {
    let mut var_types: Vec<Type> = Vec::with_capacity(vars.len());
    for b in &vars {
        var_types.push(decode_type(b)?);
    }
    let mut dep_constraints: Vec<Constraint> = Vec::with_capacity(constraints.len());
    for b in &constraints {
        let mut buf = ReadBuffer::new(b);
        match crate::constraints::Constraint::read(&mut buf) {
            Ok(c) => dep_constraints.push(c),
            Err(_) => return None,
        }
    }
    let out = match solve_with_dependent_native(
        &var_types,
        &dep_constraints,
        infer_unions,
        strict_optional,
        resolver.resolver(),
    ) {
        Ok(o) => o,
        Err(()) => return None,
    };
    match out {
        // Empty solutions `({}, [])`: signal 0 entries.
        Some((None, None)) => {
            let mut empty = WriteBuffer::new();
            wire::write_int(&mut empty, 0).ok()?;
            let b = empty.into_bytes();
            Some((0, Some(b.clone()), Some(b)))
        }
        Some((sol, free)) => Some((0, sol, free)),
        None => None,
    }
}

/// `upper_bound_of`: extract the `upper_bound` field of a TypeVar-like
/// wire type (all three variants carry one).
fn upper_bound_of(t: &Type) -> Option<Type> {
    match t {
        Type::TypeVarType { upper_bound, .. }
        | Type::ParamSpecType { upper_bound, .. }
        | Type::TypeVarTupleType { upper_bound, .. } => Some((**upper_bound).clone()),
        _ => None,
    }
}

/// `is_callable_protocol` (solve.py:832-836): an `Instance` whose type
/// info is a protocol exposing `__call__`.
fn is_callable_protocol(t: &Type, resolver: &crate::typeinfo::TypeResolver) -> bool {
    if let Type::Instance { type_ref, .. } = t {
        resolver
            .get(type_ref)
            .is_some_and(|s| s.is_protocol && s.protocol_members.iter().any(|m| m == "__call__"))
    } else {
        false
    }
}

/// Non-polymorphic `solve_constraints` (solve.py:263-296) in Rust: cmap
/// grouping, per-var `solve_one`, extra-var leak check, ordered `res`
/// assembly with strict/Any defaults, and `pre_validate_solutions`.
/// Returns `(sol_blob, free_blob)`; free is always empty. `Err(())`
/// defers the whole call to Python.
#[allow(clippy::too_many_arguments)]
fn solve_constraints_native(
    vars: &[Type],
    original_vars: &[Type],
    constraints: &[Constraint],
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
    skip_unsatisfied: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<(Vec<u8>, Vec<u8>), ()> {
    let original_ids: HashSet<TvId> = original_vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()?;
    let all_ids: HashSet<TvId> = vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()?;
    let extra_ids: HashSet<TvId> = all_ids.difference(&original_ids).cloned().collect();

    // cmap (solve.py:249-251): constraints grouped by origin var id; only
    // ids in the solving set participate.
    let mut cmap: HashMap<TvId, Vec<Constraint>> = HashMap::new();
    for c in constraints {
        if let Some(k) = tv_id(&c.origin_type_var) {
            if all_ids.contains(&k) {
                cmap.entry(k).or_default().push(c.clone());
            }
        }
    }

    // Per-var solve (solve.py:265-275). Each solve is independent, so
    // cmap iteration order does not affect the result.
    let mut solutions: Vec<(TvId, Option<Type>)> = Vec::new();
    for (tv, cs) in &cmap {
        if cs.is_empty() {
            continue;
        }
        let lowers: Vec<Type> = cs
            .iter()
            .filter(|c| c.op == crate::constraints::SUPERTYPE_OF)
            .map(|c| c.target.clone())
            .collect();
        let uppers: Vec<Type> = cs
            .iter()
            .filter(|c| c.op == crate::constraints::SUBTYPE_OF)
            .map(|c| c.target.clone())
            .collect();
        // Filter ambiguous UninhabitedType uppers (solve.py:462-467).
        let filtered_uppers: Vec<Type> = uppers
            .iter()
            .filter(|u| !matches!(u, Type::UninhabitedType { ambiguous: true }))
            .cloned()
            .collect();

        let candidate: Option<Type> = if lowers.is_empty() && filtered_uppers.is_empty() {
            // No usable bounds: ambiguous Never (solve.py:470-474).
            Some(Type::UninhabitedType { ambiguous: true })
        } else {
            let out = match solve_one_inner(
                &lowers,
                &filtered_uppers,
                infer_unions,
                strict_optional,
                resolver,
            ) {
                Some(o) => o,
                None => {
                    return Err(());
                }
            };
            match out {
                (0, Some(bytes)) | (1, Some(bytes)) | (3, Some(bytes)) => {
                    let typ = decode_type(&bytes).ok_or(())?;
                    if wire_unsafe_solution(&typ) {
                        return Err(());
                    }
                    Some(typ)
                }
                // No-blob kinds: no solution (Python returns None).
                (0, None) | (1, None) => None,
                // kind=2: ambiguous Never.
                _ => Some(Type::UninhabitedType { ambiguous: true }),
            }
        };

        // Do not leak extra-var ids into non-polymorphic solutions
        // (solve.py:272-275); leak-skips record nothing.
        if let Some(c) = &candidate {
            if !get_vars(c, &extra_ids).is_empty() {
                continue;
            }
        }
        solutions.push((tv.clone(), candidate));
    }

    // Ordered `res` assembly over `vars` only (solve.py:277-289).
    let mut ordered: Vec<(TvId, Option<Type>)> = Vec::with_capacity(original_vars.len());
    for t in original_vars {
        let tv = tv_id(t).ok_or(())?;
        match solutions.iter().find(|(k, _)| *k == tv) {
            Some((_, sol)) => ordered.push((tv.clone(), sol.clone())),
            None => {
                // Unconstrained or leak-skipped: strict Never / lax Any.
                // `TypeOfAny.special_form` = 6.
                let cand = if strict {
                    Type::UninhabitedType { ambiguous: true }
                } else {
                    Type::AnyType {
                        type_of_any: 6,
                        source_any: None,
                        missing_import_name: None,
                    }
                };
                ordered.push((tv.clone(), Some(cand)));
            }
        }
    }

    // pre_validate_solutions (solve.py:291-295, 799-829): replace a
    // solution that violates its var's upper bound with the bound itself
    // when the bound satisfies every constraint.
    if !skip_unsatisfied {
        let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
        let mut validated: Vec<(TvId, Option<Type>)> = Vec::with_capacity(ordered.len());
        for (t, s) in original_vars.iter().zip(ordered.iter()) {
            let tv = tv_id(t).ok_or(())?;
            let ub = upper_bound_of(t).ok_or(())?;
            if is_callable_protocol(&ub, resolver) {
                validated.push((tv, s.1.clone()));
                continue;
            }
            if let Some(sol) = &s.1 {
                if !subtypes::is_subtype(sol, &ub, &ctx, resolver).ok_or(())? {
                    let mut bound_satisfies_all = true;
                    for c in constraints {
                        if c.op == crate::constraints::SUBTYPE_OF
                            && !subtypes::is_subtype(&ub, &c.target, &ctx, resolver).ok_or(())?
                        {
                            bound_satisfies_all = false;
                            break;
                        }
                        if c.op == crate::constraints::SUPERTYPE_OF
                            && !subtypes::is_subtype(&c.target, &ub, &ctx, resolver).ok_or(())?
                        {
                            bound_satisfies_all = false;
                            break;
                        }
                    }
                    if bound_satisfies_all {
                        validated.push((tv, Some(ub)));
                        continue;
                    }
                }
            }
            validated.push((tv, s.1.clone()));
        }
        ordered = validated;
    }

    let sol_blob = encode_solutions_blob(&ordered)?;
    let free_blob = encode_free_vars_blob(&[])?;
    Ok((sol_blob, free_blob))
}

/// `#[pyfunction]` entry for the non-polymorphic `solve_constraints`.
///
/// `vars` are serialized originals for `vars + extra_vars`; `original_vars`
/// for `vars` only (the ordered positions returned in `res`); `constraints`
/// serialized `Constraint` blobs. Returns `None` to defer the whole call
/// to Python; otherwise `(num_solved, sol_blob, free_blob)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_solve_constraints(
    vars: Vec<Vec<u8>>,
    original_vars: Vec<Vec<u8>>,
    constraints: Vec<Vec<u8>>,
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
    skip_unsatisfied: bool,
    resolver: &NativeTypeResolver,
) -> SolveDependentOut {
    let mut var_types: Vec<Type> = Vec::with_capacity(vars.len());
    for b in &vars {
        var_types.push(decode_type(b)?);
    }
    let mut orig_types: Vec<Type> = Vec::with_capacity(original_vars.len());
    for b in &original_vars {
        orig_types.push(decode_type(b)?);
    }
    let mut con_list: Vec<Constraint> = Vec::with_capacity(constraints.len());
    for b in &constraints {
        let mut buf = ReadBuffer::new(b);
        con_list.push(crate::constraints::Constraint::read(&mut buf).ok()?);
    }
    let out = match solve_constraints_native(
        &var_types,
        &orig_types,
        &con_list,
        strict,
        infer_unions,
        strict_optional,
        skip_unsatisfied,
        resolver.resolver(),
    ) {
        Ok(o) => o,
        Err(()) => return None,
    };
    Some((0, Some(out.0), Some(out.1)))
}

/// Stage 20 (#382): pass-1 generic-call type-argument inference.
///
/// Ports `infer_function_type_arguments` (infer.py:27-64) for the
/// first pass: `infer_constraints_for_callable` (constraints.py:346-512,
/// minus ParamSpec/TypeVarTuple/UnpackType branches) followed by
/// non-polymorphic `solve_constraints`. Returns an optional-type list
/// blob (`count` + per-var `0|1 + Type`) in `callee.variables` order,
/// or `None` to defer the whole call to Python.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_infer_function_type_arguments(
    _py: Python<'_>,
    resolver: &NativeTypeResolver,
    callee_bytes: &[u8],
    arg_types: Vec<Option<Vec<u8>>>,
    arg_kinds: Vec<i64>,
    formal_to_actual: Vec<Vec<i64>>,
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = wire::read_type(&mut buf, None).ok()?;
    let Type::CallableType {
        arg_types: formal_types,
        arg_kinds: formal_kinds,
        arg_names: formal_names,
        variables,
        ..
    } = &callee
    else {
        return None;
    };
    // ParamSpec/TypeVarTuple variables use the deferred constraint paths
    // (constraints.py:475-494, filter_imprecise_kinds): defer.
    if variables.iter().any(|v| {
        matches!(
            v,
            Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
        )
    }) {
        return None;
    }
    // UnpackType formals use the star-unpack branch (constraints.py:388-438): defer.
    if formal_types
        .iter()
        .any(|t| matches!(t, Type::UnpackType { .. }))
    {
        return None;
    }
    let vars_types = variables.clone();
    let mut arg_types_vec: Vec<Option<Type>> = Vec::with_capacity(arg_types.len());
    for b in &arg_types {
        match b {
            None => arg_types_vec.push(None),
            Some(b2) => {
                let mut b3 = ReadBuffer::new(b2);
                arg_types_vec.push(Some(wire::read_type(&mut b3, None).ok()?));
            }
        }
    }
    let mut tuple_index: i64 = 0;
    let mut kwargs_used: Option<Vec<String>> = None;
    let mut constraints: Vec<Constraint> = Vec::new();
    for (i, actuals) in formal_to_actual.iter().enumerate() {
        let formal_type = formal_types.get(i)?;
        let formal_kind = *formal_kinds.get(i)?;
        let formal_name = formal_names.get(i).and_then(|o| o.as_deref());
        for &ai in actuals {
            let actual_arg = match arg_types_vec.get(ai as usize) {
                Some(Some(t)) => t,
                _ => continue, // None actual (deferred pass) or OOB.
            };
            let actual_kind = *arg_kinds.get(ai as usize)?;
            let expanded = expand_actual_arg(
                &mut tuple_index,
                &mut kwargs_used,
                actual_arg,
                actual_kind,
                formal_name,
                formal_kind,
            )?;
            constraints.extend(crate::constraints::infer_constraints_full_inner(
                formal_type,
                &expanded,
                crate::constraints::SUPERTYPE_OF,
                resolver.resolver(),
                resolver.alias_resolver(),
                strict_optional,
            )?);
        }
    }
    // Empty constraints fall through to `solve_constraints_native`:
    // Python has no fast path either; its empty-cmap fill yields strict
    // Never / lax Any defaults per variable, which native solve mirrors.
    let (sol_blob, _free_blob) = solve_constraints_native(
        &vars_types,
        &vars_types,
        &constraints,
        strict,
        infer_unions,
        strict_optional,
        false,
        resolver.resolver(),
    )
    .ok()?;
    // Re-encode in `variables` order as an optional-type list. The Python
    // shim decodes this with `read_int` + `read_type` (count per var).
    let solutions = decode_solve_solutions_here(&sol_blob)?;
    let tvids: Vec<TvId> = vars_types
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()
        .ok()?;
    let mut out = WriteBuffer::new();
    wire::write_int(&mut out, tvids.len() as i64).ok()?;
    for tv in &tvids {
        match solutions.iter().find(|(k, _)| k == tv) {
            Some((_, Some(t))) => {
                out.push(1);
                wire::write_type(&mut out, t).ok()?;
            }
            _ => out.push(0),
        }
    }
    Some(out.into_bytes())
}

/// One arg-expansion pass of `ArgTypeExpander.expand_actual_type`
/// (argmap.py:269-364), returning the expanded `Type` directly. Branches
/// that need `is_subtype` (Iterable/Mapping unpacking, TypeVarTuple upper
/// bounds) or arbitrary-key TypedDict popping return `None` so the whole
/// call defers to Python.
fn expand_actual_arg(
    tuple_index: &mut i64,
    kwargs_used: &mut Option<Vec<String>>,
    actual: &Type,
    actual_kind: i64,
    formal_name: Option<&str>,
    formal_kind: i64,
) -> Option<Type> {
    // `get_proper_type`: wire has no alias target, defer.
    if matches!(
        actual,
        Type::TypeAliasType { .. } | Type::UnboundType { .. }
    ) {
        return None;
    }
    match actual_kind {
        2 => match actual {
            Type::TupleType { items, .. } => {
                let len = items.len() as i64;
                *tuple_index = if *tuple_index >= len {
                    1
                } else {
                    *tuple_index + 1
                };
                let mut item = items.get((*tuple_index - 1) as usize)?.clone();
                if let Type::UnpackType { typ: inner } = &item {
                    let unpacked = match inner.as_ref() {
                        Type::TypeVarTupleType { upper_bound, .. } => upper_bound.as_ref(),
                        other => other,
                    };
                    let Type::Instance { type_ref, args, .. } = unpacked else {
                        return None;
                    };
                    if type_ref != "builtins.tuple" {
                        return None;
                    }
                    item = args.first()?.clone();
                }
                Some(item)
            }
            Type::ParamSpecType { .. } => Some(actual.clone()),
            // Iterable unpacking / TypeVarTuple upper bound: defer.
            Type::Instance { .. } | Type::TypeVarTupleType { .. } => None,
            _ => Some(Type::AnyType {
                type_of_any: 2, // TypeOfAny.from_error
                source_any: None,
                missing_import_name: None,
            }),
        },
        4 => match actual {
            Type::TypedDictType { items, .. } => {
                if formal_kind == 4 {
                    return None;
                }
                let chosen = formal_name?;
                if !items.iter().any(|(k, _)| k == chosen) {
                    return None;
                }
                let used = kwargs_used.get_or_insert_with(Vec::new);
                if !used.iter().any(|k| k == chosen) {
                    used.push(chosen.to_string());
                }
                Some(items.iter().find(|(k, _)| k == chosen)?.1.clone())
            }
            Type::ParamSpecType { .. } => Some(actual.clone()),
            // Mapping unpacking: defer.
            Type::Instance { .. } => None,
            _ => Some(Type::AnyType {
                type_of_any: 2,
                source_any: None,
                missing_import_name: None,
            }),
        },
        // No translation for other kinds: 1:1 mapping.
        _ => Some(actual.clone()),
    }
}

/// `decode_solve_solutions` (checkcall.py:614) — local copy.
#[allow(clippy::type_complexity)]
fn decode_solve_solutions_here(blob: &[u8]) -> Option<Vec<((i64, i64, String), Option<Type>)>> {
    let mut buf = ReadBuffer::new(blob);
    let count = wire::read_int(&mut buf).ok()?;
    let mut result = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let raw = wire::read_int(&mut buf).ok()?;
        let meta = wire::read_int(&mut buf).ok()?;
        let ns = wire::read_str(&mut buf).ok()?;
        let has_sol = wire::read_int(&mut buf).ok()?;
        let typ = if has_sol == 1 {
            Some(wire::read_type(&mut buf, None).ok()?)
        } else {
            None
        };
        result.push(((raw, meta, ns), typ));
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// Standalone PyO3 exports for pure-computation helpers (solve.py).

// These mirror small pure functions that take `Type` objects and return
// values without mutation or side effects. Each returns `None` to defer
// to the Python fallback for any unhandled type variant.

// ---------------------------------------------------------------------------

/// `_join_sorted_key` (solve.py:488-497): sort key for `join_type_list`.
/// UnionType=-2, NoneType=-1, Overloaded=1, else 0.
/// Unwraps TypeAliasType to its first type argument (mirroring
/// Python's `get_proper_type`), falling through to the inner type.
#[pyfunction]
pub(crate) fn rust_join_sorted_key(t_bytes: &[u8]) -> Option<i64> {
    let t = decode_type(t_bytes)?;
    // Unwrap TypeAliasType: Python's _join_sorted_key calls get_proper_type
    // first, which resolves the alias to its target.
    let t = match t {
        Type::TypeAliasType { args, .. } => {
            let arg = args.first()?;
            arg.clone()
        }
        other => other,
    };
    Some(join_sorted_key(&t))
}

/// `get_vars` (solve.py:880-882): ids of type variables in `target` that
/// are also in `vars`. `vars` is a list of `(raw_id, meta_level,
/// namespace)` triples. Returns the matching subset as a Vec of triples
/// (set semantics on the Python side; duplicates are harmless).
#[pyfunction]
pub(crate) fn rust_get_vars(
    target_bytes: &[u8],
    vars: Vec<(i64, i64, String)>,
) -> Option<Vec<(i64, i64, String)>> {
    let t = decode_type(target_bytes)?;
    let var_set: HashSet<TvId> = vars.into_iter().collect();
    let found = get_vars(&t, &var_set);
    Some(found.into_iter().collect())
}

/// `is_callable_protocol` (solve.py:918-922): True when `t` is an
/// `Instance` whose `TypeInfo` is a protocol with `__call__` in its
/// `protocol_members`. Unwraps TypeAliasType to target first.
#[pyfunction]
pub(crate) fn rust_is_callable_protocol(
    _py: Python<'_>,
    resolver: &NativeTypeResolver,
    t_bytes: &[u8],
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    // Unwrap TypeAliasType to its target, mirroring Python's get_proper_type.
    // The wire format cannot carry the alias target, so we resolve it here.
    let t = match t {
        Type::TypeAliasType { args, .. } => {
            let arg = args.first()?;
            arg.clone()
        }
        other => other,
    };
    Some(is_callable_protocol(&t, resolver.resolver()))
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
        assert_eq!(out.0, 0);
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
        // A and B unrelated -> candidate = None (kind=0, no bytes).
        let r = make_resolver(vec![snap("a.A"), snap("a.B")]);
        let lo = instance("a.A", vec![]);
        let up = instance("a.B", vec![]);
        let out = solve_one_inner(&[lo], &[up], false, true, &r).unwrap();
        assert_eq!(out.0, 0);
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

    // ------------------------------------------------------------------
    // is_trivial_bound / find_linear connectors
    // ------------------------------------------------------------------

    fn tv_type(raw_id: i64, name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{}", name),
            raw_id,
            namespace: "fn".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 1,
        }
    }

    /// Encode a constraint blob: origin_type_var, op, target.
    fn encode_constraint(origin: Type, op: i64, target: Type) -> Vec<u8> {
        let c = crate::constraints::Constraint {
            origin_type_var: origin,
            op,
            target,
        };
        let mut cb = crate::wire::WriteBuffer::new();
        c.write(&mut cb).unwrap();
        cb.into_bytes()
    }

    #[test]
    fn trivial_bound_object_yes_tuple_no() {
        let obj = instance("builtins.object", vec![]);
        assert_eq!(is_trivial_bound_inner(&obj, false), Some(true));
        let tup = instance("builtins.tuple", vec![obj]);
        // allow_tuple=False rejects the tuple; True recurses into its arg.
        assert_eq!(is_trivial_bound_inner(&tup, false), Some(false));
        assert_eq!(is_trivial_bound_inner(&tup, true), Some(true));
    }

    #[test]
    fn trivial_bound_nontrivial_no() {
        let int_t = instance("builtins.int", vec![]);
        assert_eq!(is_trivial_bound_inner(&int_t, false), Some(false));
        let str_t = instance("builtins.str", vec![]);
        // tuple[str] with allow_tuple=True recurses and rejects str (not object).
        let tup_str = instance("builtins.tuple", vec![str_t]);
        assert_eq!(is_trivial_bound_inner(&tup_str, true), Some(false));
    }

    #[test]
    fn trivial_bound_alias_defers() {
        // Python expands aliases via get_proper_type before recursing; the
        // wire cannot, so an alias must defer the whole check to Python.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.IntAlias".to_string(),
        };
        assert_eq!(is_trivial_bound_inner(&alias, false), None);
    }
    #[test]
    fn find_linear_typevar_like() {
        let s = tv_type(7, "S");
        let c = encode_constraint(s.clone(), 0, s.clone());
        let (lin, id) = rust_find_linear(&c).unwrap();
        assert!(lin);
        assert_eq!(id, Some((7, 1, "fn".to_string())));
    }

    #[test]
    fn find_linear_rejects_constant_target() {
        let s = tv_type(7, "S");
        let int_t = instance("builtins.int", vec![]);
        let c = encode_constraint(s, 0, int_t);
        let (lin, id) = rust_find_linear(&c).unwrap();
        assert!(!lin);
        assert_eq!(id, None);
    }

    #[test]
    fn find_linear_param_spec_needs_empty_prefix() {
        // P with no prefix args is linear.
        let p = Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".into(),
            fullname: "mod.P".into(),
            raw_id: 9,
            namespace: "fn".to_string(),
            flavor: 0,
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
        };
        let (lin, id) = {
            let c = crate::constraints::Constraint {
                origin_type_var: p.clone(),
                op: 0,
                target: p.clone(),
            };
            find_linear(&c)
        };
        assert!(lin);
        assert_eq!(id, Some((9, 0, "fn".to_string())));

        // A prefixed target (prefix args non-empty) is not linear.
        let prefixed = Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: vec![instance("builtins.int", vec![])],
                arg_kinds: vec![1],
                arg_names: vec![None],
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".into(),
            fullname: "mod.P".into(),
            raw_id: 9,
            namespace: "fn".to_string(),
            flavor: 0,
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
        };
        let prefixed_c = crate::constraints::Constraint {
            origin_type_var: p.clone(),
            op: 0,
            target: prefixed,
        };
        let (lin, id) = find_linear(&prefixed_c);
        assert!(!lin);
        assert_eq!(id, None);
    }

    #[test]
    fn find_linear_type_var_tuple_unpack() {
        // Ts captured as Tuple[*Ts] is linear.
        let tvv = Type::TypeVarTupleType {
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            name: "Ts".into(),
            fullname: "mod.Ts".into(),
            raw_id: 11,
            namespace: "fn".to_string(),
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            min_len: 0,
        };
        let target = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![Type::UnpackType {
                typ: Box::new(tvv.clone()),
            }],
            implicit: true,
        };
        let c = crate::constraints::Constraint {
            origin_type_var: tvv.clone(),
            op: 0,
            target: target.clone(),
        };
        let (lin, id) = find_linear(&c);
        assert!(lin);
        assert_eq!(id, Some((11, 0, "fn".to_string())));

        // Tuple[*Ts, U] is non-linear.
        let two_items = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![
                Type::UnpackType {
                    typ: Box::new(tvv.clone()),
                },
                instance("builtins.int", vec![]),
            ],
            implicit: true,
        };
        let c = crate::constraints::Constraint {
            origin_type_var: tvv,
            op: 0,
            target: two_items,
        };
        let (lin, id) = find_linear(&c);
        assert!(!lin);
        assert_eq!(id, None);
    }

    #[test]
    fn any_top_with_bottom_returns_from_another_any() {
        // Uppers = [Any], lowers = [A] -> Any(from_another_any, Any).
        // Top is the Any side, so the source is the Any itself
        // (solve.py:604-607), never the lower.
        let r = make_resolver(vec![snap("a.A")]);
        let lo = instance("a.A", vec![]);
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let out = solve_one_inner(&[lo], std::slice::from_ref(&any), false, true, &r).unwrap();
        assert_eq!(out.0, 3);
        let bytes = out.1.unwrap();
        let decoded = decode_type(&bytes).unwrap();
        let Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } = decoded
        else {
            panic!("expected AnyType, got {decoded:?}");
        };
        assert_eq!(type_of_any, 7); // TypeOfAny.from_another_any
        assert_eq!(*source_any.unwrap(), any);
        assert_eq!(missing_import_name, None);
    }

    #[test]
    fn any_bottom_with_top_returns_from_another_any() {
        // Uppers = [B], lowers = [Any] -> Any(from_another_any, Any).
        // Bottom is the Any side, so the source is the Any itself.
        let r = make_resolver(vec![snap("a.B")]);
        let up = instance("a.B", vec![]);
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let out = solve_one_inner(std::slice::from_ref(&any), &[up], false, true, &r).unwrap();
        assert_eq!(out.0, 3);
        let decoded = decode_type(&out.1.unwrap()).unwrap();
        let Type::AnyType { source_any, .. } = decoded else {
            panic!("expected AnyType, got {decoded:?}");
        };
        assert_eq!(*source_any.unwrap(), any);
    }

    #[test]
    fn any_top_and_any_bottom_keep_any_source_and_missing_import() {
        // Both sides Any -> Any(from_another_any, <the Any side>) with
        // the source's missing_import_name carried over (AnyType.__init__
        // types.py:1386-1392).
        let r = make_resolver(vec![]);
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: Some("mod.thing".to_string()),
        };
        let out = solve_one_inner(
            std::slice::from_ref(&any),
            std::slice::from_ref(&any),
            false,
            true,
            &r,
        )
        .unwrap();
        assert_eq!(out.0, 3);
        let decoded = decode_type(&out.1.unwrap()).unwrap();
        let Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } = decoded
        else {
            panic!("expected AnyType, got {decoded:?}");
        };
        assert_eq!(type_of_any, 7);
        assert_eq!(*source_any.unwrap(), any);
        assert_eq!(missing_import_name.as_deref(), Some("mod.thing"));
    }

    #[test]
    fn any_side_unrelated_bottom_delegates_only_after_absorption() {
        // Any in a lower position absorbs even when the upper is
        // unrelated (no subtype check runs).
        let r = make_resolver(vec![snap("a.A"), snap("a.B")]);
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        let up = instance("a.B", vec![]);
        let out = solve_one_inner(std::slice::from_ref(&any), &[up], false, true, &r).unwrap();
        assert_eq!(out.0, 3);
        let decoded = decode_type(&out.1.unwrap()).unwrap();
        let Type::AnyType { source_any, .. } = decoded else {
            panic!("expected AnyType, got {decoded:?}");
        };
        assert_eq!(*source_any.unwrap(), any);
    }

    #[test]
    fn top_only_without_any_solves_to_top() {
        // Uppers = [A], no lowers -> candidate = A (kind=0 with bytes).
        let r = make_resolver(vec![snap("a.A")]);
        let up = instance("a.A", vec![]);
        let out = solve_one_inner(&[], std::slice::from_ref(&up), false, true, &r).unwrap();
        assert_eq!(out.0, 0);
        assert_eq!(decode_type(&out.1.unwrap()).unwrap(), up);
    }

    #[test]
    fn non_subtype_pair_is_no_solution() {
        // A and B unrelated -> solve_one returns None (kind=0, no bytes).
        let r = make_resolver(vec![snap("a.A"), snap("a.B")]);
        let lo = instance("a.A", vec![]);
        let up = instance("a.B", vec![]);
        let out = solve_one_inner(&[lo], &[up], false, true, &r).unwrap();
        assert_eq!(out.0, 0);
        assert!(out.1.is_none());
    }

    // ------------------------------------------------------------------
    // solve_one_for_dependent no-op ports (issue #853)
    // ------------------------------------------------------------------

    #[test]
    fn dependent_noop_single_lower_returns_bound() {
        // T :> A only -> solve_one_for_dependent returns A itself,
        // mirroring solve.py's `bottom = UnionType.make_union([A])`.
        let r = make_resolver(vec![snap("a.A")]);
        let lo = instance("a.A", vec![]);
        let out = solve_one_for_dependent(std::slice::from_ref(&lo), &[], false, true, &r);
        assert_eq!(out, Ok(Some(lo)));
    }

    #[test]
    fn dependent_noop_single_lower_union_passthrough() {
        // A | B as the only lower passes through unchanged: Python's
        // UnionType.make_union (solve.py:591) is a raw constructor and a
        // single bound is returned as-is (no flatten, no dedupe).
        let r = make_resolver(vec![snap("a.A"), snap("a.B")]);
        let u = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let out = solve_one_for_dependent(std::slice::from_ref(&u), &[], true, true, &r).unwrap();
        assert_eq!(out, Some(u));
    }

    #[test]
    fn dependent_noop_single_upper_returns_bound() {
        // T <: A only -> candidate = the raw upper A.
        let r = make_resolver(vec![snap("a.A")]);
        let up = instance("a.A", vec![]);
        let out = solve_one_for_dependent(&[], std::slice::from_ref(&up), false, true, &r);
        assert_eq!(out, Ok(Some(up)));
    }

    #[test]
    fn dependent_noop_any_lower_absorbs() {
        // Single Any lower -> AnyType(from_another_any, the Any itself),
        // with the source's missing_import_name carried over
        // (solve.py:612-617, types.py:1386-1392).
        let r = make_resolver(vec![]);
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: Some("mod.thing".to_string()),
        };
        let out =
            solve_one_for_dependent(std::slice::from_ref(&any), &[], false, true, &r).unwrap();
        let Some(typ) = out else {
            panic!("expected a solution, got None");
        };
        let Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } = typ
        else {
            panic!("expected AnyType, got {typ:?}");
        };
        assert_eq!(type_of_any, 7); // TypeOfAny.from_another_any
        assert_eq!(*source_any.unwrap(), any);
        assert_eq!(missing_import_name.as_deref(), Some("mod.thing"));
    }

    #[test]
    fn dependent_noop_typevar_bound_defers() {
        // A bound carrying a live TypeVar must still defer (identity).
        let r = make_resolver(vec![snap("a.A")]);
        let lo = Type::UnionType {
            items: vec![instance("a.A", vec![]), tv_type(7, "T")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let out = solve_one_for_dependent(&[lo], &[], false, true, &r);
        assert_eq!(out, Err(()));
    }

    // ------------------------------------------------------------------
    // empty-constraint defaults + any_type alignment (issue #853)
    // ------------------------------------------------------------------

    #[test]
    fn empty_constraints_fill_strict_never() {
        // solve_constraints with no constraints: every var gets a strict
        // ambiguous Never (solve.py:326-331), so `rust_infer_function_
        // type_arguments` no longer needs its empty-constraints defer.
        let r = make_resolver(vec![]);
        let tv = tv_type(7, "T");
        let (sol_blob, _free) = solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[],
            true,
            false,
            true,
            false,
            &r,
        )
        .unwrap();
        let sols = decode_solve_solutions_here(&sol_blob).unwrap();
        assert_eq!(sols.len(), 1);
        assert_eq!(sols[0].0, (7, 1, "fn".to_string()));
        assert_eq!(sols[0].1, Some(Type::UninhabitedType { ambiguous: true }));
    }

    #[test]
    fn empty_constraints_fill_lax_any() {
        // Non-strict: every var gets AnyType(TypeOfAny.special_form).
        let r = make_resolver(vec![]);
        let tv = tv_type(7, "T");
        let (sol_blob, _free) = solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[],
            false,
            false,
            true,
            false,
            &r,
        )
        .unwrap();
        let sols = decode_solve_solutions_here(&sol_blob).unwrap();
        assert_eq!(sols.len(), 1);
        assert_eq!(
            sols[0].1,
            Some(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            })
        );
    }

    #[test]
    fn any_type_is_special_form_and_wire_safe() {
        // any_type() must encode TypeOfAny.special_form (6); the old
        // enum-invalid 0 made the wire-safety probe defer every
        // meet/join fold that materialized an Any.
        let any = any_type();
        let Type::AnyType { type_of_any, .. } = &any else {
            panic!("expected AnyType");
        };
        assert_eq!(*type_of_any, 6);
        assert!(!wire_unsafe_solution(&any));
        let bytes = encode_type(&any).unwrap();
        assert_eq!(decode_type(&bytes).unwrap(), any);
    }
}
