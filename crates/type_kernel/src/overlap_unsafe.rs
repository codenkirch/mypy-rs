//! `is_unsafe_overlapping_overload_signatures` port (mypy.checker).
//!
//! Mirrors `mypy/checker.py:is_unsafe_overlapping_overload_signatures` as a
//! single Rust entry. The two callable wire blobs are detached, expanded
//! into all type-variable combinations, and judged (subset, overlap, and
//! callable compatibility) entirely in Rust:
//!
//! ```text
//! rust_is_unsafe_overlapping_overload_signatures(
//!     signature_bytes, other_bytes, class_type_vars_bytes,
//!     partial_only, strict_optional, resolver) -> Option<bool>
//! ```
//!
//! Returns `None` ("defer") on any unsupported shape; the Python seam then
//! falls through to the pure-Python implementation (strangler-fig per-call
//! gate). Deferral sources: non-`CallableType` inputs, bound methods during
//! expansion, `ParamSpecType`/`TypeVarTupleType` variables, `UnpackType`
//! arguments or `unpack_kwargs`, non-empty `left.variables` left after
//! expansion, and any `is_subtype`/`overlap` recursion the Rust core
//! cannot decide.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::callable_compat::{any_unpack_anywhere, are_parameters_compatible, callable_fields};
use crate::expandtype::{expand_type_inner, result_has_typevar};
use crate::meet::overlap;
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type};

/// Decode one full tagged `Type` from a blob.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// Decode a `LIST_GEN + bare size + N types` wire list.
fn decode_type_list(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type_list(&mut buf).ok()
}

/// `mypy.checker.detach_callable` on wire types (checker.py:9950-9977):
/// extends the callable's `variables` with `class_type_vars` so the signature
/// is independent of the class context. The empty fast path returns the type
/// unchanged.
fn detach_variables(c: &Type, class_type_vars: &[Type]) -> Option<Type> {
    if class_type_vars.is_empty() {
        return Some(c.clone());
    }
    let mut out = c.clone();
    let Type::CallableType { variables, .. } = &mut out else {
        return None;
    };
    variables.extend(class_type_vars.iter().cloned());
    Some(out)
}

