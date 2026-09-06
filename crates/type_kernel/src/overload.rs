//! Overload dispatch: first-match index for `check_overload_call`.
//!
//! Ports the "Step 3" loop of `ExpressionChecker.check_overload_call`
//! (checkexpr.py:~3570) to a Rust-native first-match indexer.
//! Rust only accelerates the no-Any, no-union-arg, no-star-actual
//! path; generic targets ride the native constraint-solve kernel.
//! Returns `Option<usize>` — the index of the first matching callable
//! target, or `None` to defer to Python.
//!
//! Rust NEVER decides "no match" or "ambiguous". Any uncertainty
//! (subtype could-not-decide, decode failure, unexpected actual kind)
//! returns `None` immediately so Python re-runs the loop.

use pyo3::prelude::*;

use crate::argmap;
use crate::checkcall;
use crate::checkexpr_functions;
use crate::subtypes;
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// Whether a CallableType is a type object (i.e. its fallback is
/// `builtins.type`). Mirrors `CallableType.is_type_obj()` — the wire
/// format stores `fallback` + `from_concatenate` but not the computed
/// `is_type_obj` boolean, so we reconstruct it here.
fn is_type_obj(fallback: &Type, from_concatenate: bool) -> bool {
    if from_concatenate {
        return false;
    }
    matches!(
        fallback,
        Type::Instance { type_ref, .. } if type_ref == "builtins.type"
    )
}

// ArgKind integer values (nodes.py:2480-2517).
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

fn is_star(kind: i64) -> bool {
    kind == ARG_STAR || kind == ARG_STAR2
}

/// Decode a single wire blob; on any failure, returns `None`.
fn decode_type(blob: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(blob);
    read_type(&mut buf, None).ok()
}

