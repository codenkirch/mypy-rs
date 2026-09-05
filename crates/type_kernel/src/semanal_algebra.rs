#![allow(non_local_definitions)]

//! Native port of pure Type-algebra functions from `mypy/semanal.py`
//! (Stage 15, Issue #96).
//!
//! Ports the module-level type-transformation functions that operate on
//! `Type` values without needing the semantic analyzer's mutable state:
//! - `make_any_non_explicit` — rewrite `AnyType(explicit)` to
//!   `AnyType(special_form)` recursively.
//! - `make_any_non_unimported` — rewrite `AnyType(from_unimported_type)`
//!   to `AnyType(special_form)` with `missing_import_name=None`.
//!
//! Both mirror `TrivialSyntheticTypeTranslator` subclasses
//! (`MakeAnyNonExplicit`, `MakeAnyNonUnimported`) which override only
//! `visit_any` and `visit_type_alias_type`, delegating the rest to the
//! default identity traversal.

use pyo3::prelude::*;

use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// TypeOfAny constants (mirror mypy/types.py:213-239).
const EXPLICIT: i64 = 2;
const FROM_UNIMPORTED_TYPE: i64 = 3;
const SPECIAL_FORM: i64 = 6;

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
// make_any_non_explicit
// ---------------------------------------------------------------------------

/// `mypy.semanal.make_any_non_explicit` — replace all `AnyType(explicit)`
/// with `AnyType(special_form)` recursively.
///
/// Mirrors `MakeAnyNonExplicit` (semanal.py:8300-8313). The translator
/// overrides `visit_any` (rewrite if `type_of_any == explicit`) and
/// `visit_type_alias_type` (recurse into args). All other types use the
/// default identity traversal, recursing into children.
#[pyfunction]
pub(crate) fn rust_make_any_non_explicit(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = make_any_non_explicit_inner(typ);
    Ok(encode_type(&result))
}

fn make_any_non_explicit_inner(t: Type) -> Type {
    match t {
        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } => {
            let new_type_of_any = if type_of_any == EXPLICIT {
                SPECIAL_FORM
            } else {
                type_of_any
            };
            let new_source_any = source_any.map(|sa| Box::new(make_any_non_explicit_inner(*sa)));
            Type::AnyType {
                type_of_any: new_type_of_any,
                source_any: new_source_any,
                missing_import_name,
            }
        }
        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive: _,
        } => Type::TypeAliasType {
            args: args.into_iter().map(make_any_non_explicit_inner).collect(),
            type_ref,
            is_recursive: false,
        },
        // Delegate to default traversal for all other variants.
        other => transform_children(other, make_any_non_explicit_inner),
    }
}

// ---------------------------------------------------------------------------
// make_any_non_unimported
// ---------------------------------------------------------------------------

/// `mypy.semanal.make_any_non_unimported` — replace all
/// `AnyType(from_unimported_type)` with `AnyType(special_form)` and clear
/// `missing_import_name`.
///
/// Mirrors `MakeAnyNonUnimported` (semanal.py:8315-8328). The translator
/// overrides `visit_any` (rewrite if `type_of_any == from_unimported_type`)
/// and `visit_type_alias_type` (recurse into args).
#[pyfunction]
pub(crate) fn rust_make_any_non_unimported(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = make_any_non_unimported_inner(typ);
    Ok(encode_type(&result))
}

fn make_any_non_unimported_inner(t: Type) -> Type {
    match t {
        Type::AnyType {
            type_of_any,
            source_any,
            ..
        } => {
            if type_of_any == FROM_UNIMPORTED_TYPE {
                let new_source_any =
                    source_any.map(|sa| Box::new(make_any_non_unimported_inner(*sa)));
                Type::AnyType {
                    type_of_any: SPECIAL_FORM,
                    source_any: new_source_any,
                    missing_import_name: None,
                }
            } else {
                let new_source_any =
                    source_any.map(|sa| Box::new(make_any_non_unimported_inner(*sa)));
                Type::AnyType {
                    type_of_any,
                    source_any: new_source_any,
                    missing_import_name: None,
                }
            }
        }
        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive: _,
        } => Type::TypeAliasType {
            args: args
                .into_iter()
                .map(make_any_non_unimported_inner)
                .collect(),
            type_ref,
            is_recursive: false,
        },
        other => transform_children(other, make_any_non_unimported_inner),
    }
}

