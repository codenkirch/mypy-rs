//! Stage 4 `check_call` dispatch classification (checkcall.rs).
//!
//! Ports the pure dispatch decision at the top of `mypy.checkexpr.check_call`:
//! given an already-proper callee type, classify it into the branch that
//! Python's `isinstance` chain would take. Purely structural; no mutation,
//! no checker state, so the kernel cannot emit/suppress errors here.

use pyo3::prelude::*;

use crate::visitor::find_unpack_in_list_inner;
use crate::wire::{read_type, write_type, ReadBuffer, Type, WireError, WriteBuffer};

/// Dispatch kinds mirroring `check_call`'s isinstance chain.
pub(crate) const CALL_PLAIN: i64 = 0; // CallableType without variables
pub(crate) const CALL_WITH_VARS: i64 = 1; // CallableType with variables
pub(crate) const CALL_OVERLOADED: i64 = 2; // Overloaded
pub(crate) const CALL_ANY: i64 = 3; // AnyType (or not checked function)
pub(crate) const CALL_UNION: i64 = 4; // UnionType
pub(crate) const CALL_INSTANCE: i64 = 5; // Instance -> __call__ member access
pub(crate) const CALL_TYPE_TYPE: i64 = 6; // TypeType (falls through to member access)
pub(crate) const CALL_OTHER: i64 = 7;

/// ArgKind values (mirrors mypy.nodes.ArgKind).
const ARG_POS: i64 = 0;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_NAMED_OPT: i64 = 5;

/// The CallableType fields that `with_unpacked_kwargs` and
/// `with_normalized_var_args` rewrite plus the immutable passes-through.
struct CallableBase {
    fallback: Box<Type>,
    instance_type: Option<Box<Type>>,
    is_ellipsis_args: bool,
    implicit: bool,
    is_bound: bool,
    from_concatenate: bool,
    imprecise_arg_kinds: bool,
    unpack_kwargs: bool,
    arg_types: Vec<Type>,
    arg_kinds: Vec<i64>,
    arg_names: Vec<Option<String>>,
    ret_type: Box<Type>,
    name: Option<String>,
    variables: Vec<Type>,
    type_guard: Option<Box<Type>>,
    type_is: Option<Box<Type>>,
}

/// Classify an already-proper callee type into the `check_call` dispatch
/// branch. Defer (None) on any wire/decode failure.
#[pyfunction]
pub(crate) fn rust_classify_call(callee_bytes: &[u8]) -> Option<i64> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    classify_call(&callee).ok()
}

/// Pure classification mirroring `check_call`'s isinstance chain.
fn classify_call(callee: &Type) -> Result<i64, WireError> {
    Ok(match callee {
        Type::CallableType { variables, .. } => {
            if variables.is_empty() {
                CALL_PLAIN
            } else {
                CALL_WITH_VARS
            }
        }
        Type::Overloaded { .. } => CALL_OVERLOADED,
        Type::AnyType { .. } => CALL_ANY,
        Type::UnionType { .. } => CALL_UNION,
        Type::Instance { .. } => CALL_INSTANCE,
        Type::TypeType { .. } => CALL_TYPE_TYPE,
        _ => CALL_OTHER,
    })
}

/// Apply `CallableType.with_unpacked_kwargs()` then
/// `with_normalized_var_args()` (types.py:2505-2613), the normalization at
/// the head of `check_callable_call`. Defer (None) on any wire/decode
/// failure or non-CallableType callee.
#[pyfunction]
pub(crate) fn rust_normalize_callable(callee_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    let normalized = normalize_callable(&callee).ok()?;
    let mut out = WriteBuffer::new();
    write_type(&mut out, &normalized).ok()?;
    Some(out.into_bytes())
}

fn normalize_callable(callee: &Type) -> Result<Type, WireError> {
    let Type::CallableType {
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
    } = callee
    else {
        return Err(WireError::invalid(
            "normalize: callee is not a CallableType",
        ));
    };
    let mut base = CallableBase {
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
        ret_type: ret_type.clone(),
        name: name.clone(),
        variables: variables.clone(),
        type_guard: type_guard.clone(),
        type_is: type_is.clone(),
    };
    with_unpacked_kwargs(&mut base)?;
    with_normalized_var_args(&mut base)?;
    Ok(base.into_type())
}

/// `with_unpacked_kwargs`: expand `**kwargs: TypedDict` into named keys.
fn with_unpacked_kwargs(base: &mut CallableBase) -> Result<(), WireError> {
    if !base.unpack_kwargs {
        return Ok(());
    }
    let Some(Type::TypedDictType {
        items,
        required_keys,
        ..
    }) = base.arg_types.last()
    else {
        // Python asserts isinstance(last_type, TypedDictType).
        return Err(WireError::invalid(
            "with_unpacked_kwargs: last arg type is not a TypedDictType",
        ));
    };
    let required: std::collections::HashSet<&String> = required_keys.iter().collect();
    let mut types = base.arg_types[..base.arg_types.len() - 1].to_vec();
    let mut kinds = base.arg_kinds[..base.arg_kinds.len() - 1].to_vec();
    let mut names = base.arg_names[..base.arg_names.len() - 1].to_vec();
    for (key, typ) in items {
        types.push(typ.clone());
        kinds.push(if required.contains(&key) {
            ARG_NAMED
        } else {
            ARG_NAMED_OPT
        });
        names.push(Some(key.clone()));
    }
    base.arg_types = types;
    base.arg_kinds = kinds;
    base.arg_names = names;
    base.unpack_kwargs = false;
    Ok(())
}

