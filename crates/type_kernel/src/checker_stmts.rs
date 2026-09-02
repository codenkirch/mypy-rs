//! Native port of standalone statement-related helper functions from
//! `mypy/checker.py` (M17, issue #207).
//!
//! The first two functions mirror simple-scalar helpers used by the
//! statement visitors. Errors are still emitted by Python; Rust returns a
//! scalar outcome and Python formats the message.
//!
//! Deferred (return None) cases:
//!   * `type_requires_usage` defers on `TypeAliasType` (the alias target
//!     is absent from the wire), and on the `__await__` branch when a
//!     class in the mro is missing from the resolver (cannot distinguish
//!     "absent" from "unknown").
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

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};

use crate::astwire::{decode_node, AstNode};
use crate::callable_compat::is_type_obj;
use crate::checkexpr_functions::expanded_alias_target;
use crate::cond_types::conditional_types_inner;
use crate::erase_typevars::{erase_typevars_inner, make_any};
use crate::setops::union_make_union;
use crate::typeinfo::{read_bool_attr, read_str_list_attr, NativeTypeResolver, TypeResolver};
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
/// Mirrors checker.py:5822-5840. The Python implementation calls
/// `get_proper_type`, so a `TypeAliasType` is expanded via the alias
/// resolver (chain-resolving) before the Instance dispatch. The
/// `typing.Coroutine` branch is decided by fullname; the `__await__`
/// branch mirrors `proper_type.type.get("__await__")` (a truthy
/// `SymbolTableNode` in the mro), which the resolver's `member_info`
/// snapshots carry. Defer (`None`) when any mro class is missing from the
/// resolver (cannot distinguish "absent" from "unknown").
///
/// Returns `Some(0)` when the note code UNUSED_COROUTINE applies,
/// `Some(1)` when UNUSED_AWAITABLE applies, `None` to defer to Python.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_type_requires_usage(
    type_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<u8>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    // The `__await__` branch needs `TypeResolver` member snapshots; the
    // TypeAliasType expansion needs the alias resolver map.
    Ok(type_requires_usage_inner(
        &typ,
        resolver.resolver(),
        resolver.alias_resolver(),
    ))
}

fn type_requires_usage_inner(
    typ: &Type,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<u8> {
    match typ {
        Type::TypeAliasType { .. } => {
            // Expands via the alias resolver chain, mirroring Python's
            // `get_proper_type`. Unexpandable aliases still defer.
            let (target, _, _) = expanded_alias_target(typ, aliases)?;
            type_requires_usage_inner(&target, resolver, aliases)
        }
        Type::Instance { type_ref, .. } => {
            if type_ref == "typing.Coroutine" {
                return Some(0);
            }
            // `type.get("__await__") is not None` mirrors TypeInfo.get
            // (nodes.py:4063): first mro class whose own `names` contains
            // the name. Missing snapshot = defer (absent vs unknown).
            match member_present_by_ref(resolver, type_ref, "__await__") {
                Some(true) => Some(1),
                Some(false) => Some(2),
                None => None,
            }
        }
        // Every other proper type fails Python's `isinstance(proper_type,
        // Instance)` check and yields no note: 2 skips the Python body.
        Type::NoneType
        | Type::AnyType { .. }
        | Type::UninhabitedType { .. }
        | Type::UnionType { .. }
        | Type::TupleType { .. }
        | Type::CallableType { .. }
        | Type::Overloaded { .. }
        | Type::TypeVarType { .. }
        | Type::ParamSpecType { .. }
        | Type::TypeVarTupleType { .. }
        | Type::LiteralType { .. }
        | Type::TypedDictType { .. }
        | Type::ErasedType
        | Type::DeletedType { .. }
        | Type::TypeType { .. }
        | Type::UnpackType { .. }
        | Type::UnboundType { .. }
        | Type::Parameters(_) => Some(2),
    }
}

/// `TypeInfo.get(name)` (nodes.py:4063-4068): walk the mro, returning the
/// first class whose own `names` dict contains `name`. Existence-only via
/// the resolver snapshot's `member_info` (built from `SymbolTableNode`
/// entries). Defer (`None`) when the class or any mro class is missing
/// from the resolver.
fn member_present_by_ref(resolver: &TypeResolver, type_ref: &str, name: &str) -> Option<bool> {
    let snap = resolver.get(type_ref)?;
    for base in &snap.mro {
        let b = resolver.get(base)?;
        if b.member_info.contains_key(name) {
            return Some(true);
        }
    }
    Some(false)
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
            crate::astwire::ChildField::NestedList(_) => "L",
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
// is_valid_inferred_type (issue #445)
// ---------------------------------------------------------------------------

// Ports `mypy.checker.is_valid_inferred_type` (checker.py:9748-9772) and its
// `InvalidInferredTypes` visitor (checker.py:9775-9799). This is a pure
// boolean query on a type tree with no diagnostics, no mutation, and no

// side effects. It is the non-diagnostic helper called from the
// `check_assignment` narrowing path for simple lvalues.

// `InvalidInferredTypes` is a `BoolTypeQuery(ANY_STRATEGY)` with these
// overrides (checker.py:9785-9799):
//   * `visit_uninhabited_type`: returns `t.ambiguous` (base: default=False).

//   * `visit_erased_type`: returns `True` — `ErasedType` IS on the wire
//     (tag 122; Python's `ErasedType.write` + unconditional Rust reader),
//     so a nested erased type ("can happen inside a lambda") reaches FFI.

//   * `visit_type_var`: returns `t.id.is_meta_var()` i.e. `meta_level > 0`
//     (base: query upper_bound + default + values).

//   * `visit_tuple_type`: returns `query_types(t.items)` (base: query items
//     + partial_fallback).

// The top-level `is_valid_inferred_type` first calls `get_proper_type`
// (top-level alias chains expand via the alias snapshot). Then:

//   * `NoneType`: `is_lvalue_final or (not is_lvalue_member and allow_redefinition)`.
//   * `UninhabitedType`: `False`.
//   * Otherwise: `not typ.accept(InvalidInferredTypes())` — note Python runs
//     the query on the ORIGINAL `typ`, not the expanded proper type
//     (checker.py:11744).

// `BoolTypeQuery.visit_type_alias_type` (type_visitor.py:599-616): alias nodes
// inside the query chain-expand via the alias snapshot (revisits guarded by
// seen_aliases); the PEP-695 edge flag is read from the node's own snapshot.

// Deferred (return None) cases:
//   * Undecodable input bytes.

//   * An alias whose snapshot is missing, cycles, or needs a substitution
//     the kernel cannot perform exactly (expanded_alias_target defers).

/// `mypy.checker.is_valid_inferred_type` — pure boolean validity query.
///
/// Returns `Some(bool)` matching the Python result, or `None` to defer to
/// the pure-Python path.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_valid_inferred_type(
    type_bytes: &[u8],
    is_lvalue_final: bool,
    is_lvalue_member: bool,
    allow_redefinition: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_valid_inferred_type_with_aliases(
        &typ,
        is_lvalue_final,
        is_lvalue_member,
        allow_redefinition,
        resolver.alias_resolver(),
    ))
}