// ---------------------------------------------------------------------------
// replace_implicit_first_type
// ---------------------------------------------------------------------------

/// `mypy.semanal.replace_implicit_first_type` — replace the first (implicit
/// self/cls) argument type of a `FunctionLike` with `new`.
///
/// Mirrors semanal.py:8281-8291. A `CallableType` with no argument types is
/// returned as-is; otherwise `arg_types[0]` is swapped for `new` and all
/// other fields are preserved (the Python `copy_modified(arg_types=...)`).
/// An `Overloaded` recurses into each item (each must be a `CallableType`).
/// Returns `None` for any input that is not a `CallableType`/`Overloaded`,
/// or when an `Overloaded` item is not a `CallableType`, so the caller
/// falls back to the pure-Python implementation.
#[pyfunction]
pub(crate) fn rust_replace_implicit_first_type(
    sig_bytes: &[u8],
    new_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let sig = match decode_type(sig_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let new = match decode_type(new_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    match replace_implicit_first_type_inner(sig, &new) {
        Some(result) => Ok(encode_type(&result)),
        None => Ok(None),
    }
}

fn replace_implicit_first_type_inner(sig: Type, new: &Type) -> Option<Type> {
    match sig {
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
            ..
        } => Some(if arg_types.is_empty() {
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
                special_sig: None,
            }
        } else {
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            new_arg_types.push(new.clone());
            new_arg_types.extend(arg_types.into_iter().skip(1));
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
                arg_types: new_arg_types,
                arg_kinds,
                arg_names,
                ret_type,
                name,
                variables,
                type_guard,
                type_is,
                special_sig: None,
            }
        }),
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                let replaced = replace_implicit_first_type_inner(item, new)?;
                if !matches!(replaced, Type::CallableType { .. }) {
                    return None;
                }
                new_items.push(replaced);
            }
            Some(Type::Overloaded { items: new_items })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Default identity traversal (mirrors TypeTranslator.visit_* defaults)
// ---------------------------------------------------------------------------

