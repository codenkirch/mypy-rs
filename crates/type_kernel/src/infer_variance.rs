//! Native port of the `infer_variance` member-direction analysis
//! (mypy.subtypes.infer_variance, subtypes.py:2585-2660).
//!
//! `infer_variance` mutates `info.defn.type_vars[i].variance` while scanning
//! the class members, so the mutation stays in Python: the shim keeps the
//! whole 3-iteration loop and calls this per-member decision function. For
//! each candidate variance the shim sets `tv.variance`, then per member:
//!
//! ```python
//! typ = find_member(member, self_type, self_type)
//! typ = erase_return_self_types(typ, self_type)   # callable-form only
//! typ2 = expand_type(typ, {tvar.id: object_type})
//! if not is_subtype(typ, typ2):
//!     co = False
//! if not is_subtype(typ2, typ):
//!     contra = False
//!     if settable:
//!         co = False
//! ```
//!
//! The two `is_subtype` calls plus the `expand_type` substitution are the
//! heavy per-member algebra. This module ports exactly that decision to
//! `rust_infer_variance_member`, which returns a bitmask of the caller's
//! `co` / `contra` flips:
//!
//! * `0` = neither direction flipped.
//! * `1` = `co` flipped (`is_subtype(typ, typ2)` was False). The settable
//!   rule (contra False implies co False) is applied by the shim, which
//!   owns `settable`.
//! * `2` = `contra` flipped (`is_subtype(typ2, typ)` was False).
//! * `3` = both flipped.
//!
//! The shim folds: `if code & 1: co = False`; `if code & 2: contra = False`.
//! It then applies `if settable and code & 2: co = False` exactly like the
//! pure-Python body, and owns the `tv.variance` mutation.
//!
//! Deferral contract (return `None`, the shim runs the original pure-Python
//! member body):
//!
//! * `typ` is falsy in Python (member lookup returned None; the member is
//!   skipped and does not affect variance).
//! * A member type that is not wire-readable (`read_type` failure).
//! * `erase_return_self_types` produced a bare `Instance == self_type`: the
//!   Python path must itself decide the self-return erasure (any type-shape
//!   change there is a semantic change), so Rust defers.
//! * `expand_type` returns None (ParamSpec-carrying callables, bound
//!   methods, TypeAliasType-bearing types, unresolved TypeVarTuple
//!   bindings, or leftover TypeVars after the substitution — all cases the
//!   existing `expandtype.rs` defers).
//! * Either `is_subtype` call defers (nominal `Instance` paths with
//!   `VARIANCE_NOT_READY` class variances, unsupported variants, unresolved
//!   type refs).
//!
//! The member scan lives in Python because it reads the live `info.names`
//! tables, `get_member_flags`, `find_member`, and the settable /
//! underscore-prefix special cases, and because a scan-level deferral on a
//! single member must re-run the exact original body. The parity suite
//! (NativeInferVarianceSuite) proves gate-on vs gate-off produce identical
//! final variances on covariant, contravariant, invariant, and
//! not-ready-deferral classes.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::expandtype::{expand_type_with_env, result_has_typevar};
use crate::subtypes::{self, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{py_type_eq, read_type, ReadBuffer, Type};

/// Bitmask of the caller's `co` / `contra` flips for one member:
/// 0 = none, 1 = co only, 2 = contra only, 3 = both.
const MEMBER_NO_USE: i32 = 0;
const MEMBER_CO_ONLY: i32 = 1;
const MEMBER_CONTRA_ONLY: i32 = 2;
const MEMBER_BOTH: i32 = 3;

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `get_proper_type` shim: a `TypeAliasType` has no proper form in the wire
/// representation (its target is unresolved), so defer. Otherwise the wire
/// type is already proper.
fn get_proper_type_or_none(t: &Type) -> Option<Type> {
    if let Type::TypeAliasType { .. } = t {
        return None;
    }
    Some(t.clone())
}

/// Exactly-one-arg-bindings env for `expand_type`, i.e. `tvar.id:
/// object_type}` in Python. The env keys use `(raw_id, 0, "")`: class
/// typevars bind `TypeVarId(raw_id)` (types.py:554 defaults meta_level=0,
/// namespace="").
fn single_binding_env(raw_id: i64, object_type: &Type) -> HashMap<(i64, i64, String), Type> {
    let mut env = HashMap::with_capacity(1);
    env.insert((raw_id, 0, String::new()), object_type.clone());
    env
}

/// Portable subset of `erase_return_self_types` (subtypes.py:2681-2695):
/// `CallableType` whose proper return type is an `Instance == self_type`
/// becomes `Any(implementation_artifact)`; `Overloaded` heads erase each
/// item recursively. Other heads pass through unchanged.
fn erase_return_self_types_wire(typ: &Type, self_type: &Type) -> Option<Type> {
    let t = get_proper_type_or_none(typ)?;
    let s = get_proper_type_or_none(self_type)?;
    // Bare `Instance == self_type` is NOT a function-like self return; the
    // Python path returns it unchanged, so it must not become Any here.
    if matches!(t, Type::Instance { .. }) && py_type_eq(&t, &s) {
        return None;
    }
    match &t {
        Type::CallableType { ret_type, .. } => {
            let ret = get_proper_type_or_none(ret_type)?;
            if matches!(ret, Type::Instance { .. }) && py_type_eq(&ret, &s) {
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
                    arg_types,
                    arg_kinds,
                    arg_names,
                    ret_type,
                    name,
                    variables,
                    type_guard,
                    type_is,
                    ..
                } = t
                else {
                    unreachable!()
                };
                let _ = &ret_type; // destructured; replaced below
                Some(Type::CallableType {
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
                    ret_type: Box::new(Type::AnyType {
                        type_of_any: 8, // TypeOfAny.implementation_artifact
                        source_any: None,
                        missing_import_name: None,
                    }),
                    name,
                    variables,
                    type_guard,
                    type_is,
                    special_sig: None,
                })
            } else {
                Some(t)
            }
        }
        Type::Overloaded { items } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let erased = erase_return_self_types_wire(item, self_type)?;
                out.push(erased);
            }
            Some(Type::Overloaded { items: out })
        }
        _ => Some(t),
    }
}

