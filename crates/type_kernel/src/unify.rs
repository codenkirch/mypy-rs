//! `unify_generic_callable` (subtypes.py:2954-3011), ported into the type
//! kernel (issue #1426, wave 37). Fixes the blanket `return None` for a
//! generic `left` in `callable_compat::callables_compatible_with_ignore_return`
//! (the is_subtype engine's biggest cold-self-check defer wall: 320
//! `_is_subtype` defers, 236 of them Callable|Callable).
//!
//! Tri-state outcome mirroring the Python call site (subtypes.py:2590-2595):
//! - [`UnifyOutcome::Unified`]: continue `is_callable_compatible` with the
//!   unified left.
//! - [`UnifyOutcome::NoUnify`]: Python's `unified is None` arm; the caller
//!   answers `Some(false)` (subtypes.py:2592-2594).
//! - [`UnifyOutcome::Defer`]: the kernel cannot decide; the caller returns
//!   `None` so the Python shim re-runs the whole pure-Python path (which
//!   reproduces Python exactly, including the `had_errors` -> None arm).

use std::cell::Cell;

use crate::aliases::TypeAliasResolver;
use crate::constraints::{Constraint, SUBTYPE_OF};
use crate::solve::solve_constraints_poly_native;
use crate::wire::Type;

thread_local! {
    /// Mirror of `type_state.infer_unions` (typestate.py:110, default
    /// False), read by Python's ambient solve (solve.py:281, 372) which the
    /// kernel engine cannot reach from an `is_subtype` FFI depth. Set at the
    /// four `is_subtype`-family FFI entries from the shim-passed ambient
    /// value; engine-internal re-entries (fresh `SubtypeContext`s) then read
    /// exactly the ambient Python semantics, the same way a Python-side
    /// re-entry reads the ambient module state.
    static INFER_UNIONS: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard restoring the ambient `type_state.infer_unions` (the
/// pre-call value) on `Drop`, so a thread-local set by one FFI entry
/// never leaks into a later FFI call on the same thread that was not
/// handed the flag. `prev` is captured at install time, so nested or
/// interleaved installs restore correctly in reverse order.
#[must_use]
pub(crate) struct InferUnionsGuard {
    prev: bool,
    active: bool,
}

impl InferUnionsGuard {
    /// Stash the previous value and store `value` as the ambient flag.
    pub(crate) fn install(value: bool) -> Self {
        let prev = INFER_UNIONS.with(|s| s.replace(value));
        Self { prev, active: true }
    }
}

impl Drop for InferUnionsGuard {
    fn drop(&mut self) {
        if self.active {
            INFER_UNIONS.with(|s| s.set(self.prev));
        }
    }
}

pub(crate) fn infer_unions() -> bool {
    INFER_UNIONS.with(|s| s.get())
}

/// `unify_generic_callable` outcome (see module docs).
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum UnifyOutcome {
    Unified(Type),
    NoUnify,
    Defer,
}

/// `(raw_id, meta_level, namespace)`: Python `TypeVarId.__eq__`
/// (types.py:707-717); same key shape as `applytype::typevar_id_key`.
type TvKey = (i64, i64, String);

fn typevar_key(t: &Type) -> Option<TvKey> {
    match t {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        }
        | Type::ParamSpecType {
            raw_id,
            meta_level,
            namespace,
            ..
        }
        | Type::TypeVarTupleType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        _ => None,
    }
}

/// `type.copy_modified(ret_type=UninhabitedType())` on the wire
/// `CallableType` (Python preserves every other field, including
/// `special_signature`: applytype's rebuild is a different path and
/// deliberately does not share this).
fn strip_ret(t: &Type) -> Option<Type> {
    let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        ref arg_types,
        ref arg_kinds,
        ref arg_names,
        ref name,
        ref variables,
        ref type_guard,
        ref type_is,
        special_sig,
        ..
    } = t
    else {
        return None;
    };
    Some(Type::CallableType {
        fallback: fallback.clone(),
        instance_type: instance_type.clone(),
        is_ellipsis_args: *is_ellipsis_args,
        implicit: *implicit,
        is_bound: *is_bound,
        from_concatenate: *from_concatenate,
        imprecise_arg_kinds: *imprecise_arg_kinds,
        unpack_kwargs: *unpack_kwargs,
        from_type_type: *from_type_type,
        arg_types: arg_types.clone(),
        arg_kinds: arg_kinds.clone(),
        arg_names: arg_names.clone(),
        ret_type: Box::new(Type::UninhabitedType { ambiguous: false }),
        name: name.clone(),
        variables: variables.clone(),
        type_guard: type_guard.clone(),
        type_is: type_is.clone(),
        special_sig: special_sig.clone(),
    })
}