/// Apply a transformation to all direct child types of a `Type`, rebuilding
/// the variant with transformed children. This mirrors the default
/// `TypeTranslator` behavior where each `visit_*` method recurses into
/// children and reconstructs the node.
fn transform_children<F: Fn(Type) -> Type>(t: Type, f: F) -> Type {
    match t {
        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } => {
            let new_source_any = source_any.map(|sa| Box::new(f(*sa)));
            Type::AnyType {
                type_of_any,
                source_any: new_source_any,
                missing_import_name,
            }
        }
        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive: _,
        } => Type::TypeAliasType {
            args: args.into_iter().map(&f).collect(),
            type_ref,
            is_recursive: false,
        },
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            let new_args: Vec<Type> = args.into_iter().map(&f).collect();
            let new_lkv = last_known_value.map(|lkv| Box::new(f(*lkv)));
            Type::Instance {
                type_ref,
                args: new_args,
                last_known_value: new_lkv,
                extra_attrs,
            }
        }
        Type::UnboundType {
            name,
            args,
            original_str_expr,
            original_str_fallback,
            ..
        } => Type::UnboundType {
            name,
            args: args.into_iter().map(&f).collect(),
            original_str_expr,
            original_str_fallback,
            empty_tuple_index: false,
            optional: false,
        },
        Type::UnpackType { typ, .. } => Type::UnpackType {
            typ: Box::new(f(*typ)),
            from_star_syntax: false,
        },
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
            ..
        } => Type::CallableType {
            fallback: Box::new(f(*fallback)),
            instance_type: instance_type.map(|it| Box::new(f(*it))),
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types: arg_types.into_iter().map(&f).collect(),
            arg_kinds,
            arg_names,
            ret_type: Box::new(f(*ret_type)),
            name,
            variables: variables.into_iter().map(&f).collect(),
            type_guard: type_guard.map(|tg| Box::new(f(*tg))),
            type_is: type_is.map(|ti| Box::new(f(*ti))),
            special_sig: None,
        },
        Type::Overloaded { items } => Type::Overloaded {
            items: items.into_iter().map(&f).collect(),
        },
        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => Type::TupleType {
            partial_fallback: Box::new(f(*partial_fallback)),
            items: items.into_iter().map(&f).collect(),
            implicit,
        },
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => Type::TypedDictType {
            fallback: Box::new(f(*fallback)),
            items: items.into_iter().map(|(k, t)| (k, f(t))).collect(),
            required_keys,
            readonly_keys,
            is_closed,
        },
        Type::LiteralType { fallback, value } => Type::LiteralType {
            fallback: Box::new(f(*fallback)),
            value,
        },
        Type::UnionType {
            items,
            uses_pep604_syntax,
            ..
        } => {
            let new_items: Vec<Type> = items.into_iter().map(&f).collect();
            // Python's visitor rebuilds unions via make_union, so truthiness
            // is recomputed from the mapped items.
            let can_be_true = new_items.iter().any(crate::setops::union_item_can_be_true);
            let can_be_false = new_items.iter().any(crate::setops::union_item_can_be_false);
            Type::UnionType {
                items: new_items,
                uses_pep604_syntax,
                can_be_true,
                can_be_false,
                is_evaluated: true,
                original_str_expr: None,
                original_str_fallback: None,
            }
        }
        Type::TypeType { item, is_type_form } => Type::TypeType {
            item: Box::new(f(*item)),
            is_type_form,
        },
        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values: values.into_iter().map(&f).collect(),
            upper_bound: Box::new(f(*upper_bound)),
            default: Box::new(f(*default)),
            variance,
            meta_level,
        },
        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
            meta_level,
        } => Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound: Box::new(f(*upper_bound)),
            default: Box::new(f(*default)),
            meta_level,
        },
        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
            meta_level,
        } => Type::TypeVarTupleType {
            tuple_fallback: Box::new(f(*tuple_fallback)),
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound: Box::new(f(*upper_bound)),
            default: Box::new(f(*default)),
            min_len,
            meta_level,
        },
        Type::DeletedType { source } => Type::DeletedType { source },
        Type::UninhabitedType { .. } => Type::UninhabitedType { ambiguous: false },
        Type::NoneType => Type::NoneType,
        Type::ErasedType => Type::ErasedType,
        Type::Parameters(_) => t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_any_with_import(type_of_any: i64, import: &str) -> Type {
        Type::AnyType {
            type_of_any,
            source_any: None,
            missing_import_name: Some(import.to_string()),
        }
    }

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    #[test]
    fn test_make_any_non_explicit_rewrites_explicit() {
        let t = make_any(EXPLICIT);
        let result = make_any_non_explicit_inner(t);
        match result {
            Type::AnyType { type_of_any, .. } => assert_eq!(type_of_any, SPECIAL_FORM),
            _ => panic!("expected AnyType"),
        }
    }

    #[test]
    fn test_make_any_non_explicit_preserves_non_explicit() {
        let t = make_any(SPECIAL_FORM);
        let result = make_any_non_explicit_inner(t);
        match result {
            Type::AnyType { type_of_any, .. } => assert_eq!(type_of_any, SPECIAL_FORM),
            _ => panic!("expected AnyType"),
        }
    }

    #[test]
    fn test_make_any_non_explicit_in_instance_args() {
        let t = make_instance("builtins.list", vec![make_any(EXPLICIT)]);
        let result = make_any_non_explicit_inner(t);
        match result {
            Type::Instance { args, .. } => match &args[0] {
                Type::AnyType { type_of_any, .. } => {
                    assert_eq!(*type_of_any, SPECIAL_FORM);
                }
                _ => panic!("expected AnyType arg"),
            },
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn test_make_any_non_explicit_in_alias_args() {
        let t = Type::TypeAliasType {
            args: vec![make_any(EXPLICIT)],
            type_ref: "my_alias".to_string(),
            is_recursive: false,
        };
        let result = make_any_non_explicit_inner(t);
        match result {
            Type::TypeAliasType { args, .. } => match &args[0] {
                Type::AnyType { type_of_any, .. } => {
                    assert_eq!(*type_of_any, SPECIAL_FORM);
                }
                _ => panic!("expected AnyType arg"),
            },
            _ => panic!("expected TypeAliasType"),
        }
    }

    #[test]
    fn test_make_any_non_explicit_in_union() {
        let t = make_union(vec![make_any(EXPLICIT), make_any(SPECIAL_FORM)]);
        let result = make_any_non_explicit_inner(t);
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                match &items[0] {
                    Type::AnyType { type_of_any, .. } => {
                        assert_eq!(*type_of_any, SPECIAL_FORM);
                    }
                    _ => panic!("expected AnyType"),
                }
                match &items[1] {
                    Type::AnyType { type_of_any, .. } => {
                        assert_eq!(*type_of_any, SPECIAL_FORM);
                    }
                    _ => panic!("expected AnyType"),
                }
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_any_non_unimported_rewrites_from_unimported() {
        let t = make_any_with_import(FROM_UNIMPORTED_TYPE, "missing_module");
        let result = make_any_non_unimported_inner(t);
        match result {
            Type::AnyType {
                type_of_any,
                missing_import_name,
                ..
            } => {
                assert_eq!(type_of_any, SPECIAL_FORM);
                assert!(missing_import_name.is_none());
            }
            _ => panic!("expected AnyType"),
        }
    }

    #[test]
    fn test_make_any_non_unimported_preserves_other_any() {
        let t = make_any(EXPLICIT);
        let result = make_any_non_unimported_inner(t);
        match result {
            Type::AnyType {
                type_of_any,
                missing_import_name,
                ..
            } => {
                assert_eq!(type_of_any, EXPLICIT);
                assert!(missing_import_name.is_none());
            }
            _ => panic!("expected AnyType"),
        }
    }

    #[test]
    fn test_make_any_non_unimported_in_instance_args() {
        let t = make_instance(
            "builtins.list",
            vec![make_any_with_import(FROM_UNIMPORTED_TYPE, "mod")],
        );
        let result = make_any_non_unimported_inner(t);
        match result {
            Type::Instance { args, .. } => match &args[0] {
                Type::AnyType {
                    type_of_any,
                    missing_import_name,
                    ..
                } => {
                    assert_eq!(*type_of_any, SPECIAL_FORM);
                    assert!(missing_import_name.is_none());
                }
                _ => panic!("expected AnyType arg"),
            },
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn test_make_any_non_unimported_in_alias_args() {
        let t = Type::TypeAliasType {
            args: vec![make_any_with_import(FROM_UNIMPORTED_TYPE, "mod")],
            type_ref: "alias".to_string(),
            is_recursive: false,
        };
        let result = make_any_non_unimported_inner(t);
        match result {
            Type::TypeAliasType { args, .. } => match &args[0] {
                Type::AnyType {
                    type_of_any,
                    missing_import_name,
                    ..
                } => {
                    assert_eq!(*type_of_any, SPECIAL_FORM);
                    assert!(missing_import_name.is_none());
                }
                _ => panic!("expected AnyType arg"),
            },
            _ => panic!("expected TypeAliasType"),
        }
    }

    #[test]
    fn test_transform_children_identity_for_none_type() {
        let t = Type::NoneType;
        let result = transform_children(t, |x| x);
        assert!(matches!(result, Type::NoneType));
    }

    #[test]
    fn test_transform_children_identity_for_uninhabited() {
        let t = Type::UninhabitedType { ambiguous: false };
        let result = transform_children(t, |x| x);
        assert!(matches!(result, Type::UninhabitedType { .. }));
    }

    fn make_callable(arg_types: Vec<Type>) -> Type {
        Type::CallableType {
            fallback: Box::new(make_instance(
                "builtins.function",
                vec![
                    Type::AnyType {
                        type_of_any: SPECIAL_FORM,
                        source_any: None,
                        missing_import_name: None,
                    },
                    Type::NoneType,
                ],
            )),
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
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    #[test]
    fn test_replace_implicit_first_type_swaps_first_arg() {
        let sig = make_callable(vec![
            make_any(EXPLICIT),
            make_instance("builtins.int", vec![]),
        ]);
        let new = make_any(SPECIAL_FORM);
        match replace_implicit_first_type_inner(sig, &new) {
            Some(Type::CallableType { arg_types, .. }) => {
                assert_eq!(arg_types.len(), 2);
                match &arg_types[0] {
                    Type::AnyType { type_of_any, .. } => assert_eq!(*type_of_any, SPECIAL_FORM),
                    _ => panic!("expected AnyType first arg"),
                }
                match &arg_types[1] {
                    Type::Instance { type_ref, .. } => {
                        assert_eq!(type_ref, "builtins.int");
                    }
                    _ => panic!("expected Instance second arg"),
                }
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_replace_implicit_first_type_preserves_empty_args() {
        let sig = make_callable(vec![]);
        let new = make_any(SPECIAL_FORM);
        let result = replace_implicit_first_type_inner(sig, &new).unwrap();
        match &result {
            Type::CallableType { arg_types, .. } if arg_types.is_empty() => {}
            _ => panic!("expected empty-arg CallableType unchanged"),
        }
        // Compare serialized form to confirm arg_types stayed empty.
        let expected = encode_bytes(&make_callable(vec![]));
        assert_eq!(encode_bytes(&result), expected);
    }

    fn encode_bytes(t: &Type) -> Vec<u8> {
        let mut wbuf = WriteBuffer::new();
        write_type(&mut wbuf, t).unwrap();
        wbuf.into_bytes()
    }

    #[test]
    fn test_replace_implicit_first_type_recurses_overloaded() {
        let sig = Type::Overloaded {
            items: vec![
                make_callable(vec![make_any(EXPLICIT)]),
                make_callable(vec![make_any(EXPLICIT)]),
            ],
        };
        let new = make_any(SPECIAL_FORM);
        match replace_implicit_first_type_inner(sig, &new) {
            Some(Type::Overloaded { items }) => {
                assert_eq!(items.len(), 2);
                for item in items {
                    match item {
                        Type::CallableType { arg_types, .. } => match &arg_types[0] {
                            Type::AnyType { type_of_any, .. } => {
                                assert_eq!(*type_of_any, SPECIAL_FORM);
                            }
                            _ => panic!("expected AnyType"),
                        },
                        _ => panic!("expected CallableType item"),
                    }
                }
            }
            _ => panic!("expected Overloaded"),
        }
    }

    #[test]
    fn test_replace_implicit_first_type_rejects_non_callable() {
        assert!(
            replace_implicit_first_type_inner(make_any(EXPLICIT), &make_any(SPECIAL_FORM))
                .is_none()
        );
        assert!(
            replace_implicit_first_type_inner(Type::NoneType, &make_any(SPECIAL_FORM)).is_none()
        );
    }

    #[test]
    fn test_replace_implicit_first_type_rejects_bad_overload_item() {
        let sig = Type::Overloaded {
            items: vec![make_any(EXPLICIT)],
        };
        assert!(replace_implicit_first_type_inner(sig, &make_any(SPECIAL_FORM)).is_none());
    }
}
