//! Native port of `mypy/erasetype.py` `erase_typevars` and
//! `replace_meta_vars` (the `TypeVarEraser` TypeTranslator), Stage 4c.
//!
//! Takes a serialized `Type` and conditionally replaces TypeVarType /
//! ParamSpecType / TypeVarTupleType references with a `replacement` type
//! (typically `Any`). When `ids_to_erase` is `None`, ALL type variables are
//! erased. When `ids_to_erase` is a set of `(raw_id, namespace)` pairs, only
//! matching variables are erased.
//!
//! Mirrors `TypeVarEraser` (erasetype.py:204-285). Returns `None` for cases
//! the Rust subset does not handle so the Python caller falls through to the
//! pure-Python visitor (the strangler-fig per-call contract).
//!
//! Deferred (return None):
//!   * `TypeAliasType` — Python's visitor calls `t.copy_modified(args=...)`
//!     which needs the live alias target. Defer to Python.
//!   * `Overloaded` — Python recurses into items; we defer to keep the
//!     parity surface narrow (Overloaded is rare in constraint contexts).
//!   * `UnboundType` — Python returns `AnyType(from_error)`, but the
//!     wire-format UnboundType has no `defn.type_vars`, so we can't
//!     fully replicate. Defer.
//!   * `PartialType` — Python raises RuntimeError; we defer.

use std::collections::HashSet;

use pyo3::prelude::*;