/// Encode a type to wire bytes; on failure returns `None`.
#[allow(dead_code)] // kept for parity/debug symmetry with Python
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Native overload-dispatch first-match indexer.
///
/// Returns the index of the first matching target in `targets_bytes`,
/// or `None` to defer to Python's `infer_overload_return_type` loop.
///
/// The Python caller must have already:
///   * Checked `not any(map(has_any_type, arg_types))`.
///   * Checked `not any(self.real_union(arg) for arg in arg_types)`.
///
/// Algorithm contract:
///   1. Any actual with ARG_STAR / ARG_STAR2 -> defer.
///   2. Each target: must be a CallableType that is not a type object.
///      Non-conforming (incl. generic type-object callables) -> None.
///   3. Generic targets (own `variables`) are decided through the native
///      constraint-solve kernel (`rust_solve_generic_call`): solve first,
///      then evaluate the fully-substituted form like a plain target. A
///      ParamSpec / TypeVarTuple variable, a solve defer, or a residual
///      (still-unsubstituted) solved callable is undecided -> whole-call
///      defer, preserving Python's first-match order.
///   4. Plain targets: rust_map_actuals_to_formals -> None = unknown ->
///      defer; count / duplicate checks mirror Python map +
///      check_argument_count.
///   5. Subtype check per mapped (actual, formal): Some(false) with a
///      possible context-flip -> defer, None -> defer immediately.
///   6. First target where all formals pass -> Some(index).
///
/// `strict` mirrors `chk.in_checked_function()` and `infer_unions`
/// mirrors `type_state.infer_unions` at the solve call site
/// (checkexpr.py), both threaded through `rust_solve_generic_call`.
/// `strict_optional` mirrors `chk.options.strict_optional`.
///
/// Returns `None` on decode failures, buffer OOB, or any "could not
/// decide" signal. Rust NEVER decides "no match" or "ambiguous".
#[pyfunction]
#[pyo3(signature = (
    resolver,
    targets_bytes,
    arg_types_bytes,
    arg_kinds,
    strict_optional,
    arg_names = None,
    strict = true,
    infer_unions = false,
))]
#[allow(clippy::too_many_arguments)]
pub fn rust_check_overload_call(
    _py: Python<'_>,
    resolver: &NativeTypeResolver,
    targets_bytes: Vec<Vec<u8>>,
    arg_types_bytes: Vec<Vec<u8>>,
    arg_kinds: Vec<i64>,
    strict_optional: bool,
    arg_names: Option<Vec<Option<String>>>,
    strict: bool,
    infer_unions: bool,
) -> Option<usize> {
    // 0 targets -> defer.
    if targets_bytes.is_empty() {
        return None;
    }

    // Ambient `type_state.infer_unions` for the engine's unify reads; the
    // RAII guard restores the pre-call value on exit. The star-args early
    // return below runs before any engine call, so install is harmless.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);

    let arg_names_inner = arg_names.as_deref();

    // Step 1: Any actual with star? -> defer (whole thing).
    if arg_kinds.iter().any(|&k| is_star(k)) {
        return None;
    }

    let nformals_hint = targets_bytes.len();

    // Decode all arg types once.
    let arg_types: Vec<Type> = match arg_types_bytes
        .iter()
        .map(|b| decode_type(b))
        .collect::<Option<Vec<_>>>()
    {
        Some(v) => v,
        None => {
            return None;
        }
    };

    // Decode all targets once, validating shape. Callable targets (plain or
    // generic) stay; type-object fallbacks and non-callables defer whole call
    // (Python's check_call applies calibration + specials the wire cannot mirror).
    let mut decoded_targets: Vec<Type> = Vec::with_capacity(nformals_hint);
    for blob in &targets_bytes {
        let t = decode_type(blob);
        match &t {
            Some(Type::CallableType {
                fallback,
                from_concatenate,
                ..
            }) if !is_type_obj(fallback, *from_concatenate) => {
                decoded_targets.push(t.unwrap());
            }
            Some(Type::CallableType { .. }) => {
                return None; // type-object target -> defer whole call
            }
            Some(_) => {
                return None; // non-conforming target -> defer whole call
            }
            None => {
                return None;
            }
        }
    }

    let ctx = subtypes::SubtypeContext::new(false, false, false, false, false, strict_optional);

    for (idx, target) in decoded_targets.iter().enumerate() {
        let Type::CallableType { .. } = target else {
            return None; // should not happen after validation
        };

        // Generic targets (own type variables) ride the constraint-solve
        // kernel; a solved form is then evaluated like a plain target.
        let decision = if target_kinds_len_variables_nonempty(target) {
            evaluate_generic_target(
                _py,
                resolver,
                &targets_bytes[idx],
                &arg_types,
                &arg_types_bytes,
                &arg_kinds,
                arg_names_inner,
                strict,
                infer_unions,
                strict_optional,
                &ctx,
            )
        } else {
            evaluate_plain_target(
                target,
                &arg_types,
                &arg_types_bytes,
                &arg_kinds,
                arg_names_inner,
                resolver,
                &ctx,
            )
        };
        match decision {
            MatchDecision::Yes => return Some(idx),
            MatchDecision::No => {} // try next target
            MatchDecision::Undecided => return None,
        }
    }

    // All targets checked, none matched. Rust never decides "no match"
    // (issue #383: Rust returns None for uncertain, not for failure).
    None
}

/// Whether a decoded callable target carries its own type variables.
fn target_kinds_len_variables_nonempty(target: &Type) -> bool {
    match target {
        Type::CallableType { variables, .. } => !variables.is_empty(),
        _ => false, // unreachable after validation
    }
}

fn is_named(kind: i64) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT
}

/// Per-target match outcome for the first-match loop. `Undecided` defers
/// the whole call to Python; `No` only ever advances to the next target.
enum MatchDecision {
    Yes,
    No,
    Undecided,
}

