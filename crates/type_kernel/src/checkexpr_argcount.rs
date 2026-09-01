//! Native port of `check_argument_count` and `check_for_extra_actual_arguments`
//! from `mypy/checkexpr.py` (checkexpr.py:3359-3498).
//!
//! These functions are pure-computation over scalar facts (scalar-fact
//! interface, ADR-0002 Decision 3): the formal arg kinds and the
//! `param_spec()`-head bool are read live from the callee by the Python
//! shim, and the shim computes per-actual shape tags + item counts from
//! the proper types, so no wire serialization happens on this path. The
//! Python caller translates each error decision record into the
//! appropriate message call.
//!
//! Strangler-fig: Rust returns `None` for any input it cannot decide
//! (e.g. a proper `TypeAliasType` in the actual shapes, or a missing name
//! on a named extra actual). `None` means "defer to Python" — the Python
//! path re-runs the full function unchanged.

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type};

// ArgKind integer values (mirror `mypy.nodes.ARG_*`).
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

// Per-actual shape tags provided by the Python shim (scalar-fact
// interface, issue #1136): the shim classifies each actual's proper type
// live, so the seam needs neither the callee nor the actual types.
const ACTUAL_PLAIN: i64 = 0;
const ACTUAL_TUPLE: i64 = 1;
const ACTUAL_TYPEDDICT: i64 = 2;
const ACTUAL_PARAM_SPEC: i64 = 3;
// A proper `TypeAliasType` actual: expansion needs the live alias node,
// so any such shape makes the whole call defer.
const ACTUAL_ALIAS: i64 = 4;

// Error kind tags returned to Python. Each maps to a specific message call.
/// Extra unnamed actual (too_many_arguments).
pub const ERR_EXTRA_UNNAMED: i64 = 0;
/// Extra named actual (unexpected_keyword_argument).
pub const ERR_EXTRA_NAMED: i64 = 1;
/// Star tuple overflow (too_many_arguments from tuple items).
pub const ERR_TOO_MANY_TUPLE: i64 = 2;
/// TypedDict **kwargs overflow (too_many_arguments_from_typed_dict).
pub const ERR_TOO_MANY_TD: i64 = 3;
/// Required positional formal has no actual (too_few_arguments).
pub const ERR_TOO_FEW_POSITIONAL: i64 = 4;
/// Required named formal has no actual (missing_named_argument).
pub const ERR_MISSING_NAMED: i64 = 5;
/// Duplicate argument value (duplicate_argument_value).
pub const ERR_DUPLICATE: i64 = 6;
/// Positional actual where named formal expected (too_many_positional_arguments).
pub const ERR_TOO_MANY_POSITIONAL: i64 = 7;
/// ParamSpec formal has no actual (too_few_arguments).
pub const ERR_PARAMSPEC_TOO_FEW: i64 = 8;
/// ParamSpec.args passed more than once (fail).
pub const ERR_PARAMSPEC_ARGS_ONCE: i64 = 9;
/// ParamSpec.kwargs passed more than once (fail).
pub const ERR_PARAMSPEC_KWARGS_ONCE: i64 = 10;
/// missing_classvar_callable_note (only emitted when object_type and
/// callable_name are present and callable_name contains ".").  Python
/// needs object_type + callable_name + context to emit; we signal the
/// formal index so Python can look them up.
pub const ERR_MISSING_CLASSVAR_NOTE: i64 = 11;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `ArgKind.is_required()`: ARG_POS or ARG_NAMED.
fn is_required(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_NAMED
}

/// `ArgKind.is_positional(star=False)`: ARG_POS or ARG_OPT.
fn is_positional(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_OPT
}

/// `ArgKind.is_named(star=False)`: ARG_NAMED or ARG_NAMED_OPT.
fn is_named(kind: i64) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT
}

/// `ArgKind.is_star()`: ARG_STAR or ARG_STAR2.
fn is_star(kind: i64) -> bool {
    kind == ARG_STAR || kind == ARG_STAR2
}