use crate::wire::{read_int_bare, read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// Key for the ids_to_erase set: `(raw_id, namespace)`. Mirrors
/// `TypeVarId.__eq__` (types.py:574-576), which compares `raw_id` and
/// `namespace`.
type IdKey = (i64, String);

/// `#[pyfunction]` entry for `erase_typevars`. The Python-side shim
/// (mypy/erasetype.py) calls this with the serialized `typ` blob. When
/// `ids_bytes` is empty, ALL type variables are erased (mirrors
/// `ids_to_erase=None`). Otherwise `ids_bytes` encodes a list of
/// `(raw_id, namespace)` pairs.
///
/// Returns `None` (Python `None`) when Rust doesn't handle the case;
/// `Some(bytes)` otherwise, holding a wire-format type blob the shim
/// decodes via `read_type`.
#[pyfunction]
pub(crate) fn rust_erase_typevars(type_bytes: &[u8], ids_bytes: &[u8]) -> Option<Vec<u8>> {
    let typ = decode_type(type_bytes)?;
    let ids = decode_ids(ids_bytes)?;
    let replacement = make_any();
    let result = erase_typevars_inner(&typ, ids.as_ref(), &replacement)?;
    encode_type(&result)
}

/// `#[pyfunction]` entry for `replace_meta_vars`. Replaces only meta-var
/// type variables (raw_id < 0, matching `TypeVarId.is_meta_var()`). The
/// `target_bytes` is the serialized replacement type.
///
/// Mirrors `replace_meta_vars` (erasetype.py:199-201).
#[pyfunction]
pub(crate) fn rust_replace_meta_vars(type_bytes: &[u8], target_bytes: &[u8]) -> Option<Vec<u8>> {
    let typ = decode_type(type_bytes)?;
    let target = decode_type(target_bytes)?;
    let result = replace_meta_vars_inner(&typ, &target)?;
    encode_type(&result)
}

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Decode the ids wire format into a `HashSet<IdKey>`. Empty bytes means
/// `None` (erase ALL type vars). Otherwise: count (bare int) + pairs of
/// (raw_id bare int + namespace tagged str).
fn decode_ids(bytes: &[u8]) -> Option<Option<HashSet<IdKey>>> {
    if bytes.is_empty() {
        // ids_to_erase is None — erase ALL type variables.
        return Some(None);
    }
    let mut buf = ReadBuffer::new(bytes);
    let count = read_int_bare(&mut buf).ok()?;
    if count < 0 {
        return None;
    }
    let mut ids = HashSet::with_capacity(count as usize);
    for _ in 0..count {
        let raw_id = read_int_bare(&mut buf).ok()?;
        let namespace = crate::wire::read_str(&mut buf).ok()?;
        ids.insert((raw_id, namespace));
    }
    Some(Some(ids))
}

/// Encode a `Type` via `write_type`. Returns `None` if the variant is not
/// writable (the caller defers to Python).
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Construct the `AnyType(special_form)` replacement that Python's
/// `erase_typevars` uses when `ids_to_erase` is None or matches.
/// `TypeOfAny.special_form` == 12 in the wire format (types.py enum).
fn make_any() -> Type {
    Type::AnyType {
        type_of_any: 12,
        source_any: None,
        missing_import_name: None,
    }
}

/// Check if a TypeVar key should be erased. When `ids` is `None`, erase all.
/// When `ids` is `Some(set)`, erase only if the key is in the set.
fn should_erase(raw_id: i64, namespace: &str, ids: Option<&HashSet<IdKey>>) -> bool {
    match ids {
        None => true,
        Some(set) => set.contains(&(raw_id, namespace.to_string())),
    }
}

/// Check if a TypeVarId is a meta var. Mirrors `TypeVarId.is_meta_var()`
/// (types.py:495-504): meta vars have negative raw_id.
fn is_meta_var(raw_id: i64) -> bool {
    raw_id < 0
}

/// The core TypeVarEraser visitor. Mirrors `TypeVarEraser` (erasetype.py:204-285).
/// Recursively transforms the type tree, replacing TypeVar references that
/// match the `ids` predicate with `replacement`.
fn erase_typevars_inner(
    typ: &Type,
    ids: Option<&HashSet<IdKey>>,
    replacement: &Type,
) -> Option<Type> {
    match typ {
        // Leaf types with no TypeVars: return as-is.
        Type::AnyType { .. }
        | Type::NoneType
        | Type::UninhabitedType
        | Type::DeletedType { .. } => Some(typ.clone()),

        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
        } => {
            if should_erase(*raw_id, namespace, ids) {
                Some(replacement.clone())
            } else {
                Some(Type::TypeVarType {
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    values: values.clone(),
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                    variance: *variance,
                })
            }
        }

        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
        } => {
            if should_erase(*raw_id, namespace, ids) {
                Some(replacement.clone())
            } else {
                // Erase the prefix recursively.
                let new_prefix = erase_typevars_parameters(prefix, ids, replacement)?;
                Some(Type::ParamSpecType {
                    prefix: Box::new(new_prefix),
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    flavor: *flavor,
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                })
            }
        }

        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
        } => {
            if should_erase(*raw_id, namespace, ids) {
                // TypeVarEraser.visit_type_var_tuple:
                // t.tuple_fallback.copy_modified(args=[self.replacement])
                // We need to produce a TupleType(replacement) with the
                // tuple_fallback. But tuple_fallback is an Instance, and
                // copy_modified(args=[replacement]) means the new tuple has
                // a single arg = replacement.
                // However, the wire format TypeVarTupleType.tuple_fallback is
                // an Instance Type. copy_modified(args=[repl]) would create
                // an Instance with args=[repl]. We can't do copy_modified on
                // a wire-format Type without knowing the TypeInfo. Defer.
                None
            } else {
                Some(Type::TypeVarTupleType {
                    tuple_fallback: tuple_fallback.clone(),
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                    min_len: *min_len,
                })
            }
        }

        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            // visit_instance: super().visit_instance(t) then normalize
            // builtins.tuple unpack.
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = erase_typevars_list(args, ids, replacement)?;
            // TypeVarEraser.visit_instance normalization (erasetype.py:238-247):
            // builtins.tuple with single UnpackType(Instance(builtins.tuple))
            // arg -> unwrap to that Instance.
            if type_ref == "builtins.tuple" && new_args.len() == 1 {
                if let Some(unwrapped) = normalize_tuple_unpack(&new_args[0]) {
                    return Some(unwrapped);
                }
            }
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            let new_items = erase_typevars_list(items, ids, replacement)?;
            let new_fallback = erase_typevars_inner(partial_fallback, ids, replacement)?;
            // TypeVarEraser.visit_tuple_type normalization (erasetype.py:258-271):
            // Tuple[*Tuple[X, ...]] -> Tuple[X, ...] (single item, unpack
            // wrapping builtins.tuple Instance, fallback is builtins.tuple).
            if new_items.len() == 1 {
                if let Some(unwrapped) = normalize_tuple_unpack(&new_items[0]) {
                    // Only normalize if fallback is builtins.tuple.
                    if is_builtins_tuple(&new_fallback) {
                        return Some(unwrapped);
                    }
                    // If it's a named tuple (non-builtins.tuple fallback),
                    // return partial_fallback.accept(self) — i.e. erase the
                    // fallback. We can't easily detect named_tuple from wire
                    // format, so defer to Python for non-builtins.tuple.
                    return None;
                }
            }
            Some(Type::TupleType {
                partial_fallback: Box::new(new_fallback),
                items: new_items,
                implicit: *implicit,
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
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            let new_fallback = erase_typevars_inner(fallback, ids, replacement)?;
            let new_instance_type = erase_typevars_opt(instance_type.as_deref(), ids, replacement)?;
            let new_arg_types = erase_typevars_list(arg_types, ids, replacement)?;
            let new_ret_type = erase_typevars_inner(ret_type, ids, replacement)?;
            let new_variables = erase_typevars_list(variables, ids, replacement)?;
            let new_type_guard = erase_typevars_opt(type_guard.as_deref(), ids, replacement)?;
            let new_type_is = erase_typevars_opt(type_is.as_deref(), ids, replacement)?;
            Some(Type::CallableType {
                fallback: Box::new(new_fallback),
                instance_type: new_instance_type.map(Box::new),
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                arg_types: new_arg_types,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(new_ret_type),
                name: name.clone(),
                variables: new_variables,
                type_guard: new_type_guard.map(Box::new),
                type_is: new_type_is.map(Box::new),
            })
        }

        Type::UnionType {
            items,
            uses_pep604_syntax,
        } => {
            let new_items = erase_typevars_list(items, ids, replacement)?;
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
            })
        }

        Type::TypeType { item, is_type_form } => {
            let new_item = erase_typevars_inner(item, ids, replacement)?;
            Some(Type::TypeType {
                item: Box::new(new_item),
                is_type_form: *is_type_form,
            })
        }

        Type::LiteralType { fallback, value } => {
            let new_fallback = erase_typevars_inner(fallback, ids, replacement)?;
            Some(Type::LiteralType {
                fallback: Box::new(new_fallback),
                value: value.clone(),
            })
        }

        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let new_fallback = erase_typevars_inner(fallback, ids, replacement)?;
            let mut new_items = Vec::with_capacity(items.len());
            for (key, val) in items {
                new_items.push((key.clone(), erase_typevars_inner(val, ids, replacement)?));
            }
            Some(Type::TypedDictType {
                fallback: Box::new(new_fallback),
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(erase_typevars_inner(item, ids, replacement)?);
            }
            Some(Type::Overloaded { items: new_items })
        }

        Type::UnpackType { typ: inner } => {
            let new_inner = erase_typevars_inner(inner, ids, replacement)?;
            Some(Type::UnpackType {
                typ: Box::new(new_inner),
            })
        }

        Type::Parameters(params) => {
            let new_params = erase_typevars_parameters(params, ids, replacement)?;
            Some(Type::Parameters(new_params))
        }

        // Deferred: TypeAliasType needs live alias target.
        Type::TypeAliasType { .. } => None,
        // Deferred: UnboundType needs defn.type_vars for erasure.
        Type::UnboundType { .. } => None,
    }
}

