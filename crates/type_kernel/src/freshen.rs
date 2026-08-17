//! Native port of `mypy/expandtype.py` `freshen_all_functions_type_vars`
//! (expandtype.py:416-424), its `FreshenCallableVisitor`
//! (expandtype.py:427-435), and `freshen_function_type_vars`
//! (expandtype.py:379-398). Stage 3c.
//!
//! Takes a serialized `Type` and the current value of `TypeVarId.next_raw_id`
//! and returns, if the type was generic, the next `next_raw_id`, the changed
//! flag, and the wire-format type blob with fresh meta-level-1 type variables
//! substituted. Returns `None` for cases the Rust subset does not handle so
//! the Python caller falls through to the pure-Python visitor (the
//! strangler-fig per-call contract).
//!
//! Deferred (return None):
//!   * Overloaded, TypeAliasType, Parameters (unwritable after freshen).
//!   * CallableType with a ParamSpecType variable (mirrors the expand path's
//!     ParamSpec deferral; fresh ParamSpecs need prefix handling).
//!   * Instance with `extra_attrs` set.
//!   * translated `last_known_value` that is not a LiteralType.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::expandtype::{expand_type_inner, make_type_normalized, EnvKey};
use crate::setops::{union_item_can_be_false, union_item_can_be_true};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// `#[pyfunction]` entry for `freshen_all_functions_type_vars`. Returns
/// `(next_raw_id, changed, wire_bytes)`: `(start_raw_id, false, [])` for the
/// non-generic fast path; `None` (Python `None`) when Rust cannot handle it.
/// The Python shim advances `TypeVarId.next_raw_id` when `changed` and only
/// decodes `wire_bytes` then.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_freshen_all_functions_type_vars(
    start_raw_id: i64,
    type_bytes: &[u8],
    strict_optional: bool,
) -> Option<(i64, bool, Vec<u8>)> {
    let typ = read_type(&mut ReadBuffer::new(type_bytes), None).ok()?;
    let mut next_raw_id = start_raw_id;
    let mut changed = false;
    let result = freshen_type(&typ, &mut next_raw_id, &mut changed, strict_optional)?;
    if !changed {
        return Some((start_raw_id, false, Vec::new()));
    }
    let wire = encode_type(&result)?;
    Some((next_raw_id, true, wire))
}