/// Evaluate one plain (non-generic, non-type-object) callable target.
///
/// Mirrors old per-target steps 3-7: map actuals to formals, count and
/// duplicate checks, then a subtype check per mapped pair. Any fact the
/// kernel cannot decide is `Undecided` (whole-call defer), preserving
/// Python's first-match order.
#[allow(clippy::too_many_arguments)]
fn evaluate_plain_target(
    target: &Type,
    arg_types: &[Type],
    arg_types_bytes: &[Vec<u8>],
    arg_kinds: &[i64],
    arg_names: Option<&[Option<String>]>,
    resolver: &NativeTypeResolver,
    ctx: &subtypes::SubtypeContext,
) -> MatchDecision {
    let Type::CallableType {
        arg_kinds: target_kinds,
        arg_names: target_names,
        arg_types: target_arg_types,
        ..
    } = target
    else {
        return MatchDecision::Undecided; // unreachable after validation
    };

    // Step 3: map actuals -> formals. None = unknown -> defer.
    let formal_to_actual = match argmap::rust_map_actuals_to_formals(
        arg_kinds.to_vec(),
        arg_names.unwrap_or(&[]).to_vec(),
        target_kinds.clone(),
        target_names.clone(),
    ) {
        Some(m) => m,
        None => {
            return MatchDecision::Undecided;
        }
    };

    // Step 4a: required formal with no mapped actual -> not a match.
    for (fi, &fk) in target_kinds.iter().enumerate() {
        if fk == ARG_POS && formal_to_actual[fi].is_empty() {
            return MatchDecision::No;
        }
    }

    // Step 4b: extra actual not appearing in any formal's mapped list.
    for ai in 0..arg_kinds.len() {
        if !formal_to_actual
            .iter()
            .any(|list| list.contains(&(ai as i64)))
        {
            return MatchDecision::No;
        }
    }

    // Step 4c: named formal matched by a positional actual -> not a match.
    for (fi, &fk) in target_kinds.iter().enumerate() {
        if is_named(fk)
            && formal_to_actual[fi].iter().any(|&ai| {
                let ak = arg_kinds.get(ai as usize);
                ak == Some(&ARG_POS) || ak == Some(&ARG_OPT)
            })
        {
            return MatchDecision::No;
        }
    }

    // Step 5: duplicate mapping check.
    for mapped_indices in formal_to_actual.iter() {
        if mapped_indices.len() > 1 {
            let dup_types: Vec<Vec<u8>> = mapped_indices
                .iter()
                .filter_map(|&mi| arg_types_bytes.get(mi as usize).cloned())
                .collect();
            let dup_kinds: Vec<i64> = mapped_indices
                .iter()
                .filter_map(|&mi| arg_kinds.get(mi as usize).copied())
                .collect();
            if dup_types.len() != mapped_indices.len() {
                return MatchDecision::Undecided;
            }
            if checkexpr_functions::rust_is_duplicate_mapping(
                mapped_indices.clone(),
                dup_types,
                dup_kinds,
                resolver,
            )
            .ok()
            .flatten()
                == Some(true)
            {
                return MatchDecision::No;
            }
        }
    }

    // Steps 6 + 7: subtype check per mapped (actual, formal) pair.
    let resolver_ref = resolver.resolver();
    for (fi, mapped_indices) in formal_to_actual.iter().enumerate() {
        let Some(formal_type) = target_arg_types.get(fi) else {
            return MatchDecision::Undecided;
        };
        // Python's check_arg applies get_proper_type to both operands
        // before the per-pair subtype gate; expand a resolvable top-level
        // alias into its frozen target snapshot.
        let alias_formal_hold = match formal_type {
            Type::TypeAliasType { .. } => {
                match checkexpr_functions::get_proper_or_expand(
                    formal_type,
                    resolver.alias_resolver(),
                ) {
                    Some(t) => Some(t),
                    None => {
                        return MatchDecision::Undecided;
                    }
                }
            }
            _ => None,
        };
        // Python's get_proper_type on a UnionType maps each item through
        // get_proper_type; expand resolvable alias items inside a union
        // formal so the engine's union-right arm decides natively.
        let formal_base: &Type = alias_formal_hold.as_ref().map_or(formal_type, |t| t);
        let formal_union_hold: Option<Type> = match formal_base {
            Type::UnionType {
                items,
                uses_pep604_syntax,
                can_be_true,
                can_be_false,
                ..
            } => {
                if items
                    .iter()
                    .any(|it| matches!(it, Type::TypeAliasType { .. }))
                {
                    let expanded: Option<Vec<Type>> = items
                        .iter()
                        .map(|it| {
                            checkexpr_functions::get_proper_or_expand(it, resolver.alias_resolver())
                        })
                        .collect();
                    if expanded.is_none() {
                        return MatchDecision::Undecided;
                    }
                    expanded.map(|new_items| Type::UnionType {
                        items: new_items,
                        uses_pep604_syntax: *uses_pep604_syntax,
                        can_be_true: *can_be_true,
                        can_be_false: *can_be_false,
                        is_evaluated: true,
                        original_str_expr: None,
                        original_str_fallback: None,
                    })
                } else {
                    None
                }
            }
            _ => None,
        };
        for &ai in mapped_indices {
            let Some(actual) = arg_types.get(ai as usize) else {
                return MatchDecision::Undecided;
            };

            // UnionType actual: the shim's real_union gate filters real
            // unions upstream, so a residual one defers. AnyType is not
            // gated: is_subtype decides Any-left faithfully.
            if matches!(actual, Type::UnionType { .. }) {
                return MatchDecision::Undecided;
            }
            let alias_actual_hold = match actual {
                Type::TypeAliasType { .. } => {
                    match checkexpr_functions::get_proper_or_expand(
                        actual,
                        resolver.alias_resolver(),
                    ) {
                        Some(t) => Some(t),
                        None => {
                            return MatchDecision::Undecided;
                        }
                    }
                }
                _ => None,
            };
            let actual_use: &Type = alias_actual_hold.as_ref().map_or(actual, |t| t);
            let formal_use: &Type = formal_union_hold.as_ref().unwrap_or(formal_base);

            match subtypes::is_subtype(actual_use, formal_use, ctx, resolver_ref) {
                Some(true) => {}
                Some(false) => {
                    // A subtype-`false` may be overturned by context
                    // re-analysis (literal refinement, TypedDict checks,
                    // typevar instantiation); defer if any flip applies.
                    if let Some(_reason) = pair_flip_reason(actual_use, formal_use) {
                        return MatchDecision::Undecided;
                    }
                    return MatchDecision::No;
                }
                None => return MatchDecision::Undecided,
            }
        }
    }

    MatchDecision::Yes
}