/// Erase typevars in a list of types.
fn erase_typevars_list(
    types: &[Type],
    ids: Option<&HashSet<IdKey>>,
    replacement: &Type,
) -> Option<Vec<Type>> {
    let mut result = Vec::with_capacity(types.len());
    for t in types {
        result.push(erase_typevars_inner(t, ids, replacement)?);
    }
    Some(result)
}

/// Erase typevars in an optional type.
fn erase_typevars_opt(
    typ: Option<&Type>,
    ids: Option<&HashSet<IdKey>>,
    replacement: &Type,
) -> Option<Option<Type>> {
    match typ {
        None => Some(None),
        Some(t) => Some(Some(erase_typevars_inner(t, ids, replacement)?)),
    }
}

/// Erase typevars in a Parameters type.
fn erase_typevars_parameters(
    params: &crate::wire::Parameters,
    ids: Option<&HashSet<IdKey>>,
    replacement: &Type,
) -> Option<crate::wire::Parameters> {
    let new_arg_types = erase_typevars_list(&params.arg_types, ids, replacement)?;
    let new_variables = erase_typevars_list(&params.variables, ids, replacement)?;
    Some(crate::wire::Parameters {
        arg_types: new_arg_types,
        arg_kinds: params.arg_kinds.clone(),
        arg_names: params.arg_names.clone(),
        imprecise_arg_kinds: params.imprecise_arg_kinds,
        variables: new_variables,
    })
}

