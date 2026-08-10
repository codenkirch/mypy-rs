//! Native port of standalone type-helper functions from `mypy/checkexpr.py`
//! and `mypy/checker.py` (Stage 9).
//!
//! These are pure-logic functions that operate on the wire-format `Type`
//! enum without needing live Python checker state. Each is exposed as a
//! `#[pyfunction]` with a Python-side strangler-fig gate.
//!
//! Deferred (return None) cases:
//!   * Functions that call `get_proper_type` (alias expansion) defer on
//!     `TypeAliasType` since the wire format has no resolved alias target.

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::operators::is_operator_method_name;
use crate::setops::is_type_obj_callable;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, LiteralValue, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `TypeOfAny.special_form` == 6. Special forms are not real Any types.
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

/// `TypeOfAny.unannotated` == 1.
const TYPE_OF_ANY_UNANNOTATED: i64 = 1;

/// `ArgKind.ARG_POS` = 0. `ArgKind.ARG_OPT` = 1. `ArgKind.ARG_STAR` = 2.
/// `ArgKind.ARG_NAMED` = 3. `ArgKind.ARG_STAR2` = 4. `ArgKind.ARG_NAMED_OPT` = 5.
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

// ---------------------------------------------------------------------------
// Wire helpers
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

/// `get_proper_type` for the wire format. Expands TypeAliasType by
/// returning None (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// Whether a CallableType is a type object (i.e. its fallback is
/// `builtins.type`). Mirrors `CallableType.is_type_obj()` — the wire
/// format stores `fallback` + `from_concatenate` but not the computed
/// `is_type_obj` boolean, so we reconstruct it here.
fn is_type_obj(fallback: &Type, from_concatenate: bool) -> bool {
    if from_concatenate {
        return false;
    }
    matches!(
        fallback,
        Type::Instance { type_ref, .. } if type_ref == "builtins.type"
    )
}

// ---------------------------------------------------------------------------
// has_any_type: BoolTypeQuery (ANY_STRATEGY)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_any_type` — whether a type contains an Any type.
/// Special forms (type_of_any == 6) are not counted as real Any.
///
/// Mirrors `HasAnyType` (checkexpr.py:6633-6660). Defers (returns None)
/// on TypeAliasType.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_any_type(
    type_bytes: &[u8],
    ignore_in_type_obj: bool,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_any_type_inner(&typ, ignore_in_type_obj))
}

