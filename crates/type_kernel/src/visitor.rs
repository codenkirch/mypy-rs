//! Native port of `mypy/type_visitor.py` and the standalone type-helper
//! functions from `mypy/types.py` and `mypy/copytype.py` (Stage 7).
//!
//! This module provides pure functions and visitor traits that operate on
//! the wire-format `Type` enum. The Python-side shims (mypy/types.py,
//! mypy/copytype.py) call these via `#[pyfunction]` and fall through to
//! the Python implementation when the Rust subset returns `None` (the
//! strangler-fig per-call contract).
//!
//! Deferred (return None) cases:
//!   * `TypeAliasType` — Python's visitors call `get_proper_type` to
//!     expand the alias, which needs the live alias target. The wire
//!     `TypeAliasType` carries only `type_ref: String` (the alias
//!     fullname), not the resolved target, so we cannot expand.
//!   * `is_recursive` — `has_recursive_types` needs `t.is_recursive`
//!     from the live `TypeAlias` node. The wire format has no such
//!     field, so we defer (return false) for TypeAliasType.
//!   * `is_named_instance` — needs `get_proper_type` to expand alias.
//!     NOT portable without alias resolution.

use pyo3::prelude::*;

use crate::wire::{read_type, write_type, LiteralValue, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// TypeOfAny values (mirrors mypy.types.TypeOfAny)
// ---------------------------------------------------------------------------

/// `TypeOfAny.unannotated` — inferred without a type annotation.
const TYPE_OF_ANY_UNANNOTATED: i64 = 1;

// ---------------------------------------------------------------------------
// ArgKind values (mirrors mypy.nodes.ArgKind)
// ---------------------------------------------------------------------------

/// `ArgKind.ARG_POS` = 0. `ArgKind.ARG_OPT` = 1. `ArgKind.ARG_STAR` = 2.
/// `ArgKind.ARG_NAMED` = 3. `ArgKind.ARG_STAR2` = 4. `ArgKind.ARG_NAMED_OPT` = 5.
#[allow(dead_code)]
const ARG_POS: i64 = 0;
#[allow(dead_code)]
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
#[allow(dead_code)]
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
#[allow(dead_code)]
const ARG_NAMED_OPT: i64 = 5;

// ---------------------------------------------------------------------------
// Wire format helpers (shared decode/encode)
// ---------------------------------------------------------------------------

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
// has_type_vars: BoolTypeQuery (ANY_STRATEGY), skips alias target
// ---------------------------------------------------------------------------

/// `mypy.types.has_type_vars` — check if a type contains any type variable
/// (TypeVarType, ParamSpecType, TypeVarTupleType) recursively.
///
/// Mirrors `BoolTypeQuery` with `ANY_STRATEGY` and `skip_alias_target=True`
/// (types.py:4205-4207). The wire format has no alias target, so the
/// skip-alias-target behavior is the natural default: we never recurse
/// into TypeAliasType, which is correct because the alias target isn't
/// available.
#[pyfunction]
pub(crate) fn rust_has_type_vars(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(has_type_vars_inner(&typ))
}

pub(crate) fn has_type_vars_inner(typ: &Type) -> bool {
    match typ {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            true
        }
        Type::UnboundType { args, .. } => args.iter().any(has_type_vars_inner),
        Type::UnpackType { typ } => has_type_vars_inner(typ),
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            args.iter().any(has_type_vars_inner)
                || last_known_value
                    .as_ref()
                    .is_some_and(|t| has_type_vars_inner(t))
        }
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            ..
        } => {
            arg_types.iter().any(has_type_vars_inner)
                || has_type_vars_inner(ret_type)
                || variables.iter().any(has_type_vars_inner)
                || instance_type
                    .as_ref()
                    .is_some_and(|t| has_type_vars_inner(t))
        }
        Type::Overloaded { items } => items.iter().any(has_type_vars_inner),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => items.iter().any(has_type_vars_inner) || has_type_vars_inner(partial_fallback),
        Type::TypedDictType {
            items, fallback, ..
        } => items.iter().any(|(_, t)| has_type_vars_inner(t)) || has_type_vars_inner(fallback),
        Type::LiteralType { fallback, .. } => has_type_vars_inner(fallback),
        Type::UnionType { items, .. } => items.iter().any(has_type_vars_inner),
        Type::TypeType { item, .. } => has_type_vars_inner(item),
        Type::AnyType { source_any, .. } => {
            source_any.as_ref().is_some_and(|t| has_type_vars_inner(t))
        }
        Type::TypeAliasType { args, .. } => {
            // skip_alias_target: do not recurse into the alias target.
            // The wire format has no target, so this is the only correct
            // behavior. Recurse into args only.
            args.iter().any(has_type_vars_inner)
        }
        // Leaves: NoneType, UninhabitedType, ErasedType, DeletedType,
        // Parameters. None contain type variables.
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// has_recursive_types: defers (returns None) because wire format lacks
// `is_recursive` on TypeAliasType
// ---------------------------------------------------------------------------