/// Replace meta-vars in a Parameters struct.
fn replace_meta_vars_parameters(
    params: &crate::wire::Parameters,
    target: &Type,
) -> Option<crate::wire::Parameters> {
    let new_arg_types: Vec<Type> = params
        .arg_types
        .iter()
        .map(|t| replace_meta_vars_inner(t, target))
        .collect::<Option<Vec<_>>>()?;
    let new_variables: Vec<Type> = params
        .variables
        .iter()
        .map(|t| replace_meta_vars_inner(t, target))
        .collect::<Option<Vec<_>>>()?;
    Some(crate::wire::Parameters {
        arg_types: new_arg_types,
        arg_kinds: params.arg_kinds.clone(),
        arg_names: params.arg_names.clone(),
        imprecise_arg_kinds: params.imprecise_arg_kinds,
        variables: new_variables,
    })
}

/// Replace only meta-var type variables (raw_id < 0) with the target type.
/// Mirrors `replace_meta_vars` (erasetype.py:199-201) which calls
/// `TypeVarEraser(erase_meta_id, target_type)`.
fn replace_meta_vars_inner(typ: &Type, target: &Type) -> Option<Type> {
    match typ {
        Type::TypeVarType { raw_id, .. } => {
            if is_meta_var(*raw_id) {
                Some(target.clone())
            } else {
                Some(typ.clone())
            }
        }
        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
        } => {
            if is_meta_var(*raw_id) {
                Some(target.clone())
            } else {
                let new_prefix = replace_meta_vars_parameters(prefix, target)?;
                Some(Type::ParamSpecType {
                    prefix: Box::new(new_prefix),
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    flavor: *flavor,
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                })
            }
        }
        Type::TypeVarTupleType { .. } => {
            // TypeVarEraser.visit_type_var_tuple with meta_var check.
            // We can't do copy_modified on wire format, so defer.
            None
        }
        // For composite types, recurse. Reuse erase_typevars_inner but with
        // a meta-var-only predicate. We can't pass a closure, so inline the
        // recursion for each variant.
        _ => erase_typevars_with_meta_check(typ, target),
    }
}