/// `is_duplicate_mapping` (checkexpr.py:8922-8940), inner logic over the
/// shim-provided shape tags. Shapes are looked up by the mapped actual
/// index (`mapping[i]`), not by the mapping position (#1152), matching
/// Python's `actual_types[m]` facts.
fn is_duplicate_mapping_inner(
    mapping: &[i64],
    actual_kinds: &[i64],
    actual_shapes: &[i64],
) -> Option<bool> {
    if mapping.len() <= 1 {
        return Some(false);
    }
    // `f(..., *args, **kwargs)`: two actuals can share a formal.
    if mapping.len() == 2 {
        let first = *actual_kinds.get(mapping[0] as usize)?;
        let second = *actual_kinds.get(mapping[1] as usize)?;
        if first == ARG_STAR && second == ARG_STAR2 {
            return Some(false);
        }
    }
    // All non-TypedDict **kwargs actuals: duplicates allowed at runtime.
    // Python reads facts via `actual_types[m]`/`actual_kinds[m]` for each
    // mapped actual m; the shape must come from the same actual.
    let mut all_non_typeddict_star2 = true;
    for &idx in mapping {
        let kind = *actual_kinds.get(idx as usize)?;
        if kind != ARG_STAR2 {
            all_non_typeddict_star2 = false;
            break;
        }
        let shape = *actual_shapes.get(idx as usize)?;
        if shape == ACTUAL_TYPEDDICT {
            all_non_typeddict_star2 = false;
            break;
        }
    }
    Some(!all_non_typeddict_star2)
}

/// Result of `check_argument_count` + `check_for_extra_actual_arguments`,
/// returned as a flat tuple `(ok, errors, is_unexpected_arg_error)` so
/// PyO3 can convert it to a Python tuple without a `#[pyclass]`.
///
/// `ok` is the overall boolean (False if any error was found).
/// `is_unexpected_arg_error` mirrors Python's flag used to suppress
/// duplicate too-few-arguments errors. `errors` is a list of
/// `(kind, index, extra)` records where:
///   * `kind` is one of the `ERR_*` constants.
///   * `index` is the formal index (for formal errors) or actual index
///     (for actual errors).
///   * `extra` is a secondary index (unused, always 0; reserved for
///     future per-error metadata).
///
/// Error record: (kind, index, extra).
type ArgCountError = (i64, i64, i64);

/// Result: (ok, errors, is_unexpected_arg_error).
type ArgCountOutput = Option<(bool, Vec<ArgCountError>, bool)>;

/// Rust port of `check_argument_count` + `check_for_extra_actual_arguments`
/// (checkexpr.py:3359-3498) over scalar facts (issue #1136, scalar-fact
/// interface): no wire bytes cross the boundary.
///
/// Computes the full set of argument-count error decisions for a call
/// against a callable. Defers (`None`) whenever any actual's shape is
/// `ACTUAL_ALIAS`.
///
/// Parameters:
///   * `formal_kinds`: `int(ArgKind.value)` per formal, read live from the
///     callee by the shim.
///   * `has_param_spec`: whether the callee's `arg_types[-2]` (raw type) is
///     a `ParamSpecType` with the last two formals `*args`/`**kwargs`
///     (the `CallableType.param_spec()` head, computed shim-side).
///   * `special_sig`: the callee's `special_sig` string (not a wire fact,
///     and not a live-object fact; carried as a scalar).
///   * `actual_kinds`: `int(ArgKind.value)` per actual.
///   * `actual_names`: per-actual name or None.
///   * `actual_shapes`: per-actual shape tag (`ACTUAL_*`), from the
///     proper type of each actual.
///   * `actual_item_counts`: for tuple/TypedDict actuals, the item count
///     of the proper type; 0 otherwise.
///   * `formal_to_actual`: binding map (list of actual-index lists, per formal).
///   * `object_type_present`: whether `object_type` was passed (affects
///     `missing_classvar_callable_note`).
///   * `callable_name`: the callable's full name, or None (affects
///     `missing_classvar_callable_note`; `"." in callable_name` is checked).
///   * `in_checked_function`: whether the caller is in a checked function
///     (affects `duplicate_argument_value` emission, mirroring Python's
///     `self.chk.in_checked_function()` guard).
///
/// Returns `Option<(bool, Vec<(i64, i64, i64)>, bool)>` = `(ok, errors,
/// is_unexpected_arg_error)`, or `None` to defer.
#[pyfunction]
#[pyo3(signature = (
    formal_kinds,
    has_param_spec,
    special_sig,
    actual_kinds,
    actual_names,
    actual_shapes,
    actual_item_counts,
    formal_to_actual,
    object_type_present,
    callable_name,
    in_checked_function,
))]
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
pub(crate) fn rust_check_argument_count(
    formal_kinds: Vec<i64>,
    has_param_spec: bool,
    special_sig: Option<String>,
    actual_kinds: Vec<i64>,
    actual_names: Vec<Option<String>>,
    actual_shapes: Vec<i64>,
    actual_item_counts: Vec<i64>,
    formal_to_actual: Vec<Vec<i64>>,
    object_type_present: bool,
    callable_name: Option<String>,
    in_checked_function: bool,
) -> ArgCountOutput {
    check_argument_count_inner(
        &formal_kinds,
        has_param_spec,
        special_sig.as_deref(),
        &actual_kinds,
        &actual_names,
        &actual_shapes,
        &actual_item_counts,
        &formal_to_actual,
        object_type_present,
        callable_name.as_deref(),
        in_checked_function,
    )
}

