#![allow(non_local_definitions)]

//! Native port of pure Type-algebra functions from `mypy/checkexpr.py`
//! (Stage 10b, Issue #95).
//!
//! Ports the module-level functions that operate on `Type` values without
//! needing the expression checker's mutable state (checker, context, plugin,
//! TypeInfo):
//! - `allow_fast_container_literal` — Instance or tuple-of-fast check.
//! - `has_erased_component` — BoolTypeQuery for ErasedType.
//! - `replace_callable_return_type` — copy a CallableType with new ret_type.
//! - `all_same_types` — all types equal (via PartialEq).
//!
//! Deferred (need checker state / TypeInfo / Expression / ArgKind):
//! `check_call`, `visit_call_expr_inner`, `check_operator_expr`,
//! `check_comparison`, `is_duplicate_mapping`, `arg_approximate_similarity`,
//! `any_causes_overload_ambiguity`, `merge_typevars_in_callables_by_name`,
//! `type_info_from_type`, `is_async_def`, `is_expr_literal_type`,
//! `get_partial_instance_type`, `has_ambiguous_uninhabited_component`
//! (wire UninhabitedType has no `ambiguous` field).

use pyo3::prelude::*;

use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

// ---------------------------------------------------------------------------
// allow_fast_container_literal
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.allow_fast_container_literal` — check if a type is an
/// Instance or a tuple whose items are all fast-container-literal types.
///
/// Mirrors `allow_fast_container_literal` (checkexpr.py:304-309). Defers
/// (returns None) for recursive `TypeAliasType` since the wire format lacks
/// the `is_recursive` field.
#[pyfunction]
pub(crate) fn rust_allow_fast_container_literal(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(allow_fast_container_literal_inner(&typ))
}

pub(crate) fn allow_fast_container_literal_inner(t: &Type) -> Option<bool> {
    // TypeAliasType: defer (no is_recursive field on wire).
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    match t {
        Type::Instance { .. } => Some(true),
        Type::TupleType { items, .. } => {
            // All items must be fast-container-literal; if any defers, defer.
            let mut result = true;
            for item in items {
                match allow_fast_container_literal_inner(item) {
                    Some(true) => {}
                    Some(false) => result = false,
                    None => return None,
                }
            }
            Some(result)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// has_erased_component: BoolTypeQuery (ANY_STRATEGY) for ErasedType
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_erased_component` — whether `t` contains an
/// `ErasedType`.
///
/// Mirrors `HasErasedComponentsQuery` (checkexpr.py:6820-6832).
/// `BoolTypeQuery` with `ANY_STRATEGY`: true if any child is ErasedType.
#[pyfunction]
pub(crate) fn rust_has_erased_component(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_erased_component_inner(&typ))
}