/// Recursive helper for replace_meta_vars on composite types. This mirrors
/// erase_typevars_inner but uses the `is_meta_var` check instead of the
/// `ids` set. We could unify these, but the meta-var case is simpler (no
/// set lookup), so keeping them separate avoids overhead on the hot path.
fn erase_typevars_with_meta_check(typ: &Type, target: &Type) -> Option<Type> {
    match typ {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args: Vec<Type> = args
                .iter()
                .map(|t| replace_meta_vars_inner(t, target))
                .collect::<Option<Vec<_>>>()?;
            if type_ref == "builtins.tuple" && new_args.len() == 1 {
                if let Some(unwrapped) = normalize_tuple_unpack(&new_args[0]) {
                    return Some(unwrapped);
                }
            }
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }
        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            let new_items: Vec<Type> = items
                .iter()
                .map(|t| replace_meta_vars_inner(t, target))
                .collect::<Option<Vec<_>>>()?;
            let new_fallback = replace_meta_vars_inner(partial_fallback, target)?;
            if new_items.len() == 1 {
                if let Some(unwrapped) = normalize_tuple_unpack(&new_items[0]) {
                    if is_builtins_tuple(&new_fallback) {
                        return Some(unwrapped);
                    }
                    return None;
                }
            }
            Some(Type::TupleType {
                partial_fallback: Box::new(new_fallback),
                items: new_items,
                implicit: *implicit,
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
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            let new_fallback = replace_meta_vars_inner(fallback, target)?;
            let new_instance_type = match instance_type {
                Some(t) => Some(Box::new(replace_meta_vars_inner(t, target)?)),
                None => None,
            };
            let new_arg_types: Vec<Type> = arg_types
                .iter()
                .map(|t| replace_meta_vars_inner(t, target))
                .collect::<Option<Vec<_>>>()?;
            let new_ret_type = replace_meta_vars_inner(ret_type, target)?;
            let new_variables: Vec<Type> = variables
                .iter()
                .map(|t| replace_meta_vars_inner(t, target))
                .collect::<Option<Vec<_>>>()?;
            let new_type_guard = match type_guard {
                Some(t) => Some(Box::new(replace_meta_vars_inner(t, target)?)),
                None => None,
            };
            let new_type_is = match type_is {
                Some(t) => Some(Box::new(replace_meta_vars_inner(t, target)?)),
                None => None,
            };
            Some(Type::CallableType {
                fallback: Box::new(new_fallback),
                instance_type: new_instance_type,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                arg_types: new_arg_types,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(new_ret_type),
                name: name.clone(),
                variables: new_variables,
                type_guard: new_type_guard,
                type_is: new_type_is,
            })
        }
        Type::UnionType {
            items,
            uses_pep604_syntax,
        } => {
            let new_items: Vec<Type> = items
                .iter()
                .map(|t| replace_meta_vars_inner(t, target))
                .collect::<Option<Vec<_>>>()?;
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
            })
        }
        Type::TypeType { item, is_type_form } => {
            let new_item = replace_meta_vars_inner(item, target)?;
            Some(Type::TypeType {
                item: Box::new(new_item),
                is_type_form: *is_type_form,
            })
        }
        Type::LiteralType { fallback, value } => {
            let new_fallback = replace_meta_vars_inner(fallback, target)?;
            Some(Type::LiteralType {
                fallback: Box::new(new_fallback),
                value: value.clone(),
            })
        }
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let new_fallback = replace_meta_vars_inner(fallback, target)?;
            let mut new_items = Vec::with_capacity(items.len());
            for (key, val) in items {
                new_items.push((key.clone(), replace_meta_vars_inner(val, target)?));
            }
            Some(Type::TypedDictType {
                fallback: Box::new(new_fallback),
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(replace_meta_vars_inner(item, target)?);
            }
            Some(Type::Overloaded { items: new_items })
        }
        Type::UnpackType { typ: inner } => {
            let new_inner = replace_meta_vars_inner(inner, target)?;
            Some(Type::UnpackType {
                typ: Box::new(new_inner),
            })
        }
        Type::Parameters(params) => {
            let new_params = replace_meta_vars_parameters(params, target)?;
            Some(Type::Parameters(new_params))
        }
        // Leaf types: return as-is.
        Type::AnyType { .. }
        | Type::NoneType
        | Type::UninhabitedType
        | Type::DeletedType { .. } => Some(typ.clone()),
        // Deferred.
        Type::TypeAliasType { .. } => None,
        Type::UnboundType { .. } => None,
        // TypeVar-like variants are handled in replace_meta_vars_inner
        // before dispatching to this function. Return None as a safety net.
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            None
        }
    }
}