#[cfg(test)]
fn is_valid_inferred_type_inner(
    typ: &Type,
    is_lvalue_final: bool,
    is_lvalue_member: bool,
    allow_redefinition: bool,
) -> Option<bool> {
    // Test-facing wrapper: no alias snapshots (tests exercise non-alias
    // shapes; alias behavior is covered by the alias-aware unit tests).
    is_valid_inferred_type_with_aliases(
        typ,
        is_lvalue_final,
        is_lvalue_member,
        allow_redefinition,
        &crate::aliases::TypeAliasResolver::default(),
    )
}

fn is_valid_inferred_type_with_aliases(
    typ: &Type,
    is_lvalue_final: bool,
    is_lvalue_member: bool,
    allow_redefinition: bool,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    // get_proper_type: a top-level TypeAliasType chain-expands via the alias
    // snapshot (mirrors _expand_once). For a non-alias input the original
    // `typ` IS the proper type, so no whole-tree clone is needed anywhere.
    let expanded: Option<Type> = if matches!(typ, Type::TypeAliasType { .. }) {
        Some(expanded_alias_target(typ, aliases)?.0)
    } else {
        None
    };
    let proper: &Type = expanded.as_ref().unwrap_or(typ);
    // Top-level NoneType short-circuit (evaluated on the expanded type).
    if matches!(proper, Type::NoneType) {
        return Some(is_lvalue_final || (!is_lvalue_member && allow_redefinition));
    }
    // Top-level UninhabitedType short-circuit.
    if let Type::UninhabitedType { .. } = proper {
        return Some(false);
    }
    // not typ.accept(InvalidInferredTypes()) — on the ORIGINAL typ.
    // Set-based seen-guard, mirroring type_visitor.py:604-608.
    let mut seen: HashSet<String> = HashSet::new();
    let invalid = invalid_inferred_types_query(typ, aliases, &mut seen)?;
    Some(!invalid)
}