/// `mypy.checker.expand_callable_variants` (checker.py:9841-9867).
///
/// A self-type (`raw_id == 0`) is expanded first with its upper bound and
/// dropped from `variables`. If variables remain, each contributes its
/// `values` (when non-empty) or `[upper_bound]` to the cartesian product;
/// every combination is substituted via `expand_type_inner` and marked
/// non-generic (`variables = []`). Defers (`None`) on
/// `ParamSpecType`/`TypeVarTupleType` variables or on any expansion the wire
/// cannot carry.
fn expand_callable_variants(c: &Type, strict_optional: bool) -> Option<Vec<Type>> {
    let mut cur = c.clone();
    let Type::CallableType { variables, .. } = &mut cur else {
        return None;
    };
    // Self-type first: it is the only variable whose upper bound can
    // reference other type variables (checker.py:9844-9849).
    let self_key = variables.iter().find_map(|v| match v {
        Type::TypeVarType {
            raw_id: 0,
            meta_level,
            namespace,
            ..
        } => Some((*meta_level, namespace.clone())),
        _ => None,
    });
    if let Some((meta_level, namespace)) = self_key {
        let bound = variables.iter().find_map(|v| match v {
            Type::TypeVarType {
                raw_id: 0,
                upper_bound,
                ..
            } => Some(upper_bound.clone()),
            _ => None,
        })?;
        let mut env: HashMap<(i64, i64, String), Type> = HashMap::new();
        env.insert((0, meta_level, namespace), *bound);
        let expanded = expand_type_inner(&cur, &env, strict_optional)?;
        cur = expanded;
        let Type::CallableType { variables: v, .. } = &mut cur else {
            return None;
        };
        v.retain(|t| !matches!(t, Type::TypeVarType { raw_id: 0, .. }));
    }

    let Type::CallableType {
        variables: vars, ..
    } = &cur
    else {
        return None;
    };
    if vars.is_empty() {
        // Fast path: not generic.
        return Some(vec![cur]);
    }
    let mut per_var: Vec<Vec<Type>> = Vec::with_capacity(vars.len());
    for v in vars {
        match v {
            Type::TypeVarType { values, .. } if !values.is_empty() => per_var.push(values.clone()),
            Type::TypeVarType { upper_bound, .. } => per_var.push(vec![(**upper_bound).clone()]),
            // ParamSpecType / TypeVarTupleType in variables defer.
            _ => return None,
        }
    }
    let mut indices = vec![0usize; per_var.len()];
    let mut variants: Vec<Type> = Vec::new();
    loop {
        let mut env: HashMap<(i64, i64, String), Type> = HashMap::with_capacity(vars.len());
        for (k, v) in vars.iter().enumerate() {
            let Type::TypeVarType {
                raw_id,
                meta_level,
                namespace,
                ..
            } = v
            else {
                unreachable!("per_var only fills from TypeVarType")
            };
            env.insert(
                (*raw_id, *meta_level, namespace.clone()),
                per_var[k][indices[k]].clone(),
            );
        }
        let mut variant = expand_type_inner(&cur, &env, strict_optional)?;
        let Type::CallableType { variables: v, .. } = &mut variant else {
            return None;
        };
        v.clear();
        // Clear `variables` before the residual check: the field still holds
        // the class's type parameters here, so scanning for type variables
        // while they are populated would spuriously defer every generic
        // callable. Python only re-checks the expanded body.
        if result_has_typevar(&variant) {
            return None;
        }
        variants.push(variant);
        // Increment the cartesian-product index; stop after the last row.
        let mut k = per_var.len();
        while k > 0 {
            k -= 1;
            if indices[k] + 1 < per_var[k].len() {
                indices[k] += 1;
                break;
            }
            indices[k] = 0;
        }
        if k == 0 {
            break;
        }
    }
    Some(variants)
}

fn ret_of(t: &Type) -> Option<&Type> {
    match t {
        Type::CallableType { ret_type, .. } => Some(ret_type),
        _ => None,
    }
}

/// `mypy.subtypes.is_callable_compatible` (subtypes.py:1883-2049) restricted
/// to the flag combination the overload-overlap check uses. `None` defers.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn is_callable_compatible_full(
    left: &Type,
    right: &Type,
    is_compat: &dyn Fn(&Type, &Type) -> Option<bool>,
    is_proper_subtype: bool,
    is_compat_return: Option<&dyn Fn(&Type, &Type) -> Option<bool>>,
    ignore_return: bool,
    check_args_covariantly: bool,
    allow_partial_overlap: bool,
) -> Option<bool> {
    let lf = callable_fields(left)?;
    let rf = callable_fields(right)?;

    // Normalization (subtypes.py:1983-1984): `with_unpacked_kwargs` rewrites
    // a TypedDict-typed **kwargs arg (the wire loses its key structure) and
    // `with_normalized_var_args` rewrites UnpackType var args. Both defer,
    // mirroring callable_compat.rs.
    if lf.unpack_kwargs || rf.unpack_kwargs {
        return None;
    }
    if any_unpack_anywhere(left) || any_unpack_anywhere(right) {
        return None;
    }
    // unify_generic_callable (subtypes.py:1929-1935) is unsupported on the
    // wire: non-empty left.variables defer. Our caller's expansion already
    // empties variables, so this only trips on shapes we never expanded.
    let Type::CallableType { variables, .. } = left else {
        return None;
    };
    if !variables.is_empty() {
        return None;
    }

    // subtypes.py:1989-1990.
    let ignore_pos_arg_names = lf.implicit || rf.implicit;
    // subtypes.py:1993-1994 (type-object branch) needs
    // allow_partial_overlap=False; every caller here passes True, so it is
    // unreachable and skipped.

    // subtypes.py:1941-1942.
    if !ignore_return {
        let compat_return = is_compat_return.unwrap_or(is_compat);
        if !compat_return(lf.ret_type, rf.ret_type)? {
            return Some(false);
        }
    }

    // subtypes.py:1944-1945.
    let params: Box<dyn Fn(&Type, &Type) -> Option<bool>> = if check_args_covariantly {
        Box::new(|l, r| is_compat(r, l))
    } else {
        Box::new(|l, r| is_compat(l, r))
    };

    // subtypes.py:1947-1950 with strict_concatenate=False.
    let strict_concatenate_check = !(lf.from_concatenate || rf.from_concatenate);

    are_parameters_compatible(
        lf.arg_types,
        lf.arg_kinds,
        lf.arg_names,
        lf.from_concatenate,
        rf.arg_types,
        rf.arg_kinds,
        rf.arg_names,
        rf.imprecise_arg_kinds,
        rf.is_ellipsis_args,
        &params,
        is_proper_subtype,
        ignore_pos_arg_names,
        allow_partial_overlap,
        strict_concatenate_check,
    )
}