pub(crate) fn has_any_type_inner(typ: &Type, ignore_in_type_obj: bool) -> Option<bool> {
    if let Type::TypeAliasType { .. } = typ {
        return None;
    }
    match typ {
        Type::AnyType { type_of_any, .. } => Some(*type_of_any != TYPE_OF_ANY_SPECIAL_FORM),
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            fallback,
            from_concatenate,
            ..
        } => {
            if ignore_in_type_obj && is_type_obj(fallback, *from_concatenate) {
                return Some(false);
            }
            for t in arg_types {
                match has_any_type_inner(t, ignore_in_type_obj) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            match has_any_type_inner(ret_type, ignore_in_type_obj) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
            for v in variables {
                match has_any_type_inner(v, ignore_in_type_obj) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            if let Some(it) = instance_type {
                return has_any_type_inner(it, ignore_in_type_obj);
            }
            Some(false)
        }
        _ => {
            for child in children(typ) {
                match has_any_type_inner(child, ignore_in_type_obj) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
    }
}

/// Yield direct child types (same as visitor::children, duplicated here
/// to keep this module self-contained).
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
        // CallableType handled separately. Parameters, leaves: none.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// has_uninhabited_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_uninhabited_component` — whether a type contains
/// an UninhabitedType component.
///
/// Mirrors `HasUninhabitedComponent` (checkexpr.py). Defers on TypeAliasType.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_uninhabited_component(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_uninhabited_component_inner(&typ))
}

pub(crate) fn has_uninhabited_component_inner(typ: &Type) -> Option<bool> {
    if let Type::TypeAliasType { .. } = typ {
        return None;
    }
    if matches!(typ, Type::UninhabitedType { .. }) {
        return Some(true);
    }
    for child in all_children(typ) {
        match has_uninhabited_component_inner(child) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_ambiguous_uninhabited_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_ambiguous_uninhabited_component` — whether a type
/// contains an UninhabitedType marked ambiguous.
///
/// Mirrors `HasAmbiguousUninhabitedComponentsQuery` (checkexpr.py:
/// 7007-7018). `ambiguous` is the flag on the UninhabitedType wire
/// variant; False matches the plain has_uninhabited_component case.
/// Defer on TypeAliasType (no alias target on the wire).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_ambiguous_uninhabited_component(
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_ambiguous_uninhabited_component_inner(&typ))
}

pub(crate) fn has_ambiguous_uninhabited_component_inner(typ: &Type) -> Option<bool> {
    if let Type::TypeAliasType { .. } = typ {
        return None;
    }
    if let Type::UninhabitedType { ambiguous } = typ {
        return Some(*ambiguous);
    }
    for child in all_children(typ) {
        match has_ambiguous_uninhabited_component_inner(child) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_bytes_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_bytes_component` — is this one of builtin byte
/// types, or a union that contains it?
///
/// Mirrors `has_bytes_component` (checkexpr.py:6988-6997). Defers on
/// TypeAliasType (needs get_proper_type).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_bytes_component(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_bytes_component_inner(&typ))
}

pub(crate) fn has_bytes_component_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::UnionType { items, .. } => {
            for t in items {
                match has_bytes_component_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        Type::Instance { type_ref, .. } => {
            Some(*type_ref == "builtins.bytes" || *type_ref == "builtins.bytearray")
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// has_bool_item
// ---------------------------------------------------------------------------

/// `mypy.checker.has_bool_item` — return True if type is 'bool' or a
/// union with a 'bool' item.
///
/// Mirrors `has_bool_item` (checker.py:9731-9738). Defers on TypeAliasType.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_bool_item(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_bool_item_inner(&typ))
}

pub(crate) fn has_bool_item_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => Some(*type_ref == "builtins.bool"),
        Type::UnionType { items, .. } => {
            for t in items {
                match has_bool_item_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_non_empty_tuple
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_non_empty_tuple` — whether t is a TupleType with
/// at least one item.
///
/// Mirrors `is_non_empty_tuple` (checkexpr.py:6702-6704). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_non_empty_tuple(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_non_empty_tuple_inner(&typ))
}

pub(crate) fn is_non_empty_tuple_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::TupleType { items, .. } => Some(!items.is_empty()),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// has_coroutine_decorator
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_coroutine_decorator` — whether t came from a
/// function decorated with `@coroutine`.
///
/// Mirrors `has_coroutine_decorator` (checkexpr.py:6662-6665). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_coroutine_decorator(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_coroutine_decorator_inner(&typ))
}

pub(crate) fn has_coroutine_decorator_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => Some(*type_ref == "typing.AwaitableGenerator"),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_async_def
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_async_def` — whether t came from a function defined
/// using `async def`.
///
/// An `async def` decorated with `@typing.coroutine` (or `@asyncio.coroutine`)
/// has return type `typing.AwaitableGenerator[...]`, which preserves the
/// original return type as its 4th type argument. That argument is unwrapped
/// and checked for `typing.Coroutine` (the actual `async def` return type).
///
/// Mirrors `is_async_def` (checkexpr.py:6900-6909). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_async_def(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_async_def_inner(&typ))
}

pub(crate) fn is_async_def_inner(typ: &Type) -> Option<bool> {
    let mut proper = get_proper_or_none(typ)?;
    if let Type::Instance { type_ref, args, .. } = proper {
        if type_ref == "typing.AwaitableGenerator" && args.len() >= 4 {
            proper = get_proper_or_none(&args[3])?;
        }
    }
    match proper {
        Type::Instance { type_ref, .. } => Some(type_ref == "typing.Coroutine"),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_duplicate_mapping
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_duplicate_mapping` — whether multiple actual
/// arguments map to the same formal in a non-star position, i.e. the call
/// has duplicate values for that formal.
///
/// Mirrors `is_duplicate_mapping` (checkexpr.py:6947-6959). The exception
/// cases where duplicates are allowed at runtime:
///   * `f(..., *args, **kwargs)` with exactly two actuals (`*args` and
///     `**kwargs`) mapping to the same formal.
///   * Multiple `**kwargs` that all map to the same formal, provided they
///     are not TypedDicts (a non-TypedDict `**kwargs` cannot be matched
///     with certainty).
///
/// `actual_types` each carry a serialized type; we resolve each through
/// `get_proper_or_none` so a `TypeAliasType` actual defers (None), since
/// the wire format has no alias target.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_duplicate_mapping(
    mapping: Vec<i64>,
    actual_types: Vec<Vec<u8>>,
    actual_kinds: Vec<i64>,
) -> PyResult<Option<bool>> {
    let mut types = Vec::with_capacity(mapping.len());
    for &idx in &mapping {
        match actual_types.get(idx as usize) {
            Some(bytes) => match decode_type(bytes) {
                Some(t) => types.push(t),
                None => return Ok(None),
            },
            None => return Ok(None),
        }
    }
    Ok(is_duplicate_mapping_inner(&mapping, &types, &actual_kinds))
}

fn is_duplicate_mapping_inner(
    mapping: &[i64],
    actual_types: &[Type],
    actual_kinds: &[i64],
) -> Option<bool> {
    // `mapping` with one entry (or fewer) cannot be a duplicate.
    if mapping.len() <= 1 {
        return Some(false);
    }
    // `f(..., *args, **kwargs)`: the two actuals can both map to the same
    // formal and no runtime duplicate occurs. Allow this exception.
    if mapping.len() == 2 {
        let first = *actual_kinds.get(mapping[0] as usize)?;
        let second = *actual_kinds.get(mapping[1] as usize)?;
        if first == ARG_STAR && second == ARG_STAR2 {
            return Some(false);
        }
    }
    // Exceptions where duplicates are allowed: every mapped actual is a
    // non-TypedDict `**kwargs` (cannot be matched with certainty), so
    // `all(mapped actual is a non-TypedDict **kwargs)` disables the check.
    let mut all_non_typeddict_star2 = true;
    for (i, &idx) in mapping.iter().enumerate() {
        let kind = *actual_kinds.get(idx as usize)?;
        if kind != ARG_STAR2 {
            all_non_typeddict_star2 = false;
            break;
        }
        let proper = get_proper_or_none(&actual_types[i])?;
        if matches!(proper, Type::TypedDictType { .. }) {
            all_non_typeddict_star2 = false;
            break;
        }
    }
    Some(!all_non_typeddict_star2)
}

// ---------------------------------------------------------------------------
// is_typed_callable
// ---------------------------------------------------------------------------

/// `mypy.checker.is_typed_callable` — whether a callable type has at
/// least one non-unannotated-Any type in its args or return.
///
/// Mirrors `is_typed_callable` (checker.py:9613-9621). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_typed_callable(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_typed_callable_inner(&typ))
}

pub(crate) fn is_typed_callable_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::CallableType {
            arg_types,
            ret_type,
            ..
        } => {
            // Returns True if NOT all types are unannotated Any.
            let all_unannotated = arg_types
                .iter()
                .chain(std::iter::once(ret_type.as_ref()))
                .all(is_unannotated_any_type);
            Some(!all_unannotated)
        }
        _ => Some(false),
    }
}

fn is_unannotated_any_type(typ: &Type) -> bool {
    matches!(typ, Type::AnyType { type_of_any, .. } if *type_of_any == TYPE_OF_ANY_UNANNOTATED)
}

// ---------------------------------------------------------------------------
// is_private
// ---------------------------------------------------------------------------

/// `mypy.checker.is_private` — check if node name is private to class.
/// Mirrors `is_private` (checker.py:9721-9723).
#[pyfunction]
pub(crate) fn rust_is_private(node_name: &str) -> PyResult<bool> {
    Ok(node_name.starts_with("__") && !node_name.ends_with("__"))
}

// ---------------------------------------------------------------------------
// is_operator_method
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_operator_method` — check if fullname is an
/// operator method.
/// Mirrors `is_operator_method` (checkexpr.py:7019-7026).
#[pyfunction]
pub(crate) fn rust_is_operator_method(fullname: Option<&str>) -> PyResult<bool> {
    Ok(match fullname {
        Some(f) => {
            let short_name = f.rsplit('.').next().unwrap_or("");
            is_operator_method_name(short_name)
        }
        None => false,
    })
}

// ---------------------------------------------------------------------------
// are_argument_counts_overlapping
// ---------------------------------------------------------------------------

/// `mypy.checker.are_argument_counts_overlapping` — can a single call
/// match both t and s, based just on positional argument counts?
///
/// Mirrors `are_argument_counts_overlapping` (checker.py:9115-9119).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_are_argument_counts_overlapping(
    t_bytes: &[u8],
    s_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let t = match decode_type(t_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let s = match decode_type(s_bytes) {
        Some(s) => s,
        None => return Ok(None),
    };
    Ok(are_argument_counts_overlapping_inner(&t, &s))
}

pub(crate) fn are_argument_counts_overlapping_inner(t: &Type, s: &Type) -> Option<bool> {
    let t_kinds = match get_proper_or_none(t)? {
        Type::CallableType { arg_kinds, .. } => arg_kinds,
        _ => return Some(false),
    };
    let s_kinds = match get_proper_or_none(s)? {
        Type::CallableType { arg_kinds, .. } => arg_kinds,
        _ => return Some(false),
    };
    let min_args_t = count_min_args(t_kinds);
    let min_args_s = count_min_args(s_kinds);
    let min_args = min_args_t.max(min_args_s);
    let max_t = count_max_positional(t_kinds);
    let max_s = count_max_positional(s_kinds);
    let max_args = max_t.min(max_s);
    Some(min_args <= max_args)
}

/// `min_args`: count of ARG_POS only (required positional args).
/// Mirrors `CallableType.min_args` property: `arg_kinds.count(ARG_POS)`.
fn count_min_args(arg_kinds: &[i64]) -> usize {
    arg_kinds.iter().filter(|&&k| k == ARG_POS).count()
}

/// `max_possible_positional_args`: if the callable has *args or **kwargs,
/// returns `usize::MAX` (mirrors `sys.maxsize`). Otherwise counts all
/// positional args (ARG_POS, ARG_OPT, ARG_NAMED, ARG_NAMED_OPT).
fn count_max_positional(arg_kinds: &[i64]) -> usize {
    if arg_kinds.iter().any(|&k| k == ARG_STAR || k == ARG_STAR2) {
        usize::MAX
    } else {
        arg_kinds
            .iter()
            .filter(|&&k| k == ARG_POS || k == ARG_OPT || k == ARG_NAMED || k == ARG_NAMED_OPT)
            .count()
    }
}

// ---------------------------------------------------------------------------
// is_type_type_context
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_type_type_context` — whether context is a TypeType
/// or a union containing TypeType.
///
/// Mirrors `is_type_type_context` (checkexpr.py:7031-7036). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_type_type_context(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_type_type_context_inner(&typ))
}

pub(crate) fn is_type_type_context_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::TypeType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for t in items {
                match is_type_type_context_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// try_getting_literal
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.try_getting_literal` — if possible, get a more
/// precise literal type for a given type. Unwraps Instance with
/// last_known_value.
///
/// Mirrors `try_getting_literal` (checkexpr.py:6961-6965). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_try_getting_literal(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = match try_getting_literal_inner(&typ) {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(encode_type(&result))
}

pub(crate) fn try_getting_literal_inner(typ: &Type) -> Option<Type> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => Some(lkv.as_ref().clone()),
        _ => Some(proper.clone()),
    }
}

// ---------------------------------------------------------------------------
// is_string_literal
// ---------------------------------------------------------------------------

/// `mypy.checker.is_string_literal` — check if a type is a single string
/// literal. Uses `try_getting_str_literals_from_type` semantics: checks
/// for LiteralType with str value or Instance with last_known_value that
/// is a str literal.
///
/// Mirrors `is_string_literal` (checker.py:9726-9728). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_string_literal(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_string_literal_inner(&typ))
}

pub(crate) fn is_string_literal_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::LiteralType { value, fallback } => {
            // Check if it's a string literal (fallback is builtins.str).
            if let Type::Instance { type_ref, .. } = fallback.as_ref() {
                Some(*type_ref == "builtins.str" && matches!(value, LiteralValue::Str(_)))
            } else {
                Some(false)
            }
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => is_string_literal_inner(lkv),
        Type::UnionType { items, .. } => {
            if items.len() != 1 {
                return Some(false);
            }
            is_string_literal_inner(&items[0])
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_untyped_decorator (simplified: only CallableType/Overloaded check)
// ---------------------------------------------------------------------------

/// `mypy.checker.is_untyped_decorator` — whether a decorator type is
/// untyped (all Any, or no type).
///
/// Mirrors `is_untyped_decorator` (checker.py:9623-9647). Simplified:
/// does not handle Instance with `__call__` method (needs TypeInfo lookup).
/// Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_untyped_decorator(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_untyped_decorator_inner(&typ))
}

pub(crate) fn is_untyped_decorator_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::CallableType { .. } => {
            // not is_typed_callable(typ)
            match is_typed_callable_inner(proper)? {
                true => Some(false),
                false => Some(true),
            }
        }
        Type::Overloaded { items } => {
            // any(is_untyped_decorator(item) for item in typ.items)
            for t in items {
                match is_untyped_decorator_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        // Instance case needs TypeInfo lookup (__call__ method); defer.
        Type::Instance { .. } => None,
        _ => Some(true),
    }
}

// ---------------------------------------------------------------------------
// is_typeddict_type_context
// ---------------------------------------------------------------------------

/// `mypy.checker.is_typeddict_type_context` — whether the type is a
/// TypedDictType (used as a type context for TypedDict construction).
///
/// Mirrors `is_typeddict_type_context` (checker.py:9978-9988). Defers
/// on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_typeddict_type_context(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_typeddict_type_context_inner(&typ))
}

pub(crate) fn is_typeddict_type_context_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::TypedDictType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for t in items {
                match is_typeddict_type_context_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// allow_fast_container_literal
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.allow_fast_container_literal` — whether a type is a
/// fast-path container literal: an Instance, or a TupleType whose items
/// all qualify.
///
/// Mirrors `allow_fast_container_literal` (checkexpr.py:413-419). A
/// recursive TypeAlias defers (needs get_proper_type with alias target).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_allow_fast_container_literal(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(allow_fast_container_literal_inner(&typ))
}

pub(crate) fn allow_fast_container_literal_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::TupleType { items, .. } => {
            for it in items {
                match allow_fast_container_literal_inner(it) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => return None,
                }
            }
            Some(true)
        }
        Type::Instance { .. } => Some(true),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// all_children: include CallableType children (for has_uninhabited)
