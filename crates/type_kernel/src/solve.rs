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
        setops::make_simplified_union(lowers, &ctx, resolver, true)
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



// ---------------------------------------------------------------------------
// solve_with_dependent (solve.py:234-292) Rust subset.
// ---------------------------------------------------------------------------

/// Type variable id = (raw_id, meta_level, namespace), mirroring
/// `mypy.types.TypeVarId` and `expandtype::EnvKey` (the wire carries the
/// same triple, so the two are interchangeable).
type TvId = (i64, i64, String);

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
            raw_id,
            namespace,
            ..
        } => Some((*raw_id, 0, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id,
            namespace,
            ..
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
        Type::TypedDictType { items, fallback, .. } => {
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
        Type::AnyType { source_any, .. } => {
            if let Some(s) = source_any {
                collect_type_var_ids(s, out);
            }
        }
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
) -> Result<(), ()> {
    if matches!(upper, Type::UnionType { .. }) && matches!(lower, Type::UnionType { .. }) {
        return Ok(());
    }
    let sub = crate::constraints::infer_constraints_full_inner(
        lower, upper, crate::constraints::SUBTYPE_OF, resolver,
    )
    .ok_or(())?;
    for c in sub {
        if !remaining.contains(&c) {
            remaining.push(c);
        }
    }
    let sup = crate::constraints::infer_constraints_full_inner(
        upper, lower, crate::constraints::SUPERTYPE_OF, resolver,
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
) -> Result<(HashSet<(TvId, TvId)>, HashMap<TvId, Vec<Type>>, HashMap<TvId, Vec<Type>>), ()> {
    let tvars_set: HashSet<TvId> = tvars.iter().cloned().collect();
    let mut uppers: HashMap<TvId, Vec<Type>> = HashMap::new();
    let mut lowers: HashMap<TvId, Vec<Type>> = HashMap::new();
    let mut graph: HashSet<(TvId, TvId)> = tvars.iter().cloned().map(|tv| (tv.clone(), tv)).collect();

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
            // graph |= {(l,u) for l in tvars for u in tvars if (l,lower) in graph and (upper,u) in graph}
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
                    add_secondary_constraints(&mut remaining, lt, ut, resolver)?;
                }
            }
        } else if c.op == crate::constraints::SUBTYPE_OF {
            let cv = tv_id(&c.origin_type_var).ok_or(())?;
            if uppers
                .get(&cv)
                .map_or(false, |bounds| bounds.contains(&c.target))
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
                add_secondary_constraints(&mut remaining, &lt, &c.target, resolver)?;
            }
        } else {
            // c.op == SUPERTYPE_OF
            let cv = tv_id(&c.origin_type_var).ok_or(())?;
            if lowers
                .get(&cv)
                .map_or(false, |bounds| bounds.contains(&c.target))
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
                add_secondary_constraints(&mut remaining, &c.target, &ut, resolver)?;
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
            if graph.contains(&(tv.clone(), other.clone())) || graph.contains(&(other.clone(), tv.clone())) {
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
            dfs(&v, &mut index, &mut boundaries, &mut stack, &mut identified, edges, &mut result);
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

    // Single-bound no-op solves return the bound itself; defer so the
    // original object identity survives (solve.py:387-390).
    let noop = (lowers.len() == 1 && filtered_uppers.is_empty())
        || (filtered_uppers.len() == 1 && lowers.is_empty());
    if noop {
        return Err(());
    }

    // Identity-bearing bounds: solving while a bound still holds a live
    // TypeVar would leak it into a candidate.
    if lowers
        .iter()
        .chain(filtered_uppers.iter())
        .any(crate::visitor::has_type_vars_inner)
    {
        return Err(());
    }

    let out = solve_one_inner(lowers, &filtered_uppers, infer_unions, strict_optional, resolver)
        .ok_or(())?;
    match out {
        (0, Some(bytes)) | (1, Some(bytes)) => {
            let typ = decode_type(&bytes).ok_or(())?;
            if wire_unsafe_solution(&typ) {
                return Err(());
            }
            Ok(Some(typ))
        }
        // kind=1 no-bytes: no solution (Python returns None).
        (1, None) => Ok(None),
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
            let has = lowers.get(tv).map_or(false, |b| !b.is_empty())
                || uppers.get(tv).map_or(false, |b| !b.is_empty());
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
) -> Result<Option<(Option<Vec<u8>>, Option<Vec<u8>>)>, ()> {
    let tvars: Vec<TvId> = vars
        .iter()
        .map(|t| tv_id(t).ok_or(()))
        .collect::<Result<_, _>>()?;

    let (mut graph, mut lowers, mut uppers) =
        transitive_closure(&tvars, constraints, resolver)?;

    let dmap = compute_dependencies(&tvars, &graph, &lowers, &uppers);

    let vertices: HashSet<TvId> = tvars.iter().cloned().collect();
    let sccs = strongly_connected_components(vertices, &dmap);
    if !sccs.iter().all(|s| check_linear(s, &lowers, &uppers)) {
        // Python returns the literal `({}, [])` (solve.py:253-254).
        return Ok(Some((None, None)));
    }
    let data = prepare_sccs(sccs, &dmap);
    let raw_batches = topsort(&data)?;

    // Free-variable pass over the first (leaf) batch only (solve.py:257-269).
    let mut free_vars: Vec<TvId> = Vec::new();
    let mut free_solutions: HashMap<TvId, Type> = HashMap::new();
    if let Some(first) = raw_batches.first() {
        for scc in first {
            let all_empty = scc
                .iter()
                .all(|tv| {
                    lowers.get(tv).map_or(true, |b| b.is_empty())
                        && uppers.get(tv).map_or(true, |b| b.is_empty())
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
            lowers.entry(u.clone()).or_default().push(free_solutions.get(&l).unwrap().clone());
        }
        if free_vars.contains(&u) {
            uppers.entry(l.clone()).or_default().push(free_solutions.get(&u).unwrap().clone());
        }
    }

    // Flatten SCC batches (solve.py:279-286).
    let mut solutions: Vec<(TvId, Option<Type>)> = Vec::new();
    for level in &raw_batches {
        let flat: Vec<TvId> = level.iter().flat_map(|s| s.iter().cloned()).collect();
        let res = solve_iteratively_native(
            &flat, &mut graph, &mut lowers, &mut uppers, infer_unions, strict_optional, resolver,
        )?;
        solutions.extend(res);
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
) -> Option<(i64, Option<Vec<u8>>, Option<Vec<u8>>)> {
    let mut var_types: Vec<Type> = Vec::with_capacity(vars.len());
    for b in &vars {
        var_types.push(decode_type(b)?);
    }
    let mut dep_constraints: Vec<Constraint> = Vec::with_capacity(constraints.len());
    for b in &constraints {
        let mut buf = ReadBuffer::new(b);
        dep_constraints.push(crate::constraints::Constraint::read(&mut buf).ok()?);
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
