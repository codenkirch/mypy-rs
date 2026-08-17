//! Native port of the checkexpr overload-result family (issue #489).
//!
//! Ports the pure-logic body of `ExpressionChecker.combine_function_signatures`
//! (checkexpr.py:4339-4413) onto the wire-format `Type` enum. The
//! `#[pyfunction]` returns `None` for cases Rust cannot handle, so the Python
//! caller falls through to the pure-Python implementation (the strangler-fig
//! per-call gate).
//!
//! Rust computes the merged callable directly (arg kinds, per-column unions,
//! return union, merged variables) and returns it as an encoded `Type`. The
//! Python caller decodes it and restores the live-only fields the wire format
//! cannot carry (`definition`, `fallback`, `special_sig`, line/column),
//! mirroring the `rust_check_callable_call` seam. `from_type_type` rides the
//! wire since issue #388.
//!
//! Also ports `merge_typevars_in_callables_by_name` (checkexpr.py:8138-8197)
//! via the freshen+expand machinery in `freshen.rs`/`expandtype.rs`:
//! each generic callable's declared `TypeVarType`s are freshened to fresh
//! unification variables (id >= `start_raw_id`, meta_level 1), the first
//! per-`fullname` fresh var becomes the exemplar, and all other same-named
//! vars are renamed to it by expanding with a `(raw_id, meta_level,
//! namespace) -> Type` env.
//!
//! Deferred (return None): not all items CallableType, `len(types) == 1`,
//! a generic callable whose declared variables include a non-`TypeVarType`
//! (ParamSpec/TypeVarTuple), a `make_simplified_union` deferral, or any
//! `expand_type` deferral. `plausible_overload_call_targets` and
//! `infer_overload_return_type` stay in Python: their leaf kernels
//! (`map_actuals_to_formals`, `check_argument_count`) are already
//! Rust-gated, and the inference spine needs `check_call`/`store_types`/
//! message wiring which the seam keeps in Python.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::setops::make_simplified_union;
use crate::subtypes::SubtypeContext;
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `ArgKind.ARG_POS` = 0. `ArgKind.ARG_STAR` = 2. `ArgKind.ARG_STAR2` = 4.
const ARG_POS: i64 = 0;
const ARG_STAR: i64 = 2;
const ARG_STAR2: i64 = 4;

/// `TypeOfAny.special_form` == 6.
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// `ArgKind.is_positional()` (mypy/nodes.py) — kinds 0, 1, 2 are positional.
fn arg_kind_is_positional(kind: i64) -> bool {
    kind <= ARG_STAR
}

fn simplified_union(
    items: &[Type],
    resolver: &NativeTypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    let sub_ctx = SubtypeContext {
        proper_subtype: false,
        strict_optional,
        ..Default::default()
    };
    make_simplified_union(items, &sub_ctx, resolver.resolver(), true)
}

/// Replace a callable's arg_types/arg_kinds/arg_names/ret_type/variables.
/// Field `None` means "keep the base value" (mirrors `copy_modified`).
fn rebuild_callable(
    base: &Type,
    arg_types: Option<Vec<Type>>,
    arg_kinds: Option<Vec<i64>>,
    arg_names: Option<Vec<Option<String>>>,
    ret_type: Option<&Type>,
    variables: Option<Vec<Type>>,
    implicit: bool,
) -> Option<Type> {
    if let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit: _,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        ret_type: base_ret,
        name,
        variables: base_variables,
        type_guard,
        type_is,
        ..
    } = base
    {
        Some(Type::CallableType {
            fallback: fallback.clone(),
            instance_type: instance_type.clone(),
            is_ellipsis_args: *is_ellipsis_args,
            implicit,
            is_bound: *is_bound,
            from_concatenate: *from_concatenate,
            imprecise_arg_kinds: *imprecise_arg_kinds,
            unpack_kwargs: *unpack_kwargs,
            from_type_type: *from_type_type,
            arg_types: arg_types.unwrap_or_else(|| arg_types_unreachable(base)),
            arg_kinds: arg_kinds.unwrap_or_else(|| arg_kinds_unreachable(base)),
            arg_names: arg_names.unwrap_or_else(|| arg_names_unreachable(base)),
            ret_type: Box::new(ret_type.map_or_else(|| base_ret.as_ref().clone(), |r| r.clone())),
            name: name.clone(),
            variables: variables.unwrap_or_else(|| base_variables.clone()),
            type_guard: type_guard.clone(),
            type_is: type_is.clone(),
        })
    } else {
        None
    }
}