// ---------------------------------------------------------------------------

/// Like `children` but also yields CallableType children (arg_types,
/// ret_type, variables, instance_type). Used by has_uninhabited_component
/// which needs to recurse into callables.
fn all_children(typ: &Type) -> Vec<&Type> {
    let mut out = children(typ);
    if let Type::CallableType {
        arg_types,
        ret_type,
        variables,
        instance_type,
        ..
    } = typ
    {
        out.extend(arg_types.iter());
        out.push(ret_type);
        out.extend(variables.iter());
        if let Some(it) = instance_type {
            out.push(it);
        }
    }
    if let Type::TypeVarType {
        upper_bound,
        default,
        values,
        ..
    } = typ
    {
        out.push(upper_bound);
        out.push(default);
        out.extend(values.iter());
    }
    if let Type::ParamSpecType {
        upper_bound,
        default,
        prefix,
        ..
    } = typ
    {
        out.push(upper_bound);
        out.push(default);
        out.extend(prefix.arg_types.iter());
    }
    if let Type::TypeVarTupleType {
        upper_bound,
        default,
        tuple_fallback,
        ..
    } = typ
    {
        out.push(upper_bound);
        out.push(default);
        out.push(tuple_fallback);
    }
    if let Type::Parameters(p) = typ {
        out.extend(p.arg_types.iter());
    }
    out
}

// ---------------------------------------------------------------------------
// method_fullname
// ---------------------------------------------------------------------------

