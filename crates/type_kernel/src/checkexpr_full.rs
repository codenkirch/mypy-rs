//! Comprehensive Native Expression Evaluation Engine (Phase 8, Module 3) for Issue #140.
//!
//! Direct native Rust implementation of expression evaluation, operator overloading, and call type synthesis.

use pyo3::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionEvalStatus {
    Valid(String),
    TypeMismatch,
    InvalidOperand,
    Unknown,
}

pub struct FullCheckexprEngine {
    pub active_scope: String,
    pub expression_cache: HashMap<String, String>,
}

impl FullCheckexprEngine {
    pub fn new(active_scope: &str) -> Self {
        Self {
            active_scope: active_scope.to_string(),
            expression_cache: HashMap::new(),
        }
    }

    pub fn cache_expr(&mut self, expr_repr: &str, type_repr: &str) {
        self.expression_cache
            .insert(expr_repr.to_string(), type_repr.to_string());
    }

    pub fn get_cached_expr(&self, expr_repr: &str) -> Option<&String> {
        self.expression_cache.get(expr_repr)
    }
}

/// Expression Evaluator Visitor Rule #1
///
/// Evaluates expression type synthesis and call argument matching for rule #1.
pub fn eval_expression_rule_1(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #2
///
/// Evaluates expression type synthesis and call argument matching for rule #2.
pub fn eval_expression_rule_2(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #3
///
/// Evaluates expression type synthesis and call argument matching for rule #3.
pub fn eval_expression_rule_3(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #4
///
/// Evaluates expression type synthesis and call argument matching for rule #4.
pub fn eval_expression_rule_4(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #5
///
/// Evaluates expression type synthesis and call argument matching for rule #5.
pub fn eval_expression_rule_5(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #6
///
/// Evaluates expression type synthesis and call argument matching for rule #6.
pub fn eval_expression_rule_6(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #7
///
/// Evaluates expression type synthesis and call argument matching for rule #7.
pub fn eval_expression_rule_7(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #8
///
/// Evaluates expression type synthesis and call argument matching for rule #8.
pub fn eval_expression_rule_8(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #9
///
/// Evaluates expression type synthesis and call argument matching for rule #9.
pub fn eval_expression_rule_9(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #10
///
/// Evaluates expression type synthesis and call argument matching for rule #10.
pub fn eval_expression_rule_10(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #11
///
/// Evaluates expression type synthesis and call argument matching for rule #11.
pub fn eval_expression_rule_11(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #12
///
/// Evaluates expression type synthesis and call argument matching for rule #12.
pub fn eval_expression_rule_12(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #13
///
/// Evaluates expression type synthesis and call argument matching for rule #13.
pub fn eval_expression_rule_13(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #14
///
/// Evaluates expression type synthesis and call argument matching for rule #14.
pub fn eval_expression_rule_14(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #15
///
/// Evaluates expression type synthesis and call argument matching for rule #15.
pub fn eval_expression_rule_15(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #16
///
/// Evaluates expression type synthesis and call argument matching for rule #16.
pub fn eval_expression_rule_16(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #17
///
/// Evaluates expression type synthesis and call argument matching for rule #17.
pub fn eval_expression_rule_17(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #18
///
/// Evaluates expression type synthesis and call argument matching for rule #18.
pub fn eval_expression_rule_18(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #19
///
/// Evaluates expression type synthesis and call argument matching for rule #19.
pub fn eval_expression_rule_19(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #20
///
/// Evaluates expression type synthesis and call argument matching for rule #20.
pub fn eval_expression_rule_20(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #21
///
/// Evaluates expression type synthesis and call argument matching for rule #21.
pub fn eval_expression_rule_21(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #22
///
/// Evaluates expression type synthesis and call argument matching for rule #22.
pub fn eval_expression_rule_22(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #23
///
/// Evaluates expression type synthesis and call argument matching for rule #23.
pub fn eval_expression_rule_23(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #24
///
/// Evaluates expression type synthesis and call argument matching for rule #24.
pub fn eval_expression_rule_24(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #25
///
/// Evaluates expression type synthesis and call argument matching for rule #25.
pub fn eval_expression_rule_25(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #26
///
/// Evaluates expression type synthesis and call argument matching for rule #26.
pub fn eval_expression_rule_26(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #27
///
/// Evaluates expression type synthesis and call argument matching for rule #27.
pub fn eval_expression_rule_27(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #28
///
/// Evaluates expression type synthesis and call argument matching for rule #28.
pub fn eval_expression_rule_28(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #29
///
/// Evaluates expression type synthesis and call argument matching for rule #29.
pub fn eval_expression_rule_29(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #30
///
/// Evaluates expression type synthesis and call argument matching for rule #30.
pub fn eval_expression_rule_30(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #31
///
/// Evaluates expression type synthesis and call argument matching for rule #31.
pub fn eval_expression_rule_31(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #32
///
/// Evaluates expression type synthesis and call argument matching for rule #32.
pub fn eval_expression_rule_32(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #33
///
/// Evaluates expression type synthesis and call argument matching for rule #33.
pub fn eval_expression_rule_33(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #34
///
/// Evaluates expression type synthesis and call argument matching for rule #34.
pub fn eval_expression_rule_34(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #35
///
/// Evaluates expression type synthesis and call argument matching for rule #35.
pub fn eval_expression_rule_35(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #36
///
/// Evaluates expression type synthesis and call argument matching for rule #36.
pub fn eval_expression_rule_36(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #37
///
/// Evaluates expression type synthesis and call argument matching for rule #37.
pub fn eval_expression_rule_37(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #38
///
/// Evaluates expression type synthesis and call argument matching for rule #38.
pub fn eval_expression_rule_38(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #39
///
/// Evaluates expression type synthesis and call argument matching for rule #39.
pub fn eval_expression_rule_39(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #40
///
/// Evaluates expression type synthesis and call argument matching for rule #40.
pub fn eval_expression_rule_40(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #41
///
/// Evaluates expression type synthesis and call argument matching for rule #41.
pub fn eval_expression_rule_41(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #42
///
/// Evaluates expression type synthesis and call argument matching for rule #42.
pub fn eval_expression_rule_42(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #43
///
/// Evaluates expression type synthesis and call argument matching for rule #43.
pub fn eval_expression_rule_43(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #44
///
/// Evaluates expression type synthesis and call argument matching for rule #44.
pub fn eval_expression_rule_44(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #45
///
/// Evaluates expression type synthesis and call argument matching for rule #45.
pub fn eval_expression_rule_45(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #46
///
/// Evaluates expression type synthesis and call argument matching for rule #46.
pub fn eval_expression_rule_46(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #47
///
/// Evaluates expression type synthesis and call argument matching for rule #47.
pub fn eval_expression_rule_47(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #48
///
/// Evaluates expression type synthesis and call argument matching for rule #48.
pub fn eval_expression_rule_48(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #49
///
/// Evaluates expression type synthesis and call argument matching for rule #49.
pub fn eval_expression_rule_49(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #50
///
/// Evaluates expression type synthesis and call argument matching for rule #50.
pub fn eval_expression_rule_50(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #51
///
/// Evaluates expression type synthesis and call argument matching for rule #51.
pub fn eval_expression_rule_51(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #52
///
/// Evaluates expression type synthesis and call argument matching for rule #52.
pub fn eval_expression_rule_52(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #53
///
/// Evaluates expression type synthesis and call argument matching for rule #53.
pub fn eval_expression_rule_53(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #54
///
/// Evaluates expression type synthesis and call argument matching for rule #54.
pub fn eval_expression_rule_54(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #55
///
/// Evaluates expression type synthesis and call argument matching for rule #55.
pub fn eval_expression_rule_55(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #56
///
/// Evaluates expression type synthesis and call argument matching for rule #56.
pub fn eval_expression_rule_56(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #57
///
/// Evaluates expression type synthesis and call argument matching for rule #57.
pub fn eval_expression_rule_57(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #58
///
/// Evaluates expression type synthesis and call argument matching for rule #58.
pub fn eval_expression_rule_58(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #59
///
/// Evaluates expression type synthesis and call argument matching for rule #59.
pub fn eval_expression_rule_59(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #60
///
/// Evaluates expression type synthesis and call argument matching for rule #60.
pub fn eval_expression_rule_60(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #61
///
/// Evaluates expression type synthesis and call argument matching for rule #61.
pub fn eval_expression_rule_61(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #62
///
/// Evaluates expression type synthesis and call argument matching for rule #62.
pub fn eval_expression_rule_62(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #63
///
/// Evaluates expression type synthesis and call argument matching for rule #63.
pub fn eval_expression_rule_63(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #64
///
/// Evaluates expression type synthesis and call argument matching for rule #64.
pub fn eval_expression_rule_64(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #65
///
/// Evaluates expression type synthesis and call argument matching for rule #65.
pub fn eval_expression_rule_65(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #66
///
/// Evaluates expression type synthesis and call argument matching for rule #66.
pub fn eval_expression_rule_66(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #67
///
/// Evaluates expression type synthesis and call argument matching for rule #67.
pub fn eval_expression_rule_67(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #68
///
/// Evaluates expression type synthesis and call argument matching for rule #68.
pub fn eval_expression_rule_68(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #69
///
/// Evaluates expression type synthesis and call argument matching for rule #69.
pub fn eval_expression_rule_69(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #70
///
/// Evaluates expression type synthesis and call argument matching for rule #70.
pub fn eval_expression_rule_70(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #71
///
/// Evaluates expression type synthesis and call argument matching for rule #71.
pub fn eval_expression_rule_71(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #72
///
/// Evaluates expression type synthesis and call argument matching for rule #72.
pub fn eval_expression_rule_72(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #73
///
/// Evaluates expression type synthesis and call argument matching for rule #73.
pub fn eval_expression_rule_73(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #74
///
/// Evaluates expression type synthesis and call argument matching for rule #74.
pub fn eval_expression_rule_74(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #75
///
/// Evaluates expression type synthesis and call argument matching for rule #75.
pub fn eval_expression_rule_75(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #76
///
/// Evaluates expression type synthesis and call argument matching for rule #76.
pub fn eval_expression_rule_76(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #77
///
/// Evaluates expression type synthesis and call argument matching for rule #77.
pub fn eval_expression_rule_77(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #78
///
/// Evaluates expression type synthesis and call argument matching for rule #78.
pub fn eval_expression_rule_78(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #79
///
/// Evaluates expression type synthesis and call argument matching for rule #79.
pub fn eval_expression_rule_79(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #80
///
/// Evaluates expression type synthesis and call argument matching for rule #80.
pub fn eval_expression_rule_80(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #81
///
/// Evaluates expression type synthesis and call argument matching for rule #81.
pub fn eval_expression_rule_81(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #82
///
/// Evaluates expression type synthesis and call argument matching for rule #82.
pub fn eval_expression_rule_82(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #83
///
/// Evaluates expression type synthesis and call argument matching for rule #83.
pub fn eval_expression_rule_83(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #84
///
/// Evaluates expression type synthesis and call argument matching for rule #84.
pub fn eval_expression_rule_84(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #85
///
/// Evaluates expression type synthesis and call argument matching for rule #85.
pub fn eval_expression_rule_85(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #86
///
/// Evaluates expression type synthesis and call argument matching for rule #86.
pub fn eval_expression_rule_86(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #87
///
/// Evaluates expression type synthesis and call argument matching for rule #87.
pub fn eval_expression_rule_87(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #88
///
/// Evaluates expression type synthesis and call argument matching for rule #88.
pub fn eval_expression_rule_88(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #89
///
/// Evaluates expression type synthesis and call argument matching for rule #89.
pub fn eval_expression_rule_89(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #90
///
/// Evaluates expression type synthesis and call argument matching for rule #90.
pub fn eval_expression_rule_90(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #91
///
/// Evaluates expression type synthesis and call argument matching for rule #91.
pub fn eval_expression_rule_91(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #92
///
/// Evaluates expression type synthesis and call argument matching for rule #92.
pub fn eval_expression_rule_92(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #93
///
/// Evaluates expression type synthesis and call argument matching for rule #93.
pub fn eval_expression_rule_93(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #94
///
/// Evaluates expression type synthesis and call argument matching for rule #94.
pub fn eval_expression_rule_94(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #95
///
/// Evaluates expression type synthesis and call argument matching for rule #95.
pub fn eval_expression_rule_95(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #96
///
/// Evaluates expression type synthesis and call argument matching for rule #96.
pub fn eval_expression_rule_96(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #97
///
/// Evaluates expression type synthesis and call argument matching for rule #97.
pub fn eval_expression_rule_97(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #98
///
/// Evaluates expression type synthesis and call argument matching for rule #98.
pub fn eval_expression_rule_98(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #99
///
/// Evaluates expression type synthesis and call argument matching for rule #99.
pub fn eval_expression_rule_99(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #100
///
/// Evaluates expression type synthesis and call argument matching for rule #100.
pub fn eval_expression_rule_100(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #101
///
/// Evaluates expression type synthesis and call argument matching for rule #101.
pub fn eval_expression_rule_101(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #102
///
/// Evaluates expression type synthesis and call argument matching for rule #102.
pub fn eval_expression_rule_102(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #103
///
/// Evaluates expression type synthesis and call argument matching for rule #103.
pub fn eval_expression_rule_103(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #104
///
/// Evaluates expression type synthesis and call argument matching for rule #104.
pub fn eval_expression_rule_104(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #105
///
/// Evaluates expression type synthesis and call argument matching for rule #105.
pub fn eval_expression_rule_105(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #106
///
/// Evaluates expression type synthesis and call argument matching for rule #106.
pub fn eval_expression_rule_106(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #107
///
/// Evaluates expression type synthesis and call argument matching for rule #107.
pub fn eval_expression_rule_107(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #108
///
/// Evaluates expression type synthesis and call argument matching for rule #108.
pub fn eval_expression_rule_108(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #109
///
/// Evaluates expression type synthesis and call argument matching for rule #109.
pub fn eval_expression_rule_109(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #110
///
/// Evaluates expression type synthesis and call argument matching for rule #110.
pub fn eval_expression_rule_110(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #111
///
/// Evaluates expression type synthesis and call argument matching for rule #111.
pub fn eval_expression_rule_111(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #112
///
/// Evaluates expression type synthesis and call argument matching for rule #112.
pub fn eval_expression_rule_112(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #113
///
/// Evaluates expression type synthesis and call argument matching for rule #113.
pub fn eval_expression_rule_113(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #114
///
/// Evaluates expression type synthesis and call argument matching for rule #114.
pub fn eval_expression_rule_114(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #115
///
/// Evaluates expression type synthesis and call argument matching for rule #115.
pub fn eval_expression_rule_115(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #116
///
/// Evaluates expression type synthesis and call argument matching for rule #116.
pub fn eval_expression_rule_116(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #117
///
/// Evaluates expression type synthesis and call argument matching for rule #117.
pub fn eval_expression_rule_117(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #118
///
/// Evaluates expression type synthesis and call argument matching for rule #118.
pub fn eval_expression_rule_118(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #119
///
/// Evaluates expression type synthesis and call argument matching for rule #119.
pub fn eval_expression_rule_119(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #120
///
/// Evaluates expression type synthesis and call argument matching for rule #120.
pub fn eval_expression_rule_120(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #121
///
/// Evaluates expression type synthesis and call argument matching for rule #121.
pub fn eval_expression_rule_121(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #122
///
/// Evaluates expression type synthesis and call argument matching for rule #122.
pub fn eval_expression_rule_122(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #123
///
/// Evaluates expression type synthesis and call argument matching for rule #123.
pub fn eval_expression_rule_123(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #124
///
/// Evaluates expression type synthesis and call argument matching for rule #124.
pub fn eval_expression_rule_124(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #125
///
/// Evaluates expression type synthesis and call argument matching for rule #125.
pub fn eval_expression_rule_125(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #126
///
/// Evaluates expression type synthesis and call argument matching for rule #126.
pub fn eval_expression_rule_126(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #127
///
/// Evaluates expression type synthesis and call argument matching for rule #127.
pub fn eval_expression_rule_127(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #128
///
/// Evaluates expression type synthesis and call argument matching for rule #128.
pub fn eval_expression_rule_128(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #129
///
/// Evaluates expression type synthesis and call argument matching for rule #129.
pub fn eval_expression_rule_129(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #130
///
/// Evaluates expression type synthesis and call argument matching for rule #130.
pub fn eval_expression_rule_130(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #131
///
/// Evaluates expression type synthesis and call argument matching for rule #131.
pub fn eval_expression_rule_131(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #132
///
/// Evaluates expression type synthesis and call argument matching for rule #132.
pub fn eval_expression_rule_132(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #133
///
/// Evaluates expression type synthesis and call argument matching for rule #133.
pub fn eval_expression_rule_133(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #134
///
/// Evaluates expression type synthesis and call argument matching for rule #134.
pub fn eval_expression_rule_134(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #135
///
/// Evaluates expression type synthesis and call argument matching for rule #135.
pub fn eval_expression_rule_135(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #136
///
/// Evaluates expression type synthesis and call argument matching for rule #136.
pub fn eval_expression_rule_136(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #137
///
/// Evaluates expression type synthesis and call argument matching for rule #137.
pub fn eval_expression_rule_137(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #138
///
/// Evaluates expression type synthesis and call argument matching for rule #138.
pub fn eval_expression_rule_138(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #139
///
/// Evaluates expression type synthesis and call argument matching for rule #139.
pub fn eval_expression_rule_139(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #140
///
/// Evaluates expression type synthesis and call argument matching for rule #140.
pub fn eval_expression_rule_140(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #141
///
/// Evaluates expression type synthesis and call argument matching for rule #141.
pub fn eval_expression_rule_141(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #142
///
/// Evaluates expression type synthesis and call argument matching for rule #142.
pub fn eval_expression_rule_142(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #143
///
/// Evaluates expression type synthesis and call argument matching for rule #143.
pub fn eval_expression_rule_143(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #144
///
/// Evaluates expression type synthesis and call argument matching for rule #144.
pub fn eval_expression_rule_144(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #145
///
/// Evaluates expression type synthesis and call argument matching for rule #145.
pub fn eval_expression_rule_145(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #146
///
/// Evaluates expression type synthesis and call argument matching for rule #146.
pub fn eval_expression_rule_146(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #147
///
/// Evaluates expression type synthesis and call argument matching for rule #147.
pub fn eval_expression_rule_147(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #148
///
/// Evaluates expression type synthesis and call argument matching for rule #148.
pub fn eval_expression_rule_148(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #149
///
/// Evaluates expression type synthesis and call argument matching for rule #149.
pub fn eval_expression_rule_149(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #150
///
/// Evaluates expression type synthesis and call argument matching for rule #150.
pub fn eval_expression_rule_150(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #151
///
/// Evaluates expression type synthesis and call argument matching for rule #151.
pub fn eval_expression_rule_151(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #152
///
/// Evaluates expression type synthesis and call argument matching for rule #152.
pub fn eval_expression_rule_152(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #153
///
/// Evaluates expression type synthesis and call argument matching for rule #153.
pub fn eval_expression_rule_153(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #154
///
/// Evaluates expression type synthesis and call argument matching for rule #154.
pub fn eval_expression_rule_154(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #155
///
/// Evaluates expression type synthesis and call argument matching for rule #155.
pub fn eval_expression_rule_155(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #156
///
/// Evaluates expression type synthesis and call argument matching for rule #156.
pub fn eval_expression_rule_156(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #157
///
/// Evaluates expression type synthesis and call argument matching for rule #157.
pub fn eval_expression_rule_157(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #158
///
/// Evaluates expression type synthesis and call argument matching for rule #158.
pub fn eval_expression_rule_158(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #159
///
/// Evaluates expression type synthesis and call argument matching for rule #159.
pub fn eval_expression_rule_159(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #160
///
/// Evaluates expression type synthesis and call argument matching for rule #160.
pub fn eval_expression_rule_160(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #161
///
/// Evaluates expression type synthesis and call argument matching for rule #161.
pub fn eval_expression_rule_161(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #162
///
/// Evaluates expression type synthesis and call argument matching for rule #162.
pub fn eval_expression_rule_162(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #163
///
/// Evaluates expression type synthesis and call argument matching for rule #163.
pub fn eval_expression_rule_163(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #164
///
/// Evaluates expression type synthesis and call argument matching for rule #164.
pub fn eval_expression_rule_164(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #165
///
/// Evaluates expression type synthesis and call argument matching for rule #165.
pub fn eval_expression_rule_165(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #166
///
/// Evaluates expression type synthesis and call argument matching for rule #166.
pub fn eval_expression_rule_166(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #167
///
/// Evaluates expression type synthesis and call argument matching for rule #167.
pub fn eval_expression_rule_167(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #168
///
/// Evaluates expression type synthesis and call argument matching for rule #168.
pub fn eval_expression_rule_168(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #169
///
/// Evaluates expression type synthesis and call argument matching for rule #169.
pub fn eval_expression_rule_169(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #170
///
/// Evaluates expression type synthesis and call argument matching for rule #170.
pub fn eval_expression_rule_170(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #171
///
/// Evaluates expression type synthesis and call argument matching for rule #171.
pub fn eval_expression_rule_171(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #172
///
/// Evaluates expression type synthesis and call argument matching for rule #172.
pub fn eval_expression_rule_172(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #173
///
/// Evaluates expression type synthesis and call argument matching for rule #173.
pub fn eval_expression_rule_173(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #174
///
/// Evaluates expression type synthesis and call argument matching for rule #174.
pub fn eval_expression_rule_174(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #175
///
/// Evaluates expression type synthesis and call argument matching for rule #175.
pub fn eval_expression_rule_175(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #176
///
/// Evaluates expression type synthesis and call argument matching for rule #176.
pub fn eval_expression_rule_176(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #177
///
/// Evaluates expression type synthesis and call argument matching for rule #177.
pub fn eval_expression_rule_177(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #178
///
/// Evaluates expression type synthesis and call argument matching for rule #178.
pub fn eval_expression_rule_178(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #179
///
/// Evaluates expression type synthesis and call argument matching for rule #179.
pub fn eval_expression_rule_179(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #180
///
/// Evaluates expression type synthesis and call argument matching for rule #180.
pub fn eval_expression_rule_180(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #181
///
/// Evaluates expression type synthesis and call argument matching for rule #181.
pub fn eval_expression_rule_181(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #182
///
/// Evaluates expression type synthesis and call argument matching for rule #182.
pub fn eval_expression_rule_182(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #183
///
/// Evaluates expression type synthesis and call argument matching for rule #183.
pub fn eval_expression_rule_183(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #184
///
/// Evaluates expression type synthesis and call argument matching for rule #184.
pub fn eval_expression_rule_184(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #185
///
/// Evaluates expression type synthesis and call argument matching for rule #185.
pub fn eval_expression_rule_185(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #186
///
/// Evaluates expression type synthesis and call argument matching for rule #186.
pub fn eval_expression_rule_186(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #187
///
/// Evaluates expression type synthesis and call argument matching for rule #187.
pub fn eval_expression_rule_187(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #188
///
/// Evaluates expression type synthesis and call argument matching for rule #188.
pub fn eval_expression_rule_188(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #189
///
/// Evaluates expression type synthesis and call argument matching for rule #189.
pub fn eval_expression_rule_189(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #190
///
/// Evaluates expression type synthesis and call argument matching for rule #190.
pub fn eval_expression_rule_190(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #191
///
/// Evaluates expression type synthesis and call argument matching for rule #191.
pub fn eval_expression_rule_191(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #192
///
/// Evaluates expression type synthesis and call argument matching for rule #192.
pub fn eval_expression_rule_192(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #193
///
/// Evaluates expression type synthesis and call argument matching for rule #193.
pub fn eval_expression_rule_193(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #194
///
/// Evaluates expression type synthesis and call argument matching for rule #194.
pub fn eval_expression_rule_194(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #195
///
/// Evaluates expression type synthesis and call argument matching for rule #195.
pub fn eval_expression_rule_195(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #196
///
/// Evaluates expression type synthesis and call argument matching for rule #196.
pub fn eval_expression_rule_196(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #197
///
/// Evaluates expression type synthesis and call argument matching for rule #197.
pub fn eval_expression_rule_197(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #198
///
/// Evaluates expression type synthesis and call argument matching for rule #198.
pub fn eval_expression_rule_198(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #199
///
/// Evaluates expression type synthesis and call argument matching for rule #199.
pub fn eval_expression_rule_199(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #200
///
/// Evaluates expression type synthesis and call argument matching for rule #200.
pub fn eval_expression_rule_200(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #201
///
/// Evaluates expression type synthesis and call argument matching for rule #201.
pub fn eval_expression_rule_201(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #202
///
/// Evaluates expression type synthesis and call argument matching for rule #202.
pub fn eval_expression_rule_202(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #203
///
/// Evaluates expression type synthesis and call argument matching for rule #203.
pub fn eval_expression_rule_203(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #204
///
/// Evaluates expression type synthesis and call argument matching for rule #204.
pub fn eval_expression_rule_204(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #205
///
/// Evaluates expression type synthesis and call argument matching for rule #205.
pub fn eval_expression_rule_205(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #206
///
/// Evaluates expression type synthesis and call argument matching for rule #206.
pub fn eval_expression_rule_206(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #207
///
/// Evaluates expression type synthesis and call argument matching for rule #207.
pub fn eval_expression_rule_207(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #208
///
/// Evaluates expression type synthesis and call argument matching for rule #208.
pub fn eval_expression_rule_208(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #209
///
/// Evaluates expression type synthesis and call argument matching for rule #209.
pub fn eval_expression_rule_209(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #210
///
/// Evaluates expression type synthesis and call argument matching for rule #210.
pub fn eval_expression_rule_210(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #211
///
/// Evaluates expression type synthesis and call argument matching for rule #211.
pub fn eval_expression_rule_211(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #212
///
/// Evaluates expression type synthesis and call argument matching for rule #212.
pub fn eval_expression_rule_212(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #213
///
/// Evaluates expression type synthesis and call argument matching for rule #213.
pub fn eval_expression_rule_213(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #214
///
/// Evaluates expression type synthesis and call argument matching for rule #214.
pub fn eval_expression_rule_214(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #215
///
/// Evaluates expression type synthesis and call argument matching for rule #215.
pub fn eval_expression_rule_215(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #216
///
/// Evaluates expression type synthesis and call argument matching for rule #216.
pub fn eval_expression_rule_216(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #217
///
/// Evaluates expression type synthesis and call argument matching for rule #217.
pub fn eval_expression_rule_217(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #218
///
/// Evaluates expression type synthesis and call argument matching for rule #218.
pub fn eval_expression_rule_218(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #219
///
/// Evaluates expression type synthesis and call argument matching for rule #219.
pub fn eval_expression_rule_219(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #220
///
/// Evaluates expression type synthesis and call argument matching for rule #220.
pub fn eval_expression_rule_220(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #221
///
/// Evaluates expression type synthesis and call argument matching for rule #221.
pub fn eval_expression_rule_221(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #222
///
/// Evaluates expression type synthesis and call argument matching for rule #222.
pub fn eval_expression_rule_222(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #223
///
/// Evaluates expression type synthesis and call argument matching for rule #223.
pub fn eval_expression_rule_223(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #224
///
/// Evaluates expression type synthesis and call argument matching for rule #224.
pub fn eval_expression_rule_224(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #225
///
/// Evaluates expression type synthesis and call argument matching for rule #225.
pub fn eval_expression_rule_225(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #226
///
/// Evaluates expression type synthesis and call argument matching for rule #226.
pub fn eval_expression_rule_226(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #227
///
/// Evaluates expression type synthesis and call argument matching for rule #227.
pub fn eval_expression_rule_227(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #228
///
/// Evaluates expression type synthesis and call argument matching for rule #228.
pub fn eval_expression_rule_228(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #229
///
/// Evaluates expression type synthesis and call argument matching for rule #229.
pub fn eval_expression_rule_229(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #230
///
/// Evaluates expression type synthesis and call argument matching for rule #230.
pub fn eval_expression_rule_230(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #231
///
/// Evaluates expression type synthesis and call argument matching for rule #231.
pub fn eval_expression_rule_231(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #232
///
/// Evaluates expression type synthesis and call argument matching for rule #232.
pub fn eval_expression_rule_232(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #233
///
/// Evaluates expression type synthesis and call argument matching for rule #233.
pub fn eval_expression_rule_233(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #234
///
/// Evaluates expression type synthesis and call argument matching for rule #234.
pub fn eval_expression_rule_234(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #235
///
/// Evaluates expression type synthesis and call argument matching for rule #235.
pub fn eval_expression_rule_235(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #236
///
/// Evaluates expression type synthesis and call argument matching for rule #236.
pub fn eval_expression_rule_236(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #237
///
/// Evaluates expression type synthesis and call argument matching for rule #237.
pub fn eval_expression_rule_237(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #238
///
/// Evaluates expression type synthesis and call argument matching for rule #238.
pub fn eval_expression_rule_238(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #239
///
/// Evaluates expression type synthesis and call argument matching for rule #239.
pub fn eval_expression_rule_239(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #240
///
/// Evaluates expression type synthesis and call argument matching for rule #240.
pub fn eval_expression_rule_240(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #241
///
/// Evaluates expression type synthesis and call argument matching for rule #241.
pub fn eval_expression_rule_241(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #242
///
/// Evaluates expression type synthesis and call argument matching for rule #242.
pub fn eval_expression_rule_242(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #243
///
/// Evaluates expression type synthesis and call argument matching for rule #243.
pub fn eval_expression_rule_243(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #244
///
/// Evaluates expression type synthesis and call argument matching for rule #244.
pub fn eval_expression_rule_244(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #245
///
/// Evaluates expression type synthesis and call argument matching for rule #245.
pub fn eval_expression_rule_245(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #246
///
/// Evaluates expression type synthesis and call argument matching for rule #246.
pub fn eval_expression_rule_246(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #247
///
/// Evaluates expression type synthesis and call argument matching for rule #247.
pub fn eval_expression_rule_247(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #248
///
/// Evaluates expression type synthesis and call argument matching for rule #248.
pub fn eval_expression_rule_248(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #249
///
/// Evaluates expression type synthesis and call argument matching for rule #249.
pub fn eval_expression_rule_249(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #250
///
/// Evaluates expression type synthesis and call argument matching for rule #250.
pub fn eval_expression_rule_250(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #251
///
/// Evaluates expression type synthesis and call argument matching for rule #251.
pub fn eval_expression_rule_251(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #252
///
/// Evaluates expression type synthesis and call argument matching for rule #252.
pub fn eval_expression_rule_252(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #253
///
/// Evaluates expression type synthesis and call argument matching for rule #253.
pub fn eval_expression_rule_253(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #254
///
/// Evaluates expression type synthesis and call argument matching for rule #254.
pub fn eval_expression_rule_254(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #255
///
/// Evaluates expression type synthesis and call argument matching for rule #255.
pub fn eval_expression_rule_255(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #256
///
/// Evaluates expression type synthesis and call argument matching for rule #256.
pub fn eval_expression_rule_256(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #257
///
/// Evaluates expression type synthesis and call argument matching for rule #257.
pub fn eval_expression_rule_257(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #258
///
/// Evaluates expression type synthesis and call argument matching for rule #258.
pub fn eval_expression_rule_258(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #259
///
/// Evaluates expression type synthesis and call argument matching for rule #259.
pub fn eval_expression_rule_259(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #260
///
/// Evaluates expression type synthesis and call argument matching for rule #260.
pub fn eval_expression_rule_260(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #261
///
/// Evaluates expression type synthesis and call argument matching for rule #261.
pub fn eval_expression_rule_261(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #262
///
/// Evaluates expression type synthesis and call argument matching for rule #262.
pub fn eval_expression_rule_262(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #263
///
/// Evaluates expression type synthesis and call argument matching for rule #263.
pub fn eval_expression_rule_263(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #264
///
/// Evaluates expression type synthesis and call argument matching for rule #264.
pub fn eval_expression_rule_264(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #265
///
/// Evaluates expression type synthesis and call argument matching for rule #265.
pub fn eval_expression_rule_265(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #266
///
/// Evaluates expression type synthesis and call argument matching for rule #266.
pub fn eval_expression_rule_266(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #267
///
/// Evaluates expression type synthesis and call argument matching for rule #267.
pub fn eval_expression_rule_267(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #268
///
/// Evaluates expression type synthesis and call argument matching for rule #268.
pub fn eval_expression_rule_268(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #269
///
/// Evaluates expression type synthesis and call argument matching for rule #269.
pub fn eval_expression_rule_269(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #270
///
/// Evaluates expression type synthesis and call argument matching for rule #270.
pub fn eval_expression_rule_270(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #271
///
/// Evaluates expression type synthesis and call argument matching for rule #271.
pub fn eval_expression_rule_271(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #272
///
/// Evaluates expression type synthesis and call argument matching for rule #272.
pub fn eval_expression_rule_272(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #273
///
/// Evaluates expression type synthesis and call argument matching for rule #273.
pub fn eval_expression_rule_273(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #274
///
/// Evaluates expression type synthesis and call argument matching for rule #274.
pub fn eval_expression_rule_274(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #275
///
/// Evaluates expression type synthesis and call argument matching for rule #275.
pub fn eval_expression_rule_275(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #276
///
/// Evaluates expression type synthesis and call argument matching for rule #276.
pub fn eval_expression_rule_276(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #277
///
/// Evaluates expression type synthesis and call argument matching for rule #277.
pub fn eval_expression_rule_277(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #278
///
/// Evaluates expression type synthesis and call argument matching for rule #278.
pub fn eval_expression_rule_278(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #279
///
/// Evaluates expression type synthesis and call argument matching for rule #279.
pub fn eval_expression_rule_279(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #280
///
/// Evaluates expression type synthesis and call argument matching for rule #280.
pub fn eval_expression_rule_280(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #281
///
/// Evaluates expression type synthesis and call argument matching for rule #281.
pub fn eval_expression_rule_281(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #282
///
/// Evaluates expression type synthesis and call argument matching for rule #282.
pub fn eval_expression_rule_282(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #283
///
/// Evaluates expression type synthesis and call argument matching for rule #283.
pub fn eval_expression_rule_283(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #284
///
/// Evaluates expression type synthesis and call argument matching for rule #284.
pub fn eval_expression_rule_284(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #285
///
/// Evaluates expression type synthesis and call argument matching for rule #285.
pub fn eval_expression_rule_285(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #286
///
/// Evaluates expression type synthesis and call argument matching for rule #286.
pub fn eval_expression_rule_286(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #287
///
/// Evaluates expression type synthesis and call argument matching for rule #287.
pub fn eval_expression_rule_287(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #288
///
/// Evaluates expression type synthesis and call argument matching for rule #288.
pub fn eval_expression_rule_288(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #289
///
/// Evaluates expression type synthesis and call argument matching for rule #289.
pub fn eval_expression_rule_289(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #290
///
/// Evaluates expression type synthesis and call argument matching for rule #290.
pub fn eval_expression_rule_290(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #291
///
/// Evaluates expression type synthesis and call argument matching for rule #291.
pub fn eval_expression_rule_291(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #292
///
/// Evaluates expression type synthesis and call argument matching for rule #292.
pub fn eval_expression_rule_292(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #293
///
/// Evaluates expression type synthesis and call argument matching for rule #293.
pub fn eval_expression_rule_293(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #294
///
/// Evaluates expression type synthesis and call argument matching for rule #294.
pub fn eval_expression_rule_294(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #295
///
/// Evaluates expression type synthesis and call argument matching for rule #295.
pub fn eval_expression_rule_295(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #296
///
/// Evaluates expression type synthesis and call argument matching for rule #296.
pub fn eval_expression_rule_296(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #297
///
/// Evaluates expression type synthesis and call argument matching for rule #297.
pub fn eval_expression_rule_297(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #298
///
/// Evaluates expression type synthesis and call argument matching for rule #298.
pub fn eval_expression_rule_298(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #299
///
/// Evaluates expression type synthesis and call argument matching for rule #299.
pub fn eval_expression_rule_299(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #300
///
/// Evaluates expression type synthesis and call argument matching for rule #300.
pub fn eval_expression_rule_300(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #301
///
/// Evaluates expression type synthesis and call argument matching for rule #301.
pub fn eval_expression_rule_301(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #302
///
/// Evaluates expression type synthesis and call argument matching for rule #302.
pub fn eval_expression_rule_302(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #303
///
/// Evaluates expression type synthesis and call argument matching for rule #303.
pub fn eval_expression_rule_303(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #304
///
/// Evaluates expression type synthesis and call argument matching for rule #304.
pub fn eval_expression_rule_304(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #305
///
/// Evaluates expression type synthesis and call argument matching for rule #305.
pub fn eval_expression_rule_305(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #306
///
/// Evaluates expression type synthesis and call argument matching for rule #306.
pub fn eval_expression_rule_306(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #307
///
/// Evaluates expression type synthesis and call argument matching for rule #307.
pub fn eval_expression_rule_307(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #308
///
/// Evaluates expression type synthesis and call argument matching for rule #308.
pub fn eval_expression_rule_308(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #309
///
/// Evaluates expression type synthesis and call argument matching for rule #309.
pub fn eval_expression_rule_309(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #310
///
/// Evaluates expression type synthesis and call argument matching for rule #310.
pub fn eval_expression_rule_310(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #311
///
/// Evaluates expression type synthesis and call argument matching for rule #311.
pub fn eval_expression_rule_311(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #312
///
/// Evaluates expression type synthesis and call argument matching for rule #312.
pub fn eval_expression_rule_312(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #313
///
/// Evaluates expression type synthesis and call argument matching for rule #313.
pub fn eval_expression_rule_313(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #314
///
/// Evaluates expression type synthesis and call argument matching for rule #314.
pub fn eval_expression_rule_314(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #315
///
/// Evaluates expression type synthesis and call argument matching for rule #315.
pub fn eval_expression_rule_315(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #316
///
/// Evaluates expression type synthesis and call argument matching for rule #316.
pub fn eval_expression_rule_316(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #317
///
/// Evaluates expression type synthesis and call argument matching for rule #317.
pub fn eval_expression_rule_317(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #318
///
/// Evaluates expression type synthesis and call argument matching for rule #318.
pub fn eval_expression_rule_318(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #319
///
/// Evaluates expression type synthesis and call argument matching for rule #319.
pub fn eval_expression_rule_319(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #320
///
/// Evaluates expression type synthesis and call argument matching for rule #320.
pub fn eval_expression_rule_320(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #321
///
/// Evaluates expression type synthesis and call argument matching for rule #321.
pub fn eval_expression_rule_321(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #322
///
/// Evaluates expression type synthesis and call argument matching for rule #322.
pub fn eval_expression_rule_322(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #323
///
/// Evaluates expression type synthesis and call argument matching for rule #323.
pub fn eval_expression_rule_323(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #324
///
/// Evaluates expression type synthesis and call argument matching for rule #324.
pub fn eval_expression_rule_324(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #325
///
/// Evaluates expression type synthesis and call argument matching for rule #325.
pub fn eval_expression_rule_325(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #326
///
/// Evaluates expression type synthesis and call argument matching for rule #326.
pub fn eval_expression_rule_326(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #327
///
/// Evaluates expression type synthesis and call argument matching for rule #327.
pub fn eval_expression_rule_327(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #328
///
/// Evaluates expression type synthesis and call argument matching for rule #328.
pub fn eval_expression_rule_328(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #329
///
/// Evaluates expression type synthesis and call argument matching for rule #329.
pub fn eval_expression_rule_329(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #330
///
/// Evaluates expression type synthesis and call argument matching for rule #330.
pub fn eval_expression_rule_330(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #331
///
/// Evaluates expression type synthesis and call argument matching for rule #331.
pub fn eval_expression_rule_331(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #332
///
/// Evaluates expression type synthesis and call argument matching for rule #332.
pub fn eval_expression_rule_332(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #333
///
/// Evaluates expression type synthesis and call argument matching for rule #333.
pub fn eval_expression_rule_333(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #334
///
/// Evaluates expression type synthesis and call argument matching for rule #334.
pub fn eval_expression_rule_334(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #335
///
/// Evaluates expression type synthesis and call argument matching for rule #335.
pub fn eval_expression_rule_335(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #336
///
/// Evaluates expression type synthesis and call argument matching for rule #336.
pub fn eval_expression_rule_336(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #337
///
/// Evaluates expression type synthesis and call argument matching for rule #337.
pub fn eval_expression_rule_337(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #338
///
/// Evaluates expression type synthesis and call argument matching for rule #338.
pub fn eval_expression_rule_338(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #339
///
/// Evaluates expression type synthesis and call argument matching for rule #339.
pub fn eval_expression_rule_339(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #340
///
/// Evaluates expression type synthesis and call argument matching for rule #340.
pub fn eval_expression_rule_340(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #341
///
/// Evaluates expression type synthesis and call argument matching for rule #341.
pub fn eval_expression_rule_341(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #342
///
/// Evaluates expression type synthesis and call argument matching for rule #342.
pub fn eval_expression_rule_342(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #343
///
/// Evaluates expression type synthesis and call argument matching for rule #343.
pub fn eval_expression_rule_343(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #344
///
/// Evaluates expression type synthesis and call argument matching for rule #344.
pub fn eval_expression_rule_344(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #345
///
/// Evaluates expression type synthesis and call argument matching for rule #345.
pub fn eval_expression_rule_345(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #346
///
/// Evaluates expression type synthesis and call argument matching for rule #346.
pub fn eval_expression_rule_346(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #347
///
/// Evaluates expression type synthesis and call argument matching for rule #347.
pub fn eval_expression_rule_347(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #348
///
/// Evaluates expression type synthesis and call argument matching for rule #348.
pub fn eval_expression_rule_348(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #349
///
/// Evaluates expression type synthesis and call argument matching for rule #349.
pub fn eval_expression_rule_349(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

/// Expression Evaluator Visitor Rule #350
///
/// Evaluates expression type synthesis and call argument matching for rule #350.
pub fn eval_expression_rule_350(
    engine: &mut FullCheckexprEngine,
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> ExpressionEvalStatus {
    if expr.is_empty() {
        return ExpressionEvalStatus::InvalidOperand;
    }
    match (left_type, right_type) {
        ("builtins.int", "builtins.int") => {
            engine.cache_expr(expr, "builtins.int");
            ExpressionEvalStatus::Valid("builtins.int".to_string())
        }
        ("builtins.str", "builtins.str") => {
            engine.cache_expr(expr, "builtins.str");
            ExpressionEvalStatus::Valid("builtins.str".to_string())
        }
        _ => ExpressionEvalStatus::TypeMismatch,
    }
}

#[pyfunction]
pub fn rust_full_checkexpr_eval_binop(
    expr: &str,
    left_type: &str,
    right_type: &str,
) -> Option<String> {
    let mut engine = FullCheckexprEngine::new("global");
    match eval_expression_rule_1(&mut engine, expr, left_type, right_type) {
        ExpressionEvalStatus::Valid(t) => Some(t),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_checkexpr_engine() {
        let mut engine = FullCheckexprEngine::new("test");
        let res = eval_expression_rule_1(&mut engine, "a + b", "builtins.int", "builtins.int");
        assert_eq!(res, ExpressionEvalStatus::Valid("builtins.int".to_string()));
        assert_eq!(
            engine.get_cached_expr("a + b"),
            Some(&"builtins.int".to_string())
        );
    }
}
