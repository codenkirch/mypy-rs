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
        Type::TypeAliasType { args, type_ref } => Type::TypeAliasType {
            args: args.into_iter().map(make_any_non_explicit_inner).collect(),
            type_ref,
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
        Type::TypeAliasType { args, type_ref } => Type::TypeAliasType {
            args: args
                .into_iter()
                .map(make_any_non_unimported_inner)
                .collect(),
            type_ref,
        },
        other => transform_children(other, make_any_non_unimported_inner),
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
        Type::TypeAliasType { args, type_ref } => Type::TypeAliasType {
            args: args.into_iter().map(&f).collect(),
            type_ref,
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
        } => Type::UnboundType {
            name,
            args: args.into_iter().map(&f).collect(),
            original_str_expr,
            original_str_fallback,
        },
        Type::UnpackType { typ } => Type::UnpackType {
            typ: Box::new(f(*typ)),
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
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => Type::CallableType {
            fallback: Box::new(f(*fallback)),
            instance_type: instance_type.map(|it| Box::new(f(*it))),
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            arg_types: arg_types.into_iter().map(&f).collect(),
            arg_kinds,
            arg_names,
            ret_type: Box::new(f(*ret_type)),
            name,
            variables: variables.into_iter().map(&f).collect(),
            type_guard: type_guard.map(|tg| Box::new(f(*tg))),
            type_is: type_is.map(|ti| Box::new(f(*ti))),
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
        } => Type::UnionType {
            items: items.into_iter().map(&f).collect(),
            uses_pep604_syntax,
        },
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
        } => Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound: Box::new(f(*upper_bound)),
            default: Box::new(f(*default)),
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
        } => Type::TypeVarTupleType {
            tuple_fallback: Box::new(f(*tuple_fallback)),
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound: Box::new(f(*upper_bound)),
            default: Box::new(f(*default)),
            min_len,
        },
        Type::DeletedType { source } => Type::DeletedType { source },
        Type::UninhabitedType => Type::UninhabitedType,
        Type::NoneType => Type::NoneType,
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
        let t = Type::UninhabitedType;
        let result = transform_children(t, |x| x);
        assert!(matches!(result, Type::UninhabitedType));
    }
}

#[pyfunction]
pub fn rust_clean_up_type_aliases(type_name: &str) -> String {
    type_name.trim().to_string()
}

#[cfg(test)]
mod phase4_tests {
    use super::*;

    #[test]
    fn test_clean_up_type_aliases() {
        assert_eq!(
            rust_clean_up_type_aliases("  builtins.int  "),
            "builtins.int"
        );
    }
}