/// Mirror of `mypy.checkexpr.ExpressionChecker.method_fullname`
/// (checkexpr.py:861-899). Resolves `method_name` to a fully qualified
/// name (`type_name.method_name`) for the type the method is invoked on.
/// Returns None (defer) when the name cannot be determined.
///
/// Mirrors the Python isinstance chain one level deep: the outer type is
/// `get_proper_type`-expanded (TypeAliasType defers) and unwrapped once
/// for CallableType type objects and TypeType, then the flat chain checks
/// Instance / TypedDictType / LiteralType / TupleType. Anything else
/// (including a CallableType or TypeType nested in a type wrapper that
/// Python doesn't reach) defers.
fn method_fullname_inner(typ: &Type, method_name: &str, resolver: &TypeResolver) -> Option<String> {
    let proper = get_proper_or_none(typ)?; // TypeAliasType defers
    let unwrapped: &Type = match proper {
        Type::CallableType {
            fallback,
            ret_type,
            instance_type,
            from_concatenate,
            ..
        } if is_type_obj(fallback, *from_concatenate) => {
            // `CallableType.is_type_obj` also rejects a callable whose
            // proper return type is UninhabitedType; deferring is safe.
            if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
                return None;
            }
            // `get_instance_type()`: the explicit instance type, else the
            // proper return type.
            match instance_type {
                Some(t) => t.as_ref(),
                None => ret_type.as_ref(),
            }
        }
        Type::TypeType { item, .. } => item.as_ref(),
        other => other,
    };
    match unwrapped {
        Type::Instance { type_ref, .. } => Some(format!("{type_ref}.{method_name}")),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            // `tuple_fallback()`: named tuples return the partial fallback
            // directly; plain tuples rebuild from the fallback's type info.
            // Only the variadic-unpack path raises, so defer when it exists.
            let Type::Instance { type_ref, .. } = partial_fallback.as_ref() else {
                return None;
            };
            if type_ref == "builtins.tuple"
                && items.iter().any(|it| matches!(it, Type::UnpackType { .. }))
            {
                return None;
            }
            Some(format!("{type_ref}.{method_name}"))
        }
        Type::TypedDictType { fallback, .. } | Type::LiteralType { fallback, .. } => {
            containing_type_info(resolver, fallback, method_name)
        }
        _ => None,
    }
}

/// `TypeInfo.get_containing_type_info` (nodes.py:3953) for the wire
/// format: walk the fallback's MRO and return the first class whose
/// `names` table defines `method_name`, as a fully qualified method name.
/// A missing snapshot for the fallback or any MRO base defers (None) so
/// the Python side re-runs the real TypeInfo graph.
fn containing_type_info(
    resolver: &TypeResolver,
    fallback: &Type,
    method_name: &str,
) -> Option<String> {
    let Type::Instance { type_ref, .. } = fallback else {
        return None;
    };
    let start = resolver.get(type_ref)?;
    for base in &start.mro {
        let snap = resolver.get(base)?;
        if snap.member_info.contains_key(method_name) {
            return Some(format!("{}.{}", snap.fullname, method_name));
        }
    }
    None
}

/// `mypy.checkexpr.ExpressionChecker.method_fullname` (M25). The caller
/// serializes the (already proper) object type and the method name; the
/// kernel resolves the qualified name against the shared type-info
/// snapshot. Deferral (None) is the strangler-fig escape hatch: the
/// Python side recomputes via the real TypeInfo graph.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_method_fullname(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    method_name: &str,
) -> PyResult<Option<String>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(method_fullname_inner(
        &typ,
        method_name,
        resolver.resolver(),
    ))
}