/// True when running the constraint inference with
/// `infer_polymorphic=False` is faithful to the Python call. The Python
/// fallback runs `visit_callable_type` under the ambient
/// `type_state.infer_polymorphic` (true for ordinary checking,
/// checkexpr.py:1325), which attaches `extra_tvars` at any proper-typed
/// actual that declares variables (constraints.py:1712, 1768), including
/// through the nested recursion where `skip_neg_op` is False again.
/// `solve_constraints` then solves the extra vars beside the formal ones,
/// and the kernel solve has no extra-var channel, so the port defers
/// those calls instead. `false` also covers the undecided walker
/// verdicts (a nested alias blocks visibility).
fn no_extra_tvar_shape(actual: &Type) -> bool {
    matches!(
        crate::visitor::callable_with_vars_reachable(actual),
        Some(false)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(name: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: name.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn callable(variables: Vec<Type>) -> Type {
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
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(instance("builtins.int", vec![])),
            name: None,
            variables,
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn overloaded(items: Vec<Type>) -> Type {
        Type::Overloaded { items }
    }

    fn tvar(raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        }
    }

    #[test]
    fn test_no_extra_shape_plain() {
        assert!(no_extra_tvar_shape(&callable(vec![])));
    }

    #[test]
    fn test_no_extra_shape_generic_root() {
        assert!(!no_extra_tvar_shape(&callable(vec![tvar(1)])));
    }

    #[test]
    fn test_no_extra_shape_overloaded_plain_items() {
        let o = overloaded(vec![callable(vec![]), callable(vec![])]);
        assert!(no_extra_tvar_shape(&o));
    }

    #[test]
    fn test_no_extra_shape_overloaded_generic_item() {
        let o = overloaded(vec![callable(vec![]), callable(vec![tvar(1)])]);
        assert!(!no_extra_tvar_shape(&o));
    }

    #[test]
    fn test_no_extra_shape_generic_nested_in_ret() {
        let mut outer = callable(vec![]);
        if let Type::CallableType { ret_type, .. } = &mut outer {
            *ret_type = Box::new(callable(vec![]));
            if let Type::CallableType {
                ret_type: inner, ..
            } = &mut outer
            {
                *inner = Box::new(callable(vec![tvar(1)]));
            }
        }
        assert!(!no_extra_tvar_shape(&outer));
    }

    #[test]
    fn test_no_extra_shape_alias_defers() {
        let a = Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
            is_recursive: false,
        };
        let mut c = callable(vec![]);
        if let Type::CallableType { arg_types, .. } = &mut c {
            arg_types.push(a);
        }
        assert!(!no_extra_tvar_shape(&c));
    }

    #[test]
    fn test_strip_ret_replaces_ret_keeps_everything_else() {
        let src = callable(vec![]);
        let stripped = strip_ret(&src).expect("callable strips");
        let Type::CallableType {
            ret_type,
            arg_types,
            variables,
            name,
            ..
        } = &stripped
        else {
            panic!("strip_ret must produce a callable");
        };
        assert!(matches!(
            ret_type.as_ref(),
            Type::UninhabitedType { ambiguous: false }
        ));
        assert!(arg_types.is_empty());
        assert!(variables.is_empty());
        assert!(name.is_none());
    }

    #[test]
    fn test_strip_ret_non_callable_defers() {
        assert!(strip_ret(&instance("builtins.int", vec![])).is_none());
    }

    // -- unify_generic_callable_core outcomes --

    fn callable_with(variables: Vec<Type>, args: Vec<Type>, ret: Type) -> Type {
        let mut c = callable(variables);
        if let Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            ..
        } = &mut c
        {
            *arg_types = args;
            *arg_kinds = vec![0; arg_types.len()];
            *arg_names = vec![None; arg_types.len()];
            *ret_type = Box::new(ret);
        }
        c
    }

    fn resolver_with(fullnames: &[&str]) -> crate::typeinfo::TypeResolver {
        let mut r = crate::typeinfo::TypeResolver::new();
        for f in fullnames {
            let mut s = crate::typeinfo::TypeInfoSnapshot {
                fullname: f.to_string(),
                name: f.to_string(),
                ..Default::default()
            };
            s.mro.push(f.to_string());
            s.has_base.insert(f.to_string());
            if *f != "builtins.object" {
                s.mro.push("builtins.object".to_string());
                s.has_base.insert("builtins.object".to_string());
            }
            r.insert(f.to_string(), s);
        }
        r
    }

    fn empty_aliases() -> TypeAliasResolver {
        TypeAliasResolver::new()
    }

    #[test]
    fn test_unify_defers_on_tvar_clash() {
        // Right's tree carries a TypeVar with the same raw id as one of
        // left's variables: freshening territory -> Defer.
        let left = callable(vec![tvar(1)]);
        let right = callable_with(vec![], vec![tvar(1)], instance("builtins.int", vec![]));
        let r = resolver_with(&["builtins.int", "builtins.object"]);
        let outcome = unify_generic_callable_core(&left, &right, false, true, &r, &empty_aliases());
        assert_eq!(outcome, UnifyOutcome::Defer);
    }

    #[test]
    fn test_unify_no_unify_on_unsolvable_var() {
        // Mixing a contravariant position with a plain position gives T a
        // lower (a.B <: T) and an unrelated upper (T <: a.A): strict solve
        // returns None (skip_unsatisfied=False) -> NoUnify.
        let left = callable_with(
            vec![tvar(1)],
            vec![
                callable_with(vec![], vec![tvar(1)], instance("builtins.int", vec![])),
                tvar(1),
            ],
            instance("builtins.int", vec![]),
        );
        let right = callable_with(
            vec![],
            vec![
                callable_with(
                    vec![],
                    vec![instance("a.B", vec![])],
                    instance("builtins.int", vec![]),
                ),
                instance("a.A", vec![]),
            ],
            instance("builtins.int", vec![]),
        );
        let r = resolver_with(&[
            "a.A",
            "a.B",
            "builtins.function",
            "builtins.object",
            // The nested callables' `-> int` ret pairs cross
            // visit_instance_native, which snapshots both classes.
            "builtins.int",
        ]);
        // ignore_return=true: the unsolvability lives in the arg
        // constraints; the ret pair is unrelated to the None verdict.
        let outcome = unify_generic_callable_core(&left, &right, true, true, &r, &empty_aliases());
        assert_eq!(outcome, UnifyOutcome::NoUnify);
    }

    #[test]
    fn test_unify_unified_substitutes_right_actuals() {
        // T's single constraint points at builtins.int: unified left gets
        // its arg substituted with int.
        let left = callable_with(
            vec![tvar(1)],
            vec![tvar(1)],
            instance("builtins.int", vec![]),
        );
        let right = callable_with(
            vec![],
            vec![instance("builtins.int", vec![])],
            instance("builtins.int", vec![]),
        );
        let r = resolver_with(&["builtins.int", "builtins.object"]);
        let outcome = unify_generic_callable_core(&left, &right, false, true, &r, &empty_aliases());
        match outcome {
            UnifyOutcome::Unified(t) => match t {
                Type::CallableType { arg_types, .. } => {
                    assert_eq!(arg_types, vec![instance("builtins.int", vec![])],);
                }
                other => panic!("expected CallableType, got {:?}", other),
            },
            other => panic!("expected Unified, got {:?}", other),
        }
    }
}

