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
//!     `has_recursive_types` is exempt: the `is_recursive` flag rides
//!     the wire on every alias node (types.py writer, #1361).
//!   * `is_named_instance` — needs `get_proper_type` to expand alias.
//!     NOT portable without alias resolution.

use pyo3::prelude::*;

use crate::checkexpr_functions::expanded_alias_target;
use crate::typeinfo::NativeTypeResolver;
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
        Type::UnpackType { typ, .. } => has_type_vars_inner(typ),
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
// has_recursive_types: total since issue #1418 (wave 34)

// ---------------------------------------------------------------------------

/// `mypy.types.has_recursive_types` — check if a type contains any
/// recursive type aliases.
///
/// Mirrors `HasRecursiveType` (types.py:5597): a `BoolTypeQuery` with
/// ANY_STRATEGY whose only custom arm is `visit_type_alias_type`
/// (`t.is_recursive or self.query_types(t.args)`). The recursion flag
/// rides the wire on every alias node (types.py writer, #1361), so the
/// port is total over the wire tree: only an undecodable blob defers.
/// The query positions follow the `BoolTypeQuery.visit_*` defaults
/// (type_visitor.py:518-571): `Instance` skips `last_known_value`,
/// `CallableType` skips `variables`, `TypeVarTupleType` skips
/// `tuple_fallback`, `TypedDictType` skips `fallback`, `LiteralType`
/// is a leaf, and `TypeAliasType` never expands the target.
#[pyfunction]
pub(crate) fn rust_has_recursive_types(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(Some(has_recursive_types_inner(&typ)))
}

pub(crate) fn has_recursive_types_inner(typ: &Type) -> bool {
    match typ {
        // visit_type_alias_type override: is_recursive or args.
        Type::TypeAliasType {
            is_recursive, args, ..
        } => *is_recursive || args.iter().any(has_recursive_types_inner),
        // visit_unbound_type: args.
        Type::UnboundType { args, .. } => args.iter().any(has_recursive_types_inner),
        // visit_unpack_type: [type].
        Type::UnpackType { typ, .. } => has_recursive_types_inner(typ),
        // visit_instance: args only.
        Type::Instance { args, .. } => args.iter().any(has_recursive_types_inner),
        // visit_callable_type: arg_types or ret_type or instance_type
        // (the "FIX generics" arm never queries `variables`).
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            arg_types.iter().any(has_recursive_types_inner)
                || has_recursive_types_inner(ret_type)
                || instance_type
                    .as_ref()
                    .is_some_and(|it| has_recursive_types_inner(it))
        }
        // visit_overloaded: items.
        Type::Overloaded { items } => items.iter().any(has_recursive_types_inner),
        // visit_tuple_type: [partial_fallback] + items.
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            has_recursive_types_inner(partial_fallback)
                || items.iter().any(has_recursive_types_inner)
        }
        // visit_typeddict_type: items.values() only (fallback skipped).
        Type::TypedDictType { items, .. } => {
            items.iter().any(|(_, t)| has_recursive_types_inner(t))
        }
        // visit_union_type: items.
        Type::UnionType { items, .. } => items.iter().any(has_recursive_types_inner),
        // visit_type_type: t.item.
        Type::TypeType { item, .. } => has_recursive_types_inner(item),
        // visit_type_var: [upper_bound, default] + values.
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            has_recursive_types_inner(upper_bound)
                || has_recursive_types_inner(default)
                || values.iter().any(has_recursive_types_inner)
        }
        // visit_param_spec: [upper_bound, default, prefix].
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            has_recursive_types_inner(upper_bound)
                || has_recursive_types_inner(default)
                || prefix.arg_types.iter().any(has_recursive_types_inner)
        }
        // visit_type_var_tuple: [upper_bound, default] (tuple_fallback
        // not queried).
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => has_recursive_types_inner(upper_bound) || has_recursive_types_inner(default),
        // visit_parameters: arg_types only (variables not queried).
        Type::Parameters(p) => p.arg_types.iter().any(has_recursive_types_inner),
        // Leaves: Python's BoolTypeQuery returns self.default (False) for
        // these; note it also does NOT visit AnyType.source_any or
        // LiteralType.fallback, so they are intentionally unqueried here.
        Type::AnyType { .. }
        | Type::UninhabitedType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::DeletedType { .. }
        | Type::LiteralType { .. } => false,
    }
}

/// True if `typ` contains any `TypeAliasType` node. On the wire a
/// self-recursive alias is only expressible as a lazy `TypeAliasType` ref,
/// so "contains an alias" is the conservative recursive-shape test used by
/// the constraints cycle guard (issue #1133), where a structural alias
/// check is needed rather than the recursive-flag predicate of
/// `has_recursive_types_inner`.
pub(crate) fn type_contains_alias(typ: &Type) -> bool {
    if matches!(typ, Type::TypeAliasType { .. }) {
        return true;
    }
    let mut kids = children(typ);
    match typ {
        // The wire decodes tvar-family sub-positions as full type blobs, so
        // an alias nested in a bound / values / default / prefix must trip
        // the guard; the generic Boolean-query children() walker skips them.
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            kids.extend(values.iter());
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            kids.extend(prefix.arg_types.iter());
            kids.extend(prefix.variables.iter());
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            kids.push(tuple_fallback);
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::Parameters(p) => {
            kids.extend(p.arg_types.iter());
            kids.extend(p.variables.iter());
        }
        _ => {}
    }
    kids.into_iter().any(type_contains_alias)
}