/// Pure-decision body of `rust_check_argument_count`, mirroring the
/// pre-shape wire seam decision graph exactly.
#[allow(clippy::too_many_arguments)]
fn check_argument_count_inner(
    formal_kinds: &[i64],
    has_param_spec: bool,
    special_sig: Option<&str>,
    actual_kinds: &[i64],
    actual_names: &[Option<String>],
    actual_shapes: &[i64],
    actual_item_counts: &[i64],
    formal_to_actual: &[Vec<i64>],
    object_type_present: bool,
    callable_name: Option<&str>,
    in_checked_function: bool,
) -> ArgCountOutput {
    // The wire-era seam decoded every actual up front and deferred on any
    // TypeAliasType; the shim reports the same condition as ACTUAL_ALIAS.
    if actual_shapes.contains(&ACTUAL_ALIAS) {
        return None;
    }

    // Check for extra actual arguments (check_for_extra_actual_arguments).
    // Build all_actuals occurrence count.
    let mut all_actuals: Vec<i64> = vec![0; actual_kinds.len()];
    for actuals in formal_to_actual {
        for &a in actuals {
            if (a as usize) < all_actuals.len() {
                all_actuals[a as usize] += 1;
            }
        }
    }

    let mut errors: Vec<(i64, i64, i64)> = Vec::new();
    let mut ok = true;
    let mut is_unexpected_arg_error = false;

    // --- check_for_extra_actual_arguments (checkexpr.py:3439-3498) ---
    for (i, &kind) in actual_kinds.iter().enumerate() {
        let matched = i < all_actuals.len() && all_actuals[i] > 0;
        // Python: `if (i not in all_actuals and (kind != ARG_STAR or
        // is_non_empty_tuple(actual_types[i])) and kind != ARG_STAR2)`.
        let extra_actual_cond = if !matched {
            let is_non_empty_star = if kind == ARG_STAR {
                actual_shapes.get(i).copied()? == ACTUAL_TUPLE
                    && actual_item_counts.get(i).copied().unwrap_or(0) > 0
            } else {
                false
            };
            // Accept non-tuple iterables as star args (could be empty).
            // Accept all types for **kwargs (could be empty dicts).
            (kind != ARG_STAR || is_non_empty_star) && kind != ARG_STAR2
        } else {
            false
        };
        if extra_actual_cond {
            ok = false;
            if kind != ARG_NAMED {
                errors.push((ERR_EXTRA_UNNAMED, i as i64, 0));
            } else {
                // Named extra: need the name. If name is missing, defer.
                let _name = actual_names.get(i).and_then(|n| n.as_deref())?;
                errors.push((ERR_EXTRA_NAMED, i as i64, 0));
                is_unexpected_arg_error = true;
            }
        }
        // Python `elif (kind == ARG_STAR and ARG_STAR not in callee.arg_kinds)
        // or kind == ARG_STAR2`: leftover-items check.
        if !extra_actual_cond
            && ((kind == ARG_STAR && !formal_kinds.contains(&ARG_STAR)) || kind == ARG_STAR2)
        {
            let shape = actual_shapes.get(i).copied()?;
            let item_count = match shape {
                ACTUAL_TUPLE | ACTUAL_TYPEDDICT => actual_item_counts.get(i).copied(),
                _ => None,
            };
            if let Some(n_items) = item_count {
                let matched_count = if i < all_actuals.len() {
                    all_actuals[i]
                } else {
                    0
                };
                if matched_count < n_items {
                    ok = false;
                    if kind != ARG_STAR2 || shape != ACTUAL_TYPEDDICT {
                        errors.push((ERR_TOO_MANY_TUPLE, i as i64, 0));
                    } else {
                        errors.push((ERR_TOO_MANY_TD, i as i64, 0));
                        is_unexpected_arg_error = true;
                    }
                }
            }
            // Non-tuple/TypedDict star: *args/**kwargs can always succeed.
        }
    }

    // --- check_argument_count main loop (checkexpr.py:3394-3437) ---
    for (i, &kind) in formal_kinds.iter().enumerate() {
        let mapped_args = formal_to_actual.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        if is_required(kind) && mapped_args.is_empty() && !is_unexpected_arg_error {
            // No actual for a mandatory formal.
            if is_positional(kind) {
                errors.push((ERR_TOO_FEW_POSITIONAL, i as i64, 0));
                if object_type_present {
                    if let Some(cn) = callable_name {
                        if cn.contains('.') {
                            errors.push((ERR_MISSING_CLASSVAR_NOTE, i as i64, 0));
                        }
                    }
                }
            } else {
                errors.push((ERR_MISSING_NAMED, i as i64, 0));
            }
            ok = false;
        } else if !is_star(kind)
            && !mapped_args.is_empty()
            && is_duplicate_mapping_inner(mapped_args, actual_kinds, actual_shapes)?
        {
            // Duplicate mapping. Python emits only if in_checked_function
            // or the first mapped actual is a TupleType.
            let first_actual = mapped_args[0] as usize;
            let should_emit = in_checked_function
                || actual_shapes
                    .get(first_actual)
                    .copied()
                    .unwrap_or(ACTUAL_PLAIN)
                    == ACTUAL_TUPLE;
            if should_emit {
                errors.push((ERR_DUPLICATE, i as i64, 0));
                ok = false;
            }
        } else if is_named(kind) && !mapped_args.is_empty() && {
            let first_kind = actual_kinds
                .get(mapped_args[0] as usize)
                .copied()
                .unwrap_or(-1);
            first_kind != ARG_NAMED && first_kind != ARG_STAR2
        } {
            errors.push((ERR_TOO_MANY_POSITIONAL, i as i64, 0));
            ok = false;
        } else if has_param_spec {
            if mapped_args.is_empty() && special_sig != Some("partial") {
                errors.push((ERR_PARAMSPEC_TOO_FEW, i as i64, 0));
                ok = false;
            } else if mapped_args.len() > 1 {
                let mut paramspec_entries = 0i64;
                for &k in mapped_args {
                    let idx = k as usize;
                    if actual_shapes.get(idx).copied().unwrap_or(ACTUAL_PLAIN) == ACTUAL_PARAM_SPEC
                    {
                        paramspec_entries += 1;
                    }
                }
                let first_kind = actual_kinds
                    .get(mapped_args[0] as usize)
                    .copied()
                    .unwrap_or(-1);
                if first_kind == ARG_STAR && paramspec_entries > 1 {
                    errors.push((ERR_PARAMSPEC_ARGS_ONCE, i as i64, 0));
                    ok = false;
                }
                if first_kind == ARG_STAR2 && paramspec_entries > 1 {
                    errors.push((ERR_PARAMSPEC_KWARGS_ONCE, i as i64, 0));
                    ok = false;
                }
            }
        }
    }

    Some((ok, errors, is_unexpected_arg_error))
}

