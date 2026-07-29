//! Native expression checker engine (Stage 9 / Phase 3) for Issue #130.
//!
//! Implements pure expression type inference routines:
//! - Binary & unary operator type resolution
//! - Container literal element type joining
//! - Call expression dispatch logic

use crate::wire::{LiteralValue, Type};
use pyo3::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionKind {
    BinaryOp { op: String },
    UnaryOp { op: String },
    ListLiteral,
    DictLiteral,
    TupleLiteral,
    Call,
    Other,
}

pub fn classify_expression_kind(tag: u8, op_name: Option<&str>) -> ExpressionKind {
    match tag {
        166 => ExpressionKind::BinaryOp {
            op: op_name.unwrap_or("").to_string(),
        },
        182 => ExpressionKind::UnaryOp {
            op: op_name.unwrap_or("").to_string(),
        },
        173 => ExpressionKind::ListLiteral,
        183 => ExpressionKind::DictLiteral,
        170 => ExpressionKind::TupleLiteral,
        161 => ExpressionKind::Call,
        _ => ExpressionKind::Other,
    }
}

pub fn infer_unary_op_type(op: &str, input_type: &Type) -> Option<Type> {
    match (op, input_type) {
        ("not", Type::Instance { type_ref, .. }) if type_ref == "builtins.bool" => {
            Some(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        ("-", Type::Instance { type_ref, .. }) if type_ref == "builtins.int" => {
            Some(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        _ => None,
    }
}

#[pyfunction]
pub fn rust_infer_binary_op_simple(
    op: &str,
    left_type_name: &str,
    right_type_name: &str,
) -> Option<String> {
    match (op, left_type_name, right_type_name) {
        ("+", "builtins.int", "builtins.int") => Some("builtins.int".to_string()),
        ("+", "builtins.str", "builtins.str") => Some("builtins.str".to_string()),
        ("-", "builtins.int", "builtins.int") => Some("builtins.int".to_string()),
        ("*", "builtins.int", "builtins.int") => Some("builtins.int".to_string()),
        ("==", _, _) => Some("builtins.bool".to_string()),
        ("<", "builtins.int", "builtins.int") => Some("builtins.bool".to_string()),
        (">", "builtins.int", "builtins.int") => Some("builtins.bool".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_expression_kind() {
        assert_eq!(
            classify_expression_kind(166, Some("+")),
            ExpressionKind::BinaryOp {
                op: "+".to_string()
            }
        );
        assert_eq!(
            classify_expression_kind(173, None),
            ExpressionKind::ListLiteral
        );
        assert_eq!(classify_expression_kind(161, None), ExpressionKind::Call);
    }

    #[test]
    fn test_infer_unary_op_type() {
        let bool_type = Type::Instance {
            type_ref: "builtins.bool".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = infer_unary_op_type("not", &bool_type);
        assert!(result.is_some());
    }

    #[test]
    fn test_rust_infer_binary_op_simple() {
        assert_eq!(
            rust_infer_binary_op_simple("+", "builtins.int", "builtins.int"),
            Some("builtins.int".to_string())
        );
        assert_eq!(
            rust_infer_binary_op_simple("==", "builtins.str", "builtins.str"),
            Some("builtins.bool".to_string())
        );
        assert_eq!(
            rust_infer_binary_op_simple("+", "builtins.int", "builtins.str"),
            None
        );
    }
}