/// Evaluate one generic target (callable with own type variables) by
/// driving the constraint-solve kernel over the first-match semantics:
/// solve, then evaluate the fully-substituted form like a plain target.
///
/// A ParamSpec / TypeVarTuple variable, a solve defer, or a residual
/// (still-unsubstituted) solved callable is `Undecided` -> whole-call
/// defer, preserving Python's first-match order.
#[allow(clippy::too_many_arguments)]
fn evaluate_generic_target(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    target_blob: &[u8],
    arg_types: &[Type],
    arg_types_bytes: &[Vec<u8>],
    arg_kinds: &[i64],
    arg_names: Option<&[Option<String>]>,
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
    ctx: &subtypes::SubtypeContext,
) -> MatchDecision {
    let Some(Type::CallableType {
        arg_kinds: target_kinds,
        arg_names: target_names,
        variables,
        ..
    }) = decode_type(target_blob)
    else {
        return MatchDecision::Undecided;
    };

    // The kernel defers on ParamSpec / TypeVarTuple variables (expand_type
    // cannot round-trip them); defer instead of asking for a defer blob.
    if variables.iter().any(|v| {
        matches!(
            v,
            Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
        )
    }) {
        return MatchDecision::Undecided;
    }

    let formal_to_actual = match argmap::rust_map_actuals_to_formals(
        arg_kinds.to_vec(),
        arg_names.unwrap_or(&[]).to_vec(),
        target_kinds,
        target_names,
    ) {
        Some(m) => m,
        None => {
            return MatchDecision::Undecided;
        }
    };

    let solved_bytes = match checkcall::rust_solve_generic_call(
        py,
        resolver,
        target_blob,
        arg_types_bytes.to_vec(),
        arg_kinds.to_vec(),
        formal_to_actual,
        strict,
        infer_unions,
        strict_optional,
        // The overload path has no Iterable/Mapping context: the expander
        // defers (Undecided) when a star arm needs one.
        None,
        None,
    ) {
        Some(b) => b,
        None => {
            return MatchDecision::Undecided;
        }
    };

    let solved = match decode_type(&solved_bytes) {
        Some(t) => t,
        None => {
            return MatchDecision::Undecided;
        }
    };

    let Type::CallableType {
        variables: solved_vars,
        ..
    } = &solved
    else {
        return MatchDecision::Undecided;
    };
    if !solved_vars.is_empty() {
        return MatchDecision::Undecided;
    }

    evaluate_plain_target(
        &solved,
        arg_types,
        arg_types_bytes,
        arg_kinds,
        arg_names,
        resolver,
        ctx,
    )
}

