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
/// be decoded (parity-only usage serializes live types, so this signals
/// defer).
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
        out.append(PyBytes::new(py, &encode_type_owned(t)))?;
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

fn encode_type_owned(t: &Type) -> Vec<u8> {
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).expect("write type");
    buf.into_bytes()
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
}