/// `with_normalized_var_args`: expand `*args: *Tuple[...]` into fixed args.
fn with_normalized_var_args(base: &mut CallableBase) -> Result<(), WireError> {
    let var_arg_index = base.arg_kinds.iter().position(|&k| k == ARG_STAR);
    let unpacked_items = match var_arg_index {
        Some(idx) => match &base.arg_types[idx] {
            Type::UnpackType { typ } => match &**typ {
                Type::TupleType { items, .. } => Some(items.clone()),
                _ => None,
            },
            _ => None,
        },
        None => None,
    };
    let Some(unpacked_items) = unpacked_items else {
        return Ok(());
    };
    let unpack_index = find_unpack_in_list_inner(&unpacked_items);
    let ui = var_arg_index.unwrap();
    if unpack_index == 0 && unpacked_items.len() > 1 {
        // Already normalized: return the callable unchanged.
        return Ok(());
    }
    let types_prefix = base.arg_types[..ui].to_vec();
    let kinds_prefix = base.arg_kinds[..ui].to_vec();
    let names_prefix = base.arg_names[..ui].to_vec();
    let types_suffix = base.arg_types[ui + 1..].to_vec();
    let kinds_suffix = base.arg_kinds[ui + 1..].to_vec();
    let names_suffix = base.arg_names[ui + 1..].to_vec();
    let (types_middle, kinds_middle, names_middle) = if unpack_index < 0 {
        // Plain *Tuple[X, Y, Z] -> replace with ARG_POS completely.
        (
            unpacked_items.clone(),
            vec![ARG_POS; unpacked_items.len()],
            vec![None; unpacked_items.len()],
        )
    } else {
        let ui_idx = unpack_index as usize;
        let Type::UnpackType { typ } = &unpacked_items[ui_idx] else {
            unreachable!("unpack_index points at an UnpackType");
        };
        let nested_unpacked = &**typ;
        let mut types_middle = unpacked_items[..ui_idx].to_vec();
        let mut kinds_middle: Vec<i64> = (0..ui_idx).map(|_| ARG_POS).collect();
        let mut names_middle: Vec<Option<String>> = (0..ui_idx).map(|_| None).collect();
        if ui_idx == unpacked_items.len() - 1 {
            // Normalize also single item tuples like
            //   *args: *Tuple[*tuple[X, ...]] -> *args: X
            //   *args: *Tuple[*Ts] -> *args: *Ts
            match nested_unpacked {
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                    types_middle.push(args[0].clone());
                    kinds_middle.push(ARG_STAR);
                    names_middle.push(base.arg_names[ui].clone());
                }
                Type::TypeVarTupleType { .. } => {
                    types_middle.push(nested_unpacked.clone());
                    kinds_middle.push(ARG_STAR);
                    names_middle.push(base.arg_names[ui].clone());
                }
                _ => {
                    // Non-normalized tuple during semanal: return as-is.
                    return Ok(());
                }
            }
        } else {
            // *Tuple[X, *Ts, Y, Z] -> prefix ARG_POS, keep the tail
            // unpacked as a single UnpackType.
            types_middle.push(unpacked_items[ui_idx].clone());
            kinds_middle.push(ARG_STAR);
            names_middle.push(base.arg_names[ui].clone());
        }
        (types_middle, kinds_middle, names_middle)
    };
    base.arg_types = [types_prefix, types_middle, types_suffix].concat();
    base.arg_kinds = [kinds_prefix, kinds_middle, kinds_suffix].concat();
    base.arg_names = [names_prefix, names_middle, names_suffix].concat();
    Ok(())
}

impl CallableBase {
    fn into_type(self) -> Type {
        Type::CallableType {
            fallback: self.fallback,
            instance_type: self.instance_type,
            is_ellipsis_args: self.is_ellipsis_args,
            implicit: self.implicit,
            is_bound: self.is_bound,
            from_concatenate: self.from_concatenate,
            imprecise_arg_kinds: self.imprecise_arg_kinds,
            unpack_kwargs: self.unpack_kwargs,
            arg_types: self.arg_types,
            arg_kinds: self.arg_kinds,
            arg_names: self.arg_names,
            ret_type: self.ret_type,
            name: self.name,
            variables: self.variables,
            type_guard: self.type_guard,
            type_is: self.type_is,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn classify_bytes(t: &Type) -> Option<i64> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok()?;
        rust_classify_call(&buf.into_bytes())
    }

    fn normalize_bytes(t: &Type) -> Option<Type> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok()?;
        let out = rust_normalize_callable(&buf.into_bytes())?;
        let mut rb = ReadBuffer::new(&out);
        read_type(&mut rb, None).ok()
    }

