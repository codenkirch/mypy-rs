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

use crate::operators::is_operator_method_name;
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
    if matches!(typ, Type::UninhabitedType) {
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
// flatten_types_if_tuple
// ---------------------------------------------------------------------------

/// `mypy.checker.flatten_types_if_tuple` — flatten a nested sequence of
/// tuples into one list of types.
///
/// Mirrors `flatten_types_if_tuple` (checker.py:9087-9097). Defers on alias.
/// Returns a list of wire-format type bytes.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_flatten_types_if_tuple(type_bytes: &[u8]) -> PyResult<Option<Vec<Vec<u8>>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = match flatten_types_if_tuple_inner(&typ) {
        Some(r) => r,
        None => return Ok(None),
    };
    let encoded: Vec<Vec<u8>> = result.iter().filter_map(encode_type).collect();
    Ok(Some(encoded))
}

pub(crate) fn flatten_types_if_tuple_inner(typ: &Type) -> Option<Vec<Type>> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::UnionType { items, .. } => {
            // Flatten each item, then wrap in a single-element union.
            let mut flat: Vec<Type> = Vec::new();
            for t in items {
                flat.extend(flatten_types_if_tuple_inner(t)?);
            }
            Some(vec![Type::UnionType {
                items: flat,
                uses_pep604_syntax: false,
            }])
        }
        Type::TupleType { items, .. } => {
            let mut flat: Vec<Type> = Vec::new();
            for t in items {
                flat.extend(flatten_types_if_tuple_inner(t)?);
            }
            Some(flat)
        }
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
            // is_named_instance(t, "builtins.tuple") -> return [t.args[0]]
            Some(args.first().cloned().map(|t| vec![t]).unwrap_or_default())
        }
        _ => Some(vec![proper.clone()]),
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
            has_uninhabited_component_inner(&Type::UninhabitedType),
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

    #[test]
    fn test_flatten_types_if_tuple_simple() {
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![]), make_instance("str", vec![])],
            implicit: false,
        };
        let result = flatten_types_if_tuple_inner(&t).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_flatten_types_if_tuple_non_tuple() {
        let t = make_instance("int", vec![]);
        let result = flatten_types_if_tuple_inner(&t).unwrap();
        assert_eq!(result.len(), 1);
    }
}