/// Whether `actual` could be re-derived by context re-analysis into a form
/// matching a literal formal whose fallback has the given base fullname.
/// Mirrors the fallback-family rule: only the exact base class (or a
/// typevar passthrough) can re-refine to that literal family.
// Context-flip analysis (issue #1094): Python re-infers each actual argument
// with the formal as expected context; a subtype-`false` on empty-context
// actuals can flip via literal refinement, TypedDict checks, or TypeVars.
fn literal_flip_possible(actual: &Type, fallback_type_ref: &str) -> bool {
    match actual {
        Type::Instance {
            type_ref,
            last_known_value: None,
            ..
        } => type_ref == fallback_type_ref,
        Type::TypeVarType { .. } => true,
        _ => false,
    }
}

/// The base fullname of a literal form's fallback, if it is a plain
/// argless Instance. `None` for anything else (defer-free no-flip).
fn literal_fallback_ref(fallback: &Type) -> Option<&str> {
    match fallback {
        Type::Instance { type_ref, .. } => Some(type_ref),
        _ => None,
    }
}

/// Why a subtype-`false` pair could still flip: `Some(reason)` defers.
fn pair_flip_reason(actual: &Type, formal: &Type) -> Option<&'static str> {
    match formal {
        // Literal families: only a same-value-class actual (or a typevar
        // passthrough) can re-refine into the literal's fallback family.
        Type::LiteralType { fallback, .. } => match literal_fallback_ref(fallback) {
            Some(fref) => literal_flip_possible(actual, fref).then_some("literal"),
            // Odd fallback: defer rather than reject.
            None => Some("literal_odd_fallback"),
        },
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => match lkv.as_ref() {
            Type::LiteralType { fallback, .. } => match literal_fallback_ref(fallback) {
                Some(fref) => literal_flip_possible(actual, fref).then_some("lkv_literal"),
                None => Some("lkv_odd_fallback"),
            },
            _ => Some("lkv_other"),
        },
        Type::TypeVarType { .. } => Some("formal_tvar"),
        Type::TypedDictType { .. } => Some("formal_typeddict"),
        // Position-matched nesting: refinement can also happen on the
        // leaves of a composite actual (tuple/dict/list literals), so
        // walk matching formal components when the actual mirrors them.
        Type::UnionType { items, .. } => items.iter().find_map(|i| pair_flip_reason(actual, i)),
        Type::Instance {
            last_known_value: None,
            args: formal_args,
            ..
        } => match actual {
            Type::Instance {
                args: actual_args,
                last_known_value: None,
                ..
            } if actual_args.len() == formal_args.len() => formal_args
                .iter()
                .zip(actual_args)
                .find_map(|(f, a)| pair_flip_reason(a, f)),
            Type::TypeVarType { .. } => Some("actual_tvar"),
            _ => None,
        },
        Type::TupleType {
            items: formal_items,
            ..
        } => match actual {
            Type::TupleType {
                items: actual_items,
                ..
            } if actual_items.len() == formal_items.len() => formal_items
                .iter()
                .zip(actual_items)
                .find_map(|(f, a)| pair_flip_reason(a, f)),
            Type::TypeVarType { .. } => Some("actual_tvar"),
            _ => None,
        },
        _ => None,
    }
}

/// Whether a wire `Type` is a TypeVarTuple/ParamSpec variant (or a
/// TypeVarType with a non-zero meta_level). The wire round-trip cannot
/// preserve the identity of these, so Python must always match them.
fn is_variadic_tvar(t: &Type) -> bool {
    match t {
        Type::TypeVarTupleType { .. } | Type::ParamSpecType { .. } => true,
        Type::TypeVarType { meta_level, .. } => *meta_level != 0,
        _ => false,
    }
}