pub(crate) fn has_erased_component_inner(t: &Type) -> Option<bool> {
    if matches!(t, Type::TypeAliasType { .. }) {
        // Wire TypeAliasType has no target to expand — can't tell if it
        // contains erased. Defer.
        return None;
    }
    // The wire Type enum has no ErasedType variant (erased types are not
    // serialized to the wire format in the current codec). When one is
    // added, is_erased() will return true for it.
    if is_erased(t) {
        return Some(true);
    }
    let mut result = false;
    for child in children(t) {
        match has_erased_component_inner(child) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    let _ = &mut result;
    Some(false)
}

/// Check if a type is ErasedType. The wire `Type` enum does not currently
/// include an `ErasedType` variant, so this always returns false — but the
/// structure is in place for when it's added.
fn is_erased(_t: &Type) -> bool {
    false
}

// ---------------------------------------------------------------------------
// replace_callable_return_type
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.replace_callable_return_type` — return a copy of a
/// CallableType with a different return type.
///
/// Mirrors `replace_callable_return_type` (checkexpr.py:6799-6800). Returns
/// None if the input is not a CallableType.
#[pyfunction]
pub(crate) fn rust_replace_callable_return_type(
    callable_bytes: &[u8],
    new_ret_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let callable = match decode_type(callable_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let new_ret = match decode_type(new_ret_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = replace_callable_return_type_inner(&callable, &new_ret);
    Ok(result.and_then(|r| encode_type(&r)))
}

pub(crate) fn replace_callable_return_type_inner(callable: &Type, new_ret: &Type) -> Option<Type> {
    if let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        arg_types,
        arg_kinds,
        arg_names,
        name,
        variables,
        type_guard,
        type_is,
        ret_type: _,
    } = callable
    {
        Some(Type::CallableType {
            fallback: fallback.clone(),
            instance_type: instance_type.clone(),
            is_ellipsis_args: *is_ellipsis_args,
            implicit: *implicit,
            is_bound: *is_bound,
            from_concatenate: *from_concatenate,
            imprecise_arg_kinds: *imprecise_arg_kinds,
            unpack_kwargs: *unpack_kwargs,
            arg_types: arg_types.clone(),
            arg_kinds: arg_kinds.clone(),
            arg_names: arg_names.clone(),
            ret_type: Box::new(new_ret.clone()),
            name: name.clone(),
            variables: variables.clone(),
            type_guard: type_guard.clone(),
            type_is: type_is.clone(),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// all_same_types: all types equal via PartialEq
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.all_same_types` — check if all types in a list are the
/// same.
///
/// Mirrors `all_same_types` (checkexpr.py:6991-6994). Uses `is_same_type`
/// in Python (structural equality after `get_proper_type`); the wire format
/// uses `PartialEq` which is structural equality without alias expansion.
/// TypeAliasType won't be expanded, so this is a conservative approximation.
#[pyfunction]
pub(crate) fn rust_all_same_types(type_bytes_list: Vec<Vec<u8>>) -> PyResult<bool> {
    let types: Vec<Type> = type_bytes_list
        .iter()
        .filter_map(|b| decode_type(b))
        .collect();
    Ok(all_same_types_inner(&types))
}

pub(crate) fn all_same_types_inner(types: &[Type]) -> bool {
    if types.is_empty() {
        return true;
    }
    let first = &types[0];
    types[1..].iter().all(|t| t == first)
}

// ---------------------------------------------------------------------------
// children: yield direct child types (shared traversal)
// ---------------------------------------------------------------------------

fn children(typ: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match typ {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            out.extend(args.iter());
            if let Some(lkv) = last_known_value {
                out.push(lkv);
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            out.extend(variables.iter());
            if let Some(it) = instance_type {
                out.push(it);
            }
        }
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            out.push(fallback);
            out.extend(items.iter().map(|(_, t)| t));
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        Type::AnyType {
            source_any: None, ..
        } => {}
        Type::TypeAliasType { args, .. } => out.extend(args.iter()),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_tuple(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items,
            implicit: false,
        }
    }

    fn make_callable(ret: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(ret),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn test_allow_fast_container_instance() {
        let inst = make_instance("builtins.list", vec![]);
        assert_eq!(allow_fast_container_literal_inner(&inst), Some(true));
    }

    #[test]
    fn test_allow_fast_container_tuple_of_instances() {
        let t = make_tuple(vec![make_instance("A", vec![]), make_instance("B", vec![])]);
        assert_eq!(allow_fast_container_literal_inner(&t), Some(true));
    }

    #[test]
    fn test_allow_fast_container_false_for_int() {
        // IntExpr is not a Type — but an Instance with args is fast.
        // Non-Instance, non-Tuple returns false.
        let t = Type::NoneType;
        assert_eq!(allow_fast_container_literal_inner(&t), Some(false));
    }

    #[test]
    fn test_allow_fast_container_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(allow_fast_container_literal_inner(&alias), None);
    }

    #[test]
    fn test_has_erased_component_false_simple() {
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(has_erased_component_inner(&inst), Some(false));
    }

    #[test]
    fn test_has_erased_component_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(has_erased_component_inner(&alias), None);
    }

    #[test]
    fn test_replace_callable_return_type() {
        let original = make_callable(make_instance("builtins.int", vec![]));
        let new_ret = make_instance("builtins.str", vec![]);
        let result = replace_callable_return_type_inner(&original, &new_ret);
        assert!(result.is_some());
        if let Some(Type::CallableType { ret_type, .. }) = result {
            assert!(
                matches!(*ret_type, Type::Instance { type_ref, .. } if type_ref == "builtins.str")
            );
        } else {
            panic!("expected CallableType");
        }
    }

    #[test]
    fn test_replace_callable_return_type_not_callable() {
        let inst = make_instance("builtins.int", vec![]);
        let new_ret = make_instance("builtins.str", vec![]);
        assert!(replace_callable_return_type_inner(&inst, &new_ret).is_none());
    }

    #[test]
    fn test_all_same_types_empty() {
        assert!(all_same_types_inner(&[]));
    }

    #[test]
    fn test_all_same_types_single() {
        let t = make_instance("A", vec![]);
        assert!(all_same_types_inner(&[t]));
    }

    #[test]
    fn test_all_same_types_same() {
        let t = make_instance("builtins.int", vec![]);
        assert!(all_same_types_inner(&[t.clone(), t.clone(), t]));
    }

    #[test]
    fn test_all_same_types_different() {
        let a = make_instance("builtins.int", vec![]);
        let b = make_instance("builtins.str", vec![]);
        assert!(!all_same_types_inner(&[a, b]));
    }
}