    fn typed_dict(required: &[&str], optional: &[(&str, Type)]) -> Type {
        let mut items: Vec<(String, Type)> = Vec::new();
        let mut required_keys: std::collections::HashSet<String> = Default::default();
        for name in required {
            items.push((name.to_string(), any_type()));
            required_keys.insert(name.to_string());
        }
        for (name, typ) in optional {
            items.push((name.to_string(), typ.clone()));
        }
        Type::TypedDictType {
            fallback: Box::new(instance()),
            items,
            required_keys,
            readonly_keys: Default::default(),
            is_closed: true,
        }
    }

    fn unpack(typ: Type) -> Type {
        Type::UnpackType { typ: Box::new(typ) }
    }

    fn tuple_of(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance()),
            items,
            implicit: false,
        }
    }

    fn callable_with(unpack_kwargs: bool, args: Vec<(Type, i64, Option<String>)>) -> Type {
        let mut arg_types = Vec::with_capacity(args.len());
        let mut arg_kinds = Vec::with_capacity(args.len());
        let mut arg_names = Vec::with_capacity(args.len());
        for (t, k, n) in args {
            arg_types.push(t);
            arg_kinds.push(k);
            arg_names.push(n);
        }
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(any_type()),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance() -> Type {
        Type::Instance {
            type_ref: "mod.C".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn type_var() -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "mod".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn callable(variables: usize) -> Type {
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(any_type()),
            name: None,
            variables: vec![type_var(); variables],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn classifies_plain_callable() {
        assert_eq!(classify_bytes(&callable(0)), Some(CALL_PLAIN));
    }

    #[test]
    fn classifies_callable_with_vars() {
        assert_eq!(classify_bytes(&callable(2)), Some(CALL_WITH_VARS));
    }

    #[test]
    fn classifies_any() {
        assert_eq!(classify_bytes(&any_type()), Some(CALL_ANY));
    }

    #[test]
    fn classifies_instance() {
        assert_eq!(classify_bytes(&instance()), Some(CALL_INSTANCE));
    }

    #[test]
    fn classifies_overloaded() {
        let t = Type::Overloaded {
            items: vec![callable(0)],
        };
        assert_eq!(classify_bytes(&t), Some(CALL_OVERLOADED));
    }

    #[test]
    fn classifies_union() {
        let t = Type::UnionType {
            items: vec![any_type(), instance()],
            uses_pep604_syntax: false,
        };
        assert_eq!(classify_bytes(&t), Some(CALL_UNION));
    }

    #[test]
    fn normalize_noop_for_plain_callable() {
        let t = callable(0);
        assert_eq!(normalize_bytes(&t), Some(t));
    }

    #[test]
    fn normalize_unpacks_kwargs_typeddict() {
        let td = typed_dict(&["x"], &[("y", any_type())]);
        let t = callable_with(
            true,
            vec![
                (any_type(), ARG_POS, None),
                (td, ARG_NAMED, Some("kwargs".into())),
            ],
        );
        let out = normalize_bytes(&t).unwrap();
        let Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            unpack_kwargs,
            ..
        } = out
        else {
            panic!("expected CallableType");
        };
        assert!(!unpack_kwargs);
        assert_eq!(arg_types.len(), 3);
        assert_eq!(arg_kinds, vec![ARG_POS, ARG_NAMED, ARG_NAMED_OPT]);
        assert_eq!(arg_names, vec![None, Some("x".into()), Some("y".into())]);
    }

    #[test]
    fn normalize_var_args_plain_tuple() {
        let t = callable_with(
            false,
            vec![(
                unpack(tuple_of(vec![any_type(), any_type()])),
                ARG_STAR,
                Some("args".into()),
            )],
        );
        let out = normalize_bytes(&t).unwrap();
        let Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ..
        } = out
        else {
            panic!("expected CallableType");
        };
        assert_eq!(arg_types.len(), 2);
        assert_eq!(arg_kinds, vec![ARG_POS, ARG_POS]);
        assert_eq!(arg_names, vec![None, None]);
    }

    #[test]
    fn normalize_var_args_non_tuple_unpack_unchanged() {
        // *args: *tuple[X, ...] is not a TupleType on the wire; the
        // Python method also leaves it unchanged.
        let star_unpack = unpack(tuple_of(vec![unpack(tuple_of(vec![instance()]))]));
        let t = callable_with(false, vec![(star_unpack, ARG_STAR, Some("args".into()))]);
        let out = normalize_bytes(&t);
        assert_eq!(
            out,
            Some(callable_with(
                false,
                vec![(
                    unpack(tuple_of(vec![unpack(tuple_of(vec![instance()]))])),
                    ARG_STAR,
                    Some("args".into())
                )]
            ))
        );
    }
}
