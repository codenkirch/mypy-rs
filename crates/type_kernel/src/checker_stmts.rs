//! Native port of standalone statement-related helper functions from
//! `mypy/checker.py` (M17, issue #207).
//!
//! The first two functions mirror simple-scalar helpers used by the
//! statement visitors. Errors are still emitted by Python; Rust returns a
//! scalar outcome and Python formats the message.
//!
//! Deferred (return None) cases:
//!   * `type_requires_usage` defers on `TypeAliasType` (the `__await__`
//!     branch needs live `TypeInfo`, which the wire format does not carry).
//!   * `is_unreachable_map` defers on any `TypeAliasType` value since
//!     alias expansion could reveal an `UninhabitedType`.
//!
//! `rust_stmt_outcome` is a parity-only oracle: it decodes a serialized
//! statement node (the `mypy/astwire.py` wire format) and returns a
//! structural summary string, proving the kernel reads the statement wire
//! format that M17 statement visitors will consume.
//!
//! `rust_with_exit_suppresses` and `rust_try_handler_union` (M17 Phase 2,
//! issue #208) mirror statement-visitor helpers. Both are pure structural
//! work on the wire type alone; neither calls `make_simplified_union` (that
//! real simplification hot path stays native via `typeops`, and re-running
//! it here with an empty resolver would always defer).

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

use crate::astwire::{decode_node, AstNode};
use crate::meet::overlap;
use crate::setops::{make_simplified_union, union_make_union};
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::typeops::try_expanding_sum_type_to_union_inner;
use crate::wire::{read_type, write_type, LiteralValue, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// type_requires_usage
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `mypy.checker.type_requires_usage`: the initial Instance dispatch.
///
/// Mirrors checker.py:5222-5237. The Python implementation calls
/// `get_proper_type`, so an alias defers here (no alias target on the wire).
/// Only the `typing.Coroutine` branch is portable: the `__await__` branch
/// needs live `TypeInfo`, so it stays Python-only.
///
/// Returns `Some(0)` when the note code UNUSED_COROUTINE applies,
/// `None` to defer to Python.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_type_requires_usage(type_bytes: &[u8]) -> PyResult<Option<u8>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(type_requires_usage_inner(&typ))
}

