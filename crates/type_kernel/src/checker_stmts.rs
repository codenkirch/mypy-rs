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

use pyo3::prelude::*;

use crate::astwire::{decode_node, AstNode};
use crate::wire::{read_type, ReadBuffer, Type};

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
}