/// Helper: extract arg_types from a callable (used with unwrap_or_else where
/// the value is guaranteed present).
fn arg_types_unreachable(t: &Type) -> Vec<Type> {
    match t {
        Type::CallableType { arg_types, .. } => arg_types.clone(),
        _ => unreachable!("checkexpr_overload: non-CallableType"),
    }
}

fn arg_kinds_unreachable(t: &Type) -> Vec<i64> {
    match t {
        Type::CallableType { arg_kinds, .. } => arg_kinds.clone(),
        _ => unreachable!("checkexpr_overload: non-CallableType"),
    }
}

fn arg_names_unreachable(t: &Type) -> Vec<Option<String>> {
    match t {
        Type::CallableType { arg_names, .. } => arg_names.clone(),
        _ => unreachable!("checkexpr_overload: non-CallableType"),
    }
}

fn set_callable_variables(typ: Type, variables: Vec<Type>) -> Type {
    if let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        type_guard,
        type_is,
        ..
    } = typ
    {
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        }
    } else {
        typ
    }
}

/// `TypeVarType.__eq__` env key: `(raw_id, meta_level, namespace)`.
fn var_env_key(v: &Type) -> (i64, i64, String) {
    match v {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => (*raw_id, *meta_level, namespace.clone()),
        _ => unreachable!("checkexpr_overload: non-TypeVarType variable"),
    }
}

// ---------------------------------------------------------------------------
// merge_typevars_in_callables_by_name (checkexpr.py:8138-8197)
// ---------------------------------------------------------------------------

/// Freshen + rename one callable, mirroring the Python loop:
/// ```python
/// target = freshen_function_type_vars(target)
/// rename = {}
/// for tv in target.variables:
///     name = tv.fullname
///     if name not in unique_typevars:
///         unique_typevars[name] = tv
///         variables.append(tv)
///     rename[tv.id] = unique_typevars[name]
/// target = expand_type(target, rename)
/// ```
///
/// Non-generic callables are returned unchanged. `next_id` is threaded so
/// fresh ids remain globally unique (Python advances `TypeVarId.next_raw_id`
/// for every minted variable, even renamed-away duplicates).
fn merge_freshen_and_rename(
    target: &Type,
    unique_typevars: &mut HashMap<String, Type>,
    variables: &mut Vec<Type>,
    next_id: &mut i64,
    strict_optional: bool,
) -> Option<Type> {
    let Type::CallableType { variables: tvs, .. } = target else {
        return None;
    };
    if tvs.is_empty() {
        return Some(target.clone());
    }

    // Step 1: freshen (`freshen_function_type_vars`, expandtype.py:379-395).
    // tvmap keys are the ORIGINAL var ids; the expanded callable carries the
    // fresh vars throughout its types and in `.variables`.
    let mut fresh_vars: Vec<Type> = Vec::with_capacity(tvs.len());
    let mut tvmap: HashMap<(i64, i64, String), Type> = HashMap::with_capacity(tvs.len());
    for v in tvs {
        if !matches!(v, Type::TypeVarType { .. }) {
            // ParamSpec/TypeVarTuple declared vars — defer to Python.
            return None;
        }
        let mut fresh = fresh_type_var(v, *next_id);
        *next_id += 1;
        if tvar_has_default(&fresh) {
            // Point defaults at fresh ids in case they depend on previous
            // variables (expandtype.py:390-392).
            let new_default = crate::expandtype::expand_type_inner(
                tvar_default(&fresh),
                &tvmap,
                strict_optional,
            )?;
            fresh = set_typevar_default(fresh, new_default);
        }
        tvmap.insert(var_env_key(v), fresh.clone());
        fresh_vars.push(fresh);
    }
    let mut freshened = crate::expandtype::expand_type_inner(target, &tvmap, strict_optional)?;
    freshened = set_callable_variables(freshened, fresh_vars.clone());

    // Step 2: rename — keyed by the freshened callable's variable ids,
    // mapping to the per-name exemplar.
    let Type::CallableType { variables: fvs, .. } = &freshened else {
        return None;
    };
    if fvs.is_empty() {
        return Some(freshened);
    }
    let mut rename: HashMap<(i64, i64, String), Type> = HashMap::with_capacity(fvs.len());
    for tv in fvs {
        let name = match tv {
            Type::TypeVarType { fullname, .. } => fullname.clone(),
            _ => return None,
        };
        let exemplar = if let Some(uniq) = unique_typevars.get(&name) {
            uniq.clone()
        } else {
            unique_typevars.insert(name.clone(), tv.clone());
            variables.push(tv.clone());
            tv.clone()
        };
        rename.insert(var_env_key(tv), exemplar);
    }
    crate::expandtype::expand_type_inner(&freshened, &rename, strict_optional)
}

