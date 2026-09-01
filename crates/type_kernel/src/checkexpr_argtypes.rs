//! Argument-expansion plan for `check_argument_types` (mypy.checkexpr).
//!
//! `ExpressionChecker.check_argument_types` (checkexpr.py:3664-3795)
//! expands each formal's actual arguments against the callee signature:
//! for every formal it derives the effective `callee_arg_types` /
//! `callee_arg_kinds` and `actual_types` / `actual_kinds`, deciding
//! between a checked expansion, a too-many, or a too-few error, then
//! drives the per-argument `ArgTypeExpander` + `check_arg` loop.
//!
//! This module ports the pure expansion/decision phase (the inner body
//! through the length-comparison) to Rust. The Python shim keeps the
//! stateful tail (error messages, `ArgTypeExpander`, `check_arg`), which
//! needs the live `ExpressionChecker` state.
//!
//! Strangler-fig contract: returns `None` (defer to the pure-Python
//! body) when Rust cannot reproduce the decision, i.e. a wire decode
//! failure, a callee-tuple `UnpackType` whose target is neither a tuple
//! nor a plain `builtins.tuple` Instance, an alias that needs argument
//! substitution on the wire (the substituted target cannot be carried),
//! a recursion/cycle in the aliases, or a TypeVarTuple/heterogeneous
//! unpack that a formal needs at call time. The per-formal plans are
//! returned as a `list[bytes]` (one blob per callee formal).
//!
//! Plan blob wire format (bare primitives, mirror-encoded for the Python
//! shim):
//!
//! - tag (bare int): 0 = success plan, 1 = too_many, 2 = too_few
//! - if tag == 0: `write_type_list` (`callee_arg_types`),
//!   `write_int_list` (`callee_arg_kinds`), `write_type_list`
//!   (`actual_types`), `write_int_list` (`actual_kinds`).
//!
//! Alias handling mirrors the wire seams' `expand_alias_target_raw`:
//! a `TypeAliasType` in a formal position is expanded to its snapped
//! target *without* type-argument substitution (`no_args` aliases or
//! empty-arg aliases only; a substitution-requiring alias defers). The
//! Python shim must not substitute again: the returned plan carries the
//! raw target, and `orig_callee_arg_type` semantics are preserved
//! because the target is the effective `callee_arg_type` Python would
//! derive (`get_proper_type`).

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

use crate::checkexpr_functions::expand_alias_target_raw;
use crate::typeinfo::NativeTypeResolver;
use crate::visitor::find_unpack_in_list_inner;
use crate::wire::{
    read_type, write_int_bare, write_int_list, write_type_list, ReadBuffer, Type, WriteBuffer,
};

const TAG_PLAN: i64 = 0;
const TAG_TOO_MANY: i64 = 1;
const TAG_TOO_FEW: i64 = 2;

/// `mypy.checkexpr.ExpressionChecker.check_argument_types`
/// (checkexpr.py:3664-3795), argument-expansion phase.
///
/// Inputs mirror the Python body: the actual argument types (wire blobs)
/// and kinds (ints; `nodes.ArgKind` values, 0-5), the `formal_to_actual`
/// mapping, and the serialized `callee` CallableType.
///
/// Returns a list of per-formal plan blobs (see module docs), or `None` to
/// defer to the pure-Python body.
#[pyfunction]
pub(crate) fn rust_check_argument_types_plan<'py>(
    py: Python<'py>,
    resolver: &NativeTypeResolver,
    arg_type_blobs: Vec<Vec<u8>>,
    arg_kinds: Vec<i64>,
    formal_to_actual: Vec<Vec<i64>>,
    callee_bytes: &[u8],
) -> PyResult<Option<&'py PyList>> {
    let plans = check_argument_types_plan_inner(
        resolver.alias_resolver(),
        &arg_type_blobs,
        &arg_kinds,
        &formal_to_actual,
        callee_bytes,
    );
    let Some(plans) = plans else {
        return Ok(None);
    };
    let out = PyList::empty(py);
    for plan in &plans {
        out.append(PyBytes::new(py, plan))?;
    }
    Ok(Some(out))
}