/// Normalize `UnpackType(Instance(builtins.tuple, [X]))` to that Instance.
/// Mirrors the normalization in TypeVarEraser.visit_instance (erasetype.py:241-247)
/// and visit_tuple_type (erasetype.py:260-271).
fn normalize_tuple_unpack(t: &Type) -> Option<Type> {
    if let Type::UnpackType { typ: inner } = t {
        if let Type::Instance { type_ref, args, .. } = inner.as_ref() {
            if type_ref == "builtins.tuple" && args.len() == 1 {
                return Some(inner.as_ref().clone());
            }
        }
    }
    None
}

/// Check if a type is `Instance(builtins.tuple, [])`.
fn is_builtins_tuple(t: &Type) -> bool {
    matches!(t, Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" && args.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_typevar(raw_id: i64, ns: &str) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: ns.to_string(),
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

    #[test]
    fn test_erase_all_typevars() {
        let t = make_typevar(1, "ns");
        let result = erase_typevars_inner(&t, None, &make_any());
        assert!(matches!(result, Some(Type::AnyType { .. })));
    }

    #[test]
    fn test_erase_matching_id() {
        let t = make_typevar(1, "ns");
        let mut ids = HashSet::new();
        ids.insert((1, "ns".to_string()));
        let result = erase_typevars_inner(&t, Some(&ids), &make_any());
        assert!(matches!(result, Some(Type::AnyType { .. })));
    }

    #[test]
    fn test_erase_non_matching_id() {
        let t = make_typevar(1, "ns");
        let mut ids = HashSet::new();
        ids.insert((2, "ns".to_string()));
        let result = erase_typevars_inner(&t, Some(&ids), &make_any());
        assert!(matches!(result, Some(Type::TypeVarType { .. })));
    }

    #[test]
    fn test_erase_in_instance_args() {
        let tvar = make_typevar(1, "ns");
        let t = make_instance("builtins.list", vec![tvar]);
        let result = erase_typevars_inner(&t, None, &make_any()).unwrap();
        match result {
            Type::Instance { args, .. } => {
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], Type::AnyType { .. }));
            }
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn test_erase_in_union() {
        let tvar = make_typevar(1, "ns");
        let t = Type::UnionType {
            items: vec![tvar, make_instance("builtins.int", vec![])],
            uses_pep604_syntax: true,
        };
        let result = erase_typevars_inner(&t, None, &make_any()).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::AnyType { .. }));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_replace_meta_vars() {
        let t = make_typevar(-1, "ns");
        let target = make_instance("builtins.int", vec![]);
        let result = replace_meta_vars_inner(&t, &target);
        assert!(
            matches!(result, Some(Type::Instance { ref type_ref, .. }) if type_ref == "builtins.int")
        );
    }

    #[test]
    fn test_replace_meta_vars_skips_non_meta() {
        let t = make_typevar(1, "ns");
        let target = make_instance("builtins.int", vec![]);
        let result = replace_meta_vars_inner(&t, &target);
        assert!(matches!(result, Some(Type::TypeVarType { .. })));
    }

    #[test]
    fn test_is_meta_var() {
        assert!(is_meta_var(-1));
        assert!(is_meta_var(-100));
        assert!(!is_meta_var(0));
        assert!(!is_meta_var(1));
    }

    #[test]
    fn test_leaf_types_unchanged() {
        let any = make_any();
        let result = erase_typevars_inner(&any, None, &make_any());
        assert!(matches!(result, Some(Type::AnyType { .. })));
        let none = Type::NoneType;
        let result = erase_typevars_inner(&none, None, &make_any());
        assert!(matches!(result, Some(Type::NoneType)));
    }
}