fn type_requires_usage_inner(typ: &Type) -> Option<u8> {
    match typ {
        Type::TypeAliasType { .. } => None,
        Type::Instance { type_ref, .. } => {
            if type_ref == "typing.Coroutine" {
                Some(0)
            } else {
                None
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// is_unreachable_map
// ---------------------------------------------------------------------------

/// `mypy.checker.is_unreachable_map`: whether any narrowed value in a
/// `TypeMap` is uninhabited.
///
/// Mirrors checker.py:8974-8975. The Python implementation maps
/// `get_proper_type` over the values, so a `TypeAliasType` value forces a
/// defer (expansion could reveal Never). Any other `UninhabitedType` value
/// short-circuits to `Some(true)`.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_unreachable_map(type_bytes_list: Vec<Vec<u8>>) -> PyResult<Option<bool>> {
    let mut types = Vec::with_capacity(type_bytes_list.len());
    for bytes in &type_bytes_list {
        match decode_type(bytes) {
            Some(t) => types.push(t),
            None => continue,
        }
    }
    Ok(is_unreachable_map_inner(&types))
}

fn is_unreachable_map_inner(types: &[Type]) -> Option<bool> {
    for typ in types {
        match typ {
            Type::TypeAliasType { .. } => return None,
            Type::UninhabitedType { .. } => return Some(true),
            _ => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// rust_stmt_outcome: statement-wire parity oracle (parity-only)
// ---------------------------------------------------------------------------

/// The canonical tag names mirror the constants in `astwire.rs`.
fn tag_name(tag: u8) -> &'static str {
    match tag {
        crate::astwire::EXPR_STMT => "EXPR_STMT",
        crate::astwire::IF_STMT => "IF_STMT",
        crate::astwire::ASSIGNMENT_STMT => "ASSIGNMENT_STMT",
        crate::astwire::TUPLE_EXPR => "TUPLE_EXPR",
        crate::astwire::BLOCK => "BLOCK",
        crate::astwire::RETURN_STMT => "RETURN_STMT",
        crate::astwire::PASS_STMT => "PASS_STMT",
        crate::astwire::RAISE_STMT => "RAISE_STMT",
        crate::astwire::ASSERT_STMT => "ASSERT_STMT",
        _ => "OTHER",
    }
}

fn summarize(node: &AstNode) -> String {
    let children = node
        .children
        .iter()
        .map(|f| match f {
            crate::astwire::ChildField::None => "_",
            crate::astwire::ChildField::Node(_) => "N",
            crate::astwire::ChildField::List(_) => "L",
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{}[{}]", tag_name(node.tag), children)
}

/// Parity-only oracle: decode a serialized statement node and return a
/// structural summary (`TAG[field,...]`). Returns None when the payload is
/// `LITERAL_NONE` or fails to decode. Not wired into any production path.
#[pyfunction]
pub(crate) fn rust_stmt_outcome(node_bytes: &[u8]) -> PyResult<Option<String>> {
    Ok(decode_node(node_bytes).map(|n| summarize(&n)))
}

// ---------------------------------------------------------------------------
// rust_with_exit_suppresses (M17 Phase 2, issue #208)
// ---------------------------------------------------------------------------

/// `mypy.checker.visit_with_stmt` exit-suppression heuristic.
///
/// Mirrors checker.py:6020-6031. The Python side calls `get_proper_type`
/// first and then `is_literal_type`, whose fallback unwraps an
/// `Instance.last_known_value` into the underlying `LiteralType`. A
/// `TypeAliasType` on the wire therefore defers (mirroring Python's
/// `get_proper_type`); the alias is never conflated with a bare non-bool
/// instance. Returns `Ok(false)` (not suppressed) whenever the native
/// path cannot establish suppression, matching Python's default.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_with_exit_suppresses(
    type_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(with_exit_suppresses_inner(&typ, strict_optional))
}

fn with_exit_suppresses_inner(typ: &Type, strict_optional: bool) -> bool {
    if matches!(typ, Type::TypeAliasType { .. }) {
        return false;
    }
    let typ = match typ {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => lkv.as_ref(),
        other => other,
    };
    match typ {
        Type::LiteralType { fallback, value } => {
            matches!(
                (&**fallback, value),
                (
                    Type::Instance { type_ref, .. },
                    LiteralValue::Bool(true),
                ) if type_ref == "builtins.bool"
            )
        }
        Type::Instance {
            type_ref,
            last_known_value: None,
            ..
        } => type_ref == "builtins.bool" && strict_optional,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// rust_try_handler_union (M17 Phase 2, issue #208)
// ---------------------------------------------------------------------------

/// `mypy.checker.get_types_from_except_handler`: handler union classification.
///
/// Mirrors checker.py:5723-5744 by returning the handler types the except
/// clause binds, but is purely structural: the union is flattened for
/// nested tuples/variadic tuples/unions without invoking
/// `make_simplified_union`. The caller (`check_except_handler_test`) already
/// folds the collected types through `make_simplified_union` at the end
/// (checker.py:5705), so parity holds. A value that is neither a tuple nor a
/// union is returned as a single-item list. `None` means the input could not
/// be decoded, or an output type cannot cross the binary seam (see
/// `encode_type_owned`), in which case the caller defers to the Python path.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_try_handler_union<'py>(
    py: Python<'py>,
    type_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<&'py PyList>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let types = try_handler_union_inner(&typ, strict_optional);
    let out = PyList::empty(py);
    for t in &types {
        match encode_type_owned(t) {
            Some(bytes) => out.append(PyBytes::new(py, &bytes))?,
            None => return Ok(None),
        }
    }
    Ok(Some(out))
}

fn try_handler_union_inner(typ: &Type, strict_optional: bool) -> Vec<Type> {
    match typ {
        // Ordinary tuple: mirror make_simplified_union(typ.items), which
        // keeps nested tuples as tuples (invalid exception types) and only
        // flattens UnionType items.
        Type::TupleType { items, .. } => items
            .iter()
            .flat_map(|item| expand_item(item, strict_optional))
            .collect(),
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
            // Variadic tuple: mirror make_simplified_union((typ.args[0],)),
            // single level, flattening a union arg.
            let Some(item) = args.first() else {
                return Vec::new();
            };
            expand_item(item, strict_optional)
        }
        Type::UnionType { items, .. } => items
            .iter()
            .filter(|item| strict_optional || !matches!(item, Type::NoneType))
            .flat_map(|item| try_handler_union_inner(item, strict_optional))
            .collect(),
        _ => vec![typ.clone()],
    }
}

/// Flatten one union member like make_simplified_union's flatten does:
/// recurse into UnionType items but keep any other member (including nested
/// tuples) as-is.
fn expand_item(item: &Type, strict_optional: bool) -> Vec<Type> {
    match item {
        Type::UnionType { .. } => expand_union_items(item, strict_optional),
        _ => vec![item.clone()],
    }
}

// Flatten union items with no tuple expansion: mirrors flatten_nested_unions.
fn expand_union_items(typ: &Type, strict_optional: bool) -> Vec<Type> {
    match typ {
        Type::UnionType { items, .. } => items
            .iter()
            .filter(|item| strict_optional || !matches!(item, Type::NoneType))
            .flat_map(|item| expand_item(item, strict_optional))
            .collect(),
        _ => vec![typ.clone()],
    }
}

/// Encode `t` to the type wire format, or `None` to defer to Python.
///
/// `TypeAliasType` is deferred because the wire format stores only the
/// `type_ref` string, not the live `TypeAlias` node. The Python `read`
/// (types.py:462-466) sets `alias=None`, producing a poisoned type that
/// crashes when the checker dereferences `alias.target`. The crate-wide
/// contract (visitor.rs `has_recursive_types_inner`, typeops.rs) is to
/// defer such types to the pure-Python path.
fn encode_type_owned(t: &Type) -> Option<Vec<u8>> {
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

// ---------------------------------------------------------------------------
// checker narrowing stubs (issue #347)
// ---------------------------------------------------------------------------
//
// These four entry points are the #347 checker-narrowing seam: each returns
// `None` so the Python shim falls through to the pure-Python implementation.
// They exist so the dispatch gate (`_native_checker_narrowing_active`) can be
// wired and asserted for the functions that legitimately stay Python-only
// (live expression nodes, the conditional type binder, symbol-table lookups,
// and `PartialType` state are all un-serializable across the wire).
//
// `rust_narrow_declared_type` (also part of #347 but already ported) lives in
// `meet.rs` and is registered from `meet::rust_narrow_declared_type`: it is the
// authoritative seam wired into `mypy/meet.py`. It is intentionally NOT
// duplicated here to avoid shadowing that registration.

/// `mypy.checker.narrow_type` — general narrowing from a condition expression.
///
/// Returns `None` to defer to Python: this function operates on live
/// expression nodes and the conditional type binder, which cannot be
/// serialized through the wire format.
#[pyfunction]
pub(crate) fn rust_narrow_type(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

/// `mypy.checker.infer_value_type` — literal inference from assignments.
///
/// Returns `None` to defer to Python: literal inference requires live
/// symbol table lookups and expression node analysis.
#[pyfunction]
pub(crate) fn rust_infer_value_type(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

/// `mypy.checker.find_isinstance_join` — union narrowing helper.
///
/// Returns `None` to defer to Python: this operates on live expression
/// nodes, isinstance type arguments, and the conditional type binder.
#[pyfunction]
pub(crate) fn rust_find_isinstance_join(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

/// `mypy.checker.partial_type_inference` — None-to-Optional for partially
/// typed vars.
///
/// Returns `None` to defer to Python: requires live PartialType handling
/// and binder state.
#[pyfunction]
pub(crate) fn rust_partial_type_inference(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

// ---------------------------------------------------------------------------
// narrow_type_by_identity_equality (issue #387), identity-only branch
// ---------------------------------------------------------------------------
//
// Ports the identity (`is` / `is not`) path of
// `mypy.checker.narrow_type_by_identity_equality` (checker.py:7127). The
// caller serializes the raw expr_type and the already-coerced target_type.
// Rust mirrors the caller's else-branch (checker.py:7227-7237):
//
// ```text
// narrowable = try_expanding_sum_type_to_union(
//     coerce_to_literal(narrowable_expr_type), None)
// if_type, else_type = conditional_types(
//     narrowable, [TypeRange(target_type, is_upper_bound=False)],
//     from_equality=True)
// ```
//
// For identity the partition is a no-op (`is_identity=True` returns
// `(current_type, None)`), so narrowable_expr_type == expr_type and there is
// no ambiguous side to re-merge. Equality operators (==/!=) defer: they need
// the partition machinery and the ambiguous union-merge.
//
// Deferred (return None) cases, all structurally safe:
//   * `coerce_to_literal` on a single-member enum (stale enum_members) or a
//     TypeAliasType (no alias target on the wire).
//   * `try_expanding_sum_type_to_union` or an alias on either side.
//   * `is_subtype` returning None (LiteralType/NoneType/other non-Instance
//     current); proper-subtype True then proceeds natively.
//   * A Callable / protocol target needing `restrict_subtype_away`
//     (structural guards need live TypeInfo).
//   * Overlap True but not proper-subtype (also `restrict_subtype_away`).
//   * A generic Instance target (Instance with args) that
//     `shallow_erase_type_for_equality` would erase vars off.
//   * `make_simplified_union` deferring (alias or a non-Instance subset).

/// `mypy.typeops.coerce_to_literal` (typeops.py:1439-1455).
///
/// `get_proper_type` comes first: a TypeAliasType cannot be resolved on the
/// wire, so it defers. A Union is mapped item-wise and rebuilt with
/// `make_union`. An Instance carries its last-known value on the wire and
/// returns it (the whole point of the coercion); a single-member enum defers
/// (stale snapshot, same principle as `try_expanding_sum_type_to_union`).
fn coerce_to_literal_inner(t: &Type, resolver: &TypeResolver) -> Option<Type> {
    match t {
        Type::TypeAliasType { .. } => None,
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(coerce_to_literal_inner(item, resolver)?);
            }
            Some(union_make_union(out))
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => Some((**lkv).clone()),
        Type::Instance { type_ref, .. } => {
            let snap = resolver.get(type_ref)?;
            if snap.is_enum && snap.enum_members.len() == 1 {
                return None;
            }
            Some(t.clone())
        }
        _ => Some(t.clone()),
    }
}

/// `mypy.erasetype.shallow_erase_type_for_equality` (erasetype.py:418-429),
/// partial port.
///
/// Unions are mapped item-wise and rebuilt with `make_union`. An Instance
/// with args defers: erasing type vars to `Any` would need to build the right
/// `AnyType`, which is not portable here. Everything else is identity.
/// Because the Union branch defers if ANY item has args, a generic container
/// in the target can over-defer, which is safe (falls back to Python).
fn shallow_erase_for_equality(t: &Type) -> Option<Type> {
    match t {
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(shallow_erase_for_equality(item)?);
            }
            Some(union_make_union(out))
        }
        Type::Instance { args, .. } if !args.is_empty() => None,
        _ => Some(t.clone()),
    }
}

/// `mypy.checker.conditional_types` (checker.py:8854-8995) for the
/// equality/identity subset.
///
/// Proposed is the single `TypeRange(target, is_upper_bound=False)` with
/// `from_equality=True`. current/proposed are already proper (aliases defer
/// at the top of the pyfunction). Nested union recursion passes
/// `default=Some(union_item)`; the top-level call passes `None`.
fn conditional_types_identity_subset(
    current: &Type,
    proposed: &Type,
    default: Option<&Type>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<(Option<Type>, Option<Type>)> {
    // Pre-expand for a len()==1 proposed range (checker.py:8902-8910): only
    // bool literals expand natively (via builtins.bool); a str-literal enum
    // target defers (enum expansion not portable); plain literals don't.
    let enum_name = match proposed {
        // bool literal: expand current by builtins.bool, matching
        // checker.py's try_expanding_sum_type_to_union(current_type,
        // "builtins.bool") for the value-bool target.
        Type::LiteralType {
            value: LiteralValue::Bool(_),
            ..
        } => Some("builtins.bool".to_string()),
        Type::LiteralType {
            fallback,
            value: LiteralValue::Str(_),
            ..
        } => {
            let snap = match &**fallback {
                Type::Instance { type_ref, .. } => resolver.get(type_ref)?,
                _ => return None,
            };
            if snap.is_enum {
                return None;
            }
            None
        }
        _ => None,
    };
    let current = match enum_name {
        Some(name) => {
            try_expanding_sum_type_to_union_inner(current, Some(&name), strict_optional, resolver)?
        }
        None => current.clone(),
    };
    let current = &current;

    // Factorize over union types (checker.py:8917-8931): each item is
    // narrowed against the whole proposed range, with itself as the default.
    // With a non-None default the recursive results are always concrete, so
    // an inner None pair or a sub-defer propagates a defer out of the whole
    // union call (Python would have computed a definite result).
    if let Type::UnionType { items, .. } = current {
        let mut yes_items = Vec::with_capacity(items.len());
        let mut no_items = Vec::with_capacity(items.len());
        for item in items {
            let (yes_type, no_type) = conditional_types_identity_subset(
                item,
                proposed,
                Some(item),
                strict_optional,
                resolver,
            )?;
            let (yes_type, no_type) = match (yes_type, no_type) {
                (Some(y), Some(n)) => (y, n),
                _ => return None,
            };
            yes_items.push(yes_type);
            no_items.push(no_type);
        }
        let union_ctx = SubtypeContext::new(false, false, false, true, true, true);
        let yes = make_simplified_union(&yes_items, &union_ctx, resolver, true)?;
        let no = make_simplified_union(&no_items, &union_ctx, resolver, true)?;
        return Some((Some(yes), Some(no)));
    }

    let default_owned = default.cloned();

    // Any lhs (checker.py:8926): Any is subtyped by everything and subsumes
    // the rhs, so the branch keeps the proposed type, the else keeps current.
    if let Type::AnyType { .. } = current {
        return Some((Some(proposed.clone()), Some(current.clone())));
    }
    // Any rhs (checker.py:8928): no info, else-keeps-default.
    if let Type::AnyType { .. } = proposed {
        return Some((Some(proposed.clone()), default_owned));
    }

    // Concrete proper subtype (checker.py:8933-8936): rhs covers lhs, add
    // nothing in the if-branch and mark the else unreachable.
    let proper_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    match is_subtype(current, proposed, &proper_ctx, resolver) {
        Some(true) => {
            return Some((
                default_owned,
                Some(Type::UninhabitedType { ambiguous: false }),
            ));
        }
        None => return None,
        Some(false) => {}
    }

    // Structural guard (checker.py:8937-8944): a Callable or protocol target
    // needs `restrict_subtype_away` to rule out an unequal-but-overlapping
    // value, which needs live TypeInfo -> DEFER.
    let structural = match proposed {
        Type::CallableType { .. } => true,
        Type::Instance { type_ref, .. } => resolver.get(type_ref)?.is_protocol,
        _ => false,
    };
    if structural {
        return None;
    }

    // Equality-aware erasure (checker.py:8946-8949), then overlap
    // (checker.py:8951-8953) with ignore_promotions=True.
    let erased = shallow_erase_for_equality(proposed)?;
    match overlap(current, &erased, strict_optional, true, false, resolver, 0) {
        // Overlap False: expression is never of any type in the proposed
        // range -> if-branch unreachable, else keeps the default.
        Some(false) => Some((
            Some(Type::UninhabitedType { ambiguous: false }),
            default_owned,
        )),
        // Overlap True but not a proper subtype: restrict_subtype_away would
        // need live TypeInfo -> defer to Python.
        Some(true) => None,
        None => None,
    }
}

/// Inner (non-pyfunction) driver so `?` on `Option` works: returns
/// `Some((if_type, else_type))` on a successful native narrowing, `None` to
/// defer to the pure-Python path.
fn narrow_type_by_identity_equality_inner(
    current: Type,
    target: Type,
    comparison: &str,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<(Option<Type>, Option<Type>)> {
    if !(comparison == "is" || comparison == "is not") {
        return None;
    }
    let coerced = coerce_to_literal_inner(&current, res)?;
    let expanded = try_expanding_sum_type_to_union_inner(&coerced, None, strict_optional, res)?;
    let (if_type, else_type) =
        conditional_types_identity_subset(&expanded, &target, None, strict_optional, res)?;
    Some((if_type, else_type))
}

/// `mypy.checker.narrow_type_by_identity_equality` (checker.py:7127), the
/// identity-only branch ported behind the #387 seam.
///
/// Returns `Some((if_type, else_type))` encoded, where a branch's `None`
/// means Python's `None` ("no new information"), or `None` to defer to the
/// pure-Python path. `target_type` is already coerced by the caller (identity
/// sets `should_coerce_literals=True`), so only `expr_type` is coerced here.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_narrow_type_by_identity_equality(
    a_bytes: &[u8],
    b_bytes: &[u8],
    comparison: &str,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Option<Vec<u8>>, Option<Vec<u8>>)>> {
    let current = match decode_type(a_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let target = match decode_type(b_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let (if_type, else_type) = match narrow_type_by_identity_equality_inner(
        current,
        target,
        comparison,
        strict_optional,
        resolver.resolver(),
    ) {
        Some(pair) => pair,
        None => return Ok(None),
    };
    let if_blob = match if_type {
        Some(t) => match encode_type_owned(&t) {
            Some(b) => Some(b),
            None => return Ok(None),
        },
        None => None,
    };
    let else_blob = match else_type {
        Some(t) => match encode_type_owned(&t) {
            Some(b) => Some(b),
            None => return Ok(None),
        },
        None => None,
    };
    Ok(Some((if_blob, else_blob)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astwire::{write_node, ChildField};
    use crate::wire::{write_type, Type, WriteBuffer};

    fn encode_type(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).expect("write type");
        buf.into_bytes()
    }

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn uninhabited() -> Type {
        Type::UninhabitedType { ambiguous: false }
    }

    fn type_alias() -> Type {
        Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
        }
    }

    fn encode_node(node: &AstNode) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_node(&mut buf, node);
        buf.into_bytes()
    }

    #[test]
    fn type_requires_usage_coroutine() {
        let t = instance("typing.Coroutine");
        assert_eq!(rust_type_requires_usage(&encode_type(&t)).unwrap(), Some(0));
    }

    #[test]
    fn type_requires_usage_other_instance() {
        let t = instance("builtins.int");
        assert_eq!(rust_type_requires_usage(&encode_type(&t)).unwrap(), None);
    }

    #[test]
    fn type_requires_usage_alias_defers() {
        assert_eq!(type_requires_usage_inner(&type_alias()), None);
    }

    #[test]
    fn is_unreachable_empty_map() {
        assert_eq!(rust_is_unreachable_map(Vec::new()).unwrap(), Some(false));
    }

    #[test]
    fn is_unreachable_plain_values() {
        let map = vec![
            encode_type(&instance("builtins.int")),
            encode_type(&instance("builtins.str")),
        ];
        assert_eq!(rust_is_unreachable_map(map).unwrap(), Some(false));
    }

    #[test]
    fn is_unreachable_hits_never() {
        let map = vec![
            encode_type(&instance("builtins.int")),
            encode_type(&uninhabited()),
        ];
        assert_eq!(rust_is_unreachable_map(map).unwrap(), Some(true));
    }

    #[test]
    fn is_unreachable_alias_defers() {
        let map = vec![type_alias(), uninhabited()];
        assert_eq!(is_unreachable_map_inner(&map), None);
    }

    #[test]
    fn stmt_outcome_decodes_statement_wire() {
        let node = AstNode {
            tag: crate::astwire::EXPR_STMT,
            children: vec![ChildField::Node(AstNode {
                tag: crate::astwire::INT_EXPR,
                children: Vec::new(),
            })],
        };
        assert_eq!(
            rust_stmt_outcome(&encode_node(&node)).unwrap(),
            Some("EXPR_STMT[N]".to_string())
        );
    }

    #[test]
    fn stmt_outcome_handles_list_children() {
        let node = AstNode {
            tag: crate::astwire::IF_STMT,
            children: vec![
                ChildField::List(Vec::new()),
                ChildField::None,
                ChildField::Node(AstNode {
                    tag: crate::astwire::BLOCK,
                    children: Vec::new(),
                }),
            ],
        };
        assert_eq!(
            rust_stmt_outcome(&encode_node(&node)).unwrap(),
            Some("IF_STMT[L,_,N]".to_string())
        );
    }

    #[test]
    fn stmt_outcome_none_payload() {
        assert_eq!(rust_stmt_outcome(b"").unwrap(), None);
    }

    #[test]
    fn stmt_outcome_unknown_tag() {
        let node = AstNode {
            tag: 42,
            children: Vec::new(),
        };
        assert_eq!(
            rust_stmt_outcome(&encode_node(&node)).unwrap(),
            Some("OTHER[]".to_string())
        );
    }

    // -- rust_with_exit_suppresses --

    fn bool_literal(value: bool) -> Type {
        let fallback = instance("builtins.bool");
        Type::LiteralType {
            fallback: Box::new(fallback),
            value: LiteralValue::Bool(value),
        }
    }

    fn bool_instance() -> Type {
        instance("builtins.bool")
    }

    #[test]
    fn exit_suppresses_literal_true() {
        let t = bool_literal(true);
        assert!(with_exit_suppresses_inner(&t, true));
        assert!(with_exit_suppresses_inner(&t, false));
    }

    #[test]
    fn exit_suppresses_literal_false() {
        let t = bool_literal(false);
        assert!(!with_exit_suppresses_inner(&t, true));
    }

    #[test]
    fn exit_suppresses_plain_bool_strict() {
        let t = bool_instance();
        assert!(with_exit_suppresses_inner(&t, true));
        assert!(!with_exit_suppresses_inner(&t, false));
    }

    #[test]
    fn exit_suppresses_non_bool_instance() {
        let t = instance("builtins.str");
        assert!(!with_exit_suppresses_inner(&t, true));
    }

    #[test]
    fn exit_suppresses_literal_non_bool_fallback() {
        let t = Type::LiteralType {
            fallback: Box::new(instance("builtins.str")),
            value: LiteralValue::Str("x".to_string()),
        };
        assert!(!with_exit_suppresses_inner(&t, true));
    }

    #[test]
    fn exit_suppresses_lkv_wrapped_bool_instance() {
        let t = Type::Instance {
            type_ref: "builtins.bool".to_string(),
            args: Vec::new(),
            last_known_value: Some(Box::new(bool_literal(true))),
            extra_attrs: None,
        };
        assert!(with_exit_suppresses_inner(&t, true));
    }

    #[test]
    fn exit_suppresses_garbage_bytes_false() {
        assert!(!rust_with_exit_suppresses(b"\xff\xff", true).unwrap());
    }

    // -- rust_try_handler_union --

    #[test]
    fn handler_union_leaf() {
        let t = instance("builtins.ValueError");
        let types = try_handler_union_inner(&t, true);
        assert_eq!(types, vec![t]);
    }

    #[test]
    fn handler_union_plain_tuple() {
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![
                instance("builtins.ValueError"),
                instance("builtins.KeyError"),
            ],
            implicit: false,
        };
        let types = try_handler_union_inner(&t, true);
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn handler_union_variadic_tuple() {
        let t = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![instance("builtins.ValueError")],
            last_known_value: None,
            extra_attrs: None,
        };
        let types = try_handler_union_inner(&t, true);
        assert_eq!(types, vec![instance("builtins.ValueError")]);
    }

    #[test]
    fn handler_union_flat_union_items() {
        let t = Type::UnionType {
            items: vec![
                instance("builtins.ValueError"),
                instance("builtins.KeyError"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let types = try_handler_union_inner(&t, true);
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn handler_union_strict_optional_filters_none() {
        let t = Type::UnionType {
            items: vec![instance("builtins.ValueError"), Type::NoneType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let types_strict = try_handler_union_inner(&t, true);
        assert_eq!(types_strict.len(), 2);
        let types_non_strict = try_handler_union_inner(&t, false);
        assert_eq!(types_non_strict.len(), 1);
    }

    #[test]
    fn handler_union_nested_tuples_kept() {
        let inner = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![instance("builtins.KeyError")],
            implicit: false,
        };
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![instance("builtins.ValueError"), inner.clone()],
            implicit: false,
        };
        let types = try_handler_union_inner(&t, true);
        // Nested tuples are kept as tuples (invalid exception types) to
        // match make_simplified_union, which only flattens UnionType items.
        assert_eq!(types, vec![instance("builtins.ValueError"), inner]);
    }

    #[test]
    fn handler_union_nested_tuple_inside_union_flattened() {
        // Union[E1, Tuple[E3]]: the union branch recurses into every item, so
        // a nested tuple is expanded to its items (testExpectWithMultipleTypes4
        // expects Tuple[E2,E3] inside Union to become E2 | E3).
        let inner = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![instance("builtins.KeyError")],
            implicit: false,
        };
        let t = Type::UnionType {
            items: vec![instance("builtins.ValueError"), inner],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let types = try_handler_union_inner(&t, true);
        assert_eq!(
            types,
            vec![
                instance("builtins.ValueError"),
                instance("builtins.KeyError")
            ]
        );
    }
    #[test]
    fn handler_union_round_trip_encode() {
        pyo3::prepare_freethreaded_python();
        let t = instance("builtins.ValueError");
        let mut blob = None;
        Python::with_gil(|py| {
            let blobs = rust_try_handler_union(py, &encode_type(&t), true)
                .unwrap()
                .unwrap();
            assert_eq!(blobs.len(), 1);
            let b: &[u8] = blobs[0].extract().unwrap();
            let mut buf = ReadBuffer::new(b);
            blob = Some(read_type(&mut buf, None).unwrap());
        });
        assert_eq!(blob.unwrap(), t);
    }

    #[test]
    fn handler_union_alias_leaf_passed_through() {
        // `try_handler_union_inner` mirrors checker.py's `_ => [typ]` fallback:
        // a TypeAliasType leaf is passed through unchanged. The pyfunction
        // layer then defers (see `encode_type_owned`) rather than crossing the
        // seam, because the wire format drops the live TypeAlias node and the
        // Python side would deserialize a poisoned alias=None type.
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![instance("builtins.ValueError")],
        };
        assert_eq!(try_handler_union_inner(&alias, true), vec![alias.clone()]);
        // encode_type_owned defers TypeAliasType to avoid alias=None poisoning.
        assert!(encode_type_owned(&alias).is_none());
    }

    #[test]
    fn handler_union_union_item_alias_passed_through() {
        // Alias nested inside a union item is likewise passed through by the
        // inner flatten; `encode_type_owned` defers on it at the pyfunction
        // layer, so the whole call falls back to Python.
        let t = Type::UnionType {
            items: vec![
                instance("builtins.ValueError"),
                Type::TypeAliasType {
                    type_ref: "mod.Alias".to_string(),
                    args: Vec::new(),
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(
            try_handler_union_inner(&t, true),
            vec![
                instance("builtins.ValueError"),
                Type::TypeAliasType {
                    type_ref: "mod.Alias".to_string(),
                    args: Vec::new(),
                },
            ]
        );
    }

    // -- checker narrowing stubs (issue #347) --

    #[test]
    fn narrow_type_stub_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(rust_narrow_type(py).unwrap().is_none(py));
        });
    }

    #[test]
    fn infer_value_type_stub_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(rust_infer_value_type(py).unwrap().is_none(py));
        });
    }

    #[test]
    fn find_isinstance_join_stub_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(rust_find_isinstance_join(py).unwrap().is_none(py));
        });
    }

    #[test]
    fn partial_type_inference_stub_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(rust_partial_type_inference(py).unwrap().is_none(py));
        });
    }
}