/// `mypy.types.has_recursive_types` — check if a type contains any
/// recursive type aliases.
///
/// Returns `None` for `TypeAliasType` because the wire format doesn't
/// carry the `is_recursive` field (types.py:388-403 derives it from
/// the live `TypeAlias` node). For all other variants, delegates to
/// `has_type_vars`-style recursion with `ANY_STRATEGY` over args.
#[pyfunction]
pub(crate) fn rust_has_recursive_types(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_recursive_types_inner(&typ))
}

pub(crate) fn has_recursive_types_inner(typ: &Type) -> Option<bool> {
    // Defer: no is_recursive field on the wire TypeAliasType.
    if matches!(typ, Type::TypeAliasType { .. }) {
        return None;
    }
    // ANY_STRATEGY over children: if any child returns Some(true), result
    // is Some(true). If any child returns None, result is None. Otherwise
    // Some(false).
    let mut result = false;
    for child in children(typ) {
        match has_recursive_types_inner(child) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    let _ = &mut result;
    Some(false)
}

/// Yield the direct child types of `typ` (for ANY_STRATEGY / ALL_STRATEGY
/// traversal). Mirrors the `query_types` calls in `BoolTypeQuery.visit_*`.
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
        // TypeAliasType is handled by the caller (deferred). Parameters,
        // NoneType, UninhabitedType, ErasedType, DeletedType: no children.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// is_literal_type
// ---------------------------------------------------------------------------

/// `mypy.types.is_literal_type` — check if a type is a `LiteralType` with
/// the given fallback fullname and value.
///
/// Mirrors `is_literal_type` (types.py:4353-4360). The `value` argument
/// is encoded as a string tag + payload: `"int:N"`, `"str:S"`, `"bytes:B"`,
/// `"bool:T|F"`, `"float:F"`. The shim translates the Python value to
/// this encoding before calling.
///
/// Returns `false` for non-literal types (Instance with last_known_value
/// is unwrapped to its LiteralType; otherwise no match).
#[pyfunction]
pub(crate) fn rust_is_literal_type(
    type_bytes: &[u8],
    fallback_fullname: &str,
    value_kind: &str,
    value_payload: &str,
) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(is_literal_type_inner(
        &typ,
        fallback_fullname,
        value_kind,
        value_payload,
    ))
}

pub(crate) fn is_literal_type_inner(
    typ: &Type,
    fallback_fullname: &str,
    value_kind: &str,
    value_payload: &str,
) -> bool {
    // Unwrap Instance with last_known_value to its LiteralType, mirroring
    // types.py:4356-4357.
    let typ = if let Type::Instance {
        last_known_value: Some(lkv),
        ..
    } = typ
    {
        lkv.as_ref()
    } else {
        typ
    };
    if let Type::LiteralType { fallback, value } = typ {
        if let Type::Instance { type_ref, .. } = fallback.as_ref() {
            if type_ref != fallback_fullname {
                return false;
            }
        } else {
            return false;
        }
        literal_value_matches(value, value_kind, value_payload)
    } else {
        false
    }
}