/// Core of `unify_generic_callable` (subtypes.py:2954-3011).
///
/// Both inputs must be the NORMALIZED callables (the caller's
/// `is_callable_compatible` head does `with_unpacked_kwargs()` +
/// `with_normalized_var_args()` first); this function does not
/// re-normalize. `return_constraint_direction` is hardcoded
/// `SUBTYPE_OF`: it is `None` at both Python call sites.
///
/// Defers (`Defer`) where Python's freshening would be needed (a
/// type-variable clash between `left`'s variables and `target`'s tree,
/// 1/236 measured) or where any kernel step cannot decide; the Python
/// fallback reproduces the call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn unify_generic_callable_core(
    left: &Type,
    right: &Type,
    ignore_return: bool,
    strict_optional: bool,
    resolver: &crate::typeinfo::TypeResolver,
    aliases: &TypeAliasResolver,
) -> UnifyOutcome {
    let Type::CallableType { variables, .. } = left else {
        return UnifyOutcome::Defer;
    };

    // subtypes.py:2966-2969: freshen when `type.type_var_ids()` clashes with
    // the type variables anywhere in `target`. Freshening needs the global
    // `TypeVarId.next_raw_id` counter (freshen.rs), so defer.
    let mut target_tvars: Vec<Type> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    if crate::typeops::collect_type_vars(right, true, Some(aliases), &mut seen, &mut target_tvars)
        .is_none()
    {
        return UnifyOutcome::Defer;
    }
    let target_keys: std::collections::HashSet<TvKey> =
        target_tvars.iter().filter_map(typevar_key).collect();
    if variables
        .iter()
        .filter_map(typevar_key)
        .any(|k| target_keys.contains(&k))
    {
        return UnifyOutcome::Defer;
    }

    // subtypes.py:2977-2983: arg constraints over the ret-stripped sides,
    // with `skip_neg_op=True`. The erased/infallible wrapper defaults ride
    // `infer_constraints_full_inner` (see `no_extra_tvar_shape` for the gate).
    let lstrip = match strip_ret(left) {
        Some(t) => t,
        None => return UnifyOutcome::Defer,
    };
    let rstrip = match strip_ret(right) {
        Some(t) => t,
        None => return UnifyOutcome::Defer,
    };
    if !no_extra_tvar_shape(&rstrip) {
        return UnifyOutcome::Defer;
    }
    let mut constraints: Vec<Constraint> = Vec::new();
    match crate::constraints::infer_constraints_full_inner(
        &lstrip,
        &rstrip,
        SUBTYPE_OF,
        resolver,
        aliases,
        strict_optional,
        true,  // skip_neg_op
        false, // infer_polymorphic
        true,  // erase_types
    ) {
        Some(cs) => constraints.extend(cs),
        None => return UnifyOutcome::Defer,
    }

    // subtypes.py:2984-2988: the return-type constraint, skipped under
    // `ignore_return` (constraints.py:1950's `ignore_return=True` shape).
    if !ignore_return {
        let (left_ret, right_ret) = match (left, right) {
            (Type::CallableType { ret_type: lr, .. }, Type::CallableType { ret_type: rr, .. }) => {
                (lr, rr)
            }
            _ => return UnifyOutcome::Defer,
        };
        // The ret call runs with `skip_neg_op=False`, so it is itself an
        // extras-attach site: gate on the proper-typed right ret the
        // dispatched call will see.

        // The dispatched call proper-types its operand, so the gate must
        // decide on the expanded shape; `expand_top_aliases` `None`
        // (snapshot miss, variadic alias, cycle) defers the call.
        let right_ret_p =
            match crate::subtypes::expand_top_aliases(right_ret, aliases, strict_optional) {
                Some(t) => t,
                None => return UnifyOutcome::Defer,
            };
        if !no_extra_tvar_shape(&right_ret_p) {
            return UnifyOutcome::Defer;
        }
        match crate::constraints::infer_constraints_full_inner(
            left_ret,
            right_ret,
            SUBTYPE_OF,
            resolver,
            aliases,
            strict_optional,
            false, // skip_neg_op
            false, // infer_polymorphic
            true,  // erase_types
        ) {
            Some(cs) => constraints.extend(cs),
            None => return UnifyOutcome::Defer,
        }
    }

    // subtypes.py:2989-2993: solve with `allow_polymorphic=True`
    // (`strict=True`, `skip_unsatisfied=False` at the call site). A `None`
    // solution is Python's `return None` -> `NoUnify`.
    let solutions = match solve_constraints_poly_native(
        variables,
        &constraints,
        infer_unions(),
        strict_optional,
        resolver,
        Some(aliases),
    ) {
        Ok(s) => s,
        Err(()) => return UnifyOutcome::Defer,
    };
    if solutions.iter().any(|s| s.is_none()) {
        return UnifyOutcome::NoUnify;
    }
    let orig_types: Vec<Option<Type>> = solutions.into_iter().flatten().map(Some).collect();

    // subtypes.py:3006-3010: `apply_generic_arguments(type, solutions,
    // report, context=target)`. `skip_unsatisfied=False`.

    // The kernel apply has no report channel: a `None` there conflates the
    // `had_errors` verdict with an undecided shape, so it defers and the
    // Python fallback reproduces the `had_errors -> None -> False` tail.
    match crate::applytype::apply_generic_arguments_inner(
        left,
        &orig_types,
        false, // skip_unsatisfied
        strict_optional,
        resolver,
        aliases,
    ) {
        Some(t) => UnifyOutcome::Unified(t),
        None => UnifyOutcome::Defer,
    }
}