fn check_argument_types_plan_inner(
    aliases: &crate::aliases::TypeAliasResolver,
    arg_type_blobs: &[Vec<u8>],
    arg_kinds: &[i64],
    formal_to_actual: &[Vec<i64>],
    callee_bytes: &[u8],
) -> Option<Vec<Vec<u8>>> {
    let arg_types = decode_type_list(arg_type_blobs)?;
    let callee = decode_type(callee_bytes)?;
    let Type::CallableType {
        arg_types: callee_arg_types,
        arg_kinds: callee_arg_kinds,
        ..
    } = &callee
    else {
        return None;
    };

    let mut plans = Vec::with_capacity(formal_to_actual.len());
    // ZIP: mypy `for i, actuals in enumerate(formal_to_actual)` and
    // indexes callee.arg_types[i] / callee.arg_kinds[i]. If the wire
    // callable does not have an entry for a formal the Python body
    // would raise; defer on that mismatch (incl. arg_kinds shorter
    // than arg_types).
    if callee_arg_types.len() != callee_arg_kinds.len() {
        return None;
    }
    for i in 0..formal_to_actual.len() {
        let plan = plan_for_formal(
            i,
            aliases,
            &arg_types,
            arg_kinds,
            formal_to_actual,
            callee_arg_types,
            callee_arg_kinds,
        )?;
        plans.push(plan);
    }
    Some(plans)
}

