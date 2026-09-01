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
use crate::typeinfo::TypeResolver;
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

/// `#[pyfunction]` entry for `freshen_function_type_vars`
/// (expandtype.py:413-432). Takes the current `TypeVarId.next_raw_id` and a
/// serialized `Type`, and returns `(next_raw_id, wire_bytes)` with fresh
/// meta-level-1 type variables substituted, or `None` (Python `None`) when
/// Rust cannot handle the case so the caller falls through to the pure-Python
/// function. The Python shim advances `TypeVarId.next_raw_id` (same contract
/// as the freshen-all seam).
///
/// Deferred (return None):
///   * CallableType with a ParamSpecType variable (same deferral as the
///     freshen-all path and the expand path).
#[pyfunction]
pub(crate) fn rust_freshen_function_type_vars(
    start_raw_id: i64,
    callee_bytes: &[u8],
) -> PyResult<Option<(i64, Vec<u8>)>> {
    let callee = match read_type(&mut ReadBuffer::new(callee_bytes), None) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let mut next_raw_id = start_raw_id;
    match freshen_function_type_vars(&callee, &mut next_raw_id) {
        Some(result) => match encode_type(&result) {
            Some(wire) => Ok(Some((next_raw_id, wire))),
            None => Ok(None),
        },
        None => Ok(None),
    }
}

/// `id_rewrite` (join.py:1142-1148): per-pair id-rewrite on a callable's
/// declared type variables, mirroring `update_callable_ids[:the body]`
/// (join.py:1142-1150). The fixture variables (`vars`) and the `expanded`
/// result are assembled by the caller (`update_callable_ids_core`).
fn id_rewrite(expanded: Type, new_vars: Vec<Type>) -> Type {
    match expanded {
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
            variables: _,
            type_guard,
            type_is,
            ..
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
            variables: new_vars,
            type_guard,
            type_is,
            special_sig: None,
        },
        _ => unreachable!("id_rewrite: non-CallableType"),
    }
}

/// Fresh id holders + `expand_type(c, tv_map)`, the two halves of
/// `update_callable_ids` (join.py:1142-1150).
///
/// `base` is the callable's declared `variables` (the reader's snapshot),
/// `operand` the full callable (`&Type::CallableType`). Returns
/// `(expanded, new_vars)`. Defers on variants the expand path cannot
/// handle (ParamSpec vars, is_bound, Unpack args) so the caller falls
/// through to Python.
fn update_callable_ids_core(
    base: &[Type],
    operand: &Type,
    new_ids: &[(i64, i64)],
    ns: &str,
) -> Option<(Type, Vec<Type>)> {
    // The exchange map mirrors the env byte layout consumed by
    // `decode_env` (expandtype.rs): each entry is
    // (raw_id, meta_level, namespace, Type).
    let mut tvmap: HashMap<EnvKey, Type> = HashMap::with_capacity(base.len());
    let mut new_vars: Vec<Type> = Vec::with_capacity(base.len());
    for (v, (raw_id, _meta_level)) in base.iter().zip(new_ids.iter()) {
        if !matches!(v, Type::TypeVarType { .. }) {
            return None;
        }
        let Type::TypeVarType {
            raw_id: old_raw_id,
            meta_level: old_meta_level,
            namespace,
            ..
        } = v
        else {
            unreachable!("TypeVarType matched above");
        };
        let old_key = (*old_raw_id, *old_meta_level, namespace.clone());
        let fresh = set_typevar_id(v.clone(), *raw_id, ns);
        tvmap.insert(old_key, fresh.clone());
        new_vars.push(fresh);
    }
    let expanded = expand_type_inner(operand, &tvmap, false)?;
    Some((expanded, new_vars))
}