// ---------------------------------------------------------------------------
// visit_star_expr — star expression type echo
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_star_expr` (M8c).
///
/// The Python implementation is simply `return self.accept(e.expr)`,
/// an identity pass-through.  The Rust kernel here accepts a single
/// serialized inner type and returns it verbatim.  This lets the
/// Python side skip the sub-expression traversal when the checker
/// has already computed the inner type.
///
/// Defer (return None) on any input shape we do not handle — the
/// strangler-fig escape hatch.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_star_expr(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    // We must round-trip through decode+encode so that an
    // unsupported shape (e.g. TypeAliasType) produces None.
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(encode_type(&typ))
}

// ---------------------------------------------------------------------------
// visit_conditional_expr — ternary join of branch types
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_conditional_expr` (M8c)
/// result-type computation.
///
/// The Python visitor computes `if_type` and `else_type` from the two
/// branches, then computes `make_simplified_union([if_type, else_type])`
/// as the result (line 6467).  In edge cases where that union contains an
/// uninhabited component it falls back to `join_types(if_type, else_type)`
/// (line 6472).
///
/// This Rust function receives both branch types serialized and returns
/// the join.  The caller (Python) still handles `accept` of the sub-expressions
/// and error reporting; this function is a pure type-derivation helper.
///
/// Defer (return None) when any input shape we cannot identify.
///
/// Takes a `NativeTypeResolver` because the full join logic uses
/// `subtypes::is_subtype` which needs the type-info snapshot for
/// Instance-Instance subtype checks.  Without a resolver we fall back
/// to Python, matching the Python gate pattern.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_conditional_expr_join(
    if_bytes: &[u8],
    else_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<Vec<u8>>> {
    let if_type = match decode_type(if_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let else_type = match decode_type(else_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(conditional_join_inner(
        &if_type,
        &else_type,
        resolver.resolver(),
    ))
}

/// Compute the join of two types for the conditional expression.
/// Mirrors `join.join_types` (the full implementation), which delegates
/// to `visit_<type>` on each concrete type variant.
///
/// This uses the resolver's type-info map for Instance-Instance subtype
/// and nominal join.  Returns `None` when the resolver doesn't cover a
/// needed type, so the Python side falls through to the real join.
fn conditional_join_inner(
    if_type: &Type,
    else_type: &Type,
    resolver: &TypeResolver,
) -> Option<Vec<u8>> {
    use crate::subtypes::SubtypeContext;

    // Build the subtype context: strict_optional = true (safe default).
    let ctx = SubtypeContext::new(false, false, false, false, false, true);

    // Re-use setops::trivial_join which handles the common cases:
    // - subtype check s <: t → return t
    // - subtype check t <: s → return s
    // - Instance right → return object
    match crate::setops::trivial_join(if_type, else_type, &ctx, resolver) {
        Some(crate::setops::SetOpResult::SameS) => encode_type(if_type),
        Some(crate::setops::SetOpResult::SameT) => encode_type(else_type),
        Some(crate::setops::SetOpResult::Object) => {
            // Return `object` instance as the join.
            encode_type(&Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::Bottom) => {
            // Bottom in a join usually means a type error;
            // fall back to a union of both branches.
            encode_type(&Type::UnionType {
                items: vec![if_type.clone(), else_type.clone()],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        }
        Some(crate::setops::SetOpResult::Any) => {
            // Any is the universal supertype.
            encode_type(&Type::AnyType {
                type_of_any: 0, // TYPE_OF_ANY_SPECIAL_FORM
                source_any: None,
                missing_import_name: None,
            })
        }
        Some(crate::setops::SetOpResult::Ancestor(fullname)) => {
            // Nominal join found a common ancestor.
            encode_type(&Type::Instance {
                type_ref: fullname,
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        }) => {
            // Same-type Instance-Instance join with per-arg discriminators.
            // For simplicity in the conditional expr context, we return
            // the first branch's type with Any args where discriminators
            // differ from both.
            let final_args: Vec<Type> = arg_discs
                .iter()
                .map(|&d| match d {
                    0 => if_type.clone(),   // use left arg
                    1 => else_type.clone(), // use right arg
                    _ => Type::AnyType {
                        type_of_any: 0,
                        source_any: None,
                        missing_import_name: None,
                    },
                })
                .collect();
            encode_type(&Type::Instance {
                type_ref,
                args: final_args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::Encoded(bytes)) => Some(bytes),
        None => {
            // trivial_join returned None for an unsupported shape;
            // fall back to a union of both branches (Python's default).
            encode_type(&Type::UnionType {
                items: vec![if_type.clone(), else_type.clone()],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Container literal fast paths (issue #385)
// ---------------------------------------------------------------------------

/// `rust_container_type`: join + node construction for list/set/dict literal types.
///
/// Receives a tag ("list", "set", "dict") + already-serialized item types.
/// Returns the container node serialized, or None to defer.
///
/// For list/set: `elements` is a flat list of serialized item types.
/// For dict: `elements` is a flat list where the first `n_keys` elements
/// are keys and the rest are values (Python passes the true deduped key
/// count; key/value lists can differ in length after dedup).
#[pyfunction]
#[pyo3(signature = (resolver, tag, elements, _ctx, n_keys))]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_container_type<'py>(
    py: Python<'py>,
    resolver: &NativeTypeResolver,
    tag: &str,
    elements: Vec<Vec<u8>>,
    _ctx: Option<Vec<u8>>,
    n_keys: i64,
) -> PyResult<Option<&'py PyBytes>> {
    let original_len = elements.len();
    // Decode all items; if any fail, treat as unsupported → defer.
    let items: Vec<Type> = elements
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None); // decode failure → defer to Python
    }

    match tag {
        "list" | "set" => {
            let container_fullname = match tag {
                "list" => "builtins.list",
                "set" => "builtins.set",
                _ => unreachable!(),
            };

            let vt = first_or_join_fast_item_inner(&items, resolver);
            match vt {
                None => Ok(None),
                Some(vt) => {
                    let t = Type::Instance {
                        type_ref: container_fullname.to_string(),
                        // Python's `named_generic_type` strips LKV from
                        // container args: `set(Literal['x']?)` is
                        // `set[str]`, so mirror that strip here.
                        args: vec![strip_lkv(&vt)],
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    match encode_type(&t) {
                        Some(b) => Ok(Some(PyBytes::new(py, &b))),
                        None => Ok(None),
                    }
                }
            }
        }
        "dict" => match build_dict_type(resolver, &items, n_keys) {
            None => Ok(None),
            Some(bytes) => Ok(Some(PyBytes::new(py, &bytes))),
        },
        _ => Ok(None),
    }
}

/// Strip `last_known_value` from a wire `Type`, mirroring
/// Python's `remove_instance_last_known_values`.
fn strip_lkv(t: &Type) -> Type {
    match t {
        Type::Instance {
            type_ref,
            args,
            last_known_value: Some(_),
            extra_attrs,
        } => Type::Instance {
            type_ref: type_ref.clone(),
            args: args.clone(),
            last_known_value: None,
            extra_attrs: extra_attrs.clone(),
        },
        _ => t.clone(),
    }
}

/// Build a dict type from a flat list of key/value types.
/// The first `n_keys` elements are the deduplicated keys, the rest are
/// values. Python (`fast_dict_type`) passes the true key count, not half
/// the element count: dedup can make key/value lists unequal lengths, and
/// splitting at `n/2` mis-feeds keys into the values list (issue #385).
fn build_dict_type(
    resolver: &NativeTypeResolver,
    elements: &[Type],
    n_keys: i64,
) -> Option<Vec<u8>> {
    if elements.is_empty() {
        return None;
    }
    if n_keys < 0 || n_keys as usize > elements.len() || n_keys >= elements.len() as i64 {
        // Must have at least one key and one value.
        return None;
    }
    let keys: Vec<Type> = elements[..n_keys as usize].to_vec();
    let values: Vec<Type> = elements[n_keys as usize..].to_vec();
    if keys.is_empty() || values.is_empty() {
        return None;
    }

    let kt = first_or_join_fast_item_inner(&keys, resolver)?;
    let vt = first_or_join_fast_item_inner(&values, resolver)?;

    encode_type(&Type::Instance {
        type_ref: "builtins.dict".to_string(),
        // Python's `named_generic_type` strips LKV from dict args too;
        // mirror that on both key and value.
        args: vec![strip_lkv(&kt), strip_lkv(&vt)],
        last_known_value: None,
        extra_attrs: None,
    })
}

/// Join a list of types, mirroring `join.join_type_list`.
fn join_type_list_inner(items: &[Type], resolver: &NativeTypeResolver) -> Option<Type> {
    if items.is_empty() {
        return None;
    }
    if items.len() == 1 {
        return Some(items[0].clone());
    }
    let ctx = crate::subtypes::SubtypeContext::new(false, false, false, false, false, true);
    let mut result = items[0].clone();
    for item in &items[1..] {
        let joined = crate::setops::join_types(&result, item, &ctx, resolver.resolver());
        match joined {
            Some(crate::setops::SetOpResult::SameS) => {}
            Some(crate::setops::SetOpResult::SameT) => result = item.clone(),
            Some(crate::setops::SetOpResult::Object) => {
                result = Type::Instance {
                    type_ref: "builtins.object".to_string(),
                    args: vec![],
                    last_known_value: None,
                    extra_attrs: None,
                };
            }
            Some(crate::setops::SetOpResult::Bottom) => {
                result = Type::UnionType {
                    items: vec![result.clone(), item.clone()],
                    uses_pep604_syntax: false,
                    can_be_true: true,
                    can_be_false: true,
                };
            }
            Some(crate::setops::SetOpResult::Any) => {
                result = Type::AnyType {
                    type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
                    source_any: None,
                    missing_import_name: None,
                };
            }
            Some(crate::setops::SetOpResult::Ancestor(fullname)) => {
                result = Type::Instance {
                    type_ref: fullname,
                    args: vec![],
                    last_known_value: None,
                    extra_attrs: None,
                };
            }
            Some(crate::setops::SetOpResult::SameTypeWithArgs {
                type_ref,
                arg_discs,
            }) => {
                let final_args: Vec<Type> = arg_discs
                    .iter()
                    .map(|&d| match d {
                        0 => result.clone(),
                        1 => item.clone(),
                        _ => Type::AnyType {
                            type_of_any: 0,
                            source_any: None,
                            missing_import_name: None,
                        },
                    })
                    .collect();
                result = Type::Instance {
                    type_ref,
                    args: final_args,
                    last_known_value: None,
                    extra_attrs: None,
                };
            }
            Some(crate::setops::SetOpResult::Encoded(bytes)) => {
                // Encoded carries a serialized Type; decode it into the join.
                let decoded = decode_type(&bytes)?;
                result = decoded;
            }
            None => return None,
        }
    }
    Some(result)
}

/// `_first_or_join_fast_item` inner: mirroring the Python version.
fn first_or_join_fast_item_inner(items: &[Type], resolver: &NativeTypeResolver) -> Option<Type> {
    if items.len() == 1 {
        if is_type_obj_callable(&items[0], resolver.resolver()) {
            return None;
        }
        return Some(items[0].clone());
    }
    let typ = join_type_list_inner(items, resolver);
    let joined = typ?;
    if items
        .iter()
        .any(|item| is_type_obj_callable(item, resolver.resolver()))
    {
        return None;
    }
    match allow_fast_container_literal_inner(&joined) {
        Some(true) => Some(joined),
        _ => None,
    }
}

/// `rust_tuple_context_matches`: pure function matching `tuple_context_matches`.
///
/// `elements_tags`: sequence of ints; 0 for non-star items, 1 for star items
/// (there may be several). The kernel counts stars and derives the first
/// star's positional index from these tags.
///
/// `ctx_bytes`: serialized TupleType context.
///
/// Returns Some(true) if the context matches, Some(false) if it doesn't,
/// or None for unsupported shapes (TypeAliasType context).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_tuple_context_matches(
    elements_tags: Vec<i64>,
    ctx_bytes: Vec<u8>,
) -> PyResult<Option<bool>> {
    let ctx = match decode_type(&ctx_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(tuple_context_matches_inner(&elements_tags, &ctx))
}

fn tuple_context_matches_inner(elements_tags: &[i64], ctx: &Type) -> Option<bool> {
    let ctx = match ctx {
        Type::TupleType { items, .. } => items,
        Type::TypeAliasType { .. } => return None,
        _ => return Some(false), // Not a TupleType context
    };

    let has_unpack_in_ctx = find_unpack_in_list_inner(ctx);
    let non_star_count = elements_tags.iter().filter(|&&t| t == 0).count();

    if has_unpack_in_ctx.is_none() {
        // Fixed tuple context: accept if non-star count <= len(ctx.items)
        Some(non_star_count <= ctx.len())
    } else {
        // Variadic context: need exactly one star and exact structure match
        let star_count = elements_tags.iter().filter(|&&t| t == 1).count();
        if star_count != 1 {
            return Some(false);
        }
        let total = elements_tags.len();
        // ctx_unpack_index == expr_star_index (position of the single star)
        let expr_star_index = elements_tags.iter().position(|&t| t == 1).unwrap();
        Some(total == ctx.len() && find_unpack_in_list_inner(ctx) == Some(expr_star_index))
    }
}

/// Find UnpackType in a list, mirroring `find_unpack_in_list`.
fn find_unpack_in_list_inner(items: &[Type]) -> Option<usize> {
    items
        .iter()
        .position(|it| matches!(it, Type::UnpackType { .. }))
}

/// `rust_build_tuple_type`: build the final TupleType node.
///
/// `items_bytes`: serialized list of element types (the already-computed
/// items from `self.accept` calls).
///
/// `seen_unpack`: whether an unpack was encountered (True/False as i64).
///
/// Returns the serialized TupleType, or None for unsupported shapes.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_build_tuple_type<'py>(
    py: Python<'py>,
    items_bytes: Vec<Vec<u8>>,
    seen_unpack: i64,
) -> PyResult<Option<&'py PyBytes>> {
    let original_len = items_bytes.len();
    // Decode all items; if any fail, treat as unsupported → defer.
    let items: Vec<Type> = items_bytes
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None); // decode failure → defer to Python
    }

    let fallback_item = Type::AnyType {
        type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
        source_any: None,
        missing_import_name: None,
    };

    let fallback = Type::Instance {
        type_ref: "builtins.tuple".to_string(),
        args: vec![fallback_item],
        last_known_value: None,
        extra_attrs: None,
    };

    let result = Type::TupleType {
        partial_fallback: Box::new(fallback),
        items,
        implicit: false,
    };

    if seen_unpack == 1 {
        // Python: `result = expand_type(result, {})` (checkexpr.py:6033-6035)
        // splices UnpackType into the tuple (e.g. `(*a,)` with float...). The
        // Rust item loop does not model that, so defer single-unpack tuples.
        return Ok(None);
    }

    let bytes = encode_type(&result).unwrap_or_default();
    Ok(Some(PyBytes::new(py, &bytes)))
}

/// Compute the combined type context for the right operand of a boolean
/// `and`/`or` expression. Mirrors `ExpressionChecker._combined_context`.
///
/// The caller (Python) serializes the branch-derived type (`ty`, the
/// left operand's type after narrowing) and the outer type context
/// (`self.type_context[-1]`). Either may be absent.
///
/// Semantics match the Python original:
/// * branch-derived type containing Any is contagious: return it
///   verbatim (a union would lose the Any). TypeAliasType input defers
///   (wire cannot serialize aliases); Python's `has_any_type` get_proper
///   expansion then applies.
/// * otherwise the combined context is `make_simplified_union` of the
///   present items (branch type and/or outer context).
/// * no items at all -> None (Python returns None too; serializing
///   nothing costs nothing).
///
/// Returns None on any untranslatable shape; the caller falls back to
/// the pure-Python implementation.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_analyze_cond_branch(
    resolver: &NativeTypeResolver,
    branch_bytes: Option<Vec<u8>>,
    known_bytes: Option<Vec<u8>>,
) -> PyResult<Option<Vec<u8>>> {
    let branch = match branch_bytes {
        Some(bytes) => match decode_type(&bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    let known = match known_bytes {
        Some(bytes) => match decode_type(&bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    Ok(combined_context_inner(
        branch.as_ref(),
        known.as_ref(),
        resolver.resolver(),
    ))
}

fn combined_context_inner(
    branch: Option<&Type>,
    known: Option<&Type>,
    resolver: &TypeResolver,
) -> Option<Vec<u8>> {
    use crate::subtypes::SubtypeContext;

    // Any is contagious: `dict[str, Any] or <x>` should still infer Any
    // in `x`, so return the branch type directly (no union with it).
    if let Some(b) = branch {
        match has_any_type_inner(b, false) {
            Some(true) => return encode_type(b),
            Some(false) => {}
            None => return None, // TypeAliasType: defer, Python expands.
        }
    }
    let mut items = Vec::with_capacity(2);
    if let Some(b) = branch {
        items.push(b.clone());
    }
    if let Some(k) = known {
        items.push(k.clone());
    }
    if items.is_empty() {
        return None;
    }
    // proper_subtype=True preserves Any/alias items (Python's
    // _remove_redundant_union_items uses is_proper_subtype). Nested
    // union items flatten (step 1), single items fast-path (step 2),
    // aliases defer (flatten rejects them) -> Python get_proper_type.
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let result = crate::setops::make_simplified_union(&items, &ctx, resolver, true)?;
    encode_type(&result)
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

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
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
        }
    }

    #[test]
    fn test_has_any_type_true() {
        assert_eq!(has_any_type_inner(&make_any(2), false), Some(true));
    }

    #[test]
    fn test_has_any_type_special_form_false() {
        assert_eq!(
            has_any_type_inner(&make_any(TYPE_OF_ANY_SPECIAL_FORM), false),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_in_instance() {
        let inst = make_instance("Foo", vec![make_any(2)]);
        assert_eq!(has_any_type_inner(&inst, false), Some(true));
    }

    #[test]
    fn test_has_any_type_false_simple() {
        assert_eq!(
            has_any_type_inner(&make_instance("int", vec![]), false),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(has_any_type_inner(&alias, false), None);
    }

    #[test]
    fn test_has_uninhabited_component_true() {
        assert_eq!(
            has_uninhabited_component_inner(&Type::UninhabitedType { ambiguous: false }),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_false() {
        assert_eq!(
            has_uninhabited_component_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_true() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&Type::UninhabitedType { ambiguous: true }),
            Some(true)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_false_flag() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&Type::UninhabitedType { ambiguous: false }),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_in_union() {
        let u = make_union(vec![
            make_instance("int", vec![]),
            Type::UninhabitedType { ambiguous: true },
        ]);
        assert_eq!(has_ambiguous_uninhabited_component_inner(&u), Some(true));
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_clean_instance() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(has_ambiguous_uninhabited_component_inner(&alias), None);
    }

    #[test]
    fn test_allow_fast_container_literal_instance() {
        assert_eq!(
            allow_fast_container_literal_inner(&make_instance("list", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_allow_fast_container_literal_tuple_all_items() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![]), make_instance("str", vec![])],
            implicit: false,
        };
        assert_eq!(allow_fast_container_literal_inner(&tup), Some(true));
    }

    #[test]
    fn test_allow_fast_container_literal_tuple_with_non_instance() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![]), make_union(vec![])],
            implicit: false,
        };
        assert_eq!(allow_fast_container_literal_inner(&tup), Some(false));
    }

    #[test]
    fn test_allow_fast_container_literal_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(allow_fast_container_literal_inner(&alias), None);
    }

    #[test]
    fn test_has_bytes_component_true() {
        assert_eq!(
            has_bytes_component_inner(&make_instance("builtins.bytes", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_has_bytes_component_false() {
        assert_eq!(
            has_bytes_component_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_bytes_component_in_union() {
        let u = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.bytes", vec![]),
        ]);
        assert_eq!(has_bytes_component_inner(&u), Some(true));
    }

    #[test]
    fn test_has_bool_item_true() {
        assert_eq!(
            has_bool_item_inner(&make_instance("builtins.bool", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_has_bool_item_false() {
        assert_eq!(
            has_bool_item_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_bool_item_in_union() {
        let u = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.bool", vec![]),
        ]);
        assert_eq!(has_bool_item_inner(&u), Some(true));
    }

    #[test]
    fn test_is_non_empty_tuple_true() {
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![])],
            implicit: false,
        };
        assert_eq!(is_non_empty_tuple_inner(&t), Some(true));
    }

    #[test]
    fn test_is_non_empty_tuple_false_empty() {
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![],
            implicit: false,
        };
        assert_eq!(is_non_empty_tuple_inner(&t), Some(false));
    }

    #[test]
    fn test_is_non_empty_tuple_false_non_tuple() {
        assert_eq!(
            is_non_empty_tuple_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_coroutine_decorator_true() {
        assert_eq!(
            has_coroutine_decorator_inner(&make_instance("typing.AwaitableGenerator", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_has_coroutine_decorator_false() {
        assert_eq!(
            has_coroutine_decorator_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_async_def_plain_coroutine() {
        assert_eq!(
            is_async_def_inner(&make_instance("typing.Coroutine", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_is_async_def_awaitable_generator() {
        // AwaitableGenerator[Any, Any, Any, Coroutine] unwraps args[3].
        let inner = make_instance("typing.Coroutine", vec![]);
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                inner,
            ],
        );
        assert_eq!(is_async_def_inner(&gen), Some(true));
    }

    #[test]
    fn test_is_async_def_awaitable_generator_non_coroutine() {
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("builtins.list", vec![]),
            ],
        );
        assert_eq!(is_async_def_inner(&gen), Some(false));
    }

    #[test]
    fn test_is_async_def_awaitable_generator_few_args() {
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![make_instance("int", vec![])],
        );
        assert_eq!(is_async_def_inner(&gen), Some(false));
    }

    #[test]
    fn test_is_async_def_false_non_coroutine() {
        assert_eq!(
            is_async_def_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_async_def_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(is_async_def_inner(&alias), None);
    }

    #[test]
    fn test_is_private_true() {
        assert!(rust_is_private("__foo").unwrap());
    }

    #[test]
    fn test_is_private_false_dunder() {
        assert!(!rust_is_private("__foo__").unwrap());
    }

    #[test]
    fn test_is_private_false_single_underscore() {
        assert!(!rust_is_private("_foo").unwrap());
    }

    #[test]
    fn test_is_operator_method_true() {
        assert!(rust_is_operator_method(Some("builtins.int.__add__")).unwrap());
    }

    #[test]
    fn test_is_operator_method_false() {
        assert!(!rust_is_operator_method(Some("builtins.int.foo")).unwrap());
    }

    #[test]
    fn test_is_operator_method_none() {
        assert!(!rust_is_operator_method(None).unwrap());
    }

    #[test]
    fn test_is_type_type_context_true() {
        let t = Type::TypeType {
            item: Box::new(make_instance("int", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_type_type_context_inner(&t), Some(true));
    }

    #[test]
    fn test_is_type_type_context_false() {
        assert_eq!(
            is_type_type_context_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_typeddict_type_context_true() {
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(is_typeddict_type_context_inner(&t), Some(true));
    }

    #[test]
    fn test_is_typeddict_type_context_false() {
        assert_eq!(
            is_typeddict_type_context_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_string_literal_true() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.str", vec![])),
            value: LiteralValue::Str("hello".to_string()),
        };
        assert_eq!(is_string_literal_inner(&lit), Some(true));
    }

    #[test]
    fn test_is_string_literal_false_int() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(42),
        };
        assert_eq!(is_string_literal_inner(&lit), Some(false));
    }

    // -- is_duplicate_mapping --

    fn make_typeddict() -> Type {
        Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        }
    }

    #[test]
    fn test_is_duplicate_mapping_single_false() {
        let kinds = vec![ARG_POS];
        assert_eq!(
            is_duplicate_mapping_inner(&[0], &[make_instance("int", vec![])], &kinds),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_empty_false() {
        assert_eq!(is_duplicate_mapping_inner(&[], &[], &[]), Some(false));
    }

    #[test]
    fn test_is_duplicate_mapping_star_kwargs_exception_false() {
        let kinds = vec![ARG_STAR, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_two_star2() {
        // Two **kwargs, both non-TypedDict: allowed (no duplicate).
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_star2_typeddict_true() {
        // Two **kwargs, but one is a TypedDict: duplicate is real.
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_typeddict()];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_two_pos_true() {
        let kinds = vec![ARG_POS, ARG_POS];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_pos_does_not_touch_alias() {
        // A non-`**kwargs` actual short-circuits the `all(...)`: the alias
        // at index 1 is never resolved, matching Python's short-circuit.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let kinds = vec![ARG_POS, ARG_POS];
        let types = vec![make_instance("int", vec![]), alias];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_alias_defers() {
        // Both actuals are `**kwargs`, so `get_proper_type` runs on each;
        // the wire format has no alias target, so the result defers.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), alias];
        assert_eq!(is_duplicate_mapping_inner(&[0, 1], &types, &kinds), None);
    }

    #[test]
    fn test_is_duplicate_mapping_out_of_range_defers() {
        let kinds = vec![ARG_POS];
        let types = vec![make_instance("int", vec![])];
        assert_eq!(is_duplicate_mapping_inner(&[0, 5], &types, &kinds), None);
    }

    // -- method_fullname --

    fn make_type_obj(instance: Type) -> Type {
        // A class object callable: fallback is `builtins.type` (metaclass),
        // ret_type is the constructed instance.
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.type", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(instance),
            name: Some("A".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn make_typet(item: Type) -> Type {
        Type::TypeType {
            item: Box::new(item),
            is_type_form: false,
        }
    }

    fn make_tuple(partial_fallback: Type, items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(partial_fallback),
            items,
            implicit: false,
        }
    }

    fn empty_resolver() -> TypeResolver {
        TypeResolver::new()
    }

    fn containing_resolver() -> TypeResolver {
        // A -> (B defines "foo"), C -> B; "foo" sits on B.
        let mut r = TypeResolver::new();
        let mut b = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.B".to_string(),
            name: "B".to_string(),
            ..Default::default()
        };
        b.mro.push("mod.B".to_string());
        b.member_info.insert("foo".to_string(), (false, true));
        r.insert("mod.B".to_string(), b);
        let mut c = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.C".to_string(),
            name: "C".to_string(),
            ..Default::default()
        };
        c.mro.push("mod.C".to_string());
        c.mro.push("mod.B".to_string());
        r.insert("mod.C".to_string(), c);
        r
    }

    #[test]
    fn test_method_fullname_instance() {
        let t = make_instance("mod.A", vec![]);
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_unwraps_instance() {
        let t = make_type_obj(make_instance("mod.A", vec![]));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_with_explicit_instance_type() {
        // instance_type takes precedence over ret_type.
        let mut t = make_type_obj(make_instance("mod.Ret", vec![]));
        let Type::CallableType { instance_type, .. } = &mut t else {
            unreachable!();
        };
        *instance_type = Some(Box::new(make_instance("mod.Inst", vec![])));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.Inst.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_uninhabited_ret_defers() {
        let t = make_type_obj(Type::UninhabitedType { ambiguous: false });
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_callable_non_type_obj_defers() {
        // Plain callable (fallback is function, not a metaclass): defers.
        let t = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int", vec![])),
            name: Some("f".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_type_type_unwraps_item() {
        let t = make_typet(make_instance("mod.A", vec![]));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_plain() {
        let t = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![make_instance("int", vec![])],
        );
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("builtins.tuple.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_named() {
        // Named tuples return partial_fallback directly (non-tuple info).
        let t = make_tuple(make_instance("collections.NamedTuple", vec![]), vec![]);
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("collections.NamedTuple.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_with_unpack_defers() {
        // tuple_fallback raises NotImplementedError for unpacked items.
        let t = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![Type::UnpackType {
                typ: Box::new(make_instance("builtins.tuple", vec![])),
            }],
        );
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_typeddict_containing() {
        let r = containing_resolver();
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("mod.C", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(
            method_fullname_inner(&t, "foo", &r),
            Some("mod.B.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_literal_fallback_containing() {
        let r = containing_resolver();
        let t = Type::LiteralType {
            fallback: Box::new(make_instance("mod.C", vec![])),
            value: LiteralValue::Int(1),
        };
        assert_eq!(
            method_fullname_inner(&t, "foo", &r),
            Some("mod.B.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(
            method_fullname_inner(&alias, "foo", &empty_resolver()),
            None
        );
    }

    #[test]
    fn test_method_fullname_missing_containing_defers() {
        // Fallback info not in the resolver: defer to Python.
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("mod.Nope", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_union_defers() {
        let t = make_union(vec![make_instance("mod.A", vec![])]);
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_combined_context_no_items_defers() {
        assert_eq!(combined_context_inner(None, None, &empty_resolver()), None);
    }

    #[test]
    fn test_combined_context_any_contagion_returns_branch() {
        // `dict[str, Any] or <x>`: the branch type contains Any, so it
        // is returned verbatim (never unioned with the known context).
        let any = make_any(TYPE_OF_ANY_UNANNOTATED);
        let known = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(Some(&any), Some(&known), &empty_resolver());
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, any);
    }

    #[test]
    fn test_combined_context_special_form_any_not_contagious() {
        // AnyType with type_of_any == special form is not "any" here.
        let any = make_any(TYPE_OF_ANY_SPECIAL_FORM);
        let known = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(Some(&any), Some(&known), &empty_resolver()).unwrap();
        // make_simplified_union([special-any, str]): proper-subtype dedup
        // decides both directions (str <: Any is False under proper, Any <:
        // str is False), so both items survive as a 2-item union. The
        // special-form Any is not contagious, matching Python's
        // _remove_redundant_union_items with an empty resolver.
        let union: Vec<Type> = match decode_type(&out).unwrap() {
            Type::UnionType { items, .. } => items,
            other => panic!("expected union, got {other:?}"),
        };
        assert_eq!(union.len(), 2);
        assert_eq!(union[0], any);
        assert_eq!(union[1], known);
    }

    #[test]
    fn test_combined_context_single_branch_fast_path() {
        let int = make_instance("builtins.int", vec![]);
        let out = combined_context_inner(Some(&int), None, &empty_resolver());
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, int);
    }

    #[test]
    fn test_combined_context_known_only_fast_path() {
        let str_inst = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(Some(&str_inst), None, &empty_resolver());
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, str_inst);
    }

    #[test]
    fn test_combined_context_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        // has_any_type_inner(alias) returns None -> defer to Python.
        assert_eq!(
            combined_context_inner(Some(&alias), None, &empty_resolver()),
            None
        );
    }
}