/// Compute the per-formal plan, mirroring checkexpr.py:3688-3761.
fn plan_for_formal(
    i: usize,
    aliases: &crate::aliases::TypeAliasResolver,
    arg_types: &[Type],
    arg_kinds: &[i64],
    formal_to_actual: &[Vec<i64>],
    callee_arg_types: &[Type],
    callee_arg_kinds: &[i64],
) -> Option<Vec<u8>> {
    let actuals: Vec<usize> = formal_to_actual
        .get(i)?
        .iter()
        .map(|&a| usize::try_from(a).ok())
        .collect::<Option<_>>()?;
    let actual_kinds: Vec<i64> = actuals
        .iter()
        .map(|&a| arg_kinds.get(a).copied())
        .collect::<Option<_>>()?;

    let orig_callee_arg_type = get_proper_type_owned(callee_arg_types.get(i)?, aliases)?;

    // Checking the case that we have more than one item but the first
    // argument is an unpack, so this is something like:
    //   [Tuple[Unpack[Ts]], int].
    let mut expanded_tuple = false;
    let mut callee_types: Vec<Type> = Vec::new();
    let mut callee_kinds: Vec<i64> = Vec::new();
    let mut actual_types: Vec<Type> = Vec::new();

    if actuals.len() > 1 {
        let p_actual_type = get_proper_type_owned(arg_types.get(actuals[0])?, aliases)?;
        if let Type::TupleType { items, .. } = &p_actual_type {
            if items.len() == 1
                && matches!(items[0], Type::UnpackType { .. })
                && actual_kinds == star_then_pos(actuals.len() - 1)
            {
                actual_types = vec![items[0].clone()];
                for a in actuals.iter().skip(1) {
                    actual_types.push(arg_types.get(*a)?.clone());
                }
                if let Type::UnpackType { typ, .. } = &orig_callee_arg_type {
                    let p_callee_type = get_proper_type_owned(typ, aliases)?;
                    if let Type::TupleType { items, .. } = &p_callee_type {
                        if items.is_empty() {
                            // assert p_callee_type.items would fail in
                            // Python; defer.
                            return None;
                        }
                        callee_types = items.clone();
                        callee_kinds = star_then_pos(items.len() - 1);
                        expanded_tuple = true;
                    }
                }
            }
        }
    } else if let Type::UnpackType { typ, .. } = &orig_callee_arg_type {
        // len(actuals) == 1: Python's expanded_tuple arm does not
        // trigger (needs >1), so the unpack expands as if it were the
        // sole formal. Expand any alias in the unpack target.
        let unpacked_type = get_proper_type_owned(typ, aliases)?;
        if !matches!(
            unpacked_type,
            Type::TupleType { .. } | Type::TypeVarTupleType { .. } | Type::Instance { .. }
        ) {
            return None;
        }
    }

    if !expanded_tuple {
        actual_types = Vec::with_capacity(actuals.len());
        for &a in &actuals {
            actual_types.push(arg_types.get(a)?.clone());
        }
        if let Type::UnpackType { typ, .. } = &orig_callee_arg_type {
            let unpacked_type = get_proper_type_owned(typ, aliases)?;
            match unpacked_type {
                Type::TupleType { items, .. } => {
                    let inner_unpack_index = find_unpack_in_list_inner(&items);
                    if inner_unpack_index < 0 {
                        callee_types = items.clone();
                        callee_kinds = vec![ARG_POS; actuals.len()];
                    } else {
                        let inner = inner_unpack_index as usize;
                        let Type::UnpackType { typ: inner_typ, .. } = items.get(inner)? else {
                            // Python asserts UnpackType; defer.
                            return None;
                        };
                        let inner_unpacked_type = get_proper_type_owned(inner_typ, aliases)?;
                        match inner_unpacked_type {
                            Type::TypeVarTupleType { .. } => {
                                callee_types = items.clone();
                                callee_kinds = (0..items.len())
                                    .map(|j| if j == inner { ARG_STAR } else { ARG_POS })
                                    .collect();
                            }
                            Type::Instance { type_ref, args, .. }
                                if type_ref == "builtins.tuple" =>
                            {
                                // Heterogeneous tuples are desugared
                                // earlier: get_proper_type gives an
                                // Instance, Python asserts its item
                                // type exists on the wire.
                                let item_type = args.first()?;
                                // Python: `[item] * (len(actuals) -
                                // len(items) + 1)`, where a negative
                                // repeat count yields the empty list.
                                let repeat = actuals.len() as i64 - items.len() as i64 + 1;
                                let repeat = repeat.max(0) as usize;
                                callee_types = Vec::with_capacity(items.len() + repeat);
                                callee_types.extend_from_slice(&items[..inner]);
                                callee_types.extend(std::iter::repeat_n(item_type.clone(), repeat));
                                callee_types.extend_from_slice(&items[inner + 1..]);
                                callee_kinds = vec![ARG_POS; actuals.len()];
                            }
                            _ => {
                                // Python asserts Instance tuple; defer
                                // on anything else.
                                return None;
                            }
                        }
                    }
                }
                Type::TypeVarTupleType { .. } => {
                    callee_types = vec![orig_callee_arg_type.clone()];
                    callee_kinds = vec![ARG_STAR];
                }
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                    let item_type = args.first()?;
                    callee_types = vec![item_type.clone(); actuals.len()];
                    callee_kinds = vec![ARG_POS; actuals.len()];
                }
                // Python asserts Instance tuple when the entry is
                // positioned past the one-item-unpack rule's handling.
                _ => return None,
            }
        } else {
            callee_types = vec![orig_callee_arg_type.clone(); actuals.len()];
            callee_kinds = vec![*callee_arg_kinds.get(i)?; actuals.len()];
        }
    }

    // Length comparison (checkexpr.py:3763-3768).
    if callee_types.len() != actual_types.len() {
        let tag = if actual_types.len() > callee_types.len() {
            TAG_TOO_MANY
        } else {
            TAG_TOO_FEW
        };
        return encode_tag_only(tag);
    }
    encode_plan(&actual_types, &actual_kinds, &callee_types, &callee_kinds)
}

const ARG_POS: i64 = 0;
// nodes.ARG_STAR == 2
const ARG_STAR: i64 = 2;