/// True if `typ` contains any `ErasedType` node. ErasedType is not
/// decodable by the Python `read_type` (tag 122), so any operand or
/// result carrying one cannot cross the wire; callers defer.
pub(crate) fn type_contains_erased(typ: &Type) -> bool {
    if matches!(typ, Type::ErasedType) {
        return true;
    }
    children(typ).into_iter().any(type_contains_erased)
}

/// Over-approximation for the `extra_tvars` attach shapes of
/// `ConstraintBuilderVisitor.visit_callable_type`: Python attaches
/// `extra_tvars` at every `visit_callable_type` whose proper-typed actual
/// declares its own type variables (constraints.py:1712, 1768) while
/// `skip_neg_op` is false and the ambient `type_state.infer_polymorphic`
/// is true (checkexpr.py:1325, true for ordinary checking, which is the
/// common case at the engine's FFI depth). Nested `infer_constraints`
/// recursion re-enters the visitor with `skip_neg_op=False`, so the shape
/// can sit anywhere reachable in the actual tree, not just at its root.
/// `None` when a nested `TypeAliasType` blocks visibility (the
/// proper-typed shape a deferred Python call would see is not provable
/// here); `Some(true)` / `Some(false)` on decided verdicts.
pub(crate) fn callable_with_vars_reachable(typ: &Type) -> Option<bool> {
    match typ {
        Type::TypeAliasType { .. } => return None,
        Type::CallableType { variables, .. } if !variables.is_empty() => return Some(true),
        Type::Overloaded { items }
            if items.iter().any(
                |it| matches!(it, Type::CallableType { variables, .. } if !variables.is_empty()),
            ) =>
        {
            return Some(true);
        }
        _ => {}
    }
    let mut kids = children(typ);
    match typ {
        Type::CallableType {
            fallback,
            type_guard,
            type_is,
            ..
        } => {
            // children() mirrors BoolTypeQuery and skips the fallback
            // and guard slots; the constraint recursion consults all
            // three for parity, so the walker must see through them.
            kids.push(fallback);
            if let Some(g) = type_guard {
                kids.push(g);
            }
            if let Some(i) = type_is {
                kids.push(i);
            }
        }
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            kids.extend(values.iter());
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            kids.extend(prefix.arg_types.iter());
            kids.extend(prefix.variables.iter());
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            kids.push(tuple_fallback);
            kids.push(upper_bound);
            kids.push(default);
        }
        Type::Parameters(p) => {
            kids.extend(p.arg_types.iter());
            kids.extend(p.variables.iter());
        }
        _ => {}
    }
    // None must win over an earlier Some(true): Python keeps scanning the
    // remaining children for a blocking alias before committing to true.
    let mut any_true = false;
    for k in kids {
        match callable_with_vars_reachable(k) {
            Some(false) => {}
            None => return None,
            Some(true) => any_true = true,
        }
    }
    Some(any_true)
}

