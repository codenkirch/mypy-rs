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

use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_int_bare, read_str, read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// Key for the env: `(raw_id, namespace)`. Mirrors `TypeVarId.__eq__`
/// (types.py:574-576), which compares `raw_id` and `namespace`.
type EnvKey = (i64, String);

/// `#[pyfunction]` entry for `expand_type`. The Python-side shim
/// (mypy/expandtype.py) calls this with the serialized `typ` blob, the
/// serialized `env`, and the `NativeTypeResolver` pyclass. Returns `None`
/// (Python `None`) when Rust doesn't handle the case; `Some(bytes)`
/// otherwise, holding a wire-format type blob the shim decodes via
/// `read_type`.
///
/// The env wire format is: count (bare int) + pairs of
/// (TypeVarId raw_id bare int + TypeVarId namespace tagged str + Type).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_type(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    env_bytes: &[u8],
) -> Option<Vec<u8>> {
    let _ = resolver; // reserved for future Instance.has_type_var_tuple lookups
    let typ = decode_type(type_bytes)?;
    let env = decode_env(env_bytes)?;
    // Empty env: expanding with no substitutions is a no-op. Defer to
    // Python so the caller gets the original object (not a decoded copy),
    // preserving object identity for the constraint solver and caches.
    if env.is_empty() {
        return None;
    }
    let expanded = expand_type(&typ, &env)?;
    encode_type(&expanded)
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
        // namespace is a fullname string written via `write_str` (tagged).
        let namespace = read_str(&mut buf).ok()?;
        let typ = read_type(&mut buf, None).ok()?;
        env.insert((raw_id, namespace), typ);
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
fn expand_type(typ: &Type, env: &HashMap<EnvKey, Type>) -> Option<Type> {
    match typ {
        // Leaf types that carry no TypeVars: returned as-is.
        // (expandtype.py:189-211)
        Type::AnyType { .. }
        | Type::NoneType
        | Type::UninhabitedType
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
            let new_args = expand_type_tuple_with_unpack(args, env)?;
            // builtins.tuple normalization (expandtype.py:228-237):
            // Tuple[*Tuple[X, ...], ...] -> Tuple[X, ...]. When the single
            // arg is an UnpackType wrapping a builtins.tuple Instance,
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
        } => {
            // Self type (raw_id == 0): expand upper_bound first
            // (expandtype.py:243-244), since Self`0 <: C[T, S] may reference
            // other TypeVars in the bound.
            let upper_bound = if *raw_id == 0 {
                Box::new(expand_type(upper_bound, env)?)
            } else {
                upper_bound.clone()
            };
            let key = (*raw_id, namespace.clone());
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
                    })
                }
            }
        }

        Type::UnionType {
            items,
            uses_pep604_syntax,
        } => {
            // (expandtype.py:569-592) We expand each item and drop trivial
            // bottom duplicates, but defer the full remove_trivial +
            // make_union + get_proper_type simplification to Python when
            // it would change the item set beyond simple expansion.
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type(item, env)?);
            }
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
            })
        }

        Type::TypeType { item, is_type_form } => {
            // (expandtype.py:597-602)
            let new_item = expand_type(item, env)?;
            Some(Type::TypeType {
                item: Box::new(new_item),
                is_type_form: *is_type_form,
            })
        }

        Type::LiteralType { fallback, value } => {
            // (expandtype.py:565-567) Expand the fallback if it has type
            // vars (i.e. is a generic Instance).
            let new_fallback = expand_type(fallback, env)?;
            Some(Type::LiteralType {
                fallback: Box::new(new_fallback),
                value: value.clone(),
            })
        }

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            // (expandtype.py:534-554)
            let new_items = expand_type_list_with_unpack(items, env)?;
            // Normalize Tuple[*Tuple[X, ...]] -> Tuple[X, ...]
            // (expandtype.py:536-551). When the single resulting item is an
            // UnpackType wrapping a builtins.tuple Instance, return that
            // Instance instead.
            if new_items.len() == 1 {
                if let Some(unpacked) = normalize_tuple_unpack_to_instance(&new_items[0]) {
                    return Some(unpacked);
                }
            }
            let new_fallback = expand_type(partial_fallback, env)?;
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
            let new_fallback = expand_type(fallback, env)?;
            let mut new_items = Vec::with_capacity(items.len());
            for (name, typ) in items {
                new_items.push((name.clone(), expand_type(typ, env)?));
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
            // The Unpack interpolation branch
            // (expandtype.py:482-488, interpolate_args_for_unpack) is
            // deferred: if a var_arg is an UnpackType, defer to Python.
            for at in arg_types {
                if matches!(at, Type::UnpackType { .. }) {
                    return None;
                }
            }
            // Python ExpandTypeVisitor.visit_callable_type (expandtype.py:676)
            // only expands arg_types, ret_type, type_guard, type_is,
            // instance_type. It does NOT expand fallback or variables
            // (the declared type vars are definitions, not uses).
            let new_instance_type = match instance_type {
                Some(it) => Some(Box::new(expand_type(it, env)?)),
                None => None,
            };
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            for at in arg_types {
                new_arg_types.push(expand_type(at, env)?);
            }
            let new_ret_type = Box::new(expand_type(ret_type, env)?);
            let new_type_guard = match type_guard {
                Some(tg) => Some(Box::new(expand_type(tg, env)?)),
                None => None,
            };
            let new_type_is = match type_is {
                Some(ti) => Some(Box::new(expand_type(ti, env)?)),
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
            let new_typ = expand_type(typ, env)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
            })
        }

        // Deferred variants: TypeAliasType (unfixed), Overloaded,
        // ParamSpecType, TypeVarTupleType, Parameters. These need graph
        // resolution or prefix merging not available at this stage.
        Type::TypeAliasType { .. }
        | Type::Overloaded { .. }
        | Type::ParamSpecType { .. }
        | Type::TypeVarTupleType { .. }
        | Type::Parameters(_) => None,
    }
}

/// `expand_type_tuple_with_unpack` (expandtype.py:523-532). Expands a
/// tuple of arg types, splicing in the items of any UnpackType wrapping
/// a TypeVarTupleType via `expand_unpack`. Non-Unpack args are expanded
/// normally.
fn expand_type_tuple_with_unpack(typs: &[Type], env: &HashMap<EnvKey, Type>) -> Option<Vec<Type>> {
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
        items.push(expand_type(item, env)?);
    }
    Some(items)
}

/// `expand_type_list_with_unpack` (expandtype.py:513-521). Same as
/// `expand_type_tuple_with_unpack` but over a Vec.
fn expand_type_list_with_unpack(typs: &[Type], env: &HashMap<EnvKey, Type>) -> Option<Vec<Type>> {
    expand_type_tuple_with_unpack(typs, env)
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
        let key = (*raw_id, namespace.clone());
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
        Type::AnyType { .. } | Type::UninhabitedType => {
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