fn literal_value_matches(value: &LiteralValue, kind: &str, payload: &str) -> bool {
    match (kind, value) {
        ("int", LiteralValue::Int(v)) => v.to_string() == payload,
        ("str", LiteralValue::Str(s)) => s == payload,
        ("bytes", LiteralValue::Bytes(b)) => {
            // Encode bytes as latin-1 string for transport.
            b.iter().map(|&x| x as char).collect::<String>() == payload
        }
        ("bool", LiteralValue::Bool(b)) => (if *b { "T" } else { "F" }) == payload,
        ("float", LiteralValue::Float(f)) => f.to_string() == payload,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// is_unannotated_any
// ---------------------------------------------------------------------------

/// `mypy.types.is_unannotated_any` — check if a type represents an
/// implicit (unannotated) Any.
///
/// Mirrors `is_unannotated_any` (types.py:4365-4372). The wire format
/// `TypeAliasType` can't be expanded (no target), so we return false
/// for aliases, which matches the Python behavior when `t` is already
/// a `ProperType` (i.e. not an alias).
#[pyfunction]
pub(crate) fn rust_is_unannotated_any(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(is_unannotated_any_inner(&typ))
}

pub(crate) fn is_unannotated_any_inner(typ: &Type) -> bool {
    if let Type::AnyType { type_of_any, .. } = typ {
        *type_of_any == TYPE_OF_ANY_UNANNOTATED
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// remove_dups: generic dedup preserving order
// ---------------------------------------------------------------------------

/// `mypy.types.remove_dups` — remove duplicates from a list, preserving
/// order of first appearance. Type has `PartialEq` (no `Hash`), so this
/// is O(n*m) where n = items, m = unique items seen.
///
/// Returns the deduped list as wire-format type bytes. The shim decodes
/// back to Python list.
#[pyfunction]
pub(crate) fn rust_remove_dups(type_bytes_list: Vec<Vec<u8>>) -> PyResult<Vec<Vec<u8>>> {
    let mut types: Vec<Type> = Vec::with_capacity(type_bytes_list.len());
    for b in &type_bytes_list {
        if let Some(t) = decode_type(b) {
            types.push(t);
        }
    }
    let deduped = remove_dups_inner(&types);
    Ok(deduped.iter().filter_map(encode_type).collect())
}

pub(crate) fn remove_dups_inner(types: &[Type]) -> Vec<Type> {
    let mut seen: Vec<Type> = Vec::new();
    for t in types {
        if !seen.contains(t) {
            seen.push(t.clone());
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// type_vars_as_args
// ---------------------------------------------------------------------------

/// `mypy.types.type_vars_as_args` — represent type variables as they
/// would appear in a type argument list. Wraps `TypeVarTupleType` in
/// `UnpackType`; other variants pass through.
///
/// Mirrors `type_vars_as_args` (types.py:4409-4418). The input is a
/// list of serialized type variables; the output is a list of
/// serialized types.
#[pyfunction]
pub(crate) fn rust_type_vars_as_args(type_bytes_list: Vec<Vec<u8>>) -> PyResult<Vec<Vec<u8>>> {
    let mut types: Vec<Type> = Vec::with_capacity(type_bytes_list.len());
    for b in &type_bytes_list {
        if let Some(t) = decode_type(b) {
            types.push(t);
        }
    }
    let result = type_vars_as_args_inner(&types);
    Ok(result.iter().filter_map(encode_type).collect())
}

pub(crate) fn type_vars_as_args_inner(type_vars: &[Type]) -> Vec<Type> {
    type_vars
        .iter()
        .map(|tv| match tv {
            Type::TypeVarTupleType { .. } => Type::UnpackType {
                typ: Box::new(tv.clone()),
            },
            _ => tv.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// callable_with_ellipsis
// ---------------------------------------------------------------------------

/// `mypy.types.callable_with_ellipsis` — construct type
/// `Callable[..., ret_type]`.
///
/// Mirrors `callable_with_ellipsis` (types.py:4384-4395). The `any_type`
/// is a serialized AnyType (typically `AnyType(special_form)`); the
/// `ret_type` is a serialized type; the `fallback` is a serialized
/// Instance used as the CallableType's fallback.
///
/// Returns the serialized CallableType, or `None` if the inputs can't
/// be decoded.
#[pyfunction]
pub(crate) fn rust_callable_with_ellipsis(
    any_bytes: &[u8],
    ret_bytes: &[u8],
    fallback_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let any_type = match decode_type(any_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let ret_type = match decode_type(ret_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let fallback = match decode_type(fallback_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = callable_with_ellipsis_inner(&any_type, &ret_type, &fallback);
    Ok(encode_type(&result))
}

pub(crate) fn callable_with_ellipsis_inner(
    any_type: &Type,
    ret_type: &Type,
    fallback: &Type,
) -> Type {
    Type::CallableType {
        fallback: Box::new(fallback.clone()),
        instance_type: None,
        is_ellipsis_args: true,
        implicit: false,
        is_bound: false,
        from_concatenate: false,
        imprecise_arg_kinds: false,
        unpack_kwargs: false,
        arg_types: vec![any_type.clone(), any_type.clone()],
        arg_kinds: vec![ARG_STAR, ARG_STAR2],
        arg_names: vec![None, None],
        ret_type: Box::new(ret_type.clone()),
        name: None,
        variables: Vec::new(),
        type_guard: None,
        type_is: None,
    }
}

// ---------------------------------------------------------------------------
// find_unpack_in_list
// ---------------------------------------------------------------------------

/// `mypy.types.find_unpack_in_list` — find the (single) UnpackType in
/// a list, asserting uniqueness.
///
/// Mirrors `find_unpack_in_list` (types.py:4307-4322). Returns the
/// 0-based index, or None if no UnpackType is present. The Python
/// version asserts uniqueness (raises if two are found); we silently
/// return the first and rely on the earlier semanal pass to flag
/// duplicates.
///
/// Returns `-1` encoded as a None-via-Option: the shim decodes `-1` as
/// "not found", any other value as the index.
#[pyfunction]
pub(crate) fn rust_find_unpack_in_list(type_bytes_list: Vec<Vec<u8>>) -> PyResult<i64> {
    let types: Vec<Type> = type_bytes_list
        .iter()
        .filter_map(|b| decode_type(b))
        .collect();
    Ok(find_unpack_in_list_inner(&types))
}

pub(crate) fn find_unpack_in_list_inner(types: &[Type]) -> i64 {
    for (i, t) in types.iter().enumerate() {
        if matches!(t, Type::UnpackType { .. }) {
            return i as i64;
        }
    }
    -1
}

// ---------------------------------------------------------------------------
// split_with_prefix_and_suffix / extend_args_for_prefix_and_suffix
// ---------------------------------------------------------------------------

/// `mypy.types.split_with_prefix_and_suffix` — split a tuple type list
/// around a variadic unpack into (head, middle, tail).
///
/// Mirrors `split_with_prefix_and_suffix` (types.py:4228-4238). Returns
/// three Vec<Type> as wire-format bytes lists.
///
/// The input must be a list of serialized types; the output is three
/// lists. If the input length is <= prefix + suffix, we delegate to
/// `extend_args_for_prefix_and_suffix` first.
#[pyfunction]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_split_with_prefix_and_suffix(
    type_bytes_list: Vec<Vec<u8>>,
    prefix: usize,
    suffix: usize,
) -> PyResult<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>)> {
    let mut types = Vec::with_capacity(type_bytes_list.len());
    for b in &type_bytes_list {
        if let Some(t) = decode_type(b) {
            types.push(t);
        }
    }
    let (head, mid, tail) = split_with_prefix_and_suffix_inner(&types, prefix, suffix);
    Ok((
        encode_type_list(&head),
        encode_type_list(&mid),
        encode_type_list(&tail),
    ))
}

pub(crate) fn split_with_prefix_and_suffix_inner(
    types: &[Type],
    prefix: usize,
    suffix: usize,
) -> (Vec<Type>, Vec<Type>, Vec<Type>) {
    let mut types: Vec<Type> = types.to_vec();
    if types.len() <= prefix + suffix {
        types = extend_args_for_prefix_and_suffix_inner(types, prefix, suffix);
    }
    if suffix > 0 {
        let mid_len = types.len() - prefix - suffix;
        let mid: Vec<Type> = types[prefix..prefix + mid_len].to_vec();
        let tail = types[prefix + mid_len..].to_vec();
        (types[..prefix].to_vec(), mid, tail)
    } else {
        (
            types[..prefix].to_vec(),
            types[prefix..].to_vec(),
            Vec::new(),
        )
    }
}

fn encode_type_list(types: &[Type]) -> Vec<Vec<u8>> {
    types.iter().filter_map(encode_type).collect()
}

/// `mypy.types.extend_args_for_prefix_and_suffix` — extend a list of
/// types by duplicating from a variadic tuple to satisfy prefix/suffix.
pub(crate) fn extend_args_for_prefix_and_suffix_inner(
    types: Vec<Type>,
    prefix: usize,
    suffix: usize,
) -> Vec<Type> {
    // Find the variadic unpack position and item type.
    let mut idx: Option<usize> = None;
    let mut item: Option<Type> = None;
    for (i, t) in types.iter().enumerate() {
        if let Type::UnpackType { typ } = t {
            if let Type::Instance { type_ref, args, .. } = typ.as_ref() {
                if type_ref == "builtins.tuple" && !args.is_empty() {
                    item = Some(args[0].clone());
                    idx = Some(i);
                    break;
                }
            }
        }
    }
    let (idx, item) = match (idx, item) {
        (Some(i), Some(it)) => (i, it),
        _ => return types,
    };
    let start: Vec<Type> = if idx < prefix {
        vec![item.clone(); prefix - idx]
    } else {
        Vec::new()
    };
    let end: Vec<Type> = if types.len() - idx - 1 < suffix {
        vec![item.clone(); suffix - (types.len() - idx - 1)]
    } else {
        Vec::new()
    };
    let mut out: Vec<Type> = Vec::with_capacity(types.len() + start.len() + end.len());
    out.extend(types[..idx].iter().cloned());
    out.extend(start);
    out.push(types[idx].clone());
    out.extend(end);
    out.extend(types[idx + 1..].iter().cloned());
    out
}

// ---------------------------------------------------------------------------
// flatten_nested_unions
// ---------------------------------------------------------------------------

/// `mypy.types.flatten_nested_unions` — flatten nested unions in a type
/// list. Defers (returns None) on `TypeAliasType` since `get_proper_type`
/// needs the live alias target.
///
/// Mirrors `flatten_nested_unions` (types.py:4267-4293). The wire format
/// has no alias target, so we can only flatten when no TypeAliasType is
/// present in the list (or its transitive closure).
#[pyfunction]
pub(crate) fn rust_flatten_nested_unions(
    type_bytes_list: Vec<Vec<u8>>,
    handle_type_alias_type: bool,
    handle_recursive: bool,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let mut types: Vec<Type> = Vec::with_capacity(type_bytes_list.len());
    for b in &type_bytes_list {
        if let Some(t) = decode_type(b) {
            types.push(t);
        }
    }
    // Fast path: nothing to flatten if no TypeAliasType or UnionType.
    if !types
        .iter()
        .any(|t| matches!(t, Type::TypeAliasType { .. } | Type::UnionType { .. }))
    {
        return Ok(Some(encode_type_list(&types)));
    }
    let flat = match flatten_nested_unions_inner(&types, handle_type_alias_type, handle_recursive) {
        Some(f) => f,
        None => return Ok(None),
    };
    Ok(Some(encode_type_list(&flat)))
}

pub(crate) fn flatten_nested_unions_inner(
    types: &[Type],
    handle_type_alias_type: bool,
    handle_recursive: bool,
) -> Option<Vec<Type>> {
    let mut flat_items: Vec<Type> = Vec::with_capacity(types.len());
    for t in types {
        let tp: Type = if handle_type_alias_type {
            if let Type::TypeAliasType { .. } = t {
                if !handle_recursive {
                    t.clone()
                } else {
                    // Defer: wire format has no expanded alias target.
                    return None;
                }
            } else {
                t.clone()
            }
        } else {
            t.clone()
        };
        if matches!(tp, Type::UnionType { .. }) {
            // Recurse into UnionType items.
            if let Type::UnionType { items, .. } = &tp {
                let inner =
                    flatten_nested_unions_inner(items, handle_type_alias_type, handle_recursive)?;
                flat_items.extend(inner);
            } else {
                unreachable!();
            }
        } else {
            flat_items.push(t.clone());
        }
    }
    Some(flat_items)
}

// ---------------------------------------------------------------------------
// flatten_nested_tuples
// ---------------------------------------------------------------------------

/// `mypy.types.flatten_nested_tuples` — recursively flatten TupleTypes
/// nested with Unpack. Defers (returns None) on `TypeAliasType`.
///
/// Mirrors `flatten_nested_tuples` (types.py:4326-4360).
#[pyfunction]
pub(crate) fn rust_flatten_nested_tuples(
    type_bytes_list: Vec<Vec<u8>>,
    handle_recursive: bool,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let mut types: Vec<Type> = Vec::with_capacity(type_bytes_list.len());
    for b in &type_bytes_list {
        if let Some(t) = decode_type(b) {
            types.push(t);
        }
    }
    let flat = match flatten_nested_tuples_inner(&types, handle_recursive) {
        Some(f) => f,
        None => return Ok(None),
    };
    Ok(Some(encode_type_list(&flat)))
}

pub(crate) fn flatten_nested_tuples_inner(
    types: &[Type],
    handle_recursive: bool,
) -> Option<Vec<Type>> {
    let mut res: Vec<Type> = Vec::with_capacity(types.len());
    for typ in types {
        if let Type::UnpackType { typ: inner } = typ {
            // Defer if the unpacked type is a TypeAliasType (no target).
            if let Type::TypeAliasType { .. } = inner.as_ref() {
                if !handle_recursive {
                    res.push(typ.clone());
                    continue;
                }
                return None;
            }
            if let Type::TupleType { items, .. } = inner.as_ref() {
                res.extend(flatten_nested_tuples_inner(items, handle_recursive)?);
                continue;
            }
        }
        res.push(typ.clone());
    }
    Some(res)
}

// ---------------------------------------------------------------------------
// copy_type: trivial — wire Type is Clone
// ---------------------------------------------------------------------------

/// `mypy.copytype.copy_type` — create a shallow copy of a type.
///
/// Mirrors `copy_type` (copytype.py:34-37) + `TypeShallowCopier`
/// (copytype.py:45-138). The wire `Type` enum is `Clone`, and there's
/// no truthiness flag model on the wire, so a shallow copy is just
/// `clone()`. The Python shim calls this only on `ProperType` (never
/// `TypeAliasType`); the wire `TypeAliasType` would be a no-op clone
/// anyway since its fields are the unresolved `type_ref` and `args`.
///
/// Included for API parity and as the foundation for future ports that
/// mutate the copy (e.g. truthiness flag re-application).
#[pyfunction]
pub(crate) fn rust_copy_type(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = copy_type_inner(&typ);
    Ok(encode_type(&result))
}

pub(crate) fn copy_type_inner(typ: &Type) -> Type {
    typ.clone()
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

    fn make_typevar(raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
        }
    }

    fn make_unannotated_any() -> Type {
        Type::AnyType {
            type_of_any: TYPE_OF_ANY_UNANNOTATED,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_explicit_any() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
        }
    }

    #[test]
    fn test_has_type_vars_true() {
        let t = make_typevar(1);
        assert!(has_type_vars_inner(&t));
    }

    #[test]
    fn test_has_type_vars_in_instance_args() {
        let tv = make_typevar(1);
        let inst = make_instance("Foo", vec![tv]);
        assert!(has_type_vars_inner(&inst));
    }

    #[test]
    fn test_has_type_vars_false_simple() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(!has_type_vars_inner(&inst));
    }

    #[test]
    fn test_has_type_vars_false_union() {
        let u = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        assert!(!has_type_vars_inner(&u));
    }

    #[test]
    fn test_has_type_vars_true_union() {
        let u = make_union(vec![make_instance("builtins.int", vec![]), make_typevar(1)]);
        assert!(has_type_vars_inner(&u));
    }

    #[test]
    fn test_has_recursive_types_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(has_recursive_types_inner(&alias), None);
    }

    #[test]
    fn test_has_recursive_types_false_simple() {
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(has_recursive_types_inner(&inst), Some(false));
    }

    #[test]
    fn test_is_unannotated_any_true() {
        assert!(is_unannotated_any_inner(&make_unannotated_any()));
    }

    #[test]
    fn test_is_unannotated_any_false_explicit() {
        assert!(!is_unannotated_any_inner(&make_explicit_any()));
    }

    #[test]
    fn test_is_unannotated_any_false_non_any() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(!is_unannotated_any_inner(&inst));
    }

    #[test]
    fn test_remove_dups_preserves_order() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let c = make_instance("C", vec![]);
        let input = vec![a.clone(), b.clone(), a.clone(), c.clone(), b.clone()];
        let result = remove_dups_inner(&input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], a);
        assert_eq!(result[1], b);
        assert_eq!(result[2], c);
    }

    #[test]
    fn test_remove_dups_single() {
        let a = make_instance("A", vec![]);
        let result = remove_dups_inner(&[a.clone()]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_remove_dups_empty() {
        let result = remove_dups_inner(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_type_vars_as_args_wraps_tuple() {
        let tvt = Type::TypeVarTupleType {
            tuple_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "Ts".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            min_len: 0,
        };
        let result = type_vars_as_args_inner(&[tvt]);
        assert!(matches!(result[0], Type::UnpackType { .. }));
    }

    #[test]
    fn test_type_vars_as_args_passthrough() {
        let tv = make_typevar(1);
        let result = type_vars_as_args_inner(&[tv.clone()]);
        assert!(matches!(result[0], Type::TypeVarType { .. }));
    }

    #[test]
    fn test_find_unpack_in_list_found() {
        let a = make_instance("A", vec![]);
        let unpack = Type::UnpackType {
            typ: Box::new(make_instance("builtins.tuple", vec![])),
        };
        let b = make_instance("B", vec![]);
        let result = find_unpack_in_list_inner(&[a, unpack, b]);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_find_unpack_in_list_not_found() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let result = find_unpack_in_list_inner(&[a, b]);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_flatten_nested_unions_simple() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let inner = make_union(vec![a.clone(), b.clone()]);
        let outer = make_union(vec![inner, a.clone()]);
        let result = flatten_nested_unions_inner(&[outer], true, true);
        assert!(result.is_some());
        let flat = result.unwrap();
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_flatten_nested_unions_no_union() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let result = flatten_nested_unions_inner(&[a.clone(), b.clone()], true, true);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_flatten_nested_unions_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let result = flatten_nested_unions_inner(&[alias], true, true);
        assert!(result.is_none());
    }

    #[test]
    fn test_flatten_nested_unions_alias_no_handle() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let result = flatten_nested_unions_inner(&[alias], false, true);
        assert!(result.is_some());
    }

    #[test]
    fn test_copy_type_identity() {
        let inst = make_instance("A", vec![]);
        let result = copy_type_inner(&inst);
        assert_eq!(result, inst);
    }

    #[test]
    fn test_is_literal_type_match() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(42),
        };
        assert!(is_literal_type_inner(&lit, "builtins.int", "int", "42"));
    }

    #[test]
    fn test_is_literal_type_wrong_fallback() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(42),
        };
        assert!(!is_literal_type_inner(&lit, "builtins.str", "int", "42"));
    }

    #[test]
    fn test_is_literal_type_wrong_value() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(42),
        };
        assert!(!is_literal_type_inner(&lit, "builtins.int", "int", "99"));
    }

    #[test]
    fn test_is_literal_type_non_literal() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(!is_literal_type_inner(&inst, "builtins.int", "int", "42"));
    }

    #[test]
    fn test_callable_with_ellipsis_structure() {
        let any = make_explicit_any();
        let ret = make_instance("builtins.int", vec![]);
        let fb = make_instance("builtins.function", vec![]);
        let result = callable_with_ellipsis_inner(&any, &ret, &fb);
        if let Type::CallableType {
            is_ellipsis_args,
            arg_kinds,
            arg_types,
            ..
        } = &result
        {
            assert!(*is_ellipsis_args);
            assert_eq!(*arg_kinds, vec![ARG_STAR, ARG_STAR2]);
            assert_eq!(arg_types.len(), 2);
        } else {
            panic!("expected CallableType");
        }
    }

    #[test]
    fn test_split_with_prefix_and_suffix_simple() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let c = make_instance("C", vec![]);
        let (head, mid, tail) = split_with_prefix_and_suffix_inner(&[a, b, c], 1, 1);
        assert_eq!(head.len(), 1);
        assert_eq!(mid.len(), 1);
        assert_eq!(tail.len(), 1);
    }

    #[test]
    fn test_split_no_suffix() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let c = make_instance("C", vec![]);
        let (head, mid, tail) = split_with_prefix_and_suffix_inner(&[a, b, c], 1, 0);
        assert_eq!(head.len(), 1);
        assert_eq!(mid.len(), 2);
        assert!(tail.is_empty());
    }
}