/// `new_unification_variable` + `TypeVarId.new(meta_level=1)`: a
/// TypeVarType with a fresh `raw_id` and `meta_level` 1 (so `namespace`
/// reads as "" on wire encode).
fn fresh_type_var(v: &Type, raw_id: i64) -> Type {
    match v {
        Type::TypeVarType {
            name,
            fullname,
            values,
            upper_bound,
            default,
            variance,
            ..
        } => Type::TypeVarType {
            name: name.clone(),
            fullname: fullname.clone(),
            raw_id,
            namespace: String::new(),
            values: values.clone(),
            upper_bound: upper_bound.clone(),
            default: default.clone(),
            variance: *variance,
            meta_level: 1,
        },
        _ => unreachable!("checkexpr_overload: non-TypeVarType variable"),
    }
}

/// `has_default` (types.py:635-637): false only for an AnyType with
/// `TypeOfAny.from_omitted_generics` (4).
fn tvar_has_default(t: &Type) -> bool {
    let default = tvar_default(t);
    if let Type::AnyType { type_of_any, .. } = default {
        return *type_of_any != 4;
    }
    true
}

fn set_typevar_default(t: Type, new_default: Type) -> Type {
    if let Type::TypeVarType {
        name,
        fullname,
        raw_id,
        namespace,
        values,
        upper_bound,
        variance,
        meta_level,
        default: _,
    } = t
    {
        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default: Box::new(new_default),
            variance,
            meta_level,
        }
    } else {
        t
    }
}

fn tvar_default(t: &Type) -> &Type {
    match t {
        Type::TypeVarType { default, .. } => default.as_ref(),
        _ => unreachable!("checkexpr_overload: non-TypeVarType variable"),
    }
}