/// Set a TypeVarType's `raw_id` to a fresh id
/// (join.py:1110-1120). `copy_modified(id=...)` replaces the whole id,
/// so the fresh id's namespace `ns` and meta_level (0) win over the old
/// id's. Mirrors `TypeVarLikeType.copy_modified(id=...)`. `ns` is `""`
/// for the FFI counter-driven path (matching `TypeVarId.new`) and
/// `NATIVE_TVAR_NAMESPACE` for the registry-based renumber path.
fn set_typevar_id(t: Type, raw_id: i64, ns: &str) -> Type {
    match t {
        Type::TypeVarType {
            name,
            fullname,
            raw_id: _,
            namespace: _,
            values,
            upper_bound,
            default,
            variance,
            meta_level: _,
        } => Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace: ns.to_string(),
            values,
            upper_bound,
            default,
            variance,
            meta_level: 0,
        },
        _ => unreachable!("set_typevar_id: non-TypeVarType"),
    }
}

/// Sentinel namespace for native-allocated `TypeVarId`s. Contains a NUL
/// byte, which can never appear in a Python `TypeVarId.namespace` (a
/// dotted qualified name), so native ids never compare `==` to any
/// Python id even if raw ids were to collide (they cannot either: the
/// native counter starts at `NATIVE_TVAR_RAW_ID_BASE`).
pub(crate) const NATIVE_TVAR_NAMESPACE: &str = "\0native";

/// Native counterpart of `match_generic_callables`
/// (join.py:1292-1317) for in-engine callers: when both operands are
/// generic callables, renumber their type variables so both share one
/// id space, using ids allocated from the resolver's native registry
/// (never Python's global counter). `min_len == 0` is a no-op
/// (Python returns the inputs unchanged). `None` defers.
///
/// Both renumbered operands carry `NATIVE_TVAR_NAMESPACE` ids; the
/// `update_callable_ids_core` exchange maps keyed on the OLD ids keep
/// the substitution consistent across arg_types/ret_type on each side.
pub(crate) fn renumber_generic_pair(
    t: &Type,
    s: &Type,
    resolver: &TypeResolver,
) -> Option<(Type, Type)> {
    let (t_vars, s_vars) = match (t, s) {
        (Type::CallableType { variables: tv, .. }, Type::CallableType { variables: sv, .. }) => {
            (tv, sv)
        }
        _ => return None,
    };
    if t_vars
        .iter()
        .chain(s_vars.iter())
        .any(|v| !matches!(v, Type::TypeVarType { .. }))
    {
        return None;
    }
    let min_len = t_vars.len().min(s_vars.len());
    if min_len == 0 {
        return Some((t.clone(), s.clone()));
    }
    let num_vars = t_vars.len().max(s_vars.len());
    let new_ids = resolver.alloc_fresh_tvar_ids(num_vars);
    let (t_expanded, t_vars_out) =
        update_callable_ids_core(t_vars, t, &new_ids, NATIVE_TVAR_NAMESPACE)?;
    let (s_expanded, s_vars_out) =
        update_callable_ids_core(s_vars, s, &new_ids, NATIVE_TVAR_NAMESPACE)?;
    Some((
        id_rewrite(t_expanded, t_vars_out),
        id_rewrite(s_expanded, s_vars_out),
    ))
}

