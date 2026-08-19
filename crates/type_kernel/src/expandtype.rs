//! Native port of `mypy/expandtype.py` `expand_type` (the TypeVar
//! substitution engine), Stage 3c.
//!
//! Takes a serialized `Type` and an `env: Mapping[TypeVarId, Type]` and
//! substitutes TypeVar references with their values, mirroring
//! `ExpandTypeVisitor` (expandtype.py:180-617). Returns `None` for cases
//! the Rust subset does not handle so the Python caller falls through to
//! the pure-Python visitor (the strangler-fig per-call contract).
//!
//! Deferred (return None):
//!   * ParamSpec (`visit_param_spec`, expandtype.py:252-285) — prefix
//!     merging and flavor handling are too complex for this stage.
//!   * TypeVarTuple substitution requiring `split_with_prefix_and_suffix`
//!     (the variadic middle of a generic instance).
//!   * `TypeAliasType` (unfixed) — defer.
//!   * `Overloaded`, `PartialType`, `Parameters` — defer.
//!   * `visit_callable_type` ParamSpec branch (expandtype.py:436-480).
//!   * `visit_type_var_tuple` (expandtype.py:355-368) raises
//!     `NotImplementedError` in Python for non-trivial replacements; we
//!     defer those to Python rather than raise over FFI.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::setops::{flatten_nested_unions, union_make_union};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{
    read_int_bare, read_str_bare, read_type, write_type, ReadBuffer, Type, WriteBuffer,
};

/// Key for the env: `(raw_id, meta_level, namespace)`. Mirrors
/// `TypeVarId.__eq__` (types.py:574-576), which compares `raw_id`,
/// `meta_level`, and `namespace`.
pub(crate) type EnvKey = (i64, i64, String);

/// `#[pyfunction]` entry for `expand_type`. The Python-side shim
/// (mypy/expandtype.py) calls this with the serialized `typ` blob, the
/// serialized `env`, and the `NativeTypeResolver` pyclass. Returns `None`
/// (Python `None`) when Rust doesn't handle the case; `Some(bytes)`
/// otherwise, holding a wire-format type blob the shim decodes via
/// `read_type`.
///
/// The env wire format is: count (bare int) + pairs of
/// (TypeVarId raw_id bare int + TypeVarId meta_level bare int +
/// TypeVarId namespace bare str + Type). Mirrors the Python-side
/// `_serialize_env` in mypy/expandtype.py.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_type(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    env_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let _ = resolver; // reserved for future Instance.has_type_var_tuple lookups
    let typ = decode_type(type_bytes)?;
    let env = decode_env(env_bytes)?;
    if env.is_empty() {
        return None;
    }
    // Wire-decoded TypeAliasType carries alias=None, which the Python graph
    // asserts against on access (types.py:362/397). Defer alias-bearing
    // inputs to Python, preserving object identity.
    if result_contains_typealias(&typ) {
        return None;
    }
    expand_with_env(&typ, &env, strict_optional)
}

/// Shared tail of the expand FFI entries: run the substitution and ship
/// only concrete (typevar-free) results. Python's solver is identity
/// based, so any leftover TypeVar after a wire round-trip defers.
fn expand_with_env(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let expanded = expand_type_with_env(typ, env, strict_optional)?;
    encode_type(&expanded)
}