/// Compute the per-member direction decision of `infer_variance` for one
/// candidate variance. See the module docs for the exact Python contract.
///
/// Arguments come pre-computed from the live graph by the Python shim:
/// `member_type_bytes` is the serialized `typ` returned by `find_member`,
/// `self_type_bytes` the `fill_typevars(info)` self type (after any
/// TupleType fallback), `object_type_bytes` the object Instance, and
/// `raw_id` the candidate typevar's `TypeVarId.raw_id` (the env key).
/// The result is a bitmask of the caller's `co` / `contra` flips.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_infer_variance_member(
    member_type_bytes: &[u8],
    self_type_bytes: &[u8],
    object_type_bytes: &[u8],
    raw_id: i64,
    resolver: &mut NativeTypeResolver,
) -> Option<i32> {
    let member_type = decode_type(member_type_bytes)?;
    let self_type = decode_type(self_type_bytes)?;
    let object_type = decode_type(object_type_bytes)?;
    // typ = erase_return_self_types(typ, self_type): the wire subset
    // (callable-form heads). A bare self-Instance defers to Python.
    let erased = erase_return_self_types_wire(&member_type, &self_type)?;
    // typ2 = expand_type(typ, {tvar.id: object_type}).
    let env = single_binding_env(raw_id, &object_type);
    let typ2 = expand_type_with_env(&erased, &env, true)?;
    // A leftover TypeVar after the substitution means the binding missed
    // (Python preserves identity and compares a different object), so defer.
    if result_has_typevar(&typ2) {
        return None;
    }
    // is_subtype(typ, typ2) and is_subtype(typ2, typ), both non-proper
    // (the Python callers pass no subtype_context -> defaults). The Rust
    // nominal subtype seam handles the Instance / typevar-free cases.
    let ctx = SubtypeContext::new(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        false, // proper_subtype
        true,  // strict_optional
    );
    let fwd = subtypes::is_subtype(&erased, &typ2, &ctx, resolver.resolver())?;
    let bwd = subtypes::is_subtype(&typ2, &erased, &ctx, resolver.resolver())?;
    // Python folds: `if not fwd: co = False` (bit 1),
    // `if not bwd: contra = False` (bit 2; the settable rule is the
    // shim's job since it owns `settable`).
    let result = match (fwd, bwd) {
        (false, false) => MEMBER_BOTH,
        (false, true) => MEMBER_CO_ONLY,
        (true, false) => MEMBER_CONTRA_ONLY,
        (true, true) => MEMBER_NO_USE,
    };
    Some(result)
}

/// Shared `TypeResolver` borrow for parity unit tests.
#[allow(dead_code)]
fn resolver_ref(resolver: &mut NativeTypeResolver) -> &TypeResolver {
    resolver.resolver()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tv(raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: String::new(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 12,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 12,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        }
    }

    fn inst(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn any_impl() -> Type {
        Type::AnyType {
            type_of_any: 8, // implementation_artifact
            source_any: None,
            missing_import_name: None,
        }
    }

    fn callable(arg_types: Vec<Type>, ret_type: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(inst("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(ret_type),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn empty_resolver() -> NativeTypeResolver {
        NativeTypeResolver::new(Default::default(), Default::default())
    }

    #[test]
    fn erase_bare_self_instance_defers() {
        let self_t = inst("mod.C", vec![tv(1)]);
        let ret = inst("mod.C", vec![tv(1)]);
        let result = erase_return_self_types_wire(&ret, &self_t);
        // Python's erase_return_self_types ignores bare Instance heads
        // (only callable-form self returns become Any), so the wire
        // subset must defer to keep the Python decision authoritative.
        assert!(result.is_none());
    }

    #[test]
    fn erase_callable_ret_instance_becomes_any() {
        let self_t = inst("mod.C", vec![tv(1)]);
        let c = callable(vec![], inst("mod.C", vec![tv(1)]));
        let result = erase_return_self_types_wire(&c, &self_t).unwrap();
        match result {
            Type::CallableType { ret_type, .. } => assert_eq!(*ret_type, any_impl()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn erase_callable_ret_non_self_keeps_type() {
        let self_t = inst("mod.C", vec![tv(1)]);
        let ret = inst("builtins.int", vec![]);
        let c = callable(vec![], ret.clone());
        let result = erase_return_self_types_wire(&c, &self_t).unwrap();
        match result {
            Type::CallableType { ret_type, .. } => assert_eq!(*ret_type, ret),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn single_binding_env_keys_class_tvar() {
        let env = single_binding_env(7, &Type::NoneType);
        assert_eq!(env.len(), 1);
        assert!(env.contains_key(&(7, 0, String::new())));
    }

    #[test]
    fn empty_resolver_is_empty() {
        assert_eq!(resolver_ref(&mut empty_resolver()).len(), 0);
    }
}
