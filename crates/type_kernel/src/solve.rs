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
use crate::setops::{self, reconstruct_any_from_another, SetOpResult};
use crate::subtypes::{self, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// Wrap `wire::read_type` into an `Option`, mirroring `setops::decode_type`.
pub(crate) fn decode_type(bytes: &[u8]) -> Option<Type> {
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
                is_evaluated: true,
                original_str_expr: None,
                original_str_fallback: None,
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
        let any = reconstruct_any_from_another(&source, missing_import_name);
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
        SetOpResult::Bottom => {
            // meet.py:1143-1146 (unrelated Instances): Bottom materially
            // means NoneType when state.strict_optional is False,
            // UninhabitedType otherwise.
            if ctx.strict_optional {
                Some(Type::UninhabitedType { ambiguous: true })
            } else {
                Some(Type::NoneType)
            }
        }
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
    // Ambient `type_state.infer_unions` for the engine's unify reads;
    // RAII so a thread-local set here cannot outlive this call.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);
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
        Type::UnpackType { typ, .. } => collect_type_var_ids(typ, out),
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
                if let Type::UnpackType { typ, .. } = item {
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
        false,
        false,
        // Python `infer_constraints` wrapper default (constraints.py:802).
        true,
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
        false,
        false,
        // Python `infer_constraints` wrapper default (constraints.py:802).
        true,
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
                    let deg = in_degree.get_mut(dependent).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
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

/// Check a candidate `Type` for wire-unsafe structures: an Any whose
/// identity cannot survive the wire, or a type var owned by the solve
/// call. Foreign vars are safe: the shim re-links them (#1215 pattern).
fn wire_unsafe_solution(typ: &Type, owned: &HashSet<TvId>) -> bool {
    wire_unsafe_reason(typ, owned).is_some()
}

fn wire_unsafe_reason(typ: &Type, owned: &HashSet<TvId>) -> Option<&'static str> {
    match typ {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            if tv_id(typ).is_some_and(|k| owned.contains(&k)) {
                Some("owned-tv")
            } else {
                None
            }
        }
        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name: _,
        } => {
            if *type_of_any == 0 {
                Some("any0")
            } else if *type_of_any == 1 && source_any.is_some() {
                Some("any1")
            } else {
                None
            }
        }
        Type::UnionType { items, .. } => items.iter().find_map(|t| wire_unsafe_reason(t, owned)),
        Type::Instance { args, .. } => args.iter().find_map(|t| wire_unsafe_reason(t, owned)),
        _ => None,
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
    owned: &HashSet<TvId>,
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
            if wire_unsafe_solution(&typ, owned) {
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
#[allow(clippy::too_many_arguments)]
fn solve_iteratively_native(
    batch: &[TvId],
    graph: &mut HashSet<(TvId, TvId)>,
    lowers: &mut HashMap<TvId, Vec<Type>>,
    uppers: &mut HashMap<TvId, Vec<Type>>,
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
    owned: &HashSet<TvId>,
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
        let result =
            solve_one_for_dependent(&lo, &up, infer_unions, strict_optional, resolver, owned)?;
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

/// Outcome of `solve_with_dependent_core`: Python's `(solutions,
/// free_vars)` pair. `EmptySolutions` is the linear-fail literal
/// `({}, [])` (solve.py:253-254).
pub(crate) enum DependentSolve {
    EmptySolutions,
    Solved {
        solutions: Vec<(TvId, Option<Type>)>,
        free_vars: Vec<TvId>,
    },
}

/// `solve_with_dependent` (solve.py:234-292), Rust subset.
///
/// `Err(())` = defer to Python.
fn solve_with_dependent_core(
    vars: &[Type],
    constraints: &[Constraint],
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<DependentSolve, ()> {
    let tvars: Vec<TvId> = vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()?;
    let owned: HashSet<TvId> = tvars.iter().cloned().collect();
    let (mut graph, mut lowers, mut uppers) =
        transitive_closure(&tvars, constraints, resolver, strict_optional)?;
    let dmap = compute_dependencies(&tvars, &graph, &lowers, &uppers);

    let vertices: HashSet<TvId> = tvars.iter().cloned().collect();
    let sccs = strongly_connected_components(vertices, &dmap);
    if !sccs.iter().all(|s| check_linear(s, &lowers, &uppers)) {
        // Python returns the literal `({}, [])` (solve.py:253-254).
        return Ok(DependentSolve::EmptySolutions);
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
            &owned,
        ) {
            Ok(res) => solutions.extend(res),
            Err(()) => return Err(()),
        }
    }
    Ok(DependentSolve::Solved {
        solutions,
        free_vars,
    })
}

/// Blob-encoding wrapper around `solve_with_dependent_core`: keeps the
/// FFI shape byte-identical for the `rust_solve_dependent` seam.
fn solve_with_dependent_native(
    vars: &[Type],
    constraints: &[Constraint],
    infer_unions: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
) -> Result<NativeDependentOut, ()> {
    match solve_with_dependent_core(vars, constraints, infer_unions, strict_optional, resolver)? {
        DependentSolve::EmptySolutions => Ok(Some((None, None))),
        DependentSolve::Solved {
            solutions,
            free_vars,
        } => Ok(Some((
            Some(encode_solutions_blob(&solutions)?),
            Some(encode_free_vars_blob(&free_vars)?),
        ))),
    }
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
    // Ambient `type_state.infer_unions` for the engine's unify reads;
    // RAII so a thread-local set here cannot outlive this call.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);
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
    resolver: &crate::typeinfo::NativeTypeResolver,
) -> Result<(Vec<u8>, Vec<u8>), ()> {
    let r = resolver.resolver();
    let alr = resolver.alias_resolver();
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
                r,
            ) {
                Some(o) => o,
                None => return Err(()),
            };
            match out {
                (0, Some(bytes)) | (1, Some(bytes)) | (3, Some(bytes)) => {
                    let typ = decode_type(&bytes).ok_or(())?;
                    if wire_unsafe_solution(&typ, &all_ids) {
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
    let mut ordered: Vec<Option<Type>> = Vec::with_capacity(original_vars.len());
    for t in original_vars {
        let tv = tv_id(t).ok_or(())?;
        match solutions.iter().find(|(k, _)| *k == tv) {
            Some((_, sol)) => ordered.push(sol.clone()),
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
                ordered.push(Some(cand));
            }
        }
    }

    // pre_validate_solutions (solve.py:291-295, 799-829): replace a
    // solution that violates its var's upper bound with the bound itself
    // when the bound satisfies every constraint.
    if !skip_unsatisfied {
        ordered = pre_validate_solutions_inner(
            ordered,
            original_vars,
            constraints,
            alr,
            r,
            strict_optional,
        )?;
    }

    let ordered_pairs: Vec<(TvId, Option<Type>)> = original_vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .zip(ordered)
        .collect();

    let sol_blob = encode_solutions_blob(&ordered_pairs)?;
    let free_blob = encode_free_vars_blob(&[])?;
    Ok((sol_blob, free_blob))
}

/// `pre_validate_solutions` (solve.py:799-829) core, shared by the
/// non-polymorphic and polymorphic solve paths. `res` must pair 1:1 with
/// `original_vars`. Returns `Err(())` = defer to Python.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pre_validate_solutions_inner(
    res: Vec<Option<Type>>,
    original_vars: &[Type],
    constraints: &[Constraint],
    lookup: &dyn crate::aliases::AliasLookup,
    r: &crate::typeinfo::TypeResolver,
    strict_optional: bool,
) -> Result<Vec<Option<Type>>, ()> {
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let mut validated: Vec<Option<Type>> = Vec::with_capacity(res.len());
    for (t, s) in original_vars.iter().zip(res) {
        let ub = upper_bound_of(t).ok_or(())?;
        if is_callable_protocol(&ub, r) {
            validated.push(s);
            continue;
        }
        // Python's `is_subtype` applies `get_proper_type` at entry
        // (subtypes.py:905); mirror it by expanding alias nodes here
        // instead of deferring on every alias-bearing bound/solution.
        if let Some(sol) = &s {
            let sol = subtypes::expand_aliases(sol, lookup, strict_optional).ok_or(())?;
            let ub_e = subtypes::expand_aliases(&ub, lookup, strict_optional).ok_or(())?;
            if !subtypes::is_subtype(&sol, &ub_e, &ctx, r).ok_or(())? {
                let mut bound_satisfies_all = true;
                for c in constraints {
                    if c.op == crate::constraints::SUBTYPE_OF {
                        let target = subtypes::expand_aliases(&c.target, lookup, strict_optional)
                            .ok_or(())?;
                        if !subtypes::is_subtype(&ub_e, &target, &ctx, r).ok_or(())? {
                            bound_satisfies_all = false;
                            break;
                        }
                    }
                    if c.op == crate::constraints::SUPERTYPE_OF {
                        let target = subtypes::expand_aliases(&c.target, lookup, strict_optional)
                            .ok_or(())?;
                        if !subtypes::is_subtype(&target, &ub_e, &ctx, r).ok_or(())? {
                            bound_satisfies_all = false;
                            break;
                        }
                    }
                }
                if bound_satisfies_all {
                    // Python pushes the raw `t.upper_bound`, not the
                    // proper-expanded copy used for the comparison.
                    validated.push(Some(ub));
                    continue;
                }
            }
        }
        validated.push(s);
    }
    Ok(validated)
}

/// `solve_constraints` polymorphic branch (solve.py:241-262 + 277-289)
/// with `allow_polymorphic=True`, used by the `unify_generic_callable`
/// port. Python's extra-tvar collection is not ported: the kernel
/// `Constraint` carries no `extra_tvars`, so `unify_generic_callable_core`
/// gates on the attach shapes instead (constraints.py:1712, 1768 via
/// `no_extra_tvar_shape`) and defers them to Python before reaching
/// this entry, keeping parity rather than making extras impossible.
/// `strict=True` / `skip_unsatisfied=False` match the unify call site
/// (subtypes.py). Returns `Err(())` = defer to Python; the returned
/// list may contain `None` (unsolved var, unify must fail).
pub(crate) fn solve_constraints_poly_native(
    original_vars: &[Type],
    constraints: &[Constraint],
    infer_unions: bool,
    strict_optional: bool,
    r: &crate::typeinfo::TypeResolver,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
) -> Result<Vec<Option<Type>>, ()> {
    if original_vars.is_empty() {
        return Ok(Vec::new());
    }
    // Python reassigns `constraints` before both the solve and the
    // pre-validation (solve.py:242-244, 288).
    let filtered = crate::constraints_filter::skip_reverse_union_kernel(constraints).ok_or(())?;

    // Dependent solve over the filtered constraints (solve.py:255-261); it
    // consumes `filtered` directly (Python's cmap is shared with the
    // non-polymorphic branch, which this poly-only port does not implement).
    let (solutions, free_vars): (Vec<(TvId, Option<Type>)>, Vec<TvId>) = if !filtered.is_empty() {
        match solve_with_dependent_core(original_vars, &filtered, infer_unions, strict_optional, r)?
        {
            DependentSolve::EmptySolutions => (Vec::new(), Vec::new()),
            DependentSolve::Solved {
                solutions,
                free_vars,
            } => (solutions, free_vars),
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // Ordered `res` over `vars` (solve.py:277-289). Unify always passes
    // strict=True, so a var without solutions gets ambiguous Never.
    let mut res: Vec<Option<Type>> = Vec::with_capacity(original_vars.len());
    for t in original_vars {
        let tv = tv_id(t).ok_or(())?;
        match solutions.iter().find(|(k, _)| *k == tv) {
            Some((_, sol)) => res.push(sol.clone()),
            None => res.push(Some(Type::UninhabitedType { ambiguous: true })),
        }
    }

    // pre_validate_solutions runs only when nothing is free
    // (solve.py:287-289); unify passes skip_unsatisfied=False.
    if free_vars.is_empty() {
        let empty = crate::aliases::TypeAliasResolver::new();
        let lookup: &dyn crate::aliases::AliasLookup = match aliases {
            Some(a) => a,
            None => &empty,
        };
        res = pre_validate_solutions_inner(
            res,
            original_vars,
            &filtered,
            lookup,
            r,
            strict_optional,
        )?;
    }
    Ok(res)
}

/// `mypy.infer.infer_type_arguments` (infer.py:67-80): constraints from a
/// single template/actual pair, solved for `type_vars`. Mirrors the Python
/// defaults: `skip_unsatisfied=False`, `strict=True`,
/// `allow_polymorphic=False` (no `skip_reverse_union_constraints`), and
/// `infer_unions` from `type_state.infer_unions` (solve.py:242-289). Used
/// by the typeops composite generic `bind_self` arm (typeops.py:1074).
#[allow(clippy::too_many_arguments)]
pub(crate) fn infer_type_arguments_inner(
    type_vars: &[Type],
    template: &Type,
    actual: &Type,
    is_supertype: bool,
    skip_unsatisfied: bool,
    erase_types: bool,
    strict_optional: bool,
    infer_unions: bool,
    resolver: &crate::typeinfo::NativeTypeResolver,
) -> Option<Vec<Option<Type>>> {
    let direction = if is_supertype {
        crate::constraints::SUPERTYPE_OF
    } else {
        crate::constraints::SUBTYPE_OF
    };
    let constraints = crate::constraints::infer_constraints_full_inner(
        template,
        actual,
        direction,
        resolver.resolver(),
        resolver.alias_resolver(),
        strict_optional,
        false,
        false,
        erase_types,
    )?;
    let (sol_blob, _free_blob) = solve_constraints_native(
        type_vars,
        type_vars,
        &constraints,
        // Python solve_constraints default strict=True.
        true,
        infer_unions,
        strict_optional,
        skip_unsatisfied,
        resolver,
    )
    .ok()?;
    let solutions = decode_solve_solutions_here(&sol_blob)?;
    let tvids: Vec<TvId> = type_vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()
        .ok()?;
    Some(
        tvids
            .iter()
            .map(|tv| {
                solutions
                    .iter()
                    .find(|(k, _)| k == tv)
                    .and_then(|(_, v)| v.clone())
            })
            .collect(),
    )
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
    // Ambient `type_state.infer_unions` for the engine's unify reads;
    // RAII so a thread-local set here cannot outlive this call.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);
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
        resolver,
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
    iterable_type: Option<Vec<u8>>,
    mapping_type: Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    // Ambient `type_state.infer_unions` for the engine's unify reads;
    // RAII so a thread-local set here cannot outlive this call.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = match wire::read_type(&mut buf, None) {
        Ok(t) => t,
        Err(_) => {
            return None;
        }
    };
    let Type::CallableType {
        arg_types: formal_types,
        arg_kinds: formal_kinds,
        arg_names: formal_names,
        variables,
        ..
    } = &callee
    else {
        {
            return None;
        };
    };
    // ParamSpec/TypeVarTuple variables use the deferred constraint paths
    // (constraints.py:475-494, filter_imprecise_kinds): defer.
    if variables.iter().any(|v| {
        matches!(
            v,
            Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
        )
    }) {
        {
            return None;
        };
    }
    // UnpackType formals use the star-unpack branch (constraints.py:388-438): defer.
    if formal_types
        .iter()
        .any(|t| matches!(t, Type::UnpackType { .. }))
    {
        {
            return None;
        };
    }
    let vars_types = variables.clone();
    let mut arg_types_vec: Vec<Option<Type>> = Vec::with_capacity(arg_types.len());
    for b in &arg_types {
        match b {
            None => arg_types_vec.push(None),
            Some(b2) => {
                let mut b3 = ReadBuffer::new(b2);
                match wire::read_type(&mut b3, None) {
                    Ok(t) => arg_types_vec.push(Some(t)),
                    Err(_) => {
                        return None;
                    }
                }
            }
        }
    }
    let mut tuple_index: i64 = 0;
    let mut kwargs_used: Option<Vec<String>> = None;
    // ArgTypeExpander Iterable/Mapping context (checkexpr.py:3725-3730).
    // A missing blob defers only when the corresponding arm is reached.
    let iterable_type = iterable_type.as_ref().and_then(|b| decode_type(b));
    let mapping_type = mapping_type.as_ref().and_then(|b| decode_type(b));
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
            let actual_kind = match arg_kinds.get(ai as usize) {
                Some(k) => *k,
                None => {
                    return None;
                }
            };
            let expanded = match expand_actual_arg(
                &mut tuple_index,
                &mut kwargs_used,
                actual_arg,
                actual_kind,
                formal_name,
                formal_kind,
                // UnpackType formals defer above (the constraints.py:644-727
                // star-unpack branch), so every expansion here is a plain
                // positional/keyword formal: allow_unpack is False.
                false,
                iterable_type.as_ref(),
                mapping_type.as_ref(),
                resolver.alias_resolver(),
                resolver.resolver(),
                strict_optional,
            ) {
                Some(e) => e,
                None => {
                    return None;
                }
            };
            let icr = crate::constraints::infer_constraints_full_inner(
                formal_type,
                &expanded,
                crate::constraints::SUPERTYPE_OF,
                resolver.resolver(),
                resolver.alias_resolver(),
                strict_optional,
                false,
                false,
                // Python `infer_constraints` wrapper default (constraints.py:802).
                true,
            );
            constraints.extend(icr?);
        }
    }
    // Empty constraints fall through to `solve_constraints_native`:
    // Python has no fast path either; its empty-cmap fill yields strict
    // Never / lax Any defaults per variable, which native solve mirrors.
    let (sol_blob, _free_blob) = match solve_constraints_native(
        &vars_types,
        &vars_types,
        &constraints,
        strict,
        infer_unions,
        strict_optional,
        false,
        resolver,
    ) {
        Ok(s) => s,
        Err(_) => {
            return None;
        }
    };
    // Re-encode in `variables` order as an optional-type list. The Python
    // shim decodes this with `read_int` + `read_type` (count per var).
    let solutions = match decode_solve_solutions_here(&sol_blob) {
        Some(s) => s,
        None => {
            return None;
        }
    };
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

fn any_from_error() -> Type {
    // AnyType(TypeOfAny.from_error) (types.py:309; value 5), mirroring
    // argmap.py:402/429.
    Type::AnyType {
        type_of_any: 5,
        source_any: None,
        missing_import_name: None,
    }
}

/// One arg-expansion pass of `ArgTypeExpander.expand_actual_type` (argmap.py:269-364).
/// Returns the expanded `Type` directly; undecidable subtype/map decisions, a
/// missing context, aliases, or arbitrary-key pops answer `None` to defer to Python.
#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_actual_arg(
    tuple_index: &mut i64,
    kwargs_used: &mut Option<Vec<String>>,
    actual: &Type,
    actual_kind: i64,
    formal_name: Option<&str>,
    formal_kind: i64,
    allow_unpack: bool,
    iterable_type: Option<&Type>,
    mapping_type: Option<&Type>,
    alias_resolver: &crate::aliases::TypeAliasResolver,
    resolver: &crate::typeinfo::TypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    if matches!(actual, Type::UnboundType { .. }) {
        return None;
    }
    // `get_proper_type` (argmap.py:300): expand a top-level alias through
    // the snapshot; an unresolvable or residual alias still defers.
    let expanded_actual;
    let actual = match actual {
        Type::TypeAliasType { .. } => {
            expanded_actual =
                crate::checkexpr_functions::get_proper_or_expand(actual, alias_resolver)?;
            let expanded = &expanded_actual;
            if matches!(
                expanded,
                Type::TypeAliasType { .. } | Type::UnboundType { .. }
            ) {
                return None;
            }
            expanded
        }
        other => other,
    };
    match actual_kind {
        2 => {
            // *Ts passed to a callable: continue with the upper bound
            // (argmap.py:358-362); an alias upper bound expands first.
            let upper_bound_holder: Option<Type>;
            let star_actual = match actual {
                Type::TypeVarTupleType { upper_bound, .. } => {
                    let bound = match upper_bound.as_ref() {
                        Type::TypeAliasType { .. } => {
                            let e = crate::checkexpr_functions::get_proper_or_expand(
                                upper_bound.as_ref(),
                                alias_resolver,
                            )?;
                            if matches!(e, Type::TypeAliasType { .. } | Type::UnboundType { .. }) {
                                return None;
                            }
                            upper_bound_holder = Some(e);
                            upper_bound_holder.as_ref()?
                        }
                        other => other,
                    };
                    bound
                }
                other => other,
            };
            match star_actual {
                Type::Instance { type_ref, args, .. } if !args.is_empty() => {
                    // Iterable unpacking (argmap.py:363-369).
                    let iterable = iterable_type?;
                    let Type::Instance {
                        type_ref: iterable_ref,
                        ..
                    } = iterable
                    else {
                        return None;
                    };
                    let ctx =
                        SubtypeContext::new(false, false, false, false, false, strict_optional);
                    match subtypes::is_subtype(star_actual, iterable, &ctx, resolver) {
                        Some(true) => {
                            let mapped = subtypes::map_instance_to_supertype(
                                type_ref,
                                args,
                                iterable_ref,
                                resolver,
                            )?;
                            Some(mapped.into_iter().next()?)
                        }
                        // Not a proper Iterable: other parts of the code
                        // raise a different error for improper use
                        // (argmap.py:370-376, 402).
                        Some(false) => Some(any_from_error()),
                        None => None,
                    }
                }
                Type::TupleType { items, .. } => {
                    let len = items.len() as i64;
                    *tuple_index = if *tuple_index >= len {
                        1
                    } else {
                        *tuple_index + 1
                    };
                    let mut item = items.get((*tuple_index - 1) as usize)?.clone();
                    if let Type::UnpackType { typ: inner, .. } = &item {
                        if allow_unpack {
                            // An unpack item with special handling: pass
                            // the node through (argmap.py:385).
                        } else {
                            // An unpack item that doesn't have special
                            // handling: use the upper bound
                            // (argmap.py:386-396).
                            let unpacked = match inner.as_ref() {
                                Type::TypeVarTupleType { upper_bound, .. } => upper_bound.as_ref(),
                                other => other,
                            };
                            let Type::Instance { type_ref, args, .. } = unpacked else {
                                return None;
                            };
                            if type_ref != "builtins.tuple" || args.is_empty() {
                                return None;
                            }
                            item = args[0].clone();
                        }
                    }
                    Some(item)
                }
                Type::ParamSpecType { .. } => {
                    // ParamSpec is valid in *args but cannot be unpacked
                    // (argmap.py:398-400).
                    Some(star_actual.clone())
                }
                // Instance without args (argmap.py:402 tail) and every
                // other shape.
                _ => Some(any_from_error()),
            }
        }
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
            Type::Instance { type_ref, args, .. } => {
                // Mapping unpacking (argmap.py:417-424). Python matches
                // every Instance here without an args guard and then
                // compares against the Mapping context directly.
                let mapping = mapping_type?;
                let Type::Instance {
                    type_ref: mapping_ref,
                    ..
                } = mapping
                else {
                    return None;
                };
                let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
                match subtypes::is_subtype(actual, mapping, &ctx, resolver) {
                    Some(true) => {
                        let mapped = subtypes::map_instance_to_supertype(
                            type_ref,
                            args,
                            mapping_ref,
                            resolver,
                        )?;
                        Some(mapped.get(1)?.clone())
                    }
                    // Only `Mapping` can be unpacked with `**`; other
                    // types produce an error somewhere else
                    // (argmap.py:420-424, 428-429).
                    Some(false) => Some(any_from_error()),
                    None => None,
                }
            }
            Type::ParamSpecType { .. } => {
                // ParamSpec is valid in **kwargs but it cannot be unpacked
                // (argmap.py:425-427).
                Some(actual.clone())
            }
            _ => Some(any_from_error()),
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
            is_recursive: false,
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
            meta_level: 0,
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
            meta_level: 0,
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
            meta_level: 0,
        };
        let target = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![Type::UnpackType {
                typ: Box::new(tvv.clone()),
                from_star_syntax: false,
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
                    from_star_syntax: false,
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
        let out = solve_one_for_dependent(
            std::slice::from_ref(&lo),
            &[],
            false,
            true,
            &r,
            &HashSet::new(),
        );
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
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let out = solve_one_for_dependent(
            std::slice::from_ref(&u),
            &[],
            true,
            true,
            &r,
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(out, Some(u));
    }

    #[test]
    fn dependent_noop_single_upper_returns_bound() {
        // T <: A only -> candidate = the raw upper A.
        let r = make_resolver(vec![snap("a.A")]);
        let up = instance("a.A", vec![]);
        let out = solve_one_for_dependent(
            &[],
            std::slice::from_ref(&up),
            false,
            true,
            &r,
            &HashSet::new(),
        );
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
        let out = solve_one_for_dependent(
            std::slice::from_ref(&any),
            &[],
            false,
            true,
            &r,
            &HashSet::new(),
        )
        .unwrap();
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
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let out = solve_one_for_dependent(&[lo], &[], false, true, &r, &HashSet::new());
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
        let r = NativeTypeResolver::from_resolver(make_resolver(vec![]));
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
        let r = NativeTypeResolver::from_resolver(make_resolver(vec![]));
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
        assert!(!wire_unsafe_solution(&any, &HashSet::new()));
        let bytes = encode_type(&any).unwrap();
        assert_eq!(decode_type(&bytes).unwrap(), any);
    }

    #[test]
    fn wire_unsafe_foreign_tv_allowed() {
        let tv = tv_type(7, "T");
        assert!(!wire_unsafe_solution(&tv, &HashSet::new()));
    }

    #[test]
    fn wire_unsafe_owned_tv_defers() {
        let tv = tv_type(7, "T");
        let owned: HashSet<TvId> = [tv_id(&tv).unwrap()].into_iter().collect();
        assert_eq!(wire_unsafe_reason(&tv, &owned), Some("owned-tv"));
    }

    #[test]
    fn wire_unsafe_nested_tv_respected_both_ways() {
        // The probe descends Instance args: an owned nested var defers,
        // the same tree with a foreign var is safe (shim relinks it).
        let tv = tv_type(7, "T");
        let owned: HashSet<TvId> = [tv_id(&tv).unwrap()].into_iter().collect();
        let inst = instance("builtins.list", vec![tv]);
        assert_eq!(wire_unsafe_reason(&inst, &owned), Some("owned-tv"));
        assert!(!wire_unsafe_solution(&inst, &HashSet::new()));
    }

    #[test]
    fn wire_unsafe_union_descends() {
        let tv = tv_type(7, "T");
        let owned: HashSet<TvId> = [tv_id(&tv).unwrap()].into_iter().collect();
        let union = Type::UnionType {
            items: vec![any_type(), tv],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(wire_unsafe_reason(&union, &owned), Some("owned-tv"));
        assert!(!wire_unsafe_solution(&union, &HashSet::new()));
    }

    #[test]
    fn wire_unsafe_any1_with_source_defers() {
        let any1 = any_type_from(any_type());
        assert!(wire_unsafe_solution(&any1, &HashSet::new()));
    }

    // ------------------------------------------------------------------
    // pre_validate alias expansion + expand_actual_arg (issue #1241)
    // ------------------------------------------------------------------

    use crate::aliases::{TypeAliasResolver, TypeAliasSnapshot};

    fn encode_for_alias(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, t).expect("encode type");
        buf.into_bytes()
    }

    fn alias_type(args: Vec<Type>, type_ref: &str) -> Type {
        Type::TypeAliasType {
            args,
            type_ref: type_ref.to_string(),
            is_recursive: false,
        }
    }

    fn no_args_alias(ref_fullname: &str, target: &Type) -> TypeAliasSnapshot {
        TypeAliasSnapshot {
            fullname: ref_fullname.to_string(),
            target: encode_for_alias(target),
            no_args: true,
            ..Default::default()
        }
    }

    fn test_int_alias_resolver() -> NativeTypeResolver {
        let r = make_resolver(vec![
            snap("builtins.int"),
            snap("builtins.str"),
            snap("builtins.object"),
        ]);
        let mut ar = TypeAliasResolver::new();
        ar.insert(
            "mod.IntAlias".to_string(),
            no_args_alias("mod.IntAlias", &instance("builtins.int", vec![])),
        );
        NativeTypeResolver::new(r, ar)
    }

    fn tv_bound(raw_id: i64, upper_bound: Type) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id,
            namespace: "fn".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(upper_bound),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 1,
        }
    }

    #[test]
    fn pre_validate_alias_bound_solution_satisfies() {
        // T bound by an alias resolving to int; solution int. The old
        // `is_subtype` deferral on the alias operand Err'd the whole solve;
        // expansion now decides true and the solution survives.
        let nr = test_int_alias_resolver();
        let tv = tv_bound(7, alias_type(vec![], "mod.IntAlias"));
        let con = crate::constraints::Constraint {
            origin_type_var: tv_type(7, "T"),
            op: crate::constraints::SUPERTYPE_OF,
            target: instance("builtins.int", vec![]),
        };
        let (sol_blob, _) = solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[con],
            true,
            false,
            true,
            false,
            &nr,
        )
        .unwrap();
        let sols = decode_solve_solutions_here(&sol_blob).unwrap();
        assert_eq!(sols.len(), 1);
        assert_eq!(sols[0].1, Some(instance("builtins.int", vec![])));
    }

    #[test]
    fn pre_validate_alias_bound_violation_implication_tests() {
        // Solution str against an alias-to-int bound: violates; the bound
        // fails its own constraint check, so the solution survives.
        let nr = test_int_alias_resolver();
        let tv = tv_bound(7, alias_type(vec![], "mod.IntAlias"));
        let con = crate::constraints::Constraint {
            origin_type_var: tv_type(7, "T"),
            op: crate::constraints::SUBTYPE_OF,
            target: instance("builtins.str", vec![]),
        };
        let (sol_blob, _) = solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[con],
            true,
            false,
            true,
            false,
            &nr,
        )
        .unwrap();
        let sols = decode_solve_solutions_here(&sol_blob).unwrap();
        assert_eq!(sols[0].1, Some(instance("builtins.str", vec![])));
    }

    #[test]
    fn pre_validate_alias_solution_expanded_for_compare() {
        // Solution is an alias node resolving to int, bound int. The alias
        // must expand for the comparison; the raw alias node is kept in
        // the returned solution (Python stores the unexpanded solution).
        let nr = test_int_alias_resolver();
        let tv = tv_bound(7, instance("builtins.int", vec![]));
        let sol_alias = alias_type(vec![], "mod.IntAlias");
        let con = crate::constraints::Constraint {
            origin_type_var: tv_type(7, "T"),
            op: crate::constraints::SUPERTYPE_OF,
            target: sol_alias.clone(),
        };
        let (sol_blob, _) = solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[con],
            true,
            false,
            true,
            false,
            &nr,
        )
        .unwrap();
        let sols = decode_solve_solutions_here(&sol_blob).unwrap();
        assert_eq!(sols[0].1, Some(sol_alias));
    }

    #[test]
    fn pre_validate_alias_bound_missing_snapshot_defers() {
        // An alias that is not in the snapshot snapshot cannot expand
        // (expand_aliases keeps the node) and engine still defers: the
        // whole solve still Errs, matching the pre-port behavior.
        let nr = NativeTypeResolver::from_resolver(make_resolver(vec![]));
        let tv = tv_bound(7, alias_type(vec![], "mod.IntAlias"));
        let con = crate::constraints::Constraint {
            origin_type_var: tv_type(7, "T"),
            op: crate::constraints::SUPERTYPE_OF,
            target: instance("builtins.int", vec![]),
        };
        assert!(solve_constraints_native(
            std::slice::from_ref(&tv),
            std::slice::from_ref(&tv),
            &[con],
            true,
            false,
            true,
            false,
            &nr,
        )
        .is_err());
    }

    #[test]
    fn expand_actual_arg_alias_to_tuple_item() {
        // *args actual typed with an alias to tuple[int, str]: the first
        // item feeds the formal; before the port the whole call deferred.
        let mut ar = TypeAliasResolver::new();
        ar.insert(
            "mod.IntStrAlias".to_string(),
            no_args_alias(
                "mod.IntStrAlias",
                &Type::TupleType {
                    partial_fallback: Box::new(instance("builtins.tuple", vec![])),
                    items: vec![
                        instance("builtins.int", vec![]),
                        instance("builtins.str", vec![]),
                    ],
                    implicit: false,
                },
            ),
        );
        let actual = alias_type(vec![], "mod.IntStrAlias");
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        let empty_resolver = make_resolver(vec![]);
        let out = expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &actual,
            2, // ARG_STAR
            None,
            2,
            false,
            None,
            None,
            &ar,
            &empty_resolver,
            true,
        )
        .unwrap();
        assert_eq!(out, instance("builtins.int", vec![]));
        // Unresolvable alias defers; UnboundType still defers.
        let ar2 = TypeAliasResolver::new();
        let mut ti2 = 0;
        assert!(expand_actual_arg(
            &mut ti2,
            &mut kwargs_used,
            &actual,
            2,
            None,
            2,
            false,
            None,
            None,
            &ar2,
            &empty_resolver,
            true,
        )
        .is_none());
        let unbound = Type::UnboundType {
            name: "x".to_string(),
            args: Vec::new(),
            original_str_expr: None,
            original_str_fallback: None,
            empty_tuple_index: false,
            optional: false,
        };
        let mut ti3 = 0;
        assert!(expand_actual_arg(
            &mut ti3,
            &mut kwargs_used,
            &unbound,
            2,
            None,
            2,
            false,
            None,
            None,
            &ar2,
            &empty_resolver,
            true,
        )
        .is_none());
    }

    #[test]
    fn expand_actual_arg_star_iterator_instance_expands_to_item() {
        // New ARG_STAR iterable arm (argmap.py:363-369): *x where `x` is an
        // Instance feeds the formal with its element type. The same-ref map
        // fast path keeps this test snapshot-free; the arity-1 Iterable yields args[0].
        let resolver = make_resolver(vec![snap("typing.Iterable")]);
        let item = instance("builtins.int", vec![]);
        let iterable = instance("typing.Iterable", vec![item.clone()]);
        let actual = instance("typing.Iterable", vec![item.clone()]);
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        let out = expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &actual,
            2,
            None,
            2,
            false,
            Some(&iterable),
            None,
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .unwrap();
        assert_eq!(out, item);
    }

    #[test]
    fn expand_actual_arg_star_iterator_no_context_defers() {
        // Missing Iterable/Mapping context blobs: the arm defers so the
        // whole solve falls back to Python instead of guessing.
        let resolver = make_resolver(vec![]);
        let item = instance("builtins.int", vec![]);
        let actual = instance("typing.Iterable", vec![item]);
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        assert!(expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &actual,
            2,
            None,
            2,
            false,
            None,
            None,
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .is_none());
    }

    #[test]
    fn expand_actual_arg_star_kwargs_getitem_instance_expands_value_type() {
        // New ARG_STAR2 mapping arm (argmap.py:417-424): **x where `x` is a
        // Mapping formal feeds the value type (mapped args[1]). Identical
        // args hit the visit_instance_nominal same-ref fast path.
        let resolver = make_resolver(vec![snap("typing.Mapping")]);
        let key = instance("builtins.str", vec![]);
        let value = instance("builtins.int", vec![]);
        let mapping = instance("typing.Mapping", vec![key.clone(), value.clone()]);
        let actual = instance("typing.Mapping", vec![key, value.clone()]);
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        let out = expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &actual,
            4,
            None,
            2,
            false,
            None,
            Some(&mapping),
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .unwrap();
        assert_eq!(out, value);
    }

    #[test]
    fn expand_actual_arg_star_kwargs_no_context_defers() {
        let resolver = make_resolver(vec![]);
        let actual = instance(
            "typing.Mapping",
            vec![instance("builtins.str", vec![]), any_from_error()],
        );
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        assert!(expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &actual,
            4,
            None,
            2,
            false,
            None,
            None,
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .is_none());
    }

    #[test]
    fn expand_actual_arg_star_tvt_upper_bound_flows_to_tuple_arm() {
        // *Ts actual (argmap.py:358-362): the upper bound is unwrapped and
        // re-enters the same chain, so a tuple upper bound feeds the
        // positional tuple-indexing arm with the shared tuple_index state.
        let resolver = make_resolver(vec![]);
        let int_item = instance("builtins.int", vec![]);
        let str_item = instance("builtins.str", vec![]);
        let tvt = Type::TypeVarTupleType {
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "".to_string(),
            raw_id: 3,
            namespace: "".to_string(),
            upper_bound: Box::new(Type::TupleType {
                partial_fallback: Box::new(instance("builtins.tuple", vec![])),
                items: vec![int_item.clone(), str_item.clone()],
                implicit: false,
            }),
            default: Box::new(any_from_error()),
            min_len: 0,
            meta_level: 0,
        };
        let mut tuple_index = 0;
        let mut kwargs_used = None;
        let first = expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &tvt,
            2,
            None,
            2,
            false,
            None,
            None,
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .unwrap();
        assert_eq!(first, int_item);
        let second = expand_actual_arg(
            &mut tuple_index,
            &mut kwargs_used,
            &tvt,
            2,
            None,
            2,
            false,
            None,
            None,
            &TypeAliasResolver::new(),
            &resolver,
            true,
        )
        .unwrap();
        assert_eq!(second, str_item);
    }

    #[test]
    fn materialize_meet_bottom_honors_strict_optional() {
        // meet.py:1143-1146 (unrelated Instances): Bottom is NoneType
        // under no strict-optional, UninhabitedType otherwise; the solve
        // fold materializes in Rust, so the branch must live there too.
        let r = make_resolver(vec![snap("a.A"), snap("b.B")]);
        let s = instance("a.A", vec![]);
        let t = instance("b.B", vec![]);
        let strict = SubtypeContext::new(false, false, false, false, false, true);
        let non_strict = SubtypeContext::new(false, false, false, false, false, false);
        let meet = materialize_meet(&s, &t, &strict, &r).unwrap();
        match &meet {
            Type::UninhabitedType { ambiguous, .. } => assert!(*ambiguous),
            other => panic!("expected UninhabitedType, got {other:?}"),
        }
        assert_eq!(
            materialize_meet(&s, &t, &non_strict, &r).unwrap(),
            Type::NoneType
        );
    }
}