/// Inner expansion returning the raw `Type`: leaves that carry no TypeVars
/// defer (Python returns the original object by identity), and any leftover
/// TypeVar after substitution defers for the same reason.
pub(crate) fn expand_type_with_env(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Type> {
    // Leaf types carry no TypeVars; Python returns the original object
    // (identity). We return a cloned copy — structurally identical, wire-safe.
    if is_leaf_type(typ) {
        return Some(typ.clone());
    }
    let expanded = expand_type_inner(typ, env, strict_optional)?;
    if result_has_typevar(&expanded) {
        return None;
    }
    // A surviving TypeAliasType decodes from the wire with alias=None, and
    // Python's TypeAliasType.is_recursive asserts alias is not None
    // (types.py:397), so an unfixed alias crashes the caller. Defer any
    // expansion whose result still contains a TypeAliasType node.
    if result_contains_typealias(&expanded) {
        return None;
    }
    Some(expanded)
}

/// `#[pyfunction]` entry for `expand_type_by_instance`
/// (mypy/expandtype.py:295-325). Serializable subset: plain class type
/// var binding (no TypeVarTuple), every arg readable, args length equal
/// to the class's `defn.type_vars`. Mirroring the Python zip-truncate, a
/// length mismatch leaves extra typevars unbound, so this defers.
///
/// Mirrors the non-TVT branch:
///   tvars = tuple(instance.type.defn.type_vars)
///   variables = {binder.id: arg for binder, arg in zip(tvars, instance.args)}
///   return expand_type(typ, variables)
///
/// The env keys use `(raw_id, 0, "")`: class typevars bind
/// `TypeVarId(raw_id)` (types.py:554 defaults meta_level=0, namespace="").
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_type_by_instance(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    instance_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let typ = decode_type(type_bytes)?;
    let instance = decode_type(instance_bytes)?;
    let expanded =
        expand_type_by_instance_core(&typ, &instance, resolver.resolver(), strict_optional)?;
    encode_type(&expanded)
}

/// Core `expand_type_by_instance` (mypy/expandtype.py:295-325): bind the
/// class typevars of `instance` in `typ`, then substitute them into `typ`.
/// Serializable subset: plain class type var binding (no TypeVarTuple),
/// every arg readable, args length equal to the class's `defn.type_vars`.
/// Mirroring the Python zip-truncate, a length mismatch leaves extra
/// typevars unbound, so this defers.
///
/// Mirrors the non-TVT branch:
///   tvars = tuple(instance.type.defn.type_vars)
///   variables = {binder.id: arg for binder, arg in zip(tvars, instance.args)}
///   return expand_type(typ, variables)
///
/// The env keys use `(raw_id, 0, "")`: class typevars bind
/// `TypeVarId(raw_id)` (types.py:554 defaults meta_level=0, namespace="").
pub(crate) fn expand_type_by_instance_core(
    typ: &Type,
    instance: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    let Type::Instance { type_ref, args, .. } = instance else {
        return None;
    };
    // Wire-decoded TypeAliasType carries alias=None, which the Python
    // graph asserts against (is_recursive/_expand_once, types.py:362/397).
    // Preserve identity by deferring any alias-bearing input to Python.
    if result_contains_typealias(typ) {
        return None;
    }
    let snap = resolver.get(type_ref)?;
    // TypeVarTuple branch (expandtype.py:302-316) stays in Python.
    if snap.has_type_var_tuple_type {
        return None;
    }
    // Python fast path (expandtype.py:298-299) returns `typ` unchanged
    // when the instance has no args and no TVT.
    if args.is_empty() {
        return Some(typ.clone());
    }
    let raw_ids = &snap.type_var_raw_ids;
    // Python `zip` truncates to the shorter; any unbound tvar makes
    // submission incomplete, so defer (the length mismatch is legal).
    if raw_ids.len() != args.len() {
        return None;
    }
    let mut env = HashMap::with_capacity(args.len());
    for (raw_id, arg) in raw_ids.iter().zip(args) {
        env.insert((*raw_id, 0, String::new()), arg.clone());
    }
    expand_type_with_env(typ, &env, strict_optional)
}

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Decode the env wire format into a `HashMap<EnvKey, Type>`. Returns
/// `None` on any read failure (truncated input, bad tag).
fn decode_env(bytes: &[u8]) -> Option<HashMap<EnvKey, Type>> {
    let mut buf = ReadBuffer::new(bytes);
    let count = read_int_bare(&mut buf).ok()?;
    if count < 0 {
        return None;
    }
    let mut env = HashMap::with_capacity(count as usize);
    for _ in 0..count {
        let raw_id = read_int_bare(&mut buf).ok()?;
        let meta_level = read_int_bare(&mut buf).ok()?;
        // namespace is a fullname string written via `librt write_str`
        // (bare short-int size + utf8, no tag). Must use the bare reader.
        let namespace = read_str_bare(&mut buf).ok()?;
        let typ = read_type(&mut buf, None).ok()?;
        env.insert((raw_id, meta_level, namespace), typ);
    }
    Some(env)
}

/// Encode a `Type` via `write_type`. Returns `None` if the variant is not
/// writable (the caller defers to Python).
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Substitute TypeVar references in `typ` using `env`, mirroring
/// `ExpandTypeVisitor`. Returns `None` for deferred cases (ParamSpec,
/// TypeAliasType, Overloaded, etc.) so the caller falls through to Python.
pub(crate) fn expand_type_inner(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Type> {
    match typ {
        // Leaf types that carry no TypeVars: returned as-is.
        // (expandtype.py:189-211)
        Type::AnyType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. }
        | Type::UnboundType { .. } => Some(typ.clone()),

        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = expand_type_tuple_with_unpack(args, env, strict_optional)?;
            // Tuple[*Tuple[X, ...], ...] -> Tuple[X, ...].
            // When single arg is UnpackType wrapping builtins.tuple,
            // unwrap to that Instance's args.
            let final_args = if type_ref == "builtins.tuple" && new_args.len() == 1 {
                normalize_tuple_unpack(&new_args[0])
            } else {
                new_args
            };
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: final_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }

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
        } => {
            // Self type (raw_id == 0): expand upper_bound first
            // (expandtype.py:243-244), since Self`0 <: C[T, S] may reference
            // other TypeVars in the bound.
            let upper_bound = if *raw_id == 0 {
                Box::new(expand_type_inner(upper_bound, env, strict_optional)?)
            } else {
                upper_bound.clone()
            };
            let key = (*raw_id, *meta_level, namespace.clone());
            let repl = env.get(&key);
            match repl {
                Some(Type::Instance {
                    type_ref,
                    args,
                    last_known_value: _,
                    extra_attrs,
                }) => {
                    // Python strips last_known_value on Instance replacements
                    // (expandtype.py:246-249).
                    Some(Type::Instance {
                        type_ref: type_ref.clone(),
                        args: args.clone(),
                        last_known_value: None,
                        extra_attrs: extra_attrs.clone(),
                    })
                }
                Some(other) => Some(other.clone()),
                None => {
                    // Unmatched TypeVar: return a copy with the (possibly
                    // expanded) upper_bound.
                    Some(Type::TypeVarType {
                        name: name.clone(),
                        fullname: fullname.clone(),
                        raw_id: *raw_id,
                        namespace: namespace.clone(),
                        values: values.clone(),
                        upper_bound,
                        default: default.clone(),
                        variance: *variance,
                        meta_level: *meta_level,
                    })
                }
            }
        }

        // UnionType: Python calls
        // make_union(remove_trivial(flatten_nested_unions(expanded))) then
        // get_proper_type, which collapses and deduplicates items.
        Type::UnionType { items, .. } => {
            let mut expanded = Vec::with_capacity(items.len());
            for item in items {
                expanded.push(expand_type_inner(item, env, strict_optional)?);
            }
            let flat = flatten_nested_unions(&expanded)?;
            let simplified = union_make_union(remove_trivial(&flat, strict_optional));
            Some(simplified)
        }

        // TypeType: Python expands the item then calls
        // TypeType.make_normalized(item, is_type_form), which distributes
        // Type[Union[A, B]] into Union[Type[A], Type[B]].
        Type::TypeType { item, is_type_form } => {
            let new_item = expand_type_inner(item, env, strict_optional)?;
            Some(make_type_normalized(new_item, *is_type_form))
        }

        // LiteralType: Python's visit_literal_type returns t as-is
        // (expandtype.py:751-753). Do not expand the fallback.
        Type::LiteralType { .. } => Some(typ.clone()),

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            // (expandtype.py:720-740)
            let new_items = expand_type_list_with_unpack(items, env, strict_optional)?;
            // Normalize Tuple[*Tuple[X, ...]] -> Tuple[X, ...].
            if new_items.len() == 1 {
                if let Type::UnpackType { typ: inner } = &new_items[0] {
                    // Python checks: not (TypeAliasType and is_recursive).
                    // Rust defers TypeAliasType entirely, so inner is never
                    // a TypeAliasType here.
                    let unpacked = inner.as_ref();
                    if let Type::Instance { type_ref, .. } = unpacked {
                        if type_ref == "builtins.tuple" {
                            // If partial_fallback is NOT builtins.tuple
                            // (named tuple), preserve the fallback.
                            let fb_is_tuple = matches!(
                                partial_fallback.as_ref(),
                                Type::Instance { type_ref: fb_ref, .. } if fb_ref == "builtins.tuple"
                            );
                            if fb_is_tuple {
                                return Some(unpacked.clone());
                            }
                            // Named tuple: return expanded fallback.
                            return expand_type_inner(partial_fallback, env, strict_optional);
                        }
                        // unpacked is not builtins.tuple: return fallback.
                        return expand_type_inner(partial_fallback, env, strict_optional);
                    }
                }
            }
            let new_fallback = expand_type_inner(partial_fallback, env, strict_optional)?;
            Some(Type::TupleType {
                partial_fallback: Box::new(new_fallback),
                items: new_items,
                implicit: *implicit,
            })
        }

        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            // (expandtype.py:556-563)
            let new_fallback = expand_type_inner(fallback, env, strict_optional)?;
            let mut new_items = Vec::with_capacity(items.len());
            for (name, typ) in items {
                new_items.push((name.clone(), expand_type_inner(typ, env, strict_optional)?));
            }
            Some(Type::TypedDictType {
                fallback: Box::new(new_fallback),
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

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
            // (expandtype.py:435-502). The ParamSpec branch
            // (expandtype.py:436-480) is deferred to Python: if any
            // variable is a ParamSpecType, return None.
            for v in variables {
                if matches!(v, Type::ParamSpecType { .. }) {
                    return None;
                }
            }
            // Bound methods (is_bound) defer: `__self`/`__cls` identity
            // does not survive a wire round-trip, and mypy's
            // bind_self/extract_callable_type relies on it.
            if *is_bound {
                return None;
            }
            // The Unpack interpolation branch
            // (expandtype.py:482-488, interpolate_args_for_unpack) is
            // deferred: if a var_arg is an UnpackType, defer to Python.
            for at in arg_types {
                if matches!(at, Type::UnpackType { .. }) {
                    return None;
                }
            }
            // ExpandTypeVisitor (expandtype.py:676) expands arg_types, ret_type,
            // type_guard, type_is, instance_type. Does NOT expand fallback or
            // variables (declared type vars are definitions).
            let new_instance_type = match instance_type {
                Some(it) => Some(Box::new(expand_type_inner(it, env, strict_optional)?)),
                None => None,
            };
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            for at in arg_types {
                new_arg_types.push(expand_type_inner(at, env, strict_optional)?);
            }
            let new_ret_type = Box::new(expand_type_inner(ret_type, env, strict_optional)?);
            let new_type_guard = match type_guard {
                Some(tg) => Some(Box::new(expand_type_inner(tg, env, strict_optional)?)),
                None => None,
            };
            let new_type_is = match type_is {
                Some(ti) => Some(Box::new(expand_type_inner(ti, env, strict_optional)?)),
                None => None,
            };
            Some(Type::CallableType {
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
                variables: variables.clone(),
                type_guard: new_type_guard,
                type_is: new_type_is,
            })
        }

        Type::UnpackType { typ } => {
            // (expandtype.py:370-380). visit_unpack_type carries a variadic
            // tuple over. We expand the inner type. The expand_unpack
            // list-expansion path is handled at the tuple/instance level.
            let new_typ = expand_type_inner(typ, env, strict_optional)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
            })
        }

        // TypeAliasType: Python's visit_type_alias_type expands the
        // arguments (expandtype.py:911-918). Target cannot contain typevars
        // (not bound by the alias itself), so we just expand the args.
        Type::TypeAliasType { args, type_ref } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = expand_type_list_with_unpack(args, env, strict_optional)?;
            Some(Type::TypeAliasType {
                args: new_args,
                type_ref: type_ref.clone(),
            })
        }

        // Overloaded: Python's visit_overloaded expands each item
        // (expandtype.py:811-818). Each item is a CallableType.
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_inner(item, env, strict_optional)?);
            }
            Some(Type::Overloaded { items: new_items })
        }

        // Parameters: Python's visit_parameters expands arg_types
        // (expandtype.py:709-710).
        Type::Parameters(params) => {
            let new_arg_types =
                expand_type_list_with_unpack(&params.arg_types, env, strict_optional)?;
            Some(Type::Parameters(crate::wire::Parameters {
                arg_types: new_arg_types,
                arg_kinds: params.arg_kinds.clone(),
                arg_names: params.arg_names.clone(),
                variables: params.variables.clone(),
                imprecise_arg_kinds: params.imprecise_arg_kinds,
            }))
        }

        // Deferred variants: ParamSpecType (prefix merging too complex for
        // this stage) and TypeVarTupleType (Python raises NotImplementedError
        // for non-trivial replacements).
        Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => None,
    }
}