/// Rust port of `check_call_expr_with_callee_type`'s pure dispatch
/// (checkexpr.py:2146-2194).
///
/// The pure part is the `callable_name` derivation:
/// `if callable_name is None and member is not None: callable_name =
/// method_fullname(object_type, member)`. The rest of the function
/// (plugin hooks, union dispatch, check_call, type_guard caching) is
/// not pure and stays in Python.
///
/// Returns the derived `callable_name` as a string, or `None` to defer
/// to Python (e.g. when `object_type` is a `TypeAliasType` needing
/// expansion, or `method_fullname` cannot decide).
///
/// Parameters:
///   * `object_type_bytes`: wire-serialized `object_type` (or empty for None).
///   * `callable_name`: the current `callable_name` (or None).
///   * `member`: the member name (or None).
///   * `has_object_type`: whether `object_type` was passed (Python asserts
///     it is not None when `callable_name is None and member is not None`).
#[pyfunction]
#[pyo3(signature = (object_type_bytes, callable_name, member, has_object_type))]
pub(crate) fn rust_check_call_expr_callable_name(
    object_type_bytes: &[u8],
    callable_name: Option<String>,
    member: Option<String>,
    has_object_type: bool,
) -> Option<String> {
    // If callable_name is already set, no derivation needed.
    if callable_name.is_some() {
        return callable_name;
    }
    let member = member?;
    if !has_object_type {
        // Python asserts object_type is not None; we can't proceed.
        return None;
    }
    let object_type = decode_type(object_type_bytes)?;
    // get_proper_type: defer on TypeAliasType.
    let proper = match &object_type {
        Type::TypeAliasType { .. } => return None,
        t => t,
    };
    // method_fullname: "typefullname.member" for Instance/TypeType;
    // for CallableType with a name, "name.member";
    // for other types, None (Python returns "" which is falsy).
    let type_fullname: Option<&str> = match proper {
        Type::Instance { type_ref, .. } => Some(type_ref.as_str()),
        Type::TypeType { item, .. } => {
            if let Type::Instance { type_ref, .. } = item.as_ref() {
                Some(type_ref.as_str())
            } else {
                None
            }
        }
        Type::CallableType { name: Some(n), .. } => Some(n.as_str()),
        Type::CallableType { name: None, .. } => None,
        Type::TupleType {
            partial_fallback, ..
        } => {
            if let Type::Instance { type_ref, .. } = partial_fallback.as_ref() {
                Some(type_ref.as_str())
            } else {
                None
            }
        }
        Type::TypedDictType { fallback, .. } => {
            if let Type::Instance { type_ref, .. } = fallback.as_ref() {
                Some(type_ref.as_str())
            } else {
                None
            }
        }
        _ => None,
    };
    let tf = type_fullname?;
    if tf.is_empty() {
        return None;
    }
    Some(format!("{tf}.{member}"))
}