/// `#[pyfunction]` entry for `rust_match_generic_callables` (the
/// `match_generic_callables` id-renumbering used before joining/meeting
/// similar callables, join.py:1110-1120).
///
/// `num_vars` = `max(len(t.variables), len(s.variables))`: one shared
/// batch of fresh `TypeVarId.new(meta_level=0)` ids is allocated and
/// passed to BOTH operands (join.py:1117-1120 allocates a single
/// `new_ids` list shared by both `update_callable_ids` calls), so the
/// renumbered operands share one id space. `start_raw_id` is the current
/// `TypeVarId.next_raw_id`.
///
/// Returns `(next_raw_id, t_wire, s_wire)`: the advanced
/// `TypeVarId.next_raw_id` and the two rewired callables (each the
/// `expand_type(c, tv_map)` result, re-`variables`-ed).
/// `None` (Python `None`) defers the whole call to the pure-Python body.
#[pyfunction]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_match_generic_callables(
    num_vars: usize,
    start_raw_id: i64,
    t_bytes: &[u8],
    s_bytes: &[u8],
) -> PyResult<Option<(i64, Vec<u8>, Vec<u8>)>> {
    if num_vars == 0 {
        return Ok(None);
    }
    let (t, s) = match (
        read_type(&mut ReadBuffer::new(t_bytes), None),
        read_type(&mut ReadBuffer::new(s_bytes), None),
    ) {
        (Ok(t), Ok(s)) => (t, s),
        _ => return Ok(None),
    };
    let (
        Type::CallableType {
            variables: t_vars, ..
        },
        Type::CallableType {
            variables: s_vars, ..
        },
    ) = (&t, &s)
    else {
        return Ok(None);
    };
    let mut new_ids: Vec<(i64, i64)> = Vec::with_capacity(num_vars);
    let mut next_raw_id = start_raw_id;
    for _ in 0..num_vars {
        // match_generic_callables allocates `TypeVarId.new(meta_level=0)`
        // (join.py:1117), so the fresh ids carry meta_level 0.
        new_ids.push((next_raw_id, 0));
        next_raw_id += 1;
    }
    let (t_expanded, t_vars_out) = match update_callable_ids_core(t_vars, &t, &new_ids, "") {
        Some(pair) => pair,
        None => return Ok(None),
    };
    let (s_expanded, s_vars_out) = match update_callable_ids_core(s_vars, &s, &new_ids, "") {
        Some(pair) => pair,
        None => return Ok(None),
    };
    let t_wire = match encode_type(&id_rewrite(t_expanded, t_vars_out)) {
        Some(w) => w,
        None => return Ok(None),
    };
    let s_wire = match encode_type(&id_rewrite(s_expanded, s_vars_out)) {
        Some(w) => w,
        None => return Ok(None),
    };
    Ok(Some((next_raw_id, t_wire, s_wire)))
}

/// Mirror `freshen_function_type_vars` (expandtype.py:413-432) on the
/// wire `Type` graph. The input comes from a wire round-trip, so every
/// node is already proper. Fresh ids are allocated from `next_raw_id` and
/// advanced (mirrors Python's global `TypeVarId.next_raw_id`). Returns
/// `None` for deferred cases.
fn freshen_function_type_vars(callee: &Type, next_raw_id: &mut i64) -> Option<Type> {
    match callee {
        Type::CallableType { variables, .. } if variables.is_empty() => Some(callee.clone()),
        Type::CallableType { .. } => {
            // Non-typevar variables (ParamSpec) need prefix handling
            // (mirrors the freshen-all/expand deferral).
            if variables_have_non_typevar(callee) {
                return None;
            }
            // Build the tvmap with fresh meta-level-1 ids and expand the
            // defaults (expandtype.py:419-423).
            let mut tvmap: HashMap<EnvKey, Type> = HashMap::new();
            let mut tvs: Vec<Type> = Vec::new();
            for v in variables_of(callee).iter() {
                let mut fresh = fresh_type_var(v, *next_raw_id);
                *next_raw_id += 1;
                if tvar_has_default(&fresh) {
                    let new_default = expand_type_inner(tvar_default(&fresh), &tvmap, true)?;
                    fresh = set_typevar_default(fresh, new_default);
                }
                tvmap.insert(var_env_key(v), fresh.clone());
                tvs.push(fresh);
            }
            // expand_type(callee, tvmap) then copy_modified(variables=tvs).
            let expanded = expand_type_inner(callee, &tvmap, true)?;
            Some(set_callable_variables(expanded, tvs))
        }
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                let freshened = freshen_function_type_vars(item, next_raw_id)?;
                // Python asserts each Overloaded item is a CallableType.
                if !matches!(freshened, Type::CallableType { .. }) {
                    return None;
                }
                new_items.push(freshened);
            }
            Some(Type::Overloaded { items: new_items })
        }
        _ => None,
    }
}