/// `mypy.constraints.find_matching_overload_items` (constraints.py:1941).
///
/// Runs the native callable-compat engine (`callable_compat::callables_compatible`
/// with `ignore_return=True` and `is_proper_subtype=False`, exactly the
/// `is_callable_compatible(is_compat=is_subtype, ignore_return=True)` the
/// Python frame calls per item) for every `(item, template)` pair and returns
/// the indices of the matching items. Returns `None` to defer the whole call
/// to Python when:
///
///   * the template is not a plain `CallableType` (an `Overloaded` /
///     `Parameters` / unwrappable template stays Python-side);
///   * the item list is empty (Python then falls back to `items.copy()`);
///   * any item is a wire-unserializable variadic / meta TypeVar shape
///     (`TypeVarTupleType`, `ParamSpecType`, non-meta `TypeVarType`), whose
///     identity a wire round-trip cannot preserve (`is_variadic_tvar`);
///   * any item the engine cannot decide (generic `variables`, `UnpackType`,
///     `unpack_kwargs`, resolver misses) — all-or-nothing between the items,
///     mirroring the engine's own per-pair deferral contract.
///
/// The caller maps the returned indices back onto its live `CallableType`
/// items, preserving object identity the constraint solver relies on.
///
/// The `strict_optional` flag is threaded through the subtype context used
/// by the engine's nested `is_subtype` calls, matching Python where
/// `is_callable_compatible` inherits it from the running state — but only
/// when `strict_optional` is the active setting (`state.strict_optional`);
/// otherwise the whole call defers so error messages cannot diverge.
#[pyfunction]
#[pyo3(signature = (resolver, items_wire, template_wire, strict_optional, infer_unions = false))]
pub fn rust_find_matching_overload_items(
    _py: Python<'_>,
    resolver: &NativeTypeResolver,
    items_wire: Vec<&[u8]>,
    template_wire: &[u8],
    strict_optional: bool,
    infer_unions: bool,
) -> Option<Vec<i64>> {
    // Ambient for the per-item `callables_compatible_with_ignore_return`
    // engine reads; generic overload items route through the wave37
    // unify kernel, whose solve must see the Python ambient value.
    let _infer_unions_guard = crate::unify::InferUnionsGuard::install(infer_unions);
    if items_wire.is_empty() {
        return None;
    }
    let template = match decode_type(template_wire) {
        Some(t @ Type::CallableType { .. }) => t,
        Some(_) | None => return None,
    };

    // `has_variadic_arg` mirrors the wire round-trip's identity loss for
    // TypeVarTuple/ParamSpec/meta TypeVar shapes; template-side unpack
    // variants defer through the engine's own `any_unpack_anywhere`.
    let mut matched: Vec<i64> = Vec::new();
    for (idx, blob) in items_wire.iter().enumerate() {
        let item = match decode_type(blob) {
            Some(t @ Type::CallableType { .. }) => t,
            Some(_) | None => return None,
        };
        if has_variadic_arg(&item) {
            return None;
        }
        let ctx = subtypes::SubtypeContext::new(false, false, false, false, false, strict_optional);
        let res = resolver.resolver();
        match crate::callable_compat::callables_compatible_with_ignore_return(
            &item, &template, false, // ignore_pos_arg_names
            false, // strict_concatenate
            &ctx, res, true, // ignore_return: template return is indeterminate
        ) {
            Some(true) => matched.push(idx as i64),
            Some(false) => {}
            None => return None, // uncertain -> defer the whole call
        }
    }
    Some(matched)
}

/// Walks a callable's argument/return types for a wire-unserializable
/// TypeVarTuple/ParamSpec variant, or a non-meta TypeVarType. Defers the
/// whole item list so matching cannot drop or misorder items whose identity
/// the wire round-trip cannot preserve.
fn has_variadic_arg(t: &Type) -> bool {
    match t {
        Type::CallableType {
            arg_types,
            ret_type,
            ..
        } => arg_types.iter().any(is_variadic_tvar) || is_variadic_tvar(ret_type),
        _ => false,
    }
}

#[cfg(test)]
mod pair_flip_tests {
    use super::*;
    use crate::wire::LiteralValue;

    // Guard shape of `pair_flip_reason`: any decided reason defers.
    fn pair_flip_possible(actual: &Type, formal: &Type) -> bool {
        pair_flip_reason(actual, formal).is_some()
    }