/// Yield the direct child types of `typ` (for ANY_STRATEGY / ALL_STRATEGY
/// traversal). Mirrors the `query_types` calls in `BoolTypeQuery.visit_*`.
fn children(typ: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match typ {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ, .. } => out.push(typ),
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
        ("int", LiteralValue::BigInt(v)) => v.to_string() == payload,
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

// Strict list-return decode helpers (#1412).

// The visitor seams above return wire blobs that the Python shim decodes
// back into live `Type` graphs; a blob carrying a `TypeAliasType` decodes
// to an alias with `alias=None`, poisoning `get_proper_type` downstream.

// Guard 1, `decode_type_scoped`: single blob, defers (`None`) on
// undecodable bytes or any alias in the tree. Guard 2,
// `decode_types_for_list_return`: all-or-nothing list of blobs.

fn decode_type_scoped(bytes: &[u8]) -> Option<Type> {
    let typ = decode_type(bytes)?;
    if type_contains_alias(&typ) {
        return None;
    }
    Some(typ)
}

fn decode_types_for_list_return(blobs: &[Vec<u8>]) -> Option<Vec<Type>> {
    let mut types = Vec::with_capacity(blobs.len());
    for b in blobs {
        types.push(decode_type_scoped(b)?);
    }
    Some(types)
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
pub(crate) fn rust_remove_dups(type_bytes_list: Vec<Vec<u8>>) -> PyResult<Option<Vec<Vec<u8>>>> {
    let types = match decode_types_for_list_return(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    let deduped = remove_dups_inner(&types);
    Ok(encode_type_list(&deduped))
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
pub(crate) fn rust_type_vars_as_args(
    type_bytes_list: Vec<Vec<u8>>,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let types = match decode_types_for_list_return(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = type_vars_as_args_inner(&types);
    Ok(encode_type_list(&result))
}

pub(crate) fn type_vars_as_args_inner(type_vars: &[Type]) -> Vec<Type> {
    type_vars
        .iter()
        .map(|tv| match tv {
            Type::TypeVarTupleType { .. } => Type::UnpackType {
                typ: Box::new(tv.clone()),
                from_star_syntax: false,
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
    let any_type = match decode_type_scoped(any_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let ret_type = match decode_type_scoped(ret_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let fallback = match decode_type_scoped(fallback_bytes) {
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
        from_type_type: false,
        arg_types: vec![any_type.clone(), any_type.clone()],
        arg_kinds: vec![ARG_STAR, ARG_STAR2],
        arg_names: vec![None, None],
        ret_type: Box::new(ret_type.clone()),
        name: None,
        variables: Vec::new(),
        type_guard: None,
        type_is: None,
        special_sig: None,
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
/// Returns the 0-based index, or `-1` (in the Some arm) if no UnpackType
/// is present; `None` defers (an input blob failed the wire decode).
/// The decision only reads the top-level item kind, so alias rows ride
/// the non-strict decode (`decode_types_for_flatten`): Python's
/// `isinstance(item, UnpackType)` is alias-blind.
/// The Python version asserts uniqueness (raises if two are found); we
/// silently return the first and rely on the earlier semanal pass to flag
/// duplicates.
#[pyfunction]
pub(crate) fn rust_find_unpack_in_list(type_bytes_list: Vec<Vec<u8>>) -> PyResult<Option<i64>> {
    let types = match decode_types_for_flatten(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(Some(find_unpack_in_list_inner(&types)))
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
) -> PyResult<Option<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>)>> {
    let types = match decode_types_for_list_return(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    let (head, mid, tail) = split_with_prefix_and_suffix_inner(&types, prefix, suffix);
    let (Some(head), Some(mid), Some(tail)) = (
        encode_type_list(&head),
        encode_type_list(&mid),
        encode_type_list(&tail),
    ) else {
        // Strict all-or-nothing (#1412): an unencodable row defers instead
        // of silently truncating the list.
        return Ok(None);
    };
    Ok(Some((head, mid, tail)))
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

fn encode_type_list(types: &[Type]) -> Option<Vec<Vec<u8>>> {
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        {
            let b = encode_type(t)?;
            out.push(b)
        }
    }
    Some(out)
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
        if let Type::UnpackType { typ, .. } = t {
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
/// list.
///
/// Mirrors `flatten_nested_unions` (types.py:5849-5905). With
/// `handle_type_alias_type`, a `TypeAliasType` row is expanded exactly
/// like Python's `get_proper_type(t)` decision: only the *union shape*
/// of the expansion matters, and the flattened items come from it. With
/// a resolver the expansion rides the alias snapshot (one chain step
/// plus argument substitution, `_expand_once` semantics); without one
/// (startup, before build wires the resolver) it rides
/// `row_expansions`, shim-precomputed per-row proper-type blobs. A
/// missing source, an alias cycle on the active expansion path, or an
/// unbuildable substitution env defers the whole call (`None`) so the
/// Python body re-runs.
#[pyfunction]
#[pyo3(signature = (
    type_bytes_list,
    handle_type_alias_type,
    handle_recursive,
    resolver,
    row_expansions = Vec::new()
))]
pub(crate) fn rust_flatten_nested_unions(
    type_bytes_list: Vec<Vec<u8>>,
    handle_type_alias_type: bool,
    handle_recursive: bool,
    resolver: Option<&mut NativeTypeResolver>,
    row_expansions: Vec<Option<Vec<u8>>>,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let types = match decode_types_for_flatten(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    // Fast path: nothing to flatten if no TypeAliasType or UnionType.
    if !types
        .iter()
        .any(|t| matches!(t, Type::TypeAliasType { .. } | Type::UnionType { .. }))
    {
        return Ok(encode_type_list(&types));
    }
    let aliases: Option<&dyn crate::aliases::AliasLookup> =
        resolver.map(|r| r.alias_resolver() as &dyn crate::aliases::AliasLookup);
    let mut active = Vec::new();
    let flat = flatten_nested_unions_inner(
        &types,
        handle_type_alias_type,
        handle_recursive,
        aliases,
        &mut active,
        Some(&row_expansions),
    );
    let flat = match flat {
        Some(f) => f,
        None => return Ok(None),
    };
    Ok(encode_type_list(&flat))
}

/// Decode a list seam's input blobs. Unlike `decode_types_for_list_return`
/// this keeps `TypeAliasType` nodes (the flatten seam expands bare ones
/// through the alias snapshot); an undecodable blob still defers.
fn decode_types_for_flatten(blobs: &[Vec<u8>]) -> Option<Vec<Type>> {
    let mut types = Vec::with_capacity(blobs.len());
    for b in blobs {
        types.push(decode_type(b)?);
    }
    Some(types)
}

pub(crate) fn flatten_nested_unions_inner(
    types: &[Type],
    handle_type_alias_type: bool,
    handle_recursive: bool,
    aliases: Option<&dyn crate::aliases::AliasLookup>,
    active: &mut Vec<String>,
    // Top-level per-row proper-type blobs for alias rows when no alias
    // snapshot is installed (startup); `None` rows defer. Nested union
    // items always need the snapshot, so recursion passes `None`.
    expansions: Option<&[Option<Vec<u8>>]>,
) -> Option<Vec<Type>> {
    let mut flat_items: Vec<Type> = Vec::with_capacity(types.len());
    for (idx, t) in types.iter().enumerate() {
        if handle_type_alias_type {
            if let Type::TypeAliasType { is_recursive, .. } = t {
                if !handle_recursive && *is_recursive {
                    // Python: `not handle_recursive and t.is_recursive`
                    // keeps the recursive alias unexpanded (and an alias
                    // is never a UnionType, so the row passes through).
                    flat_items.push(t.clone());
                    continue;
                }
                // Python: `tp = get_proper_type(t)`; only the UnionType
                // check consumes the expansion. The wire `is_recursive`
                // flag is the same fact Python reads on the live node.
                let target: Option<Type> = match aliases {
                    Some(a) => {
                        let type_ref = match t {
                            Type::TypeAliasType { type_ref, .. } => type_ref.clone(),
                            _ => unreachable!(),
                        };
                        // Recursive re-entry would expand the same
                        // snapshot target forever; Python's
                        // get_proper_type stays lazy (guard #1149).
                        if active.contains(&type_ref) {
                            return None;
                        }
                        // Mirror Python's get_proper_type: one chain
                        // step with applied-args substitution
                        // (_expand_once); undecidable shape defers.
                        expanded_alias_target(t, a).map(|(target, _, _)| target)
                    }
                    None => {
                        let blob = expansions?.get(idx)?.as_ref()?;
                        let mut buf = ReadBuffer::new(blob);
                        read_type(&mut buf, None).ok()
                    }
                };
                let target = target?;
                let union_items = match &target {
                    Type::UnionType { items, .. } => Some(items),
                    // Non-union proper type: Python appends the original
                    // alias, not the expansion ("Must preserve original
                    // aliases when possible").
                    _ => None,
                };
                if let Some(items) = union_items {
                    // The cycle guard only matters on the snapshot path
                    // (re-expansion consults it); nested alias items on
                    // the snapshot-free path defer via `expansions=None`.
                    let guard = if aliases.is_some() {
                        let type_ref = match t {
                            Type::TypeAliasType { type_ref, .. } => type_ref.clone(),
                            _ => unreachable!(),
                        };
                        active.push(type_ref);
                        true
                    } else {
                        false
                    };
                    let inner = flatten_nested_unions_inner(
                        items,
                        handle_type_alias_type,
                        handle_recursive,
                        aliases,
                        active,
                        None,
                    );
                    if guard {
                        active.pop();
                    }
                    flat_items.extend(inner?);
                } else {
                    flat_items.push(t.clone());
                }
                continue;
            }
        }
        if let Type::UnionType { items, .. } = t {
            // Recurse into UnionType items.
            let inner = flatten_nested_unions_inner(
                items,
                handle_type_alias_type,
                handle_recursive,
                aliases,
                active,
                None,
            )?;
            flat_items.extend(inner);
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
/// nested with Unpack.
///
/// Mirrors `flatten_nested_tuples` (types.py:5943-5991). Rows with no
/// Unpack pass through unchanged, so alias-free-of-unpack trees ride the
/// non-strict decode. An UnpackType whose inner is a TypeAliasType
/// expands through the alias snapshot exactly like Python's
/// `get_proper_type(typ.type)` one-chain-step: a TupleType expansion
/// flattens, anything else (and a recursive alias under
/// `handle_recursive=False`) passes the row through unchanged. Defers
/// (`None`) on an undecodable blob, an alias the snapshot cannot
/// resolve (missing target, cycle, unbuildable substitution env), or a
/// recursive re-entry of an already-active alias (guard #1149).
#[pyfunction]
pub(crate) fn rust_flatten_nested_tuples(
    type_bytes_list: Vec<Vec<u8>>,
    handle_recursive: bool,
    resolver: Option<&mut NativeTypeResolver>,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let types = match decode_types_for_flatten(&type_bytes_list) {
        Some(t) => t,
        None => return Ok(None),
    };
    let aliases: Option<&dyn crate::aliases::AliasLookup> =
        resolver.map(|r| r.alias_resolver() as &dyn crate::aliases::AliasLookup);
    let mut active = Vec::new();
    let flat = match flatten_nested_tuples_inner(&types, handle_recursive, aliases, &mut active) {
        Some(f) => f,
        None => return Ok(None),
    };
    Ok(encode_type_list(&flat))
}

pub(crate) fn flatten_nested_tuples_inner(
    types: &[Type],
    handle_recursive: bool,
    aliases: Option<&dyn crate::aliases::AliasLookup>,
    active: &mut Vec<String>,
) -> Option<Vec<Type>> {
    let mut res: Vec<Type> = Vec::with_capacity(types.len());
    for typ in types {
        if let Type::UnpackType { typ: inner, .. } = typ {
            let is_alias = matches!(&**inner, Type::TypeAliasType { .. });
            // Python computes p_type = get_proper_type(typ.type); for a
            // non-alias the proper type is the node itself.
            let alias_ref = match &**inner {
                Type::TypeAliasType { type_ref, .. } => Some(type_ref.clone()),
                _ => None,
            };
            let p_type = if is_alias {
                let aliases = aliases?;
                // Recursive re-entry would re-expand the same snapshot
                // target forever; Python also loops here, so defer to
                // the fallback (guard #1149, mirrors the union path).
                if let Some(ref r) = alias_ref {
                    if active.contains(r) {
                        return None;
                    }
                }
                expanded_alias_target(inner, aliases).map(|(target, _, _)| target)?
            } else {
                *inner.clone()
            };
            let skip_recursive_alias = !handle_recursive
                && matches!(
                    &**inner,
                    Type::TypeAliasType {
                        is_recursive: true,
                        ..
                    }
                );
            if skip_recursive_alias || !matches!(p_type, Type::TupleType { .. }) {
                res.push(typ.clone());
                continue;
            }
            let items = match p_type {
                Type::TupleType { items, .. } => items,
                _ => unreachable!(),
            };
            let guarded = if let Some(r) = alias_ref {
                active.push(r);
                true
            } else {
                false
            };
            let inner_flat = flatten_nested_tuples_inner(&items, handle_recursive, aliases, active);
            if guarded {
                active.pop();
            }
            res.extend(inner_flat?);
            continue;
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
            meta_level: 0,
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
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
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

    fn make_alias(type_ref: &str, args: Vec<Type>, is_recursive: bool) -> Type {
        Type::TypeAliasType {
            args,
            type_ref: type_ref.to_string(),
            is_recursive,
        }
    }

    fn make_callable() -> Type {
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int", vec![])),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn make_typeddict(fallback: Type, items: Vec<(String, Type)>) -> Type {
        Type::TypedDictType {
            fallback: Box::new(fallback),
            items,
            required_keys: std::collections::HashSet::new(),
            readonly_keys: std::collections::HashSet::new(),
            is_closed: false,
        }
    }

    #[test]
    fn test_has_recursive_types_alias_flag_true() {
        let alias = make_alias("mod.Alias", vec![], true);
        assert!(has_recursive_types_inner(&alias));
    }

    #[test]
    fn test_has_recursive_types_alias_flag_false_args_recursive() {
        let inner = make_alias("mod.Inner", vec![], true);
        let alias = make_alias("mod.Alias", vec![inner], false);
        assert!(has_recursive_types_inner(&alias));
    }

    #[test]
    fn test_has_recursive_types_alias_flag_false_plain_args() {
        let alias = make_alias(
            "mod.Alias",
            vec![make_instance("builtins.int", vec![])],
            false,
        );
        assert!(!has_recursive_types_inner(&alias));
    }

    #[test]
    fn test_has_recursive_types_false_simple() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(!has_recursive_types_inner(&inst));
    }

    // Position parity with BoolTypeQuery (type_visitor.py): the
    // visited slots differ per variant; an alias in a non-visited slot
    // must NOT answer true.

    #[test]
    fn test_has_recursive_types_true_typevar_upper_bound() {
        let mut tv = make_typevar(1);
        if let Type::TypeVarType { upper_bound, .. } = &mut tv {
            *upper_bound = Box::new(make_alias("mod.A", vec![], true));
        }
        assert!(has_recursive_types_inner(&tv));
    }

    #[test]
    fn test_has_recursive_types_true_typevar_default() {
        let mut tv = make_typevar(1);
        if let Type::TypeVarType { default, .. } = &mut tv {
            *default = Box::new(make_alias("mod.A", vec![], true));
        }
        assert!(has_recursive_types_inner(&tv));
    }

    #[test]
    fn test_has_recursive_types_true_typevar_values() {
        let mut tv = make_typevar(1);
        if let Type::TypeVarType { values, .. } = &mut tv {
            *values = vec![make_alias("mod.A", vec![], true)];
        }
        assert!(has_recursive_types_inner(&tv));
    }

    #[test]
    fn test_has_recursive_types_false_callable_variables_only() {
        // visit_callable_type never queries `variables`.
        let mut c = make_callable();
        if let Type::CallableType { variables, .. } = &mut c {
            *variables = vec![make_alias("mod.A", vec![], true)];
        }
        assert!(!has_recursive_types_inner(&c));
    }

    #[test]
    fn test_has_recursive_types_false_instance_lkv_only() {
        // visit_instance queries args only.
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(make_alias("mod.A", vec![], true))),
            extra_attrs: None,
        };
        assert!(!has_recursive_types_inner(&inst));
    }

    #[test]
    fn test_has_recursive_types_false_typeddict_fallback_only() {
        // visit_typeddict_type queries item values only.
        let td = make_typeddict(make_alias("mod.A", vec![], true), vec![]);
        assert!(!has_recursive_types_inner(&td));
    }

    #[test]
    fn test_has_recursive_types_true_typeddict_item() {
        let td = make_typeddict(
            make_instance("builtins.dict", vec![]),
            vec![("k".to_string(), make_alias("mod.A", vec![], true))],
        );
        assert!(has_recursive_types_inner(&td));
    }

    #[test]
    fn test_has_recursive_types_false_tvt_tuple_fallback_only() {
        // visit_type_var_tuple queries upper_bound/default, not
        // tuple_fallback.
        let tvt = Type::TypeVarTupleType {
            tuple_fallback: Box::new(make_alias("mod.A", vec![], true)),
            name: "Ts".to_string(),
            fullname: "Ts".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            upper_bound: Box::new(make_unannotated_any()),
            default: Box::new(make_unannotated_any()),
            min_len: 0,
            meta_level: 0,
        };
        assert!(!has_recursive_types_inner(&tvt));
    }

    #[test]
    fn test_has_recursive_types_true_paramspec_prefix() {
        // visit_param_spec queries [upper_bound, default, prefix].
        let params = crate::wire::Parameters {
            arg_types: vec![make_alias("mod.A", vec![], true)],
            arg_kinds: vec![0],
            arg_names: vec![None],
            variables: vec![],
            imprecise_arg_kinds: false,
            is_ellipsis_args: false,
        };
        let ps = Type::ParamSpecType {
            prefix: Box::new(params),
            name: "P".to_string(),
            fullname: "P".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            flavor: 0,
            upper_bound: Box::new(make_unannotated_any()),
            default: Box::new(make_unannotated_any()),
            meta_level: 0,
        };
        assert!(has_recursive_types_inner(&ps));
    }

    #[test]
    fn test_has_recursive_types_false_parameters_variables_only() {
        // visit_parameters queries arg_types only.
        let params = crate::wire::Parameters {
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            variables: vec![make_alias("mod.A", vec![], true)],
            imprecise_arg_kinds: false,
            is_ellipsis_args: false,
        };
        assert!(!has_recursive_types_inner(&Type::Parameters(params)));
    }

    #[test]
    fn test_has_recursive_types_nested_union_instance_args() {
        let alias = make_alias("mod.A", vec![], true);
        let inst = make_instance("builtins.list", vec![alias]);
        assert!(has_recursive_types_inner(&inst));
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
        let result = remove_dups_inner(std::slice::from_ref(&a));
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
            meta_level: 0,
        };
        let result = type_vars_as_args_inner(&[tvt]);
        assert!(matches!(result[0], Type::UnpackType { .. }));
    }

    #[test]
    fn test_type_vars_as_args_passthrough() {
        let tv = make_typevar(1);
        let result = type_vars_as_args_inner(std::slice::from_ref(&tv));
        assert!(matches!(result[0], Type::TypeVarType { .. }));
    }

    #[test]
    fn test_find_unpack_in_list_found() {
        let a = make_instance("A", vec![]);
        let unpack = Type::UnpackType {
            typ: Box::new(make_instance("builtins.tuple", vec![])),
            from_star_syntax: false,
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

    fn flatten_inner(
        types: &[Type],
        htaa: bool,
        hr: bool,
        aliases: Option<&dyn crate::aliases::AliasLookup>,
    ) -> Option<Vec<Type>> {
        let mut active = Vec::new();
        flatten_nested_unions_inner(types, htaa, hr, aliases, &mut active, None)
    }

    /// Build a zero-argument alias snapshot whose target is the wire
    /// encoding of `target`.
    fn bare_alias_snapshot(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        use crate::wire::write_type;
        let mut wbuf = WriteBuffer::new();
        write_type(&mut wbuf, target).unwrap();
        crate::aliases::TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: wbuf.into_bytes(),
            ..Default::default()
        }
    }

    #[test]
    fn test_flatten_nested_unions_simple() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let inner = make_union(vec![a.clone(), b.clone()]);
        let outer = make_union(vec![inner, a.clone()]);
        let result = flatten_inner(&[outer], true, true, None);
        assert!(result.is_some());
        let flat = result.unwrap();
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_flatten_nested_unions_no_union() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let result = flatten_inner(&[a.clone(), b.clone()], true, true, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_flatten_nested_unions_alias_defers_without_resolver() {
        let alias = make_alias("mod.A", vec![], false);
        let result = flatten_inner(&[alias], true, true, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_flatten_nested_unions_alias_no_handle() {
        let alias = make_alias("mod.A", vec![], false);
        let result = flatten_inner(&[alias.clone()], false, true, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![alias]);
    }

    #[test]
    fn test_flatten_nested_unions_bare_alias_union_target_expands() {
        use crate::aliases::TypeAliasResolver;
        let target = make_union(vec![make_instance("A", vec![]), make_instance("B", vec![])]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.U".to_string(), bare_alias_snapshot("mod.U", &target));
        let alias = make_alias("mod.U", vec![], false);
        let result = flatten_inner(&[alias], true, true, Some(&resolver));
        let flat = result.unwrap();
        assert_eq!(flat.len(), 2);
        assert!(matches!(flat[0], Type::Instance { .. }));
        assert!(matches!(flat[1], Type::Instance { .. }));
    }

    #[test]
    fn test_flatten_nested_unions_bare_alias_instance_target_kept() {
        use crate::aliases::TypeAliasResolver;
        let target = make_instance("builtins.list", vec![]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.L".to_string(), bare_alias_snapshot("mod.L", &target));
        let alias = make_alias("mod.L", vec![], false);
        // Python appends the ORIGINAL alias for a non-union expansion.
        let result = flatten_inner(&[alias.clone()], true, true, Some(&resolver));
        assert_eq!(result.unwrap(), vec![alias]);
    }

    #[test]
    fn test_flatten_nested_unions_nested_bare_alias_in_union_items() {
        use crate::aliases::TypeAliasResolver;
        let target = make_union(vec![make_instance("C", vec![])]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.N".to_string(), bare_alias_snapshot("mod.N", &target));
        // Union[int, mod.N] where mod.N expands to Union[C].
        let alias = make_alias("mod.N", vec![], false);
        let row = make_union(vec![make_instance("builtins.int", vec![]), alias]);
        let result = flatten_inner(&[row], true, true, Some(&resolver));
        let flat = result.unwrap();
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn test_flatten_nested_unions_applied_alias_expands() {
        use crate::aliases::TypeAliasResolver;
        let target = make_union(vec![make_instance("A", vec![])]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.G".to_string(), bare_alias_snapshot("mod.G", &target));
        // An applied alias (non-empty args) expands through the
        // _expand_once substitution, matching Python's get_proper_type,
        // so it no longer defers: the result is the flattened target.
        let alias = make_alias("mod.G", vec![make_instance("builtins.int", vec![])], false);
        let result = flatten_inner(&[alias], true, true, Some(&resolver));
        assert!(result.is_some());
        let flat = result.unwrap();
        assert_eq!(flat.len(), 1);
    }

    #[test]
    fn test_flatten_nested_unions_missing_snapshot_defers() {
        let alias = make_alias("mod.Missing", vec![], false);
        let result = flatten_inner(
            &[alias],
            true,
            true,
            Some(&crate::aliases::TypeAliasResolver::new()),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_flatten_nested_unions_recursive_alias_cycle_cuts() {
        use crate::aliases::TypeAliasResolver;
        // mod.S = Union[int, mod.S] (self-referential bare alias): the
        // active-path cut defers instead of looping.
        let self_ref = make_alias("mod.S", vec![], false);
        let target = make_union(vec![make_instance("builtins.int", vec![]), self_ref]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.S".to_string(), bare_alias_snapshot("mod.S", &target));
        let alias = make_alias("mod.S", vec![], true);
        let result = flatten_inner(&[alias], true, true, Some(&resolver));
        assert!(result.is_none());
    }

    #[test]
    fn test_flatten_nested_unions_recursive_alias_no_handle_kept() {
        // handle_recursive=False keeps recursive aliases unexpanded
        // (the wire is_recursive flag drives the decision, no resolver
        // needed).
        let alias = make_alias("mod.S", vec![], true);
        let result = flatten_inner(&[alias.clone()], true, false, None);
        assert_eq!(result.unwrap(), vec![alias]);
    }

    #[test]
    fn test_flatten_nested_unions_recursive_alias_no_handle_nonrecursive_expands() {
        use crate::aliases::TypeAliasResolver;
        // handle_recursive=False still expands NON-recursive aliases.
        let target = make_union(vec![make_instance("A", vec![])]);
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.NR".to_string(), bare_alias_snapshot("mod.NR", &target));
        let alias = make_alias("mod.NR", vec![], false);
        let result = flatten_inner(&[alias], true, false, Some(&resolver));
        let flat = result.unwrap();
        assert_eq!(flat.len(), 1);
        assert!(matches!(flat[0], Type::Instance { .. }));
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

    fn insert_alias_snapshot(resolver: &mut crate::aliases::TypeAliasResolver, target: &Type) {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, target).expect("target must encode");
        let fullname = "m.Rec".to_string();
        resolver.insert(
            fullname.clone(),
            crate::aliases::TypeAliasSnapshot {
                fullname,
                target: buf.into_bytes(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_flatten_nested_tuples_recursive_alias_defers() {
        // Rec = tuple[int, Unpack[Rec]]: the target contains the alias
        // itself, so a handle_recursive=True re-entry must defer via
        // the active guard, not re-expand forever.
        let rec = make_alias("m.Rec", vec![], true);
        let target = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![
                make_instance("builtins.int", vec![]),
                Type::UnpackType {
                    typ: Box::new(rec),
                    from_star_syntax: false,
                },
            ],
            implicit: false,
        };
        let mut resolver = crate::aliases::TypeAliasResolver::new();
        insert_alias_snapshot(&mut resolver, &target);
        let row = Type::UnpackType {
            typ: Box::new(make_alias("m.Rec", vec![], true)),
            from_star_syntax: false,
        };
        let mut active = Vec::new();
        let out = flatten_nested_tuples_inner(
            &[row],
            true,
            Some(&resolver as &dyn crate::aliases::AliasLookup),
            &mut active,
        );
        assert!(out.is_none());
    }

    #[test]
    fn test_flatten_nested_tuples_non_recursive_alias_expands() {
        // Positive control: a non-recursive tuple alias expands and
        // flattens natively (the pre-guard behavior stays intact).
        let alias = make_alias("m.Plain", vec![], false);
        let target = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![
                make_instance("builtins.int", vec![]),
                make_instance("builtins.str", vec![]),
            ],
            implicit: false,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &target).expect("target must encode");
        let mut resolver = crate::aliases::TypeAliasResolver::new();
        resolver.insert(
            "m.Plain".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "m.Plain".to_string(),
                target: buf.into_bytes(),
                ..Default::default()
            },
        );
        let row = Type::UnpackType {
            typ: Box::new(alias),
            from_star_syntax: false,
        };
        let mut active = Vec::new();
        let out = flatten_nested_tuples_inner(
            &[row],
            true,
            Some(&resolver as &dyn crate::aliases::AliasLookup),
            &mut active,
        );
        let flat = out.expect("non-recursive alias must expand");
        assert!(matches!(&flat[0], Type::Instance { .. }));
        assert!(matches!(&flat[1], Type::Instance { .. }));
        assert!(active.is_empty());
    }

    fn make_generic_callable() -> Type {
        let mut c = make_callable();
        if let Type::CallableType { variables, .. } = &mut c {
            variables.push(make_typevar(1));
        }
        c
    }

    fn make_overloaded(items: Vec<Type>) -> Type {
        Type::Overloaded { items }
    }

    #[test]
    fn test_gate_plain_callable() {
        assert_eq!(callable_with_vars_reachable(&make_callable()), Some(false));
    }

    #[test]
    fn test_gate_generic_root() {
        assert_eq!(
            callable_with_vars_reachable(&make_generic_callable()),
            Some(true)
        );
    }

    #[test]
    fn test_gate_generic_nested_in_arg() {
        let mut c = make_callable();
        if let Type::CallableType { arg_types, .. } = &mut c {
            arg_types.push(make_generic_callable());
        }
        assert_eq!(callable_with_vars_reachable(&c), Some(true));
    }

    #[test]
    fn test_gate_generic_via_type_guard() {
        let mut c = make_callable();
        if let Type::CallableType { type_guard, .. } = &mut c {
            *type_guard = Some(Box::new(make_generic_callable()));
        }
        assert_eq!(callable_with_vars_reachable(&c), Some(true));
    }

    #[test]
    fn test_gate_generic_via_fallback() {
        let mut c = make_callable();
        if let Type::CallableType { fallback, .. } = &mut c {
            let mut f = make_callable();
            if let Type::CallableType { ret_type, .. } = &mut f {
                *ret_type = Box::new(make_generic_callable());
            }
            **fallback = f;
        }
        assert_eq!(callable_with_vars_reachable(&c), Some(true));
    }

    #[test]
    fn test_gate_generic_inside_typevar_upper_bound() {
        let mut tv = make_typevar(1);
        if let Type::TypeVarType { upper_bound, .. } = &mut tv {
            *upper_bound = Box::new(make_generic_callable());
        }
        assert_eq!(callable_with_vars_reachable(&tv), Some(true));
    }

    #[test]
    fn test_gate_generic_inside_parameters_args() {
        let p = Type::Parameters(crate::wire::Parameters {
            arg_types: vec![make_generic_callable()],
            arg_kinds: vec![0],
            arg_names: vec![None],
            variables: vec![],
            imprecise_arg_kinds: false,
            is_ellipsis_args: false,
        });
        assert_eq!(callable_with_vars_reachable(&p), Some(true));
    }

    #[test]
    fn test_gate_overloaded_with_generic_item() {
        let o = make_overloaded(vec![make_callable(), make_generic_callable()]);
        assert_eq!(callable_with_vars_reachable(&o), Some(true));
    }

    #[test]
    fn test_gate_overloaded_plain_items() {
        let o = make_overloaded(vec![make_callable(), make_callable()]);
        assert_eq!(callable_with_vars_reachable(&o), Some(false));
    }

    #[test]
    fn test_gate_alias_blocks_verdict() {
        let mut c = make_callable();
        if let Type::CallableType { arg_types, .. } = &mut c {
            arg_types.push(make_alias(
                "m.A",
                vec![make_instance("builtins.int", vec![])],
                false,
            ));
        }
        assert_eq!(callable_with_vars_reachable(&c), None);
    }

    #[test]
    fn test_gate_alias_beats_generic() {
        let u = make_union(vec![
            make_alias("m.A", vec![], false),
            make_generic_callable(),
        ]);
        assert_eq!(callable_with_vars_reachable(&u), None);
    }

    #[test]
    fn test_gate_alias_beats_generic_reversed_order() {
        let u = make_union(vec![
            make_generic_callable(),
            make_alias("m.A", vec![], false),
        ]);
        assert_eq!(callable_with_vars_reachable(&u), None);
    }
}