/// Mirror `check_union_call_expr`'s pure decision: whether the callee
/// object type is a `UnionType` that should be dispatched per-item.
/// Returns `Some(true)` if `object_type` is a (proper) `UnionType` and
/// `member is not None` and `callable_name is None` (the condition under
/// which Python takes the union-call branch). Returns `Some(false)` if
/// the union branch should NOT be taken. Returns `None` to defer (e.g.
/// `TypeAliasType` object type).
#[pyfunction]
pub(crate) fn rust_should_dispatch_union_call(
    object_type_bytes: &[u8],
    callable_name: Option<String>,
    member: Option<String>,
) -> Option<bool> {
    // Python: `elif member is not None and isinstance(object_type, UnionType)`.
    // Only reached when callable_name is None (the `if callable_name:` branch
    // fires first).  The caller already handles callable_name, so we only

    // check member + union.
    if member.is_none() {
        return Some(false);
    }
    if callable_name.is_some() {
        // If callable_name is set, the `if callable_name:` branch fires,
        // not the `elif`.  So the union path is not taken.
        return Some(false);
    }
    let object_type = decode_type(object_type_bytes)?;
    let proper = match &object_type {
        Type::TypeAliasType { .. } => return None,
        t => t,
    };
    Some(matches!(proper, Type::UnionType { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn object_type() -> Type {
        Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    // --- check_argument_count tests (scalar-fact seam) ---

    #[allow(clippy::too_many_arguments)]
    fn argcount_output(
        formal_kinds: &[i64],
        has_param_spec: bool,
        special_sig: Option<&str>,
        actual_kinds: &[i64],
        actual_names: &[Option<&str>],
        shapes: &[i64],
        counts: &[i64],
        formal_to_actual: Vec<Vec<i64>>,
        object_type_present: bool,
        callable_name: Option<&str>,
        in_checked_function: bool,
    ) -> ArgCountOutput {
        let names: Vec<Option<String>> = actual_names.iter().map(|n| n.map(String::from)).collect();
        check_argument_count_inner(
            formal_kinds,
            has_param_spec,
            special_sig,
            actual_kinds,
            &names,
            shapes,
            counts,
            &formal_to_actual,
            object_type_present,
            callable_name,
            in_checked_function,
        )
    }

    const PLAIN: i64 = ACTUAL_PLAIN;
    const TUPLE: i64 = ACTUAL_TUPLE;
    const TYPEDDICT: i64 = ACTUAL_TYPEDDICT;
    const PARAM_SPEC: i64 = ACTUAL_PARAM_SPEC;
    const ALIAS: i64 = ACTUAL_ALIAS;

    #[test]
    fn test_exact_pos_match_ok() {
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(r.0);
        assert!(r.1.is_empty());
        assert!(!r.2);
    }

    #[test]
    fn test_too_few_positional() {
        let result = argcount_output(
            &[ARG_POS, ARG_POS],
            false,
            None,
            &[ARG_POS],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0], vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        // formal 1 has no actual -> ERR_TOO_FEW_POSITIONAL
        assert!(r
            .1
            .iter()
            .any(|&(k, i, _)| k == ERR_TOO_FEW_POSITIONAL && i == 1));
    }

    #[test]
    fn test_too_few_with_classvar_note() {
        let result = argcount_output(
            &[ARG_POS, ARG_POS],
            false,
            None,
            &[ARG_POS],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0], vec![]],
            true,
            Some("mymod.MyClass.method"),
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_FEW_POSITIONAL));
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_MISSING_CLASSVAR_NOTE));
    }

    #[test]
    fn test_classvar_note_requires_dot() {
        let result = argcount_output(
            &[ARG_POS, ARG_POS],
            false,
            None,
            &[ARG_POS],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0], vec![]],
            true,
            Some("name_without_dot"),
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.1.iter().any(|&(k, _, _)| k == ERR_MISSING_CLASSVAR_NOTE));
    }

    #[test]
    fn test_missing_named_argument() {
        let result = argcount_output(
            &[ARG_NAMED],
            false,
            None,
            &[],
            &[],
            &[],
            &[],
            vec![vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_MISSING_NAMED));
    }

    #[test]
    fn test_extra_unnamed_actual() {
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_POS],
            &[None, None],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        // actual 1 is not in all_actuals -> ERR_EXTRA_UNNAMED
        assert!(r
            .1
            .iter()
            .any(|&(k, i, _)| k == ERR_EXTRA_UNNAMED && i == 1));
    }

    #[test]
    fn test_extra_named_actual() {
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_NAMED],
            &[None, Some("z")],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.2);
        assert!(r.1.iter().any(|&(k, i, _)| k == ERR_EXTRA_NAMED && i == 1));
    }

    #[test]
    fn test_extra_named_missing_name_defers() {
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_NAMED],
            &[None, None],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0]],
            false,
            None,
            true,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_duplicate_mapping() {
        // Two positional actuals mapping to the same formal -> duplicate.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_POS],
            &[None, None],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0, 1]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, i, _)| k == ERR_DUPLICATE && i == 0));
    }

    #[test]
    fn test_duplicate_suppressed_without_checked_function() {
        // in_checked_function=False and a non-tuple first actual: the
        // duplicate is detected but not emitted.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_POS],
            &[None, None],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0, 1]],
            false,
            None,
            false,
        );
        let r = result.expect("should decide");
        assert!(r.1.iter().all(|&(k, _, _)| k != ERR_DUPLICATE));
    }

    #[test]
    fn test_duplicate_emitted_for_tuple_actual() {
        // TupleType first actual stamps the duplicate even with the
        // checked-function guard off.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_POS, ARG_POS],
            &[None, None],
            &[TUPLE, PLAIN],
            &[1, 0],
            vec![vec![0, 1]],
            false,
            None,
            false,
        );
        let r = result.expect("should decide");
        assert!(r.1.iter().any(|&(k, i, _)| k == ERR_DUPLICATE && i == 0));
    }

    #[test]
    fn test_duplicate_mapping_star_args_kwargs_allowed() {
        // *args + **kwargs mapping to same formal: not a duplicate.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR, ARG_STAR2],
            &[None, None],
            &[PLAIN, TYPEDDICT],
            &[0, 2],
            vec![vec![0, 1]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.1.iter().any(|&(k, _, _)| k == ERR_DUPLICATE));
    }

    #[test]
    fn test_duplicate_mapping_all_kwargs_non_typeddict_allowed() {
        // Two **kwargs actuals typed as plain (non-TypedDict) instances:
        // duplicates allowed at runtime.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR2, ARG_STAR2],
            &[None, None],
            &[PLAIN, PLAIN],
            &[0, 0],
            vec![vec![0, 1]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.1.iter().any(|&(k, _, _)| k == ERR_DUPLICATE));
    }

    #[test]
    fn test_duplicate_mapping_typeddict_kwargs_defers() {
        // The TypedDict check on **kwargs actuals still defers when an
        // actual shape is ACTUAL_ALIAS.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR2, ARG_STAR2],
            &[None, None],
            &[PLAIN, ALIAS],
            &[0, 0],
            vec![vec![0, 1]],
            false,
            None,
            true,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_duplicate_mapping_shape_by_mapped_actual() {
        // Issue #1152 repro: 4x ARG_STAR2 actuals, only actual 0 is a
        // TypedDict; the shared formal maps actuals 2 and 3 (both plain).
        // Shapes must be indexed by mapped actual, not by position.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR2, ARG_STAR2, ARG_STAR2, ARG_STAR2],
            &[None; 4],
            &[TYPEDDICT, PLAIN, PLAIN, PLAIN],
            &[1, 0, 0, 0],
            vec![vec![2, 3]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        // Actual 0 (TypedDict, 1 item) is unmapped, so its leftover-items
        // error still fires; the duplicate record must not.
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_MANY_TD));
        assert!(r.1.iter().all(|&(k, _, _)| k != ERR_DUPLICATE));
    }

    #[test]
    fn test_too_many_positional_for_named_formal() {
        // Formal is ARG_NAMED but actual is ARG_POS.
        let result = argcount_output(
            &[ARG_NAMED],
            false,
            None,
            &[ARG_POS],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_MANY_POSITIONAL));
    }

    #[test]
    fn test_star_actual_not_matched_empty_iterable_ok() {
        // *args: list (not tuple) not matched to any formal. The star
        // actual itself is NOT an error (non-tuple iterables can be
        // empty), but the required ARG_POS formal with no actual IS an

        // error (too few positional). The star-actual branch must NOT
        // emit ERR_EXTRA_UNNAMED.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_FEW_POSITIONAL));
        assert!(!r.1.iter().any(|&(k, _, _)| k == ERR_EXTRA_UNNAMED));
    }

    #[test]
    fn test_star_actual_not_matched_empty_tuple_ok() {
        // *args: tuple[] is empty and matched to no formal, but the
        // star actual itself is not an error.
        let result = argcount_output(
            &[ARG_OPT],
            false,
            None,
            &[ARG_STAR],
            &[None],
            &[TUPLE],
            &[0],
            vec![vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(r.0);
        assert!(r.1.is_empty());
    }

    #[test]
    fn test_star_tuple_too_many_items() {
        // *args: tuple[int, str] only partially matched (1 of 2 items).
        // formal_to_actual: formal 0 gets actual 0 (1 item matched out of 2).
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR],
            &[None],
            &[TUPLE],
            &[2],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        // all_actuals[0] = 1, but tuple has 2 items -> too many.
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_MANY_TUPLE));
    }

    #[test]
    fn test_typeddict_kwargs_too_many_items() {
        // **kwargs of a 2-item TypedDict matched only 1 item.
        let result = argcount_output(
            &[ARG_POS],
            false,
            None,
            &[ARG_STAR2],
            &[None],
            &[TYPEDDICT],
            &[2],
            vec![vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.2);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_TOO_MANY_TD));
    }

    #[test]
    fn test_alias_anywhere_defers() {
        let result = argcount_output(
            &[ARG_OPT],
            false,
            None,
            &[ARG_STAR],
            &[None],
            &[ALIAS],
            &[0],
            vec![vec![]],
            false,
            None,
            true,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_call_no_formals_ok() {
        let result = argcount_output(
            &[],
            false,
            None,
            &[],
            &[],
            &[],
            &[],
            vec![],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(r.0);
        assert!(r.1.is_empty());
    }

    // --- param-spec tests (has_param_spec is now a shim-provided bool) ---

    #[test]
    fn test_param_spec_too_few() {
        let result = argcount_output(
            &[ARG_STAR, ARG_STAR2],
            true,
            None,
            &[],
            &[],
            &[],
            &[],
            vec![vec![], vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_PARAMSPEC_TOO_FEW));
    }

    #[test]
    fn test_param_spec_ok_with_single_actuals() {
        let result = argcount_output(
            &[ARG_STAR, ARG_STAR2],
            true,
            None,
            &[ARG_STAR],
            &[None],
            &[PLAIN],
            &[0],
            vec![vec![0], vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(r.0);
        assert!(r.1.is_empty());
    }

    #[test]
    fn test_param_spec_partial_special_sig_skip() {
        let result = argcount_output(
            &[ARG_STAR, ARG_STAR2],
            true,
            Some("partial"),
            &[],
            &[],
            &[],
            &[],
            vec![vec![], vec![]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(r.0);
        assert!(r.1.is_empty());
    }

    #[test]
    fn test_param_spec_args_once() {
        // Two ParamSpec actuals riding *args -> ParamSpec.args passed twice.
        let result = argcount_output(
            &[ARG_STAR, ARG_STAR2],
            true,
            None,
            &[ARG_STAR, ARG_STAR],
            &[None, None],
            &[PARAM_SPEC, PARAM_SPEC],
            &[0, 0],
            vec![vec![0, 1], vec![0]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_PARAMSPEC_ARGS_ONCE));
    }

    #[test]
    fn test_param_spec_kwargs_once() {
        // Two ParamSpec actuals riding **kwargs -> kwargs_once error.
        let result = argcount_output(
            &[ARG_STAR, ARG_STAR2],
            true,
            None,
            &[ARG_STAR2, ARG_STAR2],
            &[None, None],
            &[PARAM_SPEC, PARAM_SPEC],
            &[0, 0],
            vec![vec![0], vec![0, 1]],
            false,
            None,
            true,
        );
        let r = result.expect("should decide");
        assert!(!r.0);
        assert!(r.1.iter().any(|&(k, _, _)| k == ERR_PARAMSPEC_KWARGS_ONCE));
    }

    // --- check_call_expr_callable_name tests ---

    #[test]
    fn test_callable_name_already_set() {
        let result = rust_check_call_expr_callable_name(
            &[],
            Some("builtins.open".to_string()),
            Some("open".to_string()),
            false,
        );
        assert_eq!(result.as_deref(), Some("builtins.open"));
    }

    #[test]
    fn test_callable_name_derived_from_instance() {
        let obj = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let obj_bytes = encode(&obj);
        let result =
            rust_check_call_expr_callable_name(&obj_bytes, None, Some("append".to_string()), true);
        assert_eq!(result.as_deref(), Some("builtins.list.append"));
    }

    #[test]
    fn test_callable_name_no_member_returns_none() {
        let result = rust_check_call_expr_callable_name(&[], None, None, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_callable_name_no_object_type_returns_none() {
        let result =
            rust_check_call_expr_callable_name(&[], None, Some("method".to_string()), false);
        assert!(result.is_none());
    }

    #[test]
    fn test_callable_name_defer_on_alias() {
        // TypeAliasType cannot be serialized; undecodable bytes -> defer.
        let result =
            rust_check_call_expr_callable_name(&[0xFF], None, Some("method".to_string()), true);
        assert!(result.is_none());
    }

    // --- should_dispatch_union_call tests ---

    #[test]
    fn test_union_dispatch_true_for_union() {
        let union = Type::UnionType {
            items: vec![object_type(), any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let union_bytes = encode(&union);
        let result =
            rust_should_dispatch_union_call(&union_bytes, None, Some("method".to_string()));
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_union_dispatch_false_for_non_union() {
        let obj = object_type();
        let obj_bytes = encode(&obj);
        let result = rust_should_dispatch_union_call(&obj_bytes, None, Some("method".to_string()));
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_union_dispatch_false_when_callable_name_set() {
        let union = Type::UnionType {
            items: vec![object_type(), any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let union_bytes = encode(&union);
        let result = rust_should_dispatch_union_call(
            &union_bytes,
            Some("builtins.open".to_string()),
            Some("method".to_string()),
        );
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_union_dispatch_false_when_no_member() {
        let union = Type::UnionType {
            items: vec![object_type(), any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let union_bytes = encode(&union);
        let result = rust_should_dispatch_union_call(&union_bytes, None, None);
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_union_dispatch_defer_on_alias() {
        // TypeAliasType cannot be serialized; undecodable bytes -> defer.
        let result = rust_should_dispatch_union_call(&[0xFF], None, Some("method".to_string()));
        assert!(result.is_none());
    }
}