    fn instance(fullname: &str) -> Type {
        Type::Instance {
            type_ref: fullname.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn list_of(args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: "builtins.list".to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn literal_of(fullname: &str, value: LiteralValue) -> Type {
        Type::LiteralType {
            fallback: Box::new(instance(fullname)),
            value,
        }
    }

    fn lkv_instance(fullname: &str, value: LiteralValue) -> Type {
        Type::Instance {
            type_ref: fullname.to_string(),
            args: vec![],
            last_known_value: Some(Box::new(literal_of(fullname, value))),
            extra_attrs: None,
        }
    }

    fn tvar() -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        }
    }

    fn tuple_of(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items,
            implicit: false,
        }
    }

    fn union_of(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    #[test]
    fn literal_formal_same_family_flips() {
        let formal = literal_of("builtins.int", LiteralValue::Int(4));
        assert!(pair_flip_possible(&instance("builtins.int"), &formal));
    }

    #[test]
    fn literal_formal_cross_family_no_flip() {
        let formal = literal_of("builtins.int", LiteralValue::Int(4));
        assert!(!pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn literal_formal_literal_actual_no_flip() {
        let formal = literal_of("builtins.int", LiteralValue::Int(4));
        assert!(!pair_flip_possible(&formal.clone(), &formal));
    }

    #[test]
    fn literal_formal_typevar_actual_defers() {
        let formal = literal_of("builtins.int", LiteralValue::Int(4));
        assert!(pair_flip_possible(&tvar(), &formal));
    }

    #[test]
    fn lkv_formal_same_family_defers() {
        let formal = lkv_instance("builtins.str", LiteralValue::Str("a".to_string()));
        assert!(pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn lkv_formal_cross_family_no_flip() {
        let formal = lkv_instance("builtins.str", LiteralValue::Str("a".to_string()));
        assert!(!pair_flip_possible(&instance("builtins.int"), &formal));
    }

    #[test]
    fn typed_dict_formal_defers() {
        let formal = Type::TypedDictType {
            fallback: Box::new(instance("typing.TypedDict")),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert!(pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn typevar_formal_defers() {
        assert!(pair_flip_possible(&instance("builtins.str"), &tvar()));
    }

    #[test]
    fn union_formal_flips_on_any_item() {
        let formal = union_of(vec![
            literal_of("builtins.int", LiteralValue::Int(4)),
            literal_of("builtins.str", LiteralValue::Str("x".to_string())),
        ]);
        assert!(pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn union_formal_all_plain_no_flip() {
        let formal = union_of(vec![instance("builtins.int"), instance("builtins.str")]);
        assert!(!pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn nested_instance_args_flip() {
        let formal = list_of(vec![literal_of("builtins.int", LiteralValue::Int(4))]);
        let actual = list_of(vec![instance("builtins.int")]);
        assert!(pair_flip_possible(&actual, &formal));
    }

    #[test]
    fn nested_instance_args_arity_mismatch_no_flip() {
        let formal = list_of(vec![literal_of("builtins.int", LiteralValue::Int(4))]);
        let actual = list_of(vec![instance("builtins.int"), instance("builtins.str")]);
        assert!(!pair_flip_possible(&actual, &formal));
    }

    #[test]
    fn tuple_items_flip() {
        let formal = tuple_of(vec![
            instance("builtins.int"),
            literal_of("builtins.str", LiteralValue::Str("x".to_string())),
        ]);
        let actual = tuple_of(vec![instance("builtins.int"), instance("builtins.str")]);
        assert!(pair_flip_possible(&actual, &formal));
    }

    #[test]
    fn tuple_shape_mismatch_no_flip() {
        let formal = tuple_of(vec![instance("builtins.int")]);
        assert!(!pair_flip_possible(&instance("builtins.str"), &formal));
    }

    #[test]
    fn plain_instances_no_flip() {
        assert!(!pair_flip_possible(
            &instance("builtins.str"),
            &instance("builtins.int")
        ));
        assert!(!pair_flip_possible(
            &instance("builtins.str"),
            &instance("builtins.str")
        ));
    }

    #[test]
    fn fallback_ref_helpers() {
        assert_eq!(
            literal_fallback_ref(&instance("builtins.int")),
            Some("builtins.int")
        );
        assert_eq!(
            literal_fallback_ref(&literal_of("builtins.int", LiteralValue::Int(1))),
            None
        );
        assert!(literal_flip_possible(&tvar(), "builtins.int"));
        assert!(!literal_flip_possible(
            &instance("builtins.str"),
            "builtins.int"
        ));
    }
}