/// `[ARG_STAR] + [ARG_POS] * n`.
fn star_then_pos(n: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(n + 1);
    v.push(ARG_STAR);
    v.extend(std::iter::repeat_n(ARG_POS, n));
    v
}

/// `mypy.types.get_proper_type` on the wire: expand a `TypeAliasType`
/// to its chain-resolved snapshot target when the alias carries no type
/// arguments and no substitution is required (mirrors
/// `expand_alias_target_raw`). Substitution-requiring aliases, missing
/// snapshots, cycles, and undecodable targets defer (`None`). Returns an
/// owned `Type` so callers clone freely.
fn get_proper_type_owned(typ: &Type, aliases: &crate::aliases::TypeAliasResolver) -> Option<Type> {
    if matches!(typ, Type::TypeAliasType { .. }) {
        return expand_alias_target_raw(typ, aliases);
    }
    Some(typ.clone())
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn decode_type_list(blobs: &[Vec<u8>]) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(blobs.len());
    for b in blobs {
        out.push(decode_type(b)?);
    }
    Some(out)
}

fn encode_tag_only(tag: i64) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_int_bare(&mut buf, tag).ok()?;
    Some(buf.into_bytes())
}

fn encode_plan(
    actual_types: &[Type],
    actual_kinds: &[i64],
    callee_types: &[Type],
    callee_kinds: &[i64],
) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_int_bare(&mut buf, TAG_PLAN).ok()?;
    write_type_list(&mut buf, callee_types).ok()?;
    write_int_list(&mut buf, callee_kinds).ok()?;
    write_type_list(&mut buf, actual_types).ok()?;
    write_int_list(&mut buf, actual_kinds).ok()?;
    Some(buf.into_bytes())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{read_int_bare, write_type};

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance(fullname: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: fullname.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tuple_type(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items,
            implicit: true,
        }
    }

    fn unpack(t: Type) -> Type {
        Type::UnpackType {
            typ: Box::new(t),
            from_star_syntax: false,
        }
    }

    fn callable(arg_types: Vec<Type>, arg_kinds: Vec<i64>) -> Type {
        Type::CallableType {
            fallback: Box::new(instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds,
            arg_names: vec![None; 0],
            ret_type: Box::new(any_type()),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    fn decode_plan(raw: &[u8]) -> (i64, Vec<Type>, Vec<i64>, Vec<Type>, Vec<i64>) {
        let mut buf = ReadBuffer::new(raw);
        let tag = read_int_bare(&mut buf).unwrap();
        if tag != TAG_PLAN {
            return (tag, vec![], vec![], vec![], vec![]);
        }
        let callee_types = crate::wire::read_type_list(&mut buf).unwrap();
        let callee_kinds = crate::wire::read_int_list(&mut buf).unwrap();
        let actual_types = crate::wire::read_type_list(&mut buf).unwrap();
        let actual_kinds = crate::wire::read_int_list(&mut buf).unwrap();
        (tag, callee_types, callee_kinds, actual_types, actual_kinds)
    }

    fn run(
        arg_types: &[Type],
        arg_kinds: &[i64],
        formal_to_actual: Vec<Vec<i64>>,
        callee: &Type,
    ) -> Option<Vec<Vec<u8>>> {
        let aliases = crate::aliases::TypeAliasResolver::new();
        let blobs: Vec<Vec<u8>> = arg_types.iter().map(encode).collect();
        check_argument_types_plan_inner(
            &aliases,
            &blobs,
            arg_kinds,
            &formal_to_actual,
            &encode(callee),
        )
    }

    #[test]
    fn simple_args_plan() {
        let callee = callable(vec![any_type(), any_type()], vec![ARG_POS, ARG_POS]);
        let arg_types = vec![any_type(), any_type()];
        let f2a = vec![vec![0], vec![1]];
        let plans = run(&arg_types, &[ARG_POS, ARG_POS], f2a, &callee).unwrap();
        assert_eq!(plans.len(), 2);
        let (tag, ct, ck, at, ak) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_PLAN);
        assert_eq!(ct.len(), 1);
        assert_eq!(ck, vec![ARG_POS]);
        assert_eq!(at.len(), 1);
        assert_eq!(ak, vec![ARG_POS]);
    }

    #[test]
    fn too_many_args() {
        // def f(x: tuple[int, str]); caller passes 3 positional actuals.
        let callee = callable(
            vec![unpack(tuple_type(vec![any_type(), any_type()]))],
            vec![ARG_POS],
        );
        let arg_types = vec![any_type(), any_type(), any_type()];
        let f2a = vec![vec![0, 1, 2]];
        let plans = run(&arg_types, &[ARG_POS, ARG_POS, ARG_POS], f2a, &callee).unwrap();
        let (tag, ..) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_TOO_MANY);
    }

    #[test]
    fn too_few_args() {
        // def f(x: tuple[int, str]); caller passes 1 positional actual.
        let callee = callable(
            vec![unpack(tuple_type(vec![any_type(), any_type()]))],
            vec![ARG_POS],
        );
        let arg_types = vec![any_type()];
        let f2a = vec![vec![0]];
        let plans = run(&arg_types, &[ARG_POS], f2a, &callee).unwrap();
        let (tag, ..) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_TOO_FEW);
    }

    #[test]
    fn empty_actuals_is_plain_plan() {
        // A formal with no mapped actuals yields an empty success plan
        // (the count errors live in check_argument_count, not here).
        let callee = callable(vec![any_type()], vec![ARG_POS]);
        let f2a = vec![vec![]];
        let plans = run(&[], &[], f2a, &callee).unwrap();
        let (tag, ct, ck, at, ak) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_PLAN);
        assert!(ct.is_empty());
        assert!(ck.is_empty());
        assert!(at.is_empty());
        assert!(ak.is_empty());
    }

    #[test]
    fn vararg_match() {
        // def f(*args: int); f(*(1,), 2) -> f2a both to formal 0.
        let callee = callable(vec![any_type()], vec![ARG_STAR]);
        let arg_types = vec![tuple_type(vec![any_type()]), any_type()];
        let f2a = vec![vec![0, 1]];
        let plans = run(&arg_types, &[ARG_STAR, ARG_POS], f2a, &callee).unwrap();
        let (tag, ct, ck, at, ak) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_PLAN);
        // Not a one-item UnpackType first tuple, so the plain branch:
        // callee_arg_types = [orig] * 2, actual_types = [tuple, int].
        assert_eq!(ct.len(), 2);
        assert_eq!(ck, vec![ARG_STAR, ARG_STAR]);
        assert_eq!(at.len(), 2);
        assert_eq!(ak, vec![ARG_STAR, ARG_POS]);
    }

    #[test]
    fn expanded_tuple_reunify() {
        // def f(x: Tuple[Unpack[Ts], int]); f(Tuple[Unpack[Ts], int])
        // with formal_to_actual [[0, 1]]: first actual is a one-item
        // unpacked tuple, second is int.
        let ts = Type::TypeVarTupleType {
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "m.Ts".to_string(),
            raw_id: 1,
            namespace: "m".to_string(),
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            min_len: 0,
        };
        let callee = callable(
            vec![unpack(tuple_type(vec![unpack(ts.clone()), any_type()]))],
            vec![ARG_POS],
        );
        // Caller passes a single tuple actual with an unpack, expanded
        // by the mapper: [Tuple[Unpack[Ts], int]].
        let caller_tuple = tuple_type(vec![unpack(tuple_type(vec![unpack(ts), any_type()]))]);
        let arg_types = vec![caller_tuple, any_type()];
        let f2a = vec![vec![0, 1]];
        let plans = run(&arg_types, &[ARG_STAR, ARG_POS], f2a, &callee).unwrap();
        let (tag, ct, ck, at, _ak) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_PLAN);
        assert_eq!(ct.len(), 2);
        assert_eq!(ck, vec![ARG_STAR, ARG_POS]);
        assert_eq!(at.len(), 2);
        assert!(matches!(at[0], Type::UnpackType { .. }));
    }

    #[test]
    fn typealias_missing_snapshot_defers() {
        // No snapshot for "m.A" in the resolver: the alias expansion
        // (which needs the snapshot) defers; the seam must refuse
        // instead of emitting a plan with the raw alias.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
        };
        let aliases = crate::aliases::TypeAliasResolver::new();
        assert!(get_proper_type_owned(&alias, &aliases).is_none());
    }

    #[test]
    fn typealias_noargs_target_resolves_shape() {
        // A no-args alias (`A = list[int]`) snaps to its raw target:
        // the shape-only expansion returns the tuple Instance.
        use crate::aliases::{AliasTvar, TypeAliasSnapshot};
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        let mut w = WriteBuffer::new();
        write_type(&mut w, &instance("builtins.tuple", vec![any_type()])).unwrap();
        aliases.insert(
            "m.A".to_string(),
            TypeAliasSnapshot {
                fullname: "m.A".to_string(),
                target: w.into_bytes(),
                alias_tvars: vec![],
                tvar_tuple_index: None,
                no_args: true,
                python_3_12_type_alias: false,
            },
        );
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
        };
        let expanded = get_proper_type_owned(&alias, &aliases).expect("no-args alias must expand");
        assert!(matches!(
            expanded,
            Type::Instance { type_ref, .. } if type_ref == "builtins.tuple"
        ));
        let _ = AliasTvar::default();
    }

    #[test]
    fn typealias_substitution_defers() {
        // `A[T] = list[T]`: the raw expansion refuses (substitution
        // required) and the plan defers to Python.
        use crate::aliases::TypeAliasSnapshot;
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        let mut w = WriteBuffer::new();
        write_type(&mut w, &instance("builtins.tuple", vec![any_type()])).unwrap();
        aliases.insert(
            "m.A".to_string(),
            TypeAliasSnapshot {
                fullname: "m.A".to_string(),
                target: w.into_bytes(),
                alias_tvars: vec![crate::aliases::AliasTvar {
                    name: "T".to_string(),
                    raw_id: 1,
                    meta_level: 0,
                    namespace: "m".to_string(),
                    is_type_var_tuple: false,
                }],
                tvar_tuple_index: None,
                no_args: false,
                python_3_12_type_alias: false,
            },
        );
        let alias = Type::TypeAliasType {
            args: vec![any_type()],
            type_ref: "m.A".to_string(),
        };
        let aliases_ref = &aliases;
        // Whole-call defer: the shim falls back to Python which expands
        // with the live target.
        let callee = callable(vec![alias], vec![ARG_POS]);
        let plans = run(&[any_type()], &[ARG_POS], vec![vec![0]], &callee);
        assert!(plans.is_none());
        let _ = aliases_ref;
    }

    #[test]
    fn unpacked_instance_tuple_match() {
        // def f(x: tuple[int, ...]); call with a plain int.
        let callee = callable(
            vec![unpack(instance("builtins.tuple", vec![any_type()]))],
            vec![ARG_POS],
        );
        let arg_types = vec![any_type(), any_type()];
        let f2a = vec![vec![0, 1]];
        let plans = run(&arg_types, &[ARG_POS, ARG_POS], f2a, &callee).unwrap();
        let (tag, ct, ck, at, ak) = decode_plan(&plans[0]);
        assert_eq!(tag, TAG_PLAN);
        assert_eq!(ct.len(), 2);
        assert_eq!(ck, vec![ARG_POS, ARG_POS]);
        assert_eq!(at.len(), 2);
        assert_eq!(ak, vec![ARG_POS, ARG_POS]);
    }
}