/// `#[pyfunction]` entry: `is_unsafe_overlapping_overload_signatures`
/// (mypy/checker.py:9870-9947). `signature`/`other` are wire blobs of the two
/// `CallableType`s, `class_type_vars` a wire type-list of the class's
/// `TypeVarLikeType`s, and `partial_only`/`strict_optional` mirror the Python
/// flags and running state. Returns `Some(bool)` when Rust decided, `None`
/// (defer to the pure-Python path) otherwise.
#[pyfunction]
#[allow(clippy::unsafe_removed_from_name)]
pub(crate) fn rust_is_unsafe_overlapping_overload_signatures(
    signature: &[u8],
    other: &[u8],
    class_type_vars: &[u8],
    partial_only: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let sig = decode_type(signature)?;
    let oth = decode_type(other)?;
    if !matches!(sig, Type::CallableType { .. }) || !matches!(oth, Type::CallableType { .. }) {
        return None;
    }
    let class_vars = decode_type_list(class_type_vars)?;
    let sig = detach_variables(&sig, &class_vars)?;
    let oth = detach_variables(&oth, &class_vars)?;
    let sig_variants = expand_callable_variants(&sig, strict_optional)?;
    let oth_variants = expand_callable_variants(&oth, strict_optional)?;

    let res = resolver.resolver();
    let subset_ctx = SubtypeContext::new(false, false, true, true, false, strict_optional);
    let is_subset_np = |l: &Type, r: &Type| is_subtype(l, r, &subset_ctx, res);
    let is_overlapping = |l: &Type, r: &Type| overlap(l, r, strict_optional, true, true, res, 0);

    for sv in &sig_variants {
        for ov in &oth_variants {
            let sv_ret = ret_of(sv)?;
            let ov_ret = ret_of(ov)?;
            // checker.py:10024-10025.
            if is_subset_np(sv_ret, ov_ret)? {
                continue;
            }
            // checker.py:10026-10038 (direction A: sig vs other) — computed
            // lazily so a decided first direction avoids the second.
            let a = is_callable_compatible_full(
                sv,
                ov,
                &|l, r| is_overlapping(l, r),
                false,
                Some(&|l, r| is_subset_np(l, r).map(|b| !b)),
                false,
                false,
                true,
            )?;
            let b = if a {
                true
            } else {
                is_callable_compatible_full(
                    ov,
                    sv,
                    &|l, r| is_overlapping(l, r),
                    false,
                    Some(&|l, r| is_subset_np(r, l).map(|b| !b)),
                    false,
                    true,
                    true,
                )?
            };
            if !a && !b {
                continue;
            }
            // checker.py:10039-10044 (partial-only guard).
            if !partial_only {
                return Some(true);
            }
            let c = is_callable_compatible_full(
                ov,
                sv,
                &|l, r| is_subset_np(l, r),
                false,
                None,
                true,
                true,
                true,
            )?;
            if !c {
                return Some(true);
            }
        }
    }
    Some(false)
}