// ---------------------------------------------------------------------------
// rust_combine_function_signatures
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for the `combine_function_signatures` body.
///
/// Inputs:
///   * `types_bytes` — serialized `list[ProperType]` (the `types` arg).
///   * `start_raw_id` — `TypeVarId.next_raw_id` at call time; fresh
///     unification variables minted during merging get ids >= this value.
///   * `strict_optional` — forwarded to subtype/union decisions.
///
/// Returns `Some((next_raw_id, merged_callable_bytes))` or `None` to defer.
/// The Python caller decodes the merged callable and restores the live-only
/// fields from `types[0]`, then advances `TypeVarId.next_raw_id` to at least
/// `next_raw_id`.
#[pyfunction]
pub(crate) fn rust_combine_function_signatures(
    resolver: &NativeTypeResolver,
    types_bytes: Vec<Vec<u8>>,
    start_raw_id: i64,
    strict_optional: bool,
) -> PyResult<Option<(i64, Vec<u8>)>> {
    if types_bytes.is_empty() {
        return Ok(None);
    }
    let mut callables: Vec<Type> = Vec::with_capacity(types_bytes.len());
    for bytes in &types_bytes {
        let Some(t) = decode_type(bytes) else {
            return Ok(None);
        };
        // `if not all(isinstance(c, CallableType) for c in types)`
        if !matches!(t, Type::CallableType { .. }) {
            return Ok(None);
        }
        callables.push(t);
    }
    // `if len(callables) == 1: return callables[0]` — deferred.
    if callables.len() == 1 {
        return Ok(None);
    }

    // --- merge_typevars_in_callables_by_name ---
    let mut unique_typevars: HashMap<String, Type> = HashMap::new();
    let mut variables: Vec<Type> = Vec::new();
    let mut next_id = start_raw_id;
    let mut merged: Vec<Type> = Vec::with_capacity(callables.len());
    for target in &callables {
        let Some(m) = merge_freshen_and_rename(
            target,
            &mut unique_typevars,
            &mut variables,
            &mut next_id,
            strict_optional,
        ) else {
            return Ok(None);
        };
        merged.push(m);
    }

    let first = match &merged.first() {
        Some(Type::CallableType {
            arg_types,
            arg_kinds,
            ..
        }) => (arg_types.len(), arg_kinds.clone()),
        _ => return Ok(None),
    };
    let (arg_count, mut new_kinds) = first;

    let mut new_args: Vec<Vec<Type>> = vec![Vec::new(); arg_count];
    let mut new_returns: Vec<Type> = Vec::new();
    let mut too_complex = false;

    for target in &merged {
        let Type::CallableType {
            arg_types,
            arg_kinds,
            ret_type,
            ..
        } = target
        else {
            return Ok(None);
        };
        if new_kinds.len() != arg_kinds.len() {
            too_complex = true;
            break;
        }
        for (new_kind, target_kind) in std::iter::zip(new_kinds.iter_mut(), arg_kinds.iter()) {
            if *new_kind == *target_kind {
                continue;
            }
            if arg_kind_is_positional(*new_kind) && arg_kind_is_positional(*target_kind) {
                *new_kind = ARG_POS;
            } else {
                too_complex = true;
                break;
            }
        }
        if too_complex {
            break;
        }
        for (i, arg) in arg_types.iter().enumerate() {
            if i < arg_count {
                new_args[i].push(arg.clone());
            }
        }
        new_returns.push(ret_type.as_ref().clone());
    }

    // `union_return = make_simplified_union(new_returns)` runs in both branches.
    let Some(union_return) = simplified_union(&new_returns, resolver, strict_optional) else {
        return Ok(None);
    };

    // Assemble the result, mirroring `callables[0].copy_modified(...)`.
    let base = merged.first().unwrap();
    let final_type = if too_complex {
        let any = Type::AnyType {
            type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
            source_any: None,
            missing_import_name: None,
        };
        rebuild_callable(
            base,
            Some(vec![any.clone(), any]),
            Some(vec![ARG_STAR, ARG_STAR2]),
            Some(vec![None, None]),
            Some(&union_return),
            Some(variables),
            true,
        )
    } else {
        let mut arg_unions: Vec<Type> = Vec::new();
        for column in &new_args {
            let Some(union) = simplified_union(column, resolver, strict_optional) else {
                return Ok(None);
            };
            arg_unions.push(union);
        }
        rebuild_callable(
            base,
            Some(arg_unions),
            Some(new_kinds),
            None,
            Some(&union_return),
            Some(variables),
            true,
        )
    };
    let Some(final_type) = final_type else {
        return Ok(None);
    };
    let Some(encoded) = encode_type(&final_type) else {
        return Ok(None);
    };
    Ok(Some((next_id, encoded)))
}
