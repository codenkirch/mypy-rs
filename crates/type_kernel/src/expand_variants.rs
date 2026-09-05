//! Native port of `mypy.checker.expand_callable_variants`
//! (checker.py:9841-9867) behind the checker type-kernel gate.
//!
//! Mirrors the Python: (1) expand the first self-type variable to its upper
//! bound and drop it from `variables`; (2) fast-path when the callable is no
//! longer generic; (3) cartesian-product every variable over its values (or
//! upper bound), expanding the callable once per combination and clearing
//! `variables` on each result. The declared type variables themselves are not
//! substituted, so we drive `expand_type_inner` directly: the
//! `expand_type_with_env` identity guard would defer on any still-generic
//! callable, which is exactly every non-fast-path input here.
//!
//! Defer to Python (return `None`) when any `variables` entry is a ParamSpec
//! or TypeVarTuple, or when the substitution produces a bound callable --
//! none of these have a faithful single-type wire representation.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::expandtype::{expand_type_inner, EnvKey};
use crate::wire::{read_type, write_type_list, ReadBuffer, Type, WriteBuffer};

#[pyfunction]
pub(crate) fn rust_expand_callable_variants(
    type_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let c = decode_type(type_bytes)?;
    let variants = expand_variants(&c, strict_optional)?;
    let mut wbuf = WriteBuffer::new();
    write_type_list(&mut wbuf, &variants).ok()?;
    Some(wbuf.into_bytes())
}

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Env key for a declared variable, mirroring the `TypeVarId` equality that
/// `expand_type_inner` uses (types.py:574-576).
fn tvar_key(v: &Type) -> Option<EnvKey> {
    match v {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        _ => None,
    }
}

/// The self-type variable has `raw_id == 0` (types.py:648-650).
fn is_self_var(v: &Type) -> bool {
    matches!(v, Type::TypeVarType { raw_id: 0, .. })
}

/// Replace a callable's `variables` (all other fields preserved).
fn with_vars(c: &Type, variables: &[Type]) -> Option<Type> {
    let mut t = c.clone();
    match &mut t {
        Type::CallableType { variables: v, .. } => *v = variables.to_vec(),
        _ => return None,
    }
    Some(t)
}

/// Substitute one combination, then drop the declared variables. The
/// substitution preserves the callable shape, so the result is always a
/// callable with `variables == []` when the call succeeds.
fn expand_variant(c: &Type, env: &HashMap<EnvKey, Type>, strict_optional: bool) -> Option<Type> {
    let expanded = expand_type_inner(c, env, strict_optional)?;
    with_vars(&expanded, &[])
}