/// True if the callable's `variables` include a non-TypeVarType entry
/// (ParamSpec / TypeVarTuple).
fn variables_have_non_typevar(callee: &Type) -> bool {
    let Type::CallableType { variables, .. } = callee else {
        return false;
    };
    variables
        .iter()
        .any(|v| !matches!(v, Type::TypeVarType { .. }))
}

/// The callable's declared `variables`.
fn variables_of(callee: &Type) -> &[Type] {
    let Type::CallableType { variables, .. } = callee else {
        return &[];
    };
    variables
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

        // Deferred: Overloaded handled above; Parameters (unwritable).
        Type::Overloaded { .. } | Type::Parameters(_) => None,

        // visit_type_alias_type (expandtype.py:640-642): only the alias's
        // args are translated; the alias node itself is kept, so the wire
        // format's missing alias target is harmless (no recursion).
        Type::TypeAliasType { args, type_ref } => {
            let mut new_args = Vec::with_capacity(args.len());
            for arg in args {
                new_args.push(freshen_type(arg, next_raw_id, changed, strict_optional)?);
            }
            Some(Type::TypeAliasType {
                args: new_args,
                type_ref: type_ref.clone(),
            })
        }

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
            ..
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
                is_evaluated: true,
                original_str_expr: None,
                original_str_fallback: None,
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

        Type::UnpackType { typ, .. } => {
            let new_typ = freshen_type(typ, next_raw_id, changed, strict_optional)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
                from_star_syntax: false,
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
            ..
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
                special_sig: None,
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
            ..
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
            special_sig: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::NATIVE_TVAR_RAW_ID_BASE;

    /// A generic callable `def f(tv: tv) -> tv`, mirroring
    /// `NativeJoinCallableIdsSuite._generic`.
    fn generic(t: &Type) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![t.clone()],
            arg_kinds: vec![1], // ARG_POS
            arg_names: vec![None],
            ret_type: Box::new(t.clone()),
            name: None,
            variables: vec![t.clone()],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn tvar(name: &str, raw_id: i64, meta_level: i64) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: name.to_string(),
            raw_id,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 4, // from_omitted_generics
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level,
        }
    }

    fn decode(bytes: &[u8]) -> Type {
        read_type(&mut ReadBuffer::new(bytes), None).expect("valid wire type")
    }

    #[test]
    fn renumbers_shared_batch() {
        // Both operands must share the same fresh ids (join.py:1117-1120).
        let t = generic(&tvar("T", 1, 0));
        let s = generic(&tvar("U", 2, 0));
        let (next, t_wire, s_wire) = rust_match_generic_callables(
            1,
            100,
            &encode_type(&t).unwrap(),
            &encode_type(&s).unwrap(),
        )
        .unwrap()
        .expect("engages");
        assert_eq!(next, 101);
        let t_out = decode(&t_wire);
        let s_out = decode(&s_wire);
        let Type::CallableType {
            variables: t_vars, ..
        } = &t_out
        else {
            panic!()
        };
        let Type::CallableType {
            variables: s_vars, ..
        } = &s_out
        else {
            panic!()
        };
        // Both first variables get the same fresh id.
        assert_eq!(t_vars[0], tvar("T", 100, 0));
        assert_eq!(s_vars[0], tvar("U", 100, 0));
    }

    #[test]
    fn renumbers_mixed_arity() {
        // t: [T], s: [U, V]. Shared batch of 2: T -> id A, U -> id A,
        // V -> id A+1.
        let t = generic(&tvar("T", 1, 0));
        let s = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![tvar("U", 2, 0), tvar("V", 3, 0)],
            arg_kinds: vec![1, 1],
            arg_names: vec![None, None],
            ret_type: Box::new(tvar("U", 2, 0)),
            name: None,
            variables: vec![tvar("U", 2, 0), tvar("V", 3, 0)],
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        let (next, t_wire, s_wire) = rust_match_generic_callables(
            2,
            50,
            &encode_type(&t).unwrap(),
            &encode_type(&s).unwrap(),
        )
        .unwrap()
        .expect("engages");
        assert_eq!(next, 52);
        let t_out = decode(&t_wire);
        let s_out = decode(&s_wire);
        let Type::CallableType {
            arg_types: t_args, ..
        } = &t_out
        else {
            panic!()
        };
        let Type::CallableType {
            variables: s_vars,
            arg_types: s_args,
            ..
        } = &s_out
        else {
            panic!()
        };
        // T and U both get id 50; V gets 51.
        assert_eq!(t_args[0], tvar("T", 50, 0));
        assert_eq!(s_args[0], tvar("U", 50, 0));
        assert_eq!(s_vars[1], tvar("V", 51, 0));
    }

    #[test]
    fn non_callable_defers() {
        let not_callable = Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        let result = rust_match_generic_callables(
            1,
            0,
            &encode_type(&not_callable).unwrap(),
            &encode_type(&not_callable).unwrap(),
        )
        .unwrap();
        assert!(result.is_none(), "non-CallableType defers");
    }

    #[test]
    fn zero_vars_defers() {
        // Python never calls for min_len == 0 (short-circuits first); the
        // seam still defers instead of mis-behaving.
        let result = rust_match_generic_callables(0, 0, b"", b"").unwrap();
        assert!(result.is_none(), "num_vars == 0 defers");
    }

    #[test]
    fn renumber_registry_shared_batch_sentinel_ns() {
        // renumber_generic_pair: both operands share one registry batch
        // and the fresh ids carry the sentinel NUL namespace (never == to
        // a Python id) with raw ids starting at NATIVE_TVAR_RAW_ID_BASE.
        let r = TypeResolver::new();
        let t = generic(&tvar("T", 1, 0));
        let s = generic(&tvar("U", 2, 0));
        let (t_out, s_out) = renumber_generic_pair(&t, &s, &r).expect("engages");
        // The fixture helper uses namespace ""; stamp the sentinel in.
        let expected = match tvar("T", NATIVE_TVAR_RAW_ID_BASE, 0) {
            Type::TypeVarType {
                name,
                fullname,
                raw_id,
                values,
                upper_bound,
                default,
                variance,
                meta_level,
                ..
            } => Type::TypeVarType {
                name,
                fullname,
                raw_id,
                namespace: NATIVE_TVAR_NAMESPACE.to_string(),
                values,
                upper_bound,
                default,
                variance,
                meta_level,
            },
            other => other,
        };
        let (t_vars, s_vars) = match (&t_out, &s_out) {
            (
                Type::CallableType { variables: tv, .. },
                Type::CallableType { variables: sv, .. },
            ) => (tv, sv),
            other => panic!("expected CallableTypes, got {other:?}"),
        };
        assert_eq!(t_vars[0], expected);
        // Both operands must share the same (raw_id, namespace) id.
        let s_expected = match tvar("U", NATIVE_TVAR_RAW_ID_BASE, 0) {
            Type::TypeVarType {
                ref name,
                ref fullname,
                raw_id,
                ref values,
                ref upper_bound,
                ref default,
                variance,
                meta_level,
                ..
            } => Type::TypeVarType {
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id,
                namespace: NATIVE_TVAR_NAMESPACE.to_string(),
                values: values.clone(),
                upper_bound: upper_bound.clone(),
                default: default.clone(),
                variance,
                meta_level,
            },
            other => other,
        };
        assert_eq!(s_vars[0], s_expected);
        // The substitution reached the arg and ret positions too.
        let (t_args, t_ret) = match &t_out {
            Type::CallableType {
                arg_types,
                ret_type,
                ..
            } => (arg_types, ret_type.as_ref()),
            other => panic!("expected CallableType, got {other:?}"),
        };
        assert_eq!(t_args[0], expected);
        assert_eq!(t_ret, &expected);
    }

    #[test]
    fn renumber_registry_min_len_zero_noop() {
        // One side non-generic: Python returns the inputs unchanged.
        let t = generic(&tvar("T", 1, 0));
        let s = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }],
            arg_kinds: vec![1],
            arg_names: vec![None],
            ret_type: Box::new(Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        let (t_out, s_out) = renumber_generic_pair(&t, &s, &TypeResolver::new()).unwrap();
        assert_eq!(t_out, t);
        assert_eq!(s_out, s);
    }

    #[test]
    fn renumber_registry_non_callable_defers() {
        let not_callable = Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(
            renumber_generic_pair(&not_callable, &not_callable, &TypeResolver::new()).is_none()
        );
    }

    #[test]
    fn renumber_registry_counter_monotonic() {
        // Two sequential renumbers allocate disjoint id batches; the
        // registry never reuses ids.
        let r = TypeResolver::new();
        let t = generic(&tvar("T", 1, 0));
        let (t1, _s1) = renumber_generic_pair(&t, &t, &r).unwrap();
        let (t2, _s2) = renumber_generic_pair(&t, &t, &r).unwrap();
        let id1 = match &t1 {
            Type::CallableType { variables, .. } => match &variables[0] {
                Type::TypeVarType { raw_id, .. } => *raw_id,
                other => panic!("expected TypeVarType, got {other:?}"),
            },
            other => panic!("expected CallableType, got {other:?}"),
        };
        let id2 = match &t2 {
            Type::CallableType { variables, .. } => match &variables[0] {
                Type::TypeVarType { raw_id, .. } => *raw_id,
                other => panic!("expected TypeVarType, got {other:?}"),
            },
            other => panic!("expected CallableType, got {other:?}"),
        };
        assert_eq!(id1, NATIVE_TVAR_RAW_ID_BASE);
        assert_eq!(id2, NATIVE_TVAR_RAW_ID_BASE + 1);
    }

    #[test]
    fn type_alias_type_visits_args_only() {
        // FreshenCallableVisitor.visit_type_alias_type (expandtype.py:640-642):
        // the alias node is kept (copy_modified(args=...)) and only its
        // args are translated; a bare TypeVar arg passes through unchanged.
        let alias = Type::TypeAliasType {
            args: vec![Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![tvar("T", 1, 0)],
                last_known_value: None,
                extra_attrs: None,
            }],
            type_ref: "m.Alias".to_string(),
        };
        let mut next_raw_id = 100;
        let mut changed = false;
        let out = freshen_type(&alias, &mut next_raw_id, &mut changed, true)
            .expect("TypeAliasType engages");
        match out {
            Type::TypeAliasType { args, type_ref } => {
                assert_eq!(type_ref, "m.Alias");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Type::Instance { type_ref, args, .. } => {
                        assert_eq!(type_ref, "builtins.tuple");
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], tvar("T", 1, 0));
                    }
                    other => panic!("expected Instance arg, got {other:?}"),
                }
            }
            other => panic!("expected TypeAliasType, got {other:?}"),
        }
    }

    #[test]
    fn type_alias_type_bad_arg_defers() {
        // An arg freshen_type cannot decide (Parameters) defers the alias.
        let alias = Type::TypeAliasType {
            args: vec![Type::Parameters(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            })],
            type_ref: "m.Alias".to_string(),
        };
        let mut next_raw_id = 7;
        let mut changed = false;
        assert!(freshen_type(&alias, &mut next_raw_id, &mut changed, true).is_none());
        assert_eq!(next_raw_id, 7);
    }
}