/// `InvalidInferredTypes` query: ANY_STRATEGY (short-circuit OR).
///
/// Returns `Some(true)` if any invalid component is found, `Some(false)`
/// if none, or `None` to defer (an alias node the snapshot cannot expand).
fn invalid_inferred_types_query(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut HashSet<String>,
) -> Option<bool> {
    // Per-variant overrides mirror InvalidInferredTypes.visit_*.
    match typ {
        // visit_uninhabited_type: returns t.ambiguous.
        Type::UninhabitedType { ambiguous } => Some(*ambiguous),
        // visit_erased_type: ErasedType is always invalid (checker.py:
        // "This can happen inside a lambda").
        Type::ErasedType => Some(true),
        // visit_type_var: returns t.id.is_meta_var() (meta_level > 0),
        // NOT the base query of upper_bound/default/values.
        Type::TypeVarType { meta_level, .. } => Some(*meta_level > 0),
        // visit_tuple_type: query_types(t.items) — excludes fallback.
        Type::TupleType { items, .. } => {
            // ANY_STRATEGY: any item invalid -> True. Short-circuit.
            for item in items {
                match invalid_inferred_types_query(item, aliases, seen) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        // BoolTypeQuery.visit_type_alias_type (type_visitor.py:599-616).
        Type::TypeAliasType { .. } => invalid_query_alias_node(typ, aliases, seen),
        // Base BoolTypeQuery(ANY_STRATEGY) for all other variants.
        _ => {
            // default = False for ANY_STRATEGY. Query children.
            for child in invalid_inferred_children(typ) {
                match invalid_inferred_types_query(child, aliases, seen) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
    }
}

/// `BoolTypeQuery.visit_type_alias_type` for the ANY_STRATEGY query
/// (type_visitor.py:599-616): a permanent seen-guard (a revisit yields the
/// default, False), then query the chain-expanded substituted target, then
/// the edge continuation `res or (python_3_12_type_alias and
/// query_types(t.args))`. The flag comes from the node's own alias
/// snapshot, mirroring Python's `t.alias.python_3_12_type_alias` (not
/// OR-ed across the chain).
fn invalid_query_alias_node(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut HashSet<String>,
) -> Option<bool> {
    let (args, type_ref) = match typ {
        Type::TypeAliasType {
            args,
            type_ref,
            is_recursive: _,
        } => (args, type_ref),
        _ => return None,
    };
    if !seen.insert(type_ref.clone()) {
        // Already visited: simple-minded cache, default (False).
        return Some(false);
    }
    // res = get_proper_type(t).accept(self)
    let (target, _, _) = expanded_alias_target(typ, aliases)?;
    let res = invalid_inferred_types_query(&target, aliases, seen)?;
    if res {
        return Some(true);
    }
    // Edge: res or (t.alias.python_3_12_type_alias and query(t.args)).
    let py312 = aliases.get(type_ref)?.python_3_12_type_alias;
    if py312 {
        for a in args {
            if invalid_inferred_types_query(a, aliases, seen)? {
                return Some(true);
            }
        }
    }
    Some(false)
}

/// Yield the child types that `InvalidInferredTypes`'s base visitor would
/// query. This mirrors `BoolTypeQuery.visit_*` for variants not overridden
/// by `InvalidInferredTypes`. `TypeVarType` and `TupleType` are excluded
/// because they have overrides that do not recurse generically.
fn invalid_inferred_children(typ: &Type) -> Vec<&Type> {
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
        Type::TypedDictType {
            items, fallback, ..
        } => {
            out.push(fallback);
            out.extend(items.iter().map(|(_, t)| t));
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::ParamSpecType {
            upper_bound,
            default,
            prefix,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            // Parameters children: arg_types (visit_parameters queries
            // t.arg_types).
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
        }
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        // NoneType, UninhabitedType, AnyType (no source_any), DeletedType,
        // ErasedType, Parameters (standalone): no children.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// checker narrowing stubs (issue #347)
// ---------------------------------------------------------------------------

// These four entry points are the #347 checker-narrowing seam: each returns
// `None` so the Python shim falls through to the pure-Python implementation.

// They exist so the dispatch gate (`_native_checker_narrowing_active`) can be
// wired and asserted for the functions that legitimately stay Python-only
// (live expression nodes, the conditional type binder, symbol-table lookups,

// and `PartialType` state are all un-serializable across the wire).

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
// narrow_type_by_identity_equality (issue #387), identity + equality branch
// ---------------------------------------------------------------------------

// Ports the identity (`is` / `is not`) and equality (`==` / `!=`)
// branches of `mypy.checker.narrow_type_by_identity_equality`
// (checker.py:8963) behind the #387 seam, extended to equality in #1126.

// The caller serializes the raw expr_type and the target_type (already
// coerced when `should_coerce_literals` is set). Rust mirrors the
// caller's fallback else-branch (checker.py:9095-9098):

// narrowable = try_expanding_sum_type_to_union(
//     coerce_to_literal(narrowable_expr_type), None)

// if_type, else_type = conditional_types(
//     narrowable, [TypeRange(target_type, is_upper_bound=False)],
//     from_equality=True)

// For `is`/`is not` the partition is a no-op (`is_identity=True` returns
// `(current_type, None)`), so there is no ambiguous side to re-merge; for
// `==`/`!=` the Python caller already partitioned ambiguous union items

// off (`rust_partition_equality_ambiguous_types`) and re-merges them
// after the seam returns, so both operator families hand the seam the
// same single-range question. The conditional_types computation itself

// delegates to cond_types::conditional_types_inner (the fuller port with
// NewType unwrap, live enum expansion, and the restrict_subtype_away
// tail).

// Deferred (return None) cases, all structurally safe:
// `coerce_to_literal` on a TypeAliasType (no alias target on the wire),
// a single-member enum, or a recursive alias inside a union rebuild.

// `try_expanding_sum_type_to_union` on an alias. Any sub-step of the
// conditional_types port that cannot decide: `is_subtype` returning
// None, a generic Instance target whose args would erase to Any, an

// undecidable `restrict_subtype_away`, a `make_simplified_union` on an
// alias, or an overlap the meet kernel cannot decide.

/// `mypy.typeops.coerce_to_literal` (typeops.py:2000-2024): resolves aliases
/// (a recursive alias defers), maps unions item-wise, returns the Instance
/// last-known value; single-member enum members are read live, never stale.
fn coerce_to_literal_inner(
    t: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    native: &NativeTypeResolver,
    py: Python<'_>,
) -> Option<Type> {
    match t {
        Type::TypeAliasType { .. } => {
            let (target, _, _) = crate::checkexpr_functions::expanded_alias_target(t, aliases)?;
            coerce_to_literal_inner(&target, aliases, native, py)
        }
        Type::UnionType { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(coerce_to_literal_inner(item, aliases, native, py)?);
            }
            Some(union_make_union(out))
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => Some((**lkv).clone()),
        Type::Instance { type_ref, .. } => {
            let info = native.live_typeinfo(py, type_ref)?;
            if read_bool_attr(info, "is_enum").unwrap_or(false) {
                let members = read_str_list_attr(info, "enum_members").unwrap_or_default();
                if members.len() == 1 {
                    return Some(Type::LiteralType {
                        fallback: Box::new(t.clone()),
                        value: LiteralValue::Str(members[0].clone()),
                    });
                }
            }
            Some(t.clone())
        }
        _ => Some(t.clone()),
    }
}

/// Inner (non-pyfunction) driver so `?` on `Option` works: returns
/// `Some((if_type, else_type))` on a successful native narrowing, `None` to
/// defer to the pure-Python path.
fn narrow_type_by_identity_equality_inner(
    py: Python<'_>,
    current: Type,
    target: Type,
    strict_optional: bool,
    native: &NativeTypeResolver,
) -> Option<(Option<Type>, Option<Type>)> {
    let res = native.resolver();
    // Caller's fallback pre-step:
    // `narrowable = try_expanding_sum_type_to_union(coerce_to_literal(narrowable), None)`.
    let coerced = coerce_to_literal_inner(&current, native.alias_resolver(), native, py)?;
    let expanded =
        crate::cond_types::expand_for_target(&coerced, None, strict_optional, native, py)?;
    let range = crate::cond_types::WireRange {
        item: target,
        is_upper_bound: false,
    };
    conditional_types_inner(
        &expanded,
        Some(&[range]),
        None,
        true,
        true,
        strict_optional,
        res,
        native,
        native.alias_resolver(),
        py,
    )
}

/// `mypy.checker.narrow_type_by_identity_equality` (checker.py:8963), the
/// identity + equality branch ported behind the #387 seam.
///
/// Returns `Some((if_type, else_type))` encoded, where a branch's `None`
/// means Python's `None` ("no new information"), or `None` to defer to the
/// pure-Python path. Handles all four comparison operators; anything else
/// defers (callers never pass them).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_narrow_type_by_identity_equality(
    py: Python<'_>,
    a_bytes: &[u8],
    b_bytes: &[u8],
    comparison: &str,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Option<Vec<u8>>, Option<Vec<u8>>)>> {
    if !(comparison == "is" || comparison == "is not" || comparison == "==" || comparison == "!=") {
        return Ok(None);
    }
    let current = match decode_type(a_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let target = match decode_type(b_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let (if_type, else_type) = match narrow_type_by_identity_equality_inner(
        py,
        current,
        target,
        strict_optional,
        resolver,
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
// rust_classify_except_handler_tests (Phase C2, issue #609)
// ---------------------------------------------------------------------------

/// One handler-test classification, mirroring the per-`ttype` dispatch in
/// `mypy.checker.check_except_handler_test` (checker.py:5941-5965).
///
/// Python's loop appends a type for `AnyType` and for a valid exception
/// type, skips `UninhabitedType`, and short-circuits with
/// `INVALID_EXCEPTION_TYPE` for anything else. The classifier below
/// reproduces exactly that dispatch on the wire type, deferring (`None`)
/// only when the class of the result is genuinely unresolvable on the
/// wire (a `TypeAliasType` test — Python `get_proper_types` would expand
/// it — or a type-object whose metaclass is not in the resolver).
#[derive(Debug)]
enum HandlerTestClass {
    /// `AnyType`: Python appends the type as-is (tag 0).
    Any(Type),
    /// `UninhabitedType`: Python continues without appending (tag 1).
    Skip,
    /// Not a valid exception type: Python fails + returns default (tag 2).
    Invalid,
    /// A valid exception type derived from `FunctionLike` / `TypeType`
    /// (tag 3). Python still runs the `is_subtype(...BaseException)` fence.
    Exc(Type),
}

/// Wire `get_proper_type`: a `TypeAliasType` has no alias target on the
/// wire, so it defers. Every other type is already proper.
fn wire_get_proper_type(t: &Type) -> Option<Type> {
    match t {
        Type::TypeAliasType { .. } => None,
        _ => Some(t.clone()),
    }
}

/// The `FunctionLike` exception-type branch (checker.py:5952-5964):
/// `is_type_obj` on `items[0]`, then `exc_type = erase_typevars(
/// item.get_instance_type())`.
fn exception_type_of_type_obj(
    callable: &Type,
    resolver: &TypeResolver,
) -> Option<HandlerTestClass> {
    match callable {
        Type::Overloaded { items } => match items.first() {
            Some(first) => exception_type_of_callable(first, resolver),
            None => Some(HandlerTestClass::Invalid),
        },
        Type::CallableType { .. } => exception_type_of_callable(callable, resolver),
        _ => Some(HandlerTestClass::Invalid),
    }
}

fn exception_type_of_callable(first: &Type, resolver: &TypeResolver) -> Option<HandlerTestClass> {
    match is_type_obj(first, resolver) {
        None => None,
        Some(false) => Some(HandlerTestClass::Invalid),
        Some(true) => {
            let Type::CallableType {
                instance_type,
                ret_type,
                ..
            } = first
            else {
                return Some(HandlerTestClass::Invalid);
            };
            // Python `get_instance_type()` (force_fallback=False):
            // `instance_type` if set, else `get_proper_type(ret_type)`.
            let exc = match instance_type {
                Some(it) => Some((**it).clone()),
                None => wire_get_proper_type(ret_type),
            }?;
            let erased = erase_typevars_inner(&exc, None, &make_any())?;
            Some(HandlerTestClass::Exc(erased))
        }
    }
}

/// Classify a single handler test type (checker.py:5941-5965), or `None`
/// to defer the whole native call to the pure-Python path.
fn classify_except_handler_test_inner(
    t: &Type,
    resolver: &TypeResolver,
) -> Option<HandlerTestClass> {
    match t {
        Type::AnyType { .. } => Some(HandlerTestClass::Any(t.clone())),
        Type::UninhabitedType { .. } => Some(HandlerTestClass::Skip),
        // Python feeds `get_proper_types(test_types)` into the loop, which
        // expands aliases; the wire cannot. Defer.
        Type::TypeAliasType { .. } => None,
        Type::CallableType { .. } | Type::Overloaded { .. } => {
            exception_type_of_type_obj(t, resolver)
        }
        Type::TypeType { item, .. } => {
            // Python uses `exc_type = ttype.item` directly (checker.py:5963),
            // then the is_subtype fence. Item is rarely an alias, but
            // `is_subtype` would expand it; defer when it is.
            if matches!(**item, Type::TypeAliasType { .. }) {
                return None;
            }
            Some(HandlerTestClass::Exc((**item).clone()))
        }
        _ => Some(HandlerTestClass::Invalid),
    }
}

/// `mypy.checker.check_except_handler_test` handler classification.
///
/// Takes the serialized handler test types (the `test_types` list, before
/// `get_proper_types`) and returns a list of `(tag, blob)` pairs, one per
/// input, preserving order and the early-return-on-invalid semantics:
///
///   * tag 0 (`Any`)   — Python appends the deserialized type and continues.
///   * tag 1 (`Skip`)  — Python continues without appending.
///   * tag 2 (`Invalid`) — Python fails with INVALID_EXCEPTION_TYPE and
///     returns `default_exception_type(is_star)` immediately.
///   * tag 3 (`Exc`)   — Python runs the `is_subtype(...BaseException)`
///     fence, appends on success, fails + returns on failure.
///
/// `None` defers the whole call to the pure-Python path. The `is_star`
/// reclassification fence (checker.py:5971-5978) and the
/// `is_subtype` fence stay in Python; this function only classifies the
/// structural shape of each handler type.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_except_handler_tests<'py>(
    py: Python<'py>,
    type_bytes_list: Vec<Vec<u8>>,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<&'py PyList>> {
    let resolver = resolver.resolver();
    let mut classes: Vec<HandlerTestClass> = Vec::with_capacity(type_bytes_list.len());
    for bytes in &type_bytes_list {
        let typ = match decode_type(bytes) {
            Some(t) => t,
            None => return Ok(None),
        };
        match classify_except_handler_test_inner(&typ, resolver) {
            Some(class) => classes.push(class),
            None => return Ok(None),
        }
    }
    let out = PyList::empty(py);
    for class in &classes {
        let (tag, blob): (i64, Option<Vec<u8>>) = match class {
            HandlerTestClass::Any(t) => {
                let blob = match encode_type_owned(t) {
                    Some(b) => b,
                    None => return Ok(None),
                };
                (0, Some(blob))
            }
            HandlerTestClass::Skip => (1, None),
            HandlerTestClass::Invalid => (2, None),
            HandlerTestClass::Exc(t) => {
                let blob = match encode_type_owned(t) {
                    Some(b) => b,
                    None => return Ok(None),
                };
                (3, Some(blob))
            }
        };
        let blob_obj: PyObject = match blob {
            Some(b) => PyBytes::new(py, &b).into_py(py),
            None => py.None(),
        };
        let tuple = PyTuple::new(py, [tag.into_py(py), blob_obj]);
        out.append(tuple)?;
    }
    Ok(Some(out))
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
            is_recursive: false,
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
        let resolver = TypeResolver::new();
        assert_eq!(
            rust_type_requires_usage(
                &encode_type(&t),
                &mut NativeTypeResolver::new(
                    resolver,
                    crate::aliases::TypeAliasResolver::default(),
                )
            )
            .unwrap(),
            Some(0)
        );
    }

    #[test]
    fn type_requires_usage_other_instance() {
        // builtins.int has no __await__ and is not typing.Coroutine: the
        // resolver must be present (else the mro walk cannot conclude).
        let resolver = TypeResolver::new();
        let t = instance("builtins.int");
        assert_eq!(
            rust_type_requires_usage(
                &encode_type(&t),
                &mut NativeTypeResolver::new(
                    resolver,
                    crate::aliases::TypeAliasResolver::default(),
                )
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn type_requires_usage_alias_defers() {
        assert_eq!(
            type_requires_usage_inner(&type_alias(), &TypeResolver::new(), &Default::default()),
            None
        );
    }

    #[test]
    fn type_requires_usage_awaitable_mro() {
        // A class whose own names carry __await__: UNUSED_AWAITABLE (1).
        let mut snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.Await".to_string(),
            mro: vec!["mod.Await".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.member_info
            .insert("__await__".to_string(), (false, true));
        let mut resolver = TypeResolver::new();
        resolver.insert("mod.Await".to_string(), snap);
        let t = instance("mod.Await");
        assert_eq!(
            type_requires_usage_inner(&t, &resolver, &Default::default()),
            Some(1)
        );
    }

    #[test]
    fn type_requires_usage_awaitable_missing_class_defers() {
        // mro references a class not in the resolver: cannot distinguish
        // "no __await__" from "unknown", so defer to Python.
        let snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.Await".to_string(),
            mro: vec!["mod.Await".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        let mut resolver = TypeResolver::new();
        resolver.insert("mod.Await".to_string(), snap);
        let t = instance("mod.Await");
        assert_eq!(
            type_requires_usage_inner(&t, &resolver, &Default::default()),
            None
        );
    }

    #[test]
    fn type_requires_usage_awaitable_absent() {
        // No __await__ anywhere in the resolved mro: decided absent (2).
        let snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.Plain".to_string(),
            mro: vec!["mod.Plain".to_string()],
            ..Default::default()
        };
        let mut resolver = TypeResolver::new();
        resolver.insert("mod.Plain".to_string(), snap);
        let t = instance("mod.Plain");
        assert_eq!(
            type_requires_usage_inner(&t, &resolver, &Default::default()),
            Some(2)
        );
    }

    #[test]
    fn type_requires_usage_awaitable_inherited() {
        // Subclass without __await__, base with it: mro walk finds it.
        let mut base = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.Base".to_string(),
            mro: vec!["mod.Base".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        base.member_info
            .insert("__await__".to_string(), (false, true));
        let sub = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.Sub".to_string(),
            mro: vec!["mod.Sub".to_string(), "mod.Base".to_string()],
            ..Default::default()
        };
        let mut resolver = TypeResolver::new();
        resolver.insert("mod.Base".to_string(), base);
        resolver.insert("mod.Sub".to_string(), sub);
        let t = instance("mod.Sub");
        assert_eq!(
            type_requires_usage_inner(&t, &resolver, &Default::default()),
            Some(1)
        );
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
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
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
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
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
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
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
            is_recursive: false,
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
                    is_recursive: false,
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(
            try_handler_union_inner(&t, true),
            vec![
                instance("builtins.ValueError"),
                Type::TypeAliasType {
                    type_ref: "mod.Alias".to_string(),
                    args: Vec::new(),
                    is_recursive: false
                },
            ]
        );
    }

    // -- is_valid_inferred_type (issue #445) --

    #[test]
    fn valid_inferred_none_final() {
        let t = Type::NoneType;
        assert_eq!(
            is_valid_inferred_type_inner(&t, true, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_none_non_member_allow_redef() {
        let t = Type::NoneType;
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, true),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_none_member_no_redef() {
        let t = Type::NoneType;
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, true, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_uninhabited() {
        let t = uninhabited();
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_erased_top_level() {
        // ErasedType is wire tag 122; visit_erased_type answers True (invalid
        // inside a lambda). The shim does not filter Erased operands, so it
        // must not fall through to the base-query default, which answers valid.
        let t = Type::ErasedType;
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_union_with_erased_item() {
        // Nested Erased items recurse through the ANY_STRATEGY union arm.
        let t = Type::UnionType {
            items: vec![instance("builtins.int"), Type::ErasedType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_plain_instance() {
        let t = instance("builtins.int");
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_ambiguous_uninhabited_inside_union() {
        let t = Type::UnionType {
            items: vec![
                instance("builtins.int"),
                Type::UninhabitedType { ambiguous: true },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_non_ambiguous_uninhabited_inside_tuple() {
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![Type::UninhabitedType { ambiguous: false }],
            implicit: false,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_meta_var_typevar() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 1,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_non_meta_var_typevar() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_typevar_with_ambiguous_inside_upper_bound() {
        // TypeVar override returns is_meta_var() and does NOT recurse into
        // upper_bound, so an ambiguous UninhabitedType inside upper_bound
        // is NOT detected.
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(Type::UninhabitedType { ambiguous: true }),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_tuple_excludes_fallback() {
        // visit_tuple_type queries only items, not partial_fallback. An
        // ambiguous UninhabitedType in the fallback must NOT be detected.
        let t = Type::TupleType {
            partial_fallback: Box::new(Type::UninhabitedType { ambiguous: true }),
            items: vec![instance("builtins.int")],
            implicit: false,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_alias_top_defers() {
        let t = type_alias();
        assert_eq!(is_valid_inferred_type_inner(&t, false, false, false), None);
    }

    #[test]
    fn valid_inferred_alias_nested_missing_snapshot_defers() {
        let t = Type::UnionType {
            items: vec![instance("builtins.int"), type_alias()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let mut seen: HashSet<String> = HashSet::new();
        assert_eq!(
            invalid_inferred_types_query(&t, &Default::default(), &mut seen),
            None
        );
    }

    #[test]
    fn valid_inferred_alias_top_expands_before_none_branch() {
        // get_proper_type runs BEFORE the NoneType/Uninhabited checks
        // (checker.py:11731-11743): an alias expanding to NoneType takes the
        // NoneType branch (Some(false) here) instead of deferring.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
            is_recursive: false,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        aliases.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&Type::NoneType),
                no_args: true,
                ..Default::default()
            },
        );
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_alias_top_expands_before_uninhabited_branch() {
        // Same ordering pin as the NoneType branch above, but for the second
        // short-circuit: an alias expanding to a (non-ambiguous) proper-type
        // UninhabitedType takes the Uninhabited branch, not the nested walk.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
            is_recursive: false,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        aliases.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&Type::UninhabitedType { ambiguous: false }),
                no_args: true,
                ..Default::default()
            },
        );
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_chained_alias_seen_guard_defaults_on_revisit() {
        // Two no_args aliases whose targets reference each other (A -> B ->
        // A). The set-based seen-guard must catch the revisit and answer the
        // default (False = valid) instead of looping; not PEP-695, no edge.
        let alias_node = |type_ref: &str| -> Type {
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: type_ref.to_string(),
                is_recursive: false,
            }
        };
        let t = alias_node("mod.A");
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        for name in ["mod.A", "mod.B"] {
            let other: String = if name == "mod.A" { "mod.B" } else { "mod.A" }.into();
            aliases.insert(
                name.to_string(),
                crate::aliases::TypeAliasSnapshot {
                    fullname: name.to_string(),
                    target: encode_type(&alias_node(&other)),
                    no_args: true,
                    ..Default::default()
                },
            );
        }
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_alias_top_expands_to_instance() {
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
            is_recursive: false,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        aliases.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&instance("builtins.int")),
                no_args: true,
                ..Default::default()
            },
        );
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_alias_edge_continuation_queries_args() {
        // BoolTypeQuery.visit_type_alias_type's ANY_STRATEGY edge (type_visitor.py
        // 609-613): with a clean target, a PEP-695 alias's written args are still
        // queried, so a meta-var arg flips the verdict to invalid.
        let t = Type::TypeAliasType {
            args: vec![Type::TypeVarType {
                name: "T".to_string(),
                fullname: "mod.T".to_string(),
                raw_id: 1,
                namespace: String::new(),
                values: Vec::new(),
                upper_bound: Box::new(instance("builtins.object")),
                default: Box::new(Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                }),
                variance: 0,
                meta_level: 1,
            }],
            type_ref: "mod.A".to_string(),
            is_recursive: false,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        aliases.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&instance("builtins.int")),
                alias_tvars: vec![crate::aliases::AliasTvar {
                    name: "T".to_string(),
                    raw_id: 2,
                    meta_level: 0,
                    namespace: String::new(),
                    is_type_var_tuple: false,
                }],
                python_3_12_type_alias: true,
                ..Default::default()
            },
        );
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_alias_old_style_args_not_queried() {
        // Same shape as the edge-continuation test but with an old-style
        // alias (python_3_12_type_alias=False): Python skips the args query,
        // so the meta-var arg must NOT flip the verdict (valid=True).
        let t = Type::TypeAliasType {
            args: vec![Type::TypeVarType {
                name: "T".to_string(),
                fullname: "mod.T".to_string(),
                raw_id: 1,
                namespace: String::new(),
                values: Vec::new(),
                upper_bound: Box::new(instance("builtins.object")),
                default: Box::new(Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                }),
                variance: 0,
                meta_level: 1,
            }],
            type_ref: "mod.A".to_string(),
            is_recursive: false,
        };
        let mut aliases = crate::aliases::TypeAliasResolver::default();
        aliases.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&instance("builtins.int")),
                alias_tvars: vec![crate::aliases::AliasTvar {
                    name: "T".to_string(),
                    raw_id: 2,
                    meta_level: 0,
                    namespace: String::new(),
                    is_type_var_tuple: false,
                }],
                python_3_12_type_alias: false,
                ..Default::default()
            },
        );
        assert_eq!(
            is_valid_inferred_type_with_aliases(&t, false, false, false, &aliases),
            Some(true)
        );
    }

    #[test]
    fn valid_inferred_instance_with_ambiguous_arg() {
        let t = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![Type::UninhabitedType { ambiguous: true }],
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
        );
    }

    #[test]
    fn valid_inferred_garbage_bytes_defers() {
        // Pure decode-defer: no pyo3 init needed, the pyfunction never
        // consults Python on an undecodable blob.
        let mut resolver = crate::typeinfo::NativeTypeResolver::new(
            crate::typeinfo::TypeResolver::default(),
            crate::aliases::TypeAliasResolver::default(),
        );
        assert_eq!(
            rust_is_valid_inferred_type(b"\xff\xff", false, false, false, &mut resolver).unwrap(),
            None
        );
    }

    #[test]
    fn valid_inferred_callable_with_ambiguous_ret() {
        let t = Type::CallableType {
            fallback: Box::new(instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![instance("builtins.int")],
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(Type::UninhabitedType { ambiguous: true }),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        assert_eq!(
            is_valid_inferred_type_inner(&t, false, false, false),
            Some(false)
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

    // -- rust_classify_except_handler_tests (Phase C2, issue #609) --

    /// A `CallableType` whose fallback is the given metaclass + ret_type.
    fn type_obj(meta_ref: &str, ret_instance: Type, instance_type: Option<Type>) -> Type {
        Type::CallableType {
            fallback: Box::new(instance(meta_ref)),
            instance_type: instance_type.map(Box::new),
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
            ret_type: Box::new(ret_instance.clone()),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    /// Resolver whose `builtins.type` snapshot reports a `builtins.type`
    /// base, so `is_type_obj` commits to `Some(true)`.
    fn meta_resolver() -> TypeResolver {
        let mut r = TypeResolver::new();
        let mut snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "builtins.type".to_string(),
            ..Default::default()
        };
        snap.has_base.insert("builtins.type".to_string());
        r.insert("builtins.type".to_string(), snap);
        r
    }

    #[test]
    fn except_handler_test_any() {
        let t = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        match classify_except_handler_test_inner(&t, &TypeResolver::new()) {
            Some(HandlerTestClass::Any(_)) => {}
            other => panic!("expected Any, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_uninhabited_is_skip() {
        let t = Type::UninhabitedType { ambiguous: false };
        match classify_except_handler_test_inner(&t, &TypeResolver::new()) {
            Some(HandlerTestClass::Skip) => {}
            other => panic!("expected Skip, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_plain_instance_is_invalid() {
        // `except int:` is an invalid exception type; Python fails there.
        let t = instance("builtins.int");
        match classify_except_handler_test_inner(&t, &TypeResolver::new()) {
            Some(HandlerTestClass::Invalid) => {}
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_alias_defers() {
        let t = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
            is_recursive: false,
        };
        assert!(classify_except_handler_test_inner(&t, &TypeResolver::new()).is_none());
    }

    #[test]
    fn except_handler_test_type_obj_erases_typevars() {
        // `except type[Foo[T]]:` binds T to Foo[T]; erase_typevars -> Foo[Any].
        let resolver = meta_resolver();
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "mod".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 12,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 0,
        };
        let foo = Type::Instance {
            type_ref: "mod.Foo".to_string(),
            args: vec![tv.clone()],
            last_known_value: None,
            extra_attrs: None,
        };
        let type_obj = type_obj("builtins.type", foo.clone(), None);
        match classify_except_handler_test_inner(&type_obj, &resolver) {
            Some(HandlerTestClass::Exc(exc)) => match exc {
                Type::Instance { args, .. } => {
                    assert_eq!(args.len(), 1);
                    assert!(matches!(args[0], Type::AnyType { .. }));
                }
                other => panic!("expected Instance after erasure, got {other:?}"),
            },
            other => panic!("expected Exc, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_type_obj_instance_type_wins() {
        // get_instance_type prefers CallableType.instance_type over ret_type.
        let resolver = meta_resolver();
        let precise = instance("mod.SelfType");
        let type_obj = type_obj(
            "builtins.type",
            instance("mod.RetType"),
            Some(precise.clone()),
        );
        match classify_except_handler_test_inner(&type_obj, &resolver) {
            Some(HandlerTestClass::Exc(exc)) => assert_eq!(exc, precise),
            other => panic!("expected Exc, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_type_obj_non_metaclass_invalid() {
        // Callable whose fallback is `builtins.function` (not a metaclass):
        // is_type_obj -> false -> Invalid. The resolver knows the fallback
        // but it has no `builtins.type` base.
        let mut r = TypeResolver::new();
        let snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "builtins.function".to_string(),
            ..Default::default()
        };
        r.insert("builtins.function".to_string(), snap);
        let type_obj = type_obj("builtins.function", instance("mod.Foo"), None);
        match classify_except_handler_test_inner(&type_obj, &r) {
            Some(HandlerTestClass::Invalid) => {}
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_type_obj_unknown_metaclass_defers() {
        // Custom metaclass not in the resolver: is_type_obj -> None -> defer.
        let type_obj = type_obj("mod.MyMeta", instance("mod.Foo"), None);
        assert!(classify_except_handler_test_inner(&type_obj, &TypeResolver::new()).is_none());
    }

    #[test]
    fn except_handler_test_overloaded_first_item() {
        // Overloaded type obj: classification uses items[0] only.
        let resolver = meta_resolver();
        let first = type_obj("builtins.type", instance("mod.Foo"), None);
        let t = Type::Overloaded {
            items: vec![first.clone()],
        };
        match classify_except_handler_test_inner(&t, &resolver) {
            Some(HandlerTestClass::Exc(exc)) => assert_eq!(exc, instance("mod.Foo")),
            other => panic!("expected Exc, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_type_type_item() {
        let t = Type::TypeType {
            item: Box::new(instance("mod.ValueError")),
            is_type_form: true,
        };
        match classify_except_handler_test_inner(&t, &TypeResolver::new()) {
            Some(HandlerTestClass::Exc(exc)) => assert_eq!(exc, instance("mod.ValueError")),
            other => panic!("expected Exc, got {other:?}"),
        }
    }

    #[test]
    fn except_handler_test_type_type_alias_item_defers() {
        let t = Type::TypeType {
            item: Box::new(Type::TypeAliasType {
                type_ref: "mod.Aliased".to_string(),
                args: vec![],
                is_recursive: false,
            }),
            is_type_form: true,
        };
        assert!(classify_except_handler_test_inner(&t, &TypeResolver::new()).is_none());
    }

    #[test]
    fn classify_except_handler_tests_pyfunc_roundtrip() {
        pyo3::prepare_freethreaded_python();
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let mut resolver = crate::typeinfo::NativeTypeResolver::new(
            meta_resolver(),
            crate::aliases::TypeAliasResolver::default(),
        );
        Python::with_gil(|py| {
            let blobs = vec![encode_type(&any), encode_type(&instance("builtins.int"))];
            let out = rust_classify_except_handler_tests(py, blobs, &mut resolver)
                .unwrap()
                .unwrap();
            assert_eq!(out.len(), 2);
            let first: &PyTuple = out[0].downcast().unwrap();
            let tag: i64 = first[0].extract().unwrap();
            assert_eq!(tag, 0); // Any
            let second: &PyTuple = out[1].downcast().unwrap();
            let tag: i64 = second[0].extract().unwrap();
            assert_eq!(tag, 2); // Invalid
            assert!(second[1].is_none());
        });
    }

    #[test]
    fn classify_except_handler_tests_pyfunc_overloaded_valid() {
        pyo3::prepare_freethreaded_python();
        // Type-object overload: classification uses items[0], emits Exc.
        let first = type_obj("builtins.type", instance("mod.Foo"), None);
        let overloaded = Type::Overloaded { items: vec![first] };
        let mut resolver = crate::typeinfo::NativeTypeResolver::new(
            meta_resolver(),
            crate::aliases::TypeAliasResolver::default(),
        );
        Python::with_gil(|py| {
            let blobs = vec![encode_type(&overloaded)];
            let out = rust_classify_except_handler_tests(py, blobs, &mut resolver)
                .unwrap()
                .unwrap();
            assert_eq!(out.len(), 1);
            let tuple: &PyTuple = out[0].downcast().unwrap();
            let tag: i64 = tuple[0].extract().unwrap();
            assert_eq!(tag, 3); // Exc
        });
    }

    #[test]
    fn classify_except_handler_tests_pyfunc_garbage_defers() {
        pyo3::prepare_freethreaded_python();
        let mut resolver = crate::typeinfo::NativeTypeResolver::new(
            TypeResolver::new(),
            crate::aliases::TypeAliasResolver::default(),
        );
        Python::with_gil(|py| {
            let out =
                rust_classify_except_handler_tests(py, vec![b"\xff\xff".to_vec()], &mut resolver)
                    .unwrap();
            assert!(out.is_none());
        });
    }
}