/// `expand_type_tuple_with_unpack` (expandtype.py:523-532). Expands a
/// tuple of arg types, splicing in the items of any UnpackType wrapping
/// a TypeVarTupleType via `expand_unpack`. Non-Unpack args are expanded
/// normally.
fn expand_type_tuple_with_unpack(
    typs: &[Type],
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Vec<Type>> {
    let mut items = Vec::with_capacity(typs.len());
    for item in typs {
        if let Type::UnpackType { typ: inner } = item {
            if let Type::TypeVarTupleType { .. } = inner.as_ref() {
                // expand_unpack (expandtype.py:382-400).
                let spliced = expand_unpack(inner, env)?;
                items.extend(spliced);
                continue;
            }
        }
        items.push(expand_type_inner(item, env, strict_optional)?);
    }
    Some(items)
}

/// `expand_type_list_with_unpack` (expandtype.py:513-521). Same as
/// `expand_type_tuple_with_unpack` but over a Vec.
fn expand_type_list_with_unpack(
    typs: &[Type],
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Vec<Type>> {
    expand_type_tuple_with_unpack(typs, env, strict_optional)
}

/// `TypeType.make_normalized` (types.py:3677-3691): distributes
/// `Type[Union[A, B]]` into `Union[Type[A], Type[B]]` unless
/// `is_type_form`. The item comes from a wire round-trip so it is already
/// proper (`get_proper_type` is a no-op). The resulting union may be a
/// single TypeType (collapsed by `make_union`).
pub(crate) fn make_type_normalized(item: Type, is_type_form: bool) -> Type {
    if !is_type_form {
        if let Type::UnionType { items, .. } = &item {
            let mut tt_items = Vec::with_capacity(items.len());
            for u in items {
                tt_items.push(make_type_normalized(u.clone(), false));
            }
            return union_make_union(tt_items);
        }
    }
    Type::TypeType {
        item: Box::new(item),
        is_type_form,
    }
}

/// `remove_trivial` (expandtype.py:845-872). Makes trivial simplifications
/// on a list of types without `is_subtype`: drop bottom types (honoring
/// `strict_optional` for NoneType), short-circuit to a lone
/// `builtins.object`, and drop strict duplicates (push-first-wins).
/// The input comes from the wire format so every type is already proper.
fn remove_trivial(types: &[Type], strict_optional: bool) -> Vec<Type> {
    let mut removed_none = false;
    let mut new_types: Vec<Type> = Vec::new();
    for t in types {
        match t {
            Type::UninhabitedType { .. } => continue,
            Type::NoneType if !strict_optional => {
                removed_none = true;
                continue;
            }
            _ => {}
        }
        if let Type::Instance { type_ref, .. } = t {
            if type_ref == "builtins.object" {
                return vec![t.clone()];
            }
        }
        if !new_types.contains(t) {
            new_types.push(t.clone());
        }
    }
    if !new_types.is_empty() {
        return new_types;
    }
    if removed_none {
        return vec![Type::NoneType];
    }
    vec![Type::UninhabitedType { ambiguous: false }]
}

/// `expand_unpack` (expandtype.py:382-400). Expands an UnpackType whose
/// inner type is a TypeVarTupleType. Looks up the TypeVarTuple in env:
///   * TupleType -> its items (spliced in).
///   * builtins.tuple Instance or TypeVarTupleType -> [UnpackType(repl)].
///   * AnyType / UninhabitedType -> [UnpackType(tuple_fallback[args=[repl]])].
///   * else (UnpackType wrapping a TupleType) -> splice the inner items.
///
/// Returns None for any other replacement (defer to Python, which would
/// raise RuntimeError).
fn expand_unpack(tvt: &Type, env: &HashMap<EnvKey, Type>) -> Option<Vec<Type>> {
    let tvt = if let Type::TypeVarTupleType {
        raw_id, namespace, ..
    } = tvt
    {
        // TypeVarTupleType wire has no meta_level yet; env meta is 0.
        let key = (*raw_id, 0, namespace.clone());
        // Unmatched TypeVarTuple: defer to Python.
        env.get(&key)?
    } else {
        return None;
    };
    // If the replacement is itself an UnpackType, unwrap once
    // (expandtype.py:385-386).
    let repl = if let Type::UnpackType { typ: inner } = tvt {
        inner.as_ref()
    } else {
        tvt
    };
    match repl {
        Type::TupleType { items, .. } => Some(items.clone()),
        Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
            Some(vec![Type::UnpackType {
                typ: Box::new(repl.clone()),
            }])
        }
        Type::TypeVarTupleType { .. } => Some(vec![Type::UnpackType {
            typ: Box::new(repl.clone()),
        }]),
        Type::AnyType { .. } | Type::UninhabitedType { .. } => {
            // (expandtype.py:395-398) Replace *Ts = Any/Never with
            // *tuple[Any, ...] using the TypeVarTuple's tuple_fallback.
            let fallback = match tvt {
                Type::TypeVarTupleType { tuple_fallback, .. } => tuple_fallback.as_ref(),
                _ => return None,
            };
            let new_fallback = if let Type::Instance {
                type_ref,
                last_known_value,
                extra_attrs,
                ..
            } = fallback
            {
                Type::Instance {
                    type_ref: type_ref.clone(),
                    args: vec![repl.clone()],
                    last_known_value: last_known_value.clone(),
                    extra_attrs: extra_attrs.clone(),
                }
            } else {
                return None;
            };
            Some(vec![Type::UnpackType {
                typ: Box::new(new_fallback),
            }])
        }
        _ => None, // invalid replacement: defer to Python
    }
}

/// builtins.tuple arg normalization (expandtype.py:228-237). When the
/// single arg of `builtins.tuple` is an UnpackType wrapping a
/// builtins.tuple Instance, replace the arg list with that Instance's
/// args. Returns `new_args` unchanged otherwise.
fn normalize_tuple_unpack(arg: &Type) -> Vec<Type> {
    if let Some(Type::Instance { args, .. }) = normalize_tuple_unpack_to_instance(arg) {
        return args.clone();
    }
    vec![arg.clone()]
}

/// Check if `typ` is a leaf type with no TypeVar references to substitute.
/// Python's `ExpandTypeVisitor` returns `t` unchanged for these, so the
/// wire round-trip must defer to preserve object identity.
fn is_leaf_type(typ: &Type) -> bool {
    matches!(
        typ,
        Type::AnyType { .. }
            | Type::NoneType
            | Type::UninhabitedType { .. }
            | Type::DeletedType { .. }
            | Type::UnboundType { .. }
    )
}

/// True if `typ` contains any TypeVar-like node. Such results do not
/// survive a wire round-trip intact (object identity is lost), so the
/// caller defers to Python.
pub(crate) fn result_has_typevar(typ: &Type) -> bool {
    let mut stack = vec![typ];
    while let Some(cur) = stack.pop() {
        match cur {
            Type::TypeVarType { .. }
            | Type::ParamSpecType { .. }
            | Type::TypeVarTupleType { .. } => {
                // A TypeVar-like node means the expansion keeps a TypeVar that Python
                // preserves by object identity; defer. Nested contents
                // cannot matter for identity.
                return true;
            }
            Type::Instance { args, .. } => stack.extend(args.iter()),
            Type::TypeAliasType { args, .. } => stack.extend(args.iter()),
            Type::CallableType {
                arg_types,
                ret_type,
                fallback,
                instance_type,
                variables,
                ..
            } => {
                stack.extend(arg_types.iter());
                stack.push(ret_type);
                stack.push(fallback);
                if let Some(it) = instance_type {
                    stack.push(it);
                }
                stack.extend(variables.iter());
            }
            Type::TupleType {
                items,
                partial_fallback,
                ..
            } => {
                stack.extend(items.iter());
                stack.push(partial_fallback);
            }
            Type::TypedDictType {
                items, fallback, ..
            } => {
                stack.extend(items.iter().map(|(_, t)| t));
                stack.push(fallback);
            }
            Type::UnionType { items, .. } => stack.extend(items.iter()),
            Type::Overloaded { items, .. } => stack.extend(items.iter()),
            Type::Parameters(params) => {
                stack.extend(params.arg_types.iter());
                stack.extend(params.variables.iter());
            }
            Type::TypeType { item, .. } => stack.push(item),
            Type::UnpackType { typ } => stack.push(typ),
            Type::LiteralType { fallback, .. } => stack.push(fallback),
            _ => {}
        }
    }
    false
}

/// True if `typ` contains any TypeAliasType node. Wire round-trips decode
/// TypeAliasType with alias=None, which Python asserts against on access
/// (`TypeAliasType.is_recursive`, types.py:397), so such results must defer
/// to the Python visitor which preserves the original alias object.
fn result_contains_typealias(typ: &Type) -> bool {
    let mut stack = vec![typ];
    while let Some(cur) = stack.pop() {
        match cur {
            Type::TypeAliasType { .. } => {
                return true;
            }
            Type::Instance { args, .. } => stack.extend(args.iter()),
            Type::CallableType {
                arg_types,
                ret_type,
                fallback,
                instance_type,
                variables,
                ..
            } => {
                stack.extend(arg_types.iter());
                stack.push(ret_type);
                stack.push(fallback);
                if let Some(it) = instance_type {
                    stack.push(it);
                }
                stack.extend(variables.iter());
            }
            Type::TupleType {
                items,
                partial_fallback,
                ..
            } => {
                stack.extend(items.iter());
                stack.push(partial_fallback);
            }
            Type::TypedDictType {
                items, fallback, ..
            } => {
                stack.extend(items.iter().map(|(_, t)| t));
                stack.push(fallback);
            }
            Type::UnionType { items, .. } => stack.extend(items.iter()),
            Type::Overloaded { items, .. } => stack.extend(items.iter()),
            Type::Parameters(params) => {
                stack.extend(params.arg_types.iter());
                stack.extend(params.variables.iter());
            }
            Type::TypeType { item, .. } => stack.push(item),
            Type::UnpackType { typ } => stack.push(typ),
            Type::LiteralType { fallback, .. } => stack.push(fallback),
            _ => {}
        }
    }
    false
}

/// If `arg` is an UnpackType wrapping a builtins.tuple Instance, return
/// that Instance. Used by the TupleType single-item normalization
/// (expandtype.py:536-551) which returns the unpacked Instance directly.
/// Returns None otherwise.
fn normalize_tuple_unpack_to_instance(arg: &Type) -> Option<Type> {
    if let Type::UnpackType { typ: inner } = arg {
        if let Type::Instance { type_ref, .. } = inner.as_ref() {
            if type_ref == "builtins.tuple" {
                return Some((**inner).clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn snap_with_tvar(fullname: &str, raw_id: i64) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: fullname.to_owned(),
            has_type_var_tuple_type: false,
            type_var_raw_ids: vec![raw_id],
            ..Default::default()
        }
    }

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn tvar(raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "__main__.T".to_string(),
            raw_id,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            variance: 0,
            meta_level: 0,
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

    #[test]
    fn expand_by_instance_substitutes_tvar_in_args() {
        // List[T] applied to List[int] expands the arg T -> int.
        let typ = instance("builtins.list", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::from([((0, 0, String::new()), any())]);
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert!(matches!(args.as_slice(), [Type::AnyType { .. }]));
            }
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn expand_by_instance_unmatched_tvar_leaves_typevar() {
        // List[T] applied with an env that lacks T stays a TypeVarType.
        let typ = instance("builtins.list", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert!(matches!(args.as_slice(), [Type::TypeVarType { .. }]));
            }
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn etbi_sentinel_raw_id_never_matches_typevar() {
        // A -1 sentinel key never matches a real TypeVar (raw_id >= 0),
        // so expand_by_instance with an unreadable typevar defers.
        let snap = snap_with_tvar("foo.Box", -1);
        assert_eq!(snap.type_var_raw_ids, vec![-1]);
        let typ = instance("foo.Box", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let out = expand_type_inner(&typ, &env, false).unwrap();
        assert!(matches!(out, Type::Instance { ref args, .. } if matches!(
            args.as_slice(), [Type::TypeVarType { .. }])));
    }
}