/// Mirror `TypeTranslator` (type_visitor.py:181-340) with the
/// `FreshenCallableVisitor`/`freshen_function_type_vars` behavior folded in:
/// children are translated, and any generic `CallableType` gets its declared
/// type variables replaced with fresh meta-level-1 variables.
///
/// Returns `None` for deferred cases so the caller falls through to Python.
pub(crate) fn freshen_type(
    typ: &Type,
    next_raw_id: &mut i64,
    changed: &mut bool,
    strict_optional: bool,
) -> Option<Type> {
    match typ {
        // Leaf types that carry no children and are returned as-is.
        // (type_visitor.py:191-223 — unbound, any, none, uninhabited,
        // erased, deleted, type_var, param_spec, type_var_tuple, partial.)
        Type::AnyType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. }
        | Type::UnboundType { .. }
        | Type::TypeVarType { .. }
        | Type::ParamSpecType { .. }
        | Type::TypeVarTupleType { .. } => Some(typ.clone()),

        // visit_instance (type_visitor.py:224-237).
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            // `write_type` rejects an Instance carrying `extra_attrs`
            // (wire.rs:2019-2025); defer to Python to avoid a half-translated
            // round-trip.
            if extra_attrs.is_some() {
                return None;
            }
            if args.is_empty() && last_known_value.is_none() {
                // No children to translate — return as-is.
                return Some(typ.clone());
            }
            let mut new_args = Vec::with_capacity(args.len());
            for arg in args {
                new_args.push(freshen_type(arg, next_raw_id, changed, strict_optional)?);
            }
            let new_lkv = match last_known_value {
                Some(l) => {
                    let nl = freshen_type(l, next_raw_id, changed, strict_optional)?;
                    // Python asserts the translated LKV is a LiteralType.
                    if !matches!(nl, Type::LiteralType { .. }) {
                        return None;
                    }
                    Some(Box::new(nl))
                }
                None => None,
            };
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: new_lkv,
                // `extra_attrs` preserved as None (the Some case returned).
                extra_attrs: None,
            })
        }

        // Deferred: Overloaded (unwritable), TypeAliasType (recursive),
        // Parameters (unwritable).
        Type::Overloaded { .. } | Type::TypeAliasType { .. } | Type::Parameters(_) => None,

        // visit_tuple_type (type_visitor.py:269-276): `implicit` resets to
        // False in the Python rebuild.
        Type::TupleType {
            partial_fallback,
            items,
            implicit: _,
        } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(freshen_type(item, next_raw_id, changed, strict_optional)?);
            }
            let new_fallback =
                freshen_type(partial_fallback, next_raw_id, changed, strict_optional)?;
            Some(Type::TupleType {
                partial_fallback: Box::new(new_fallback),
                items: new_items,
                implicit: false,
            })
        }

        // visit_typeddict_type (type_visitor.py:278-294). No graph cache in
        // Rust (no TypedDictType recursion here).
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let mut new_items = Vec::with_capacity(items.len());
            for (name, typ) in items {
                new_items.push((
                    name.clone(),
                    freshen_type(typ, next_raw_id, changed, strict_optional)?,
                ));
            }
            let new_fallback = freshen_type(fallback, next_raw_id, changed, strict_optional)?;
            Some(Type::TypedDictType {
                fallback: Box::new(new_fallback),
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

        // visit_union_type (type_visitor.py:301-316). The Python `UnionType`
        // ctor re-derives `can_be_true`/`can_be_false` from the translated
        // items; mirrors that via `union_item_can_be_true/false`.
        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true: _,
            can_be_false: _,
        } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(freshen_type(item, next_raw_id, changed, strict_optional)?);
            }
            Some(Type::UnionType {
                can_be_true: new_items.iter().any(union_item_can_be_true),
                can_be_false: new_items.iter().any(union_item_can_be_false),
                uses_pep604_syntax: *uses_pep604_syntax,
                items: new_items,
            })
        }

        // visit_type_type (type_visitor.py:337-340) via `make_normalized`.
        Type::TypeType { item, is_type_form } => {
            let new_item = freshen_type(item, next_raw_id, changed, strict_optional)?;
            Some(make_type_normalized(new_item, *is_type_form))
        }

        // visit_literal_type (type_visitor.py:296-299): asserts the fallback
        // is an Instance.
        Type::LiteralType { fallback, value } => {
            let new_fallback = freshen_type(fallback, next_raw_id, changed, strict_optional)?;
            if !matches!(new_fallback, Type::Instance { .. }) {
                return None;
            }
            Some(Type::LiteralType {
                fallback: Box::new(new_fallback),
                value: value.clone(),
            })
        }

        Type::UnpackType { typ } => {
            let new_typ = freshen_type(typ, next_raw_id, changed, strict_optional)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
            })
        }

        // Mirrors visit_callable_type (type_visitor.py:257-267) then
        // freshen_function_type_vars (expandtype.py:379-398). Does not
        // translate `variables` or `type_guard`/`type_is` (visitor omits).
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
        } => {
            let new_instance_type = match instance_type {
                Some(it) => Some(Box::new(freshen_type(
                    it,
                    next_raw_id,
                    changed,
                    strict_optional,
                )?)),
                None => None,
            };
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            for at in arg_types {
                new_arg_types.push(freshen_type(at, next_raw_id, changed, strict_optional)?);
            }
            let new_ret_type = Box::new(freshen_type(
                ret_type,
                next_raw_id,
                changed,
                strict_optional,
            )?);
            let translated = Type::CallableType {
                fallback: fallback.clone(),
                instance_type: new_instance_type,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                from_type_type: *from_type_type,
                arg_types: new_arg_types,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: new_ret_type,
                name: name.clone(),
                // Not translated (identity).
                variables: variables.clone(),
                type_guard: type_guard.clone(),
                type_is: type_is.clone(),
            };

            if variables.is_empty() {
                // `if not callee.is_generic(): return callee` — but the
                // translated `result` from the super() visitor is what is
                // freshened, matching FreshenCallableVisitor.
                return Some(translated);
            }
            // Freshen the declared type vars (expandtype.py:384-393).
            let mut tvmap: HashMap<EnvKey, Type> = HashMap::with_capacity(variables.len());
            let mut tvs: Vec<Type> = Vec::with_capacity(variables.len());
            for v in variables {
                // ParamSpec fresh vars need prefix handling; defer to Python
                // (mirrors the expand path's ParamSpec deferral).
                if !matches!(v, Type::TypeVarType { .. }) {
                    return None;
                }
                let key = var_env_key(v);
                let mut fresh = fresh_type_var(v, *next_raw_id);
                *next_raw_id += 1;
                *changed = true;
                if tvar_has_default(&fresh) {
                    // Point to fresh ids in case defaults depend on
                    // previous variables (expandtype.py:390-392).
                    let new_default =
                        expand_type_inner(tvar_default(&fresh), &tvmap, strict_optional)?;
                    fresh = set_typevar_default(fresh, new_default);
                }
                tvmap.insert(key, fresh.clone());
                tvs.push(fresh);
            }
            let expanded = expand_type_inner(&translated, &tvmap, strict_optional)?;
            Some(set_callable_variables(expanded, tvs))
        }
    }
}

/// `TypeVarType.__eq__` env key: `(raw_id, meta_level, namespace)`.
fn var_env_key(v: &Type) -> EnvKey {
    match v {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => (*raw_id, *meta_level, namespace.clone()),
        _ => unreachable!("freshen: non-TypeVarType variable"),
    }
}

/// `new_unification_variable` + `TypeVarId.new(meta_level=1)`
/// (types.py:631-633, 560-563): a TypeVarType with a fresh `raw_id` and
/// `meta_level` 1 (so `namespace` reads as "" on wire encode).
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
        _ => unreachable!("freshen: non-TypeVarType variable"),
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

/// Set a TypeVarType's (already-expanded) default.
fn set_typevar_default(t: Type, new_default: Type) -> Type {
    match t {
        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            variance,
            meta_level,
            default: _,
        } => Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default: Box::new(new_default),
            variance,
            meta_level,
        },
        _ => unreachable!("freshen: non-TypeVarType variable"),
    }
}

/// Replace a callable's `variables` with the fresh ones
/// (`copy_modified(variables=tvs)`, expandtype.py:393).
fn set_callable_variables(t: Type, tvs: Vec<Type>) -> Type {
    match t {
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
            type_guard,
            type_is,
            variables: _,
        } => Type::CallableType {
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
            variables: tvs,
            type_guard,
            type_is,
        },
        _ => unreachable!("freshen: non-CallableType"),
    }
}

/// Unwrap a TypeVarType's `default`.
fn tvar_default(t: &Type) -> &Type {
    match t {
        Type::TypeVarType { default, .. } => default.as_ref(),
        _ => unreachable!("freshen: non-TypeVarType variable"),
    }
}

/// Encode a `Type` via `write_type`. Returns `None` if the variant is not
/// writable.
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}