/// Core mirror of `expand_callable_variants`; `None` means "defer to Python".
fn expand_variants(c: &Type, strict_optional: bool) -> Option<Vec<Type>> {
    let Type::CallableType {
        variables,
        is_bound,
        ..
    } = c
    else {
        return None;
    };
    // ParamSpec and TypeVarTuple variables cannot be substituted individually,
    // matching the caller's requirement never to return a partial result.
    for v in variables {
        if matches!(
            v,
            Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
        ) {
            return None;
        }
    }
    // Bound methods defer: `__self`/`__cls` identity does not survive the
    // wire round-trip (consistent with `expand_type_inner`).
    if *is_bound {
        return None;
    }

    // Self-type branch (checker.py:9842-9848): expand the first self
    // variable to its upper bound, then drop it from `variables`.
    let (working, combo_vars) = if let Some(self_tv) = variables.iter().find(|v| is_self_var(v)) {
        let bound = match self_tv {
            Type::TypeVarType { upper_bound, .. } => *upper_bound.clone(),
            _ => return None,
        };
        let key = tvar_key(self_tv)?;
        let env = HashMap::from([(key, bound)]);
        let expanded = expand_type_inner(c, &env, strict_optional)?;
        let remaining: Vec<Type> = variables
            .iter()
            .filter(|v| !is_self_var(v))
            .cloned()
            .collect();
        (with_vars(&expanded, &remaining)?, remaining)
    } else {
        (c.clone(), variables.clone())
    };

    // Fast path (checker.py:9850-9852): no variables left, single variant.
    if combo_vars.is_empty() {
        return Some(vec![working]);
    }

    // Per-variable substitution choices (checker.py:9854-9858): literal
    // values when present, else the upper bound.
    let mut choices: Vec<Vec<Type>> = Vec::with_capacity(combo_vars.len());
    for v in &combo_vars {
        let vals = match v {
            Type::TypeVarType { values, .. } if !values.is_empty() => values.clone(),
            Type::TypeVarType { upper_bound, .. } => vec![*upper_bound.clone()],
            _ => return None,
        };
        if vals.is_empty() {
            return None;
        }
        choices.push(vals);
    }

    // Cartesian product, last factor varying fastest (itertools.product).
    let total: usize = choices.iter().map(|c| c.len()).product();
    let mut variants = Vec::with_capacity(total);
    let mut idx: Vec<usize> = vec![0; choices.len()];
    for _ in 0..total {
        let mut env = HashMap::with_capacity(choices.len());
        for (i, tv) in combo_vars.iter().enumerate() {
            let key = tvar_key(tv)?;
            env.insert(key, choices[i][idx[i]].clone());
        }
        variants.push(expand_variant(&working, &env, strict_optional)?);
        // Increment the counter rightmost-first to match `itertools.product`.
        let mut carry = choices.len();
        while carry > 0 {
            carry -= 1;
            idx[carry] += 1;
            if idx[carry] < choices[carry].len() {
                break;
            }
            idx[carry] = 0;
        }
    }
    Some(variants)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tvar(raw_id: i64, values: Vec<Type>) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "__main__.T".to_string(),
            raw_id,
            namespace: String::new(),
            values,
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn param_spec(raw_id: i64) -> Type {
        Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "__main__.P".to_string(),
            raw_id,
            namespace: String::new(),
            flavor: 0,
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            meta_level: 0,
        }
    }

    fn callable(arg_types: Vec<Type>, variables: Vec<Type>) -> Type {
        Type::CallableType {
            fallback: Box::new(instance("builtins.function", Vec::new())),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(any()),
            name: None,
            variables,
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    #[test]
    fn non_generic_fast_path_returns_single_variant() {
        let c = callable(vec![instance("builtins.int", Vec::new())], Vec::new());
        let variants = expand_variants(&c, false).unwrap();
        assert_eq!(variants.len(), 1);
        assert!(matches!(
            variants[0],
            Type::CallableType { variables: ref v, .. } if v.is_empty()
        ));
    }

    #[test]
    fn generic_substitutes_bounds_and_clears_variables() {
        // def f[T]() -> T with T substituted by its upper bound.
        let c = callable(Vec::new(), vec![tvar(1, Vec::new())]);
        let variants = expand_variants(&c, false).unwrap();
        assert_eq!(variants.len(), 1);
        // The env built from the upper bound (Any) substituted into the
        // ret_type (Any): assert the produced callable has no variables.
        match &variants[0] {
            Type::CallableType { variables, .. } => assert!(variables.is_empty()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn values_product_produces_count_variants() {
        // def f[V](x: V) with V in {int, str} -> 2 variants.
        let c = callable(
            vec![tvar(1, Vec::new())],
            vec![tvar(
                2,
                vec![
                    instance("builtins.int", Vec::new()),
                    instance("builtins.str", Vec::new()),
                ],
            )],
        );
        let variants = expand_variants(&c, false).unwrap();
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn param_spec_variable_defers() {
        let c = callable(Vec::new(), vec![param_spec(1)]);
        assert!(expand_variants(&c, false).is_none());
    }

    #[test]
    fn self_type_expands_to_upper_bound() {
        // Self (raw_id 0) with upper_bound A expands A into the arg_types.
        let self_var = Type::TypeVarType {
            name: "Self".to_string(),
            fullname: "__main__.Self".to_string(),
            raw_id: 0,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(instance("main.A", Vec::new())),
            default: Box::new(any()),
            variance: 0,
            meta_level: 0,
        };
        let c = callable(vec![self_var.clone()], vec![self_var]);
        let variants = expand_variants(&c, false).unwrap();
        assert_eq!(variants.len(), 1);
        match &variants[0] {
            Type::CallableType {
                arg_types,
                variables,
                ..
            } => {
                assert!(matches!(
                    arg_types.as_slice(),
                    [Type::Instance { type_ref, .. }] if type_ref == "main.A"
                ));
                assert!(variables.is_empty());
            }
            _ => unreachable!(),
        }
    }
}
