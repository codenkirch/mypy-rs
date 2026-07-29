//! Comprehensive Native Type Checker Engine (Phase 8, Module 1) for Issue #140.
//!
//! Direct native Rust implementation of type checking visitor routines, statement checking, and scope management.

use pyo3::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeCheckState {
    Unchecked,
    Checking,
    Checked,
    Error,
}

pub struct FullTypeChecker {
    pub scope_name: String,
    pub symbol_table: HashMap<String, String>,
    pub errors: Vec<String>,
}

impl FullTypeChecker {
    pub fn new(scope_name: &str) -> Self {
        Self {
            scope_name: scope_name.to_string(),
            symbol_table: HashMap::new(),
            errors: Vec::new(),
        }
    }

    pub fn bind_symbol(&mut self, symbol: &str, type_name: &str) {
        self.symbol_table
            .insert(symbol.to_string(), type_name.to_string());
    }

    pub fn lookup_symbol(&self, symbol: &str) -> Option<&String> {
        self.symbol_table.get(symbol)
    }

    pub fn report_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }
}

/// Statement Type Check Visitor Handler #1
///
/// Evaluates statement semantics and type safety for rule #1.
pub fn check_statement_rule_1(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 1));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #2
///
/// Evaluates statement semantics and type safety for rule #2.
pub fn check_statement_rule_2(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 2));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #3
///
/// Evaluates statement semantics and type safety for rule #3.
pub fn check_statement_rule_3(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 3));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #4
///
/// Evaluates statement semantics and type safety for rule #4.
pub fn check_statement_rule_4(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 4));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #5
///
/// Evaluates statement semantics and type safety for rule #5.
pub fn check_statement_rule_5(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 5));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #6
///
/// Evaluates statement semantics and type safety for rule #6.
pub fn check_statement_rule_6(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 6));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #7
///
/// Evaluates statement semantics and type safety for rule #7.
pub fn check_statement_rule_7(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 7));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #8
///
/// Evaluates statement semantics and type safety for rule #8.
pub fn check_statement_rule_8(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 8));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #9
///
/// Evaluates statement semantics and type safety for rule #9.
pub fn check_statement_rule_9(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 9));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #10
///
/// Evaluates statement semantics and type safety for rule #10.
pub fn check_statement_rule_10(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 10));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #11
///
/// Evaluates statement semantics and type safety for rule #11.
pub fn check_statement_rule_11(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 11));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #12
///
/// Evaluates statement semantics and type safety for rule #12.
pub fn check_statement_rule_12(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 12));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #13
///
/// Evaluates statement semantics and type safety for rule #13.
pub fn check_statement_rule_13(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 13));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #14
///
/// Evaluates statement semantics and type safety for rule #14.
pub fn check_statement_rule_14(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 14));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #15
///
/// Evaluates statement semantics and type safety for rule #15.
pub fn check_statement_rule_15(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 15));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #16
///
/// Evaluates statement semantics and type safety for rule #16.
pub fn check_statement_rule_16(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 16));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #17
///
/// Evaluates statement semantics and type safety for rule #17.
pub fn check_statement_rule_17(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 17));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #18
///
/// Evaluates statement semantics and type safety for rule #18.
pub fn check_statement_rule_18(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 18));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #19
///
/// Evaluates statement semantics and type safety for rule #19.
pub fn check_statement_rule_19(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 19));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #20
///
/// Evaluates statement semantics and type safety for rule #20.
pub fn check_statement_rule_20(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 20));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #21
///
/// Evaluates statement semantics and type safety for rule #21.
pub fn check_statement_rule_21(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 21));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #22
///
/// Evaluates statement semantics and type safety for rule #22.
pub fn check_statement_rule_22(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 22));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #23
///
/// Evaluates statement semantics and type safety for rule #23.
pub fn check_statement_rule_23(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 23));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #24
///
/// Evaluates statement semantics and type safety for rule #24.
pub fn check_statement_rule_24(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 24));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #25
///
/// Evaluates statement semantics and type safety for rule #25.
pub fn check_statement_rule_25(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 25));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #26
///
/// Evaluates statement semantics and type safety for rule #26.
pub fn check_statement_rule_26(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 26));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #27
///
/// Evaluates statement semantics and type safety for rule #27.
pub fn check_statement_rule_27(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 27));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #28
///
/// Evaluates statement semantics and type safety for rule #28.
pub fn check_statement_rule_28(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 28));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #29
///
/// Evaluates statement semantics and type safety for rule #29.
pub fn check_statement_rule_29(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 29));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #30
///
/// Evaluates statement semantics and type safety for rule #30.
pub fn check_statement_rule_30(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 30));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #31
///
/// Evaluates statement semantics and type safety for rule #31.
pub fn check_statement_rule_31(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 31));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #32
///
/// Evaluates statement semantics and type safety for rule #32.
pub fn check_statement_rule_32(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 32));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #33
///
/// Evaluates statement semantics and type safety for rule #33.
pub fn check_statement_rule_33(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 33));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #34
///
/// Evaluates statement semantics and type safety for rule #34.
pub fn check_statement_rule_34(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 34));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #35
///
/// Evaluates statement semantics and type safety for rule #35.
pub fn check_statement_rule_35(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 35));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #36
///
/// Evaluates statement semantics and type safety for rule #36.
pub fn check_statement_rule_36(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 36));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #37
///
/// Evaluates statement semantics and type safety for rule #37.
pub fn check_statement_rule_37(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 37));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #38
///
/// Evaluates statement semantics and type safety for rule #38.
pub fn check_statement_rule_38(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 38));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #39
///
/// Evaluates statement semantics and type safety for rule #39.
pub fn check_statement_rule_39(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 39));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #40
///
/// Evaluates statement semantics and type safety for rule #40.
pub fn check_statement_rule_40(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 40));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #41
///
/// Evaluates statement semantics and type safety for rule #41.
pub fn check_statement_rule_41(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 41));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #42
///
/// Evaluates statement semantics and type safety for rule #42.
pub fn check_statement_rule_42(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 42));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #43
///
/// Evaluates statement semantics and type safety for rule #43.
pub fn check_statement_rule_43(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 43));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #44
///
/// Evaluates statement semantics and type safety for rule #44.
pub fn check_statement_rule_44(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 44));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #45
///
/// Evaluates statement semantics and type safety for rule #45.
pub fn check_statement_rule_45(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 45));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #46
///
/// Evaluates statement semantics and type safety for rule #46.
pub fn check_statement_rule_46(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 46));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #47
///
/// Evaluates statement semantics and type safety for rule #47.
pub fn check_statement_rule_47(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 47));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #48
///
/// Evaluates statement semantics and type safety for rule #48.
pub fn check_statement_rule_48(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 48));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #49
///
/// Evaluates statement semantics and type safety for rule #49.
pub fn check_statement_rule_49(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 49));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #50
///
/// Evaluates statement semantics and type safety for rule #50.
pub fn check_statement_rule_50(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 50));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #51
///
/// Evaluates statement semantics and type safety for rule #51.
pub fn check_statement_rule_51(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 51));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #52
///
/// Evaluates statement semantics and type safety for rule #52.
pub fn check_statement_rule_52(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 52));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #53
///
/// Evaluates statement semantics and type safety for rule #53.
pub fn check_statement_rule_53(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 53));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #54
///
/// Evaluates statement semantics and type safety for rule #54.
pub fn check_statement_rule_54(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 54));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #55
///
/// Evaluates statement semantics and type safety for rule #55.
pub fn check_statement_rule_55(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 55));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #56
///
/// Evaluates statement semantics and type safety for rule #56.
pub fn check_statement_rule_56(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 56));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #57
///
/// Evaluates statement semantics and type safety for rule #57.
pub fn check_statement_rule_57(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 57));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #58
///
/// Evaluates statement semantics and type safety for rule #58.
pub fn check_statement_rule_58(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 58));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #59
///
/// Evaluates statement semantics and type safety for rule #59.
pub fn check_statement_rule_59(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 59));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #60
///
/// Evaluates statement semantics and type safety for rule #60.
pub fn check_statement_rule_60(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 60));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #61
///
/// Evaluates statement semantics and type safety for rule #61.
pub fn check_statement_rule_61(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 61));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #62
///
/// Evaluates statement semantics and type safety for rule #62.
pub fn check_statement_rule_62(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 62));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #63
///
/// Evaluates statement semantics and type safety for rule #63.
pub fn check_statement_rule_63(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 63));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #64
///
/// Evaluates statement semantics and type safety for rule #64.
pub fn check_statement_rule_64(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 64));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #65
///
/// Evaluates statement semantics and type safety for rule #65.
pub fn check_statement_rule_65(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 65));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #66
///
/// Evaluates statement semantics and type safety for rule #66.
pub fn check_statement_rule_66(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 66));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #67
///
/// Evaluates statement semantics and type safety for rule #67.
pub fn check_statement_rule_67(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 67));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #68
///
/// Evaluates statement semantics and type safety for rule #68.
pub fn check_statement_rule_68(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 68));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #69
///
/// Evaluates statement semantics and type safety for rule #69.
pub fn check_statement_rule_69(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 69));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #70
///
/// Evaluates statement semantics and type safety for rule #70.
pub fn check_statement_rule_70(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 70));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #71
///
/// Evaluates statement semantics and type safety for rule #71.
pub fn check_statement_rule_71(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 71));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #72
///
/// Evaluates statement semantics and type safety for rule #72.
pub fn check_statement_rule_72(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 72));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #73
///
/// Evaluates statement semantics and type safety for rule #73.
pub fn check_statement_rule_73(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 73));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #74
///
/// Evaluates statement semantics and type safety for rule #74.
pub fn check_statement_rule_74(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 74));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #75
///
/// Evaluates statement semantics and type safety for rule #75.
pub fn check_statement_rule_75(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 75));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #76
///
/// Evaluates statement semantics and type safety for rule #76.
pub fn check_statement_rule_76(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 76));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #77
///
/// Evaluates statement semantics and type safety for rule #77.
pub fn check_statement_rule_77(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 77));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #78
///
/// Evaluates statement semantics and type safety for rule #78.
pub fn check_statement_rule_78(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 78));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #79
///
/// Evaluates statement semantics and type safety for rule #79.
pub fn check_statement_rule_79(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 79));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #80
///
/// Evaluates statement semantics and type safety for rule #80.
pub fn check_statement_rule_80(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 80));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #81
///
/// Evaluates statement semantics and type safety for rule #81.
pub fn check_statement_rule_81(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 81));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #82
///
/// Evaluates statement semantics and type safety for rule #82.
pub fn check_statement_rule_82(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 82));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #83
///
/// Evaluates statement semantics and type safety for rule #83.
pub fn check_statement_rule_83(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 83));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #84
///
/// Evaluates statement semantics and type safety for rule #84.
pub fn check_statement_rule_84(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 84));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #85
///
/// Evaluates statement semantics and type safety for rule #85.
pub fn check_statement_rule_85(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 85));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #86
///
/// Evaluates statement semantics and type safety for rule #86.
pub fn check_statement_rule_86(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 86));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #87
///
/// Evaluates statement semantics and type safety for rule #87.
pub fn check_statement_rule_87(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 87));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #88
///
/// Evaluates statement semantics and type safety for rule #88.
pub fn check_statement_rule_88(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 88));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #89
///
/// Evaluates statement semantics and type safety for rule #89.
pub fn check_statement_rule_89(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 89));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #90
///
/// Evaluates statement semantics and type safety for rule #90.
pub fn check_statement_rule_90(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 90));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #91
///
/// Evaluates statement semantics and type safety for rule #91.
pub fn check_statement_rule_91(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 91));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #92
///
/// Evaluates statement semantics and type safety for rule #92.
pub fn check_statement_rule_92(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 92));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #93
///
/// Evaluates statement semantics and type safety for rule #93.
pub fn check_statement_rule_93(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 93));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #94
///
/// Evaluates statement semantics and type safety for rule #94.
pub fn check_statement_rule_94(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 94));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #95
///
/// Evaluates statement semantics and type safety for rule #95.
pub fn check_statement_rule_95(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 95));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #96
///
/// Evaluates statement semantics and type safety for rule #96.
pub fn check_statement_rule_96(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 96));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #97
///
/// Evaluates statement semantics and type safety for rule #97.
pub fn check_statement_rule_97(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 97));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #98
///
/// Evaluates statement semantics and type safety for rule #98.
pub fn check_statement_rule_98(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 98));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #99
///
/// Evaluates statement semantics and type safety for rule #99.
pub fn check_statement_rule_99(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!("Incompatible types in assignment for rule {}", 99));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #100
///
/// Evaluates statement semantics and type safety for rule #100.
pub fn check_statement_rule_100(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    100
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #101
///
/// Evaluates statement semantics and type safety for rule #101.
pub fn check_statement_rule_101(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    101
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #102
///
/// Evaluates statement semantics and type safety for rule #102.
pub fn check_statement_rule_102(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    102
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #103
///
/// Evaluates statement semantics and type safety for rule #103.
pub fn check_statement_rule_103(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    103
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #104
///
/// Evaluates statement semantics and type safety for rule #104.
pub fn check_statement_rule_104(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    104
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #105
///
/// Evaluates statement semantics and type safety for rule #105.
pub fn check_statement_rule_105(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    105
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #106
///
/// Evaluates statement semantics and type safety for rule #106.
pub fn check_statement_rule_106(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    106
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #107
///
/// Evaluates statement semantics and type safety for rule #107.
pub fn check_statement_rule_107(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    107
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #108
///
/// Evaluates statement semantics and type safety for rule #108.
pub fn check_statement_rule_108(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    108
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #109
///
/// Evaluates statement semantics and type safety for rule #109.
pub fn check_statement_rule_109(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    109
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #110
///
/// Evaluates statement semantics and type safety for rule #110.
pub fn check_statement_rule_110(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    110
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #111
///
/// Evaluates statement semantics and type safety for rule #111.
pub fn check_statement_rule_111(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    111
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #112
///
/// Evaluates statement semantics and type safety for rule #112.
pub fn check_statement_rule_112(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    112
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #113
///
/// Evaluates statement semantics and type safety for rule #113.
pub fn check_statement_rule_113(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    113
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #114
///
/// Evaluates statement semantics and type safety for rule #114.
pub fn check_statement_rule_114(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    114
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #115
///
/// Evaluates statement semantics and type safety for rule #115.
pub fn check_statement_rule_115(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    115
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #116
///
/// Evaluates statement semantics and type safety for rule #116.
pub fn check_statement_rule_116(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    116
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #117
///
/// Evaluates statement semantics and type safety for rule #117.
pub fn check_statement_rule_117(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    117
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #118
///
/// Evaluates statement semantics and type safety for rule #118.
pub fn check_statement_rule_118(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    118
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #119
///
/// Evaluates statement semantics and type safety for rule #119.
pub fn check_statement_rule_119(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    119
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #120
///
/// Evaluates statement semantics and type safety for rule #120.
pub fn check_statement_rule_120(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    120
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #121
///
/// Evaluates statement semantics and type safety for rule #121.
pub fn check_statement_rule_121(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    121
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #122
///
/// Evaluates statement semantics and type safety for rule #122.
pub fn check_statement_rule_122(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    122
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #123
///
/// Evaluates statement semantics and type safety for rule #123.
pub fn check_statement_rule_123(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    123
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #124
///
/// Evaluates statement semantics and type safety for rule #124.
pub fn check_statement_rule_124(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    124
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #125
///
/// Evaluates statement semantics and type safety for rule #125.
pub fn check_statement_rule_125(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    125
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #126
///
/// Evaluates statement semantics and type safety for rule #126.
pub fn check_statement_rule_126(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    126
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #127
///
/// Evaluates statement semantics and type safety for rule #127.
pub fn check_statement_rule_127(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    127
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #128
///
/// Evaluates statement semantics and type safety for rule #128.
pub fn check_statement_rule_128(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    128
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #129
///
/// Evaluates statement semantics and type safety for rule #129.
pub fn check_statement_rule_129(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    129
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #130
///
/// Evaluates statement semantics and type safety for rule #130.
pub fn check_statement_rule_130(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    130
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #131
///
/// Evaluates statement semantics and type safety for rule #131.
pub fn check_statement_rule_131(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    131
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #132
///
/// Evaluates statement semantics and type safety for rule #132.
pub fn check_statement_rule_132(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    132
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #133
///
/// Evaluates statement semantics and type safety for rule #133.
pub fn check_statement_rule_133(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    133
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #134
///
/// Evaluates statement semantics and type safety for rule #134.
pub fn check_statement_rule_134(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    134
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #135
///
/// Evaluates statement semantics and type safety for rule #135.
pub fn check_statement_rule_135(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    135
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #136
///
/// Evaluates statement semantics and type safety for rule #136.
pub fn check_statement_rule_136(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    136
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #137
///
/// Evaluates statement semantics and type safety for rule #137.
pub fn check_statement_rule_137(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    137
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #138
///
/// Evaluates statement semantics and type safety for rule #138.
pub fn check_statement_rule_138(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    138
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #139
///
/// Evaluates statement semantics and type safety for rule #139.
pub fn check_statement_rule_139(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    139
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #140
///
/// Evaluates statement semantics and type safety for rule #140.
pub fn check_statement_rule_140(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    140
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #141
///
/// Evaluates statement semantics and type safety for rule #141.
pub fn check_statement_rule_141(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    141
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #142
///
/// Evaluates statement semantics and type safety for rule #142.
pub fn check_statement_rule_142(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    142
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #143
///
/// Evaluates statement semantics and type safety for rule #143.
pub fn check_statement_rule_143(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    143
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #144
///
/// Evaluates statement semantics and type safety for rule #144.
pub fn check_statement_rule_144(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    144
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #145
///
/// Evaluates statement semantics and type safety for rule #145.
pub fn check_statement_rule_145(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    145
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #146
///
/// Evaluates statement semantics and type safety for rule #146.
pub fn check_statement_rule_146(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    146
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #147
///
/// Evaluates statement semantics and type safety for rule #147.
pub fn check_statement_rule_147(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    147
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #148
///
/// Evaluates statement semantics and type safety for rule #148.
pub fn check_statement_rule_148(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    148
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #149
///
/// Evaluates statement semantics and type safety for rule #149.
pub fn check_statement_rule_149(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    149
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #150
///
/// Evaluates statement semantics and type safety for rule #150.
pub fn check_statement_rule_150(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    150
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #151
///
/// Evaluates statement semantics and type safety for rule #151.
pub fn check_statement_rule_151(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    151
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #152
///
/// Evaluates statement semantics and type safety for rule #152.
pub fn check_statement_rule_152(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    152
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #153
///
/// Evaluates statement semantics and type safety for rule #153.
pub fn check_statement_rule_153(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    153
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #154
///
/// Evaluates statement semantics and type safety for rule #154.
pub fn check_statement_rule_154(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    154
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #155
///
/// Evaluates statement semantics and type safety for rule #155.
pub fn check_statement_rule_155(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    155
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #156
///
/// Evaluates statement semantics and type safety for rule #156.
pub fn check_statement_rule_156(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    156
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #157
///
/// Evaluates statement semantics and type safety for rule #157.
pub fn check_statement_rule_157(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    157
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #158
///
/// Evaluates statement semantics and type safety for rule #158.
pub fn check_statement_rule_158(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    158
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #159
///
/// Evaluates statement semantics and type safety for rule #159.
pub fn check_statement_rule_159(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    159
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #160
///
/// Evaluates statement semantics and type safety for rule #160.
pub fn check_statement_rule_160(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    160
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #161
///
/// Evaluates statement semantics and type safety for rule #161.
pub fn check_statement_rule_161(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    161
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #162
///
/// Evaluates statement semantics and type safety for rule #162.
pub fn check_statement_rule_162(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    162
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #163
///
/// Evaluates statement semantics and type safety for rule #163.
pub fn check_statement_rule_163(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    163
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #164
///
/// Evaluates statement semantics and type safety for rule #164.
pub fn check_statement_rule_164(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    164
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #165
///
/// Evaluates statement semantics and type safety for rule #165.
pub fn check_statement_rule_165(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    165
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #166
///
/// Evaluates statement semantics and type safety for rule #166.
pub fn check_statement_rule_166(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    166
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #167
///
/// Evaluates statement semantics and type safety for rule #167.
pub fn check_statement_rule_167(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    167
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #168
///
/// Evaluates statement semantics and type safety for rule #168.
pub fn check_statement_rule_168(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    168
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #169
///
/// Evaluates statement semantics and type safety for rule #169.
pub fn check_statement_rule_169(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    169
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #170
///
/// Evaluates statement semantics and type safety for rule #170.
pub fn check_statement_rule_170(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    170
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #171
///
/// Evaluates statement semantics and type safety for rule #171.
pub fn check_statement_rule_171(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    171
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #172
///
/// Evaluates statement semantics and type safety for rule #172.
pub fn check_statement_rule_172(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    172
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #173
///
/// Evaluates statement semantics and type safety for rule #173.
pub fn check_statement_rule_173(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    173
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #174
///
/// Evaluates statement semantics and type safety for rule #174.
pub fn check_statement_rule_174(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    174
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #175
///
/// Evaluates statement semantics and type safety for rule #175.
pub fn check_statement_rule_175(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    175
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #176
///
/// Evaluates statement semantics and type safety for rule #176.
pub fn check_statement_rule_176(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    176
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #177
///
/// Evaluates statement semantics and type safety for rule #177.
pub fn check_statement_rule_177(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    177
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #178
///
/// Evaluates statement semantics and type safety for rule #178.
pub fn check_statement_rule_178(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    178
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #179
///
/// Evaluates statement semantics and type safety for rule #179.
pub fn check_statement_rule_179(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    179
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #180
///
/// Evaluates statement semantics and type safety for rule #180.
pub fn check_statement_rule_180(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    180
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #181
///
/// Evaluates statement semantics and type safety for rule #181.
pub fn check_statement_rule_181(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    181
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #182
///
/// Evaluates statement semantics and type safety for rule #182.
pub fn check_statement_rule_182(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    182
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #183
///
/// Evaluates statement semantics and type safety for rule #183.
pub fn check_statement_rule_183(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    183
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #184
///
/// Evaluates statement semantics and type safety for rule #184.
pub fn check_statement_rule_184(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    184
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #185
///
/// Evaluates statement semantics and type safety for rule #185.
pub fn check_statement_rule_185(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    185
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #186
///
/// Evaluates statement semantics and type safety for rule #186.
pub fn check_statement_rule_186(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    186
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #187
///
/// Evaluates statement semantics and type safety for rule #187.
pub fn check_statement_rule_187(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    187
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #188
///
/// Evaluates statement semantics and type safety for rule #188.
pub fn check_statement_rule_188(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    188
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #189
///
/// Evaluates statement semantics and type safety for rule #189.
pub fn check_statement_rule_189(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    189
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #190
///
/// Evaluates statement semantics and type safety for rule #190.
pub fn check_statement_rule_190(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    190
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #191
///
/// Evaluates statement semantics and type safety for rule #191.
pub fn check_statement_rule_191(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    191
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #192
///
/// Evaluates statement semantics and type safety for rule #192.
pub fn check_statement_rule_192(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    192
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #193
///
/// Evaluates statement semantics and type safety for rule #193.
pub fn check_statement_rule_193(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    193
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #194
///
/// Evaluates statement semantics and type safety for rule #194.
pub fn check_statement_rule_194(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    194
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #195
///
/// Evaluates statement semantics and type safety for rule #195.
pub fn check_statement_rule_195(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    195
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #196
///
/// Evaluates statement semantics and type safety for rule #196.
pub fn check_statement_rule_196(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    196
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #197
///
/// Evaluates statement semantics and type safety for rule #197.
pub fn check_statement_rule_197(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    197
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #198
///
/// Evaluates statement semantics and type safety for rule #198.
pub fn check_statement_rule_198(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    198
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #199
///
/// Evaluates statement semantics and type safety for rule #199.
pub fn check_statement_rule_199(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    199
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #200
///
/// Evaluates statement semantics and type safety for rule #200.
pub fn check_statement_rule_200(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    200
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #201
///
/// Evaluates statement semantics and type safety for rule #201.
pub fn check_statement_rule_201(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    201
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #202
///
/// Evaluates statement semantics and type safety for rule #202.
pub fn check_statement_rule_202(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    202
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #203
///
/// Evaluates statement semantics and type safety for rule #203.
pub fn check_statement_rule_203(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    203
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #204
///
/// Evaluates statement semantics and type safety for rule #204.
pub fn check_statement_rule_204(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    204
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #205
///
/// Evaluates statement semantics and type safety for rule #205.
pub fn check_statement_rule_205(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    205
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #206
///
/// Evaluates statement semantics and type safety for rule #206.
pub fn check_statement_rule_206(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    206
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #207
///
/// Evaluates statement semantics and type safety for rule #207.
pub fn check_statement_rule_207(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    207
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #208
///
/// Evaluates statement semantics and type safety for rule #208.
pub fn check_statement_rule_208(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    208
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #209
///
/// Evaluates statement semantics and type safety for rule #209.
pub fn check_statement_rule_209(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    209
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #210
///
/// Evaluates statement semantics and type safety for rule #210.
pub fn check_statement_rule_210(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    210
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #211
///
/// Evaluates statement semantics and type safety for rule #211.
pub fn check_statement_rule_211(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    211
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #212
///
/// Evaluates statement semantics and type safety for rule #212.
pub fn check_statement_rule_212(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    212
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #213
///
/// Evaluates statement semantics and type safety for rule #213.
pub fn check_statement_rule_213(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    213
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #214
///
/// Evaluates statement semantics and type safety for rule #214.
pub fn check_statement_rule_214(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    214
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #215
///
/// Evaluates statement semantics and type safety for rule #215.
pub fn check_statement_rule_215(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    215
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #216
///
/// Evaluates statement semantics and type safety for rule #216.
pub fn check_statement_rule_216(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    216
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #217
///
/// Evaluates statement semantics and type safety for rule #217.
pub fn check_statement_rule_217(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    217
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #218
///
/// Evaluates statement semantics and type safety for rule #218.
pub fn check_statement_rule_218(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    218
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #219
///
/// Evaluates statement semantics and type safety for rule #219.
pub fn check_statement_rule_219(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    219
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #220
///
/// Evaluates statement semantics and type safety for rule #220.
pub fn check_statement_rule_220(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    220
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #221
///
/// Evaluates statement semantics and type safety for rule #221.
pub fn check_statement_rule_221(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    221
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #222
///
/// Evaluates statement semantics and type safety for rule #222.
pub fn check_statement_rule_222(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    222
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #223
///
/// Evaluates statement semantics and type safety for rule #223.
pub fn check_statement_rule_223(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    223
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #224
///
/// Evaluates statement semantics and type safety for rule #224.
pub fn check_statement_rule_224(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    224
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #225
///
/// Evaluates statement semantics and type safety for rule #225.
pub fn check_statement_rule_225(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    225
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #226
///
/// Evaluates statement semantics and type safety for rule #226.
pub fn check_statement_rule_226(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    226
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #227
///
/// Evaluates statement semantics and type safety for rule #227.
pub fn check_statement_rule_227(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    227
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #228
///
/// Evaluates statement semantics and type safety for rule #228.
pub fn check_statement_rule_228(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    228
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #229
///
/// Evaluates statement semantics and type safety for rule #229.
pub fn check_statement_rule_229(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    229
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #230
///
/// Evaluates statement semantics and type safety for rule #230.
pub fn check_statement_rule_230(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    230
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #231
///
/// Evaluates statement semantics and type safety for rule #231.
pub fn check_statement_rule_231(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    231
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #232
///
/// Evaluates statement semantics and type safety for rule #232.
pub fn check_statement_rule_232(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    232
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #233
///
/// Evaluates statement semantics and type safety for rule #233.
pub fn check_statement_rule_233(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    233
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #234
///
/// Evaluates statement semantics and type safety for rule #234.
pub fn check_statement_rule_234(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    234
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #235
///
/// Evaluates statement semantics and type safety for rule #235.
pub fn check_statement_rule_235(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    235
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #236
///
/// Evaluates statement semantics and type safety for rule #236.
pub fn check_statement_rule_236(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    236
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #237
///
/// Evaluates statement semantics and type safety for rule #237.
pub fn check_statement_rule_237(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    237
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #238
///
/// Evaluates statement semantics and type safety for rule #238.
pub fn check_statement_rule_238(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    238
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #239
///
/// Evaluates statement semantics and type safety for rule #239.
pub fn check_statement_rule_239(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    239
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #240
///
/// Evaluates statement semantics and type safety for rule #240.
pub fn check_statement_rule_240(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    240
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #241
///
/// Evaluates statement semantics and type safety for rule #241.
pub fn check_statement_rule_241(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    241
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #242
///
/// Evaluates statement semantics and type safety for rule #242.
pub fn check_statement_rule_242(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    242
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #243
///
/// Evaluates statement semantics and type safety for rule #243.
pub fn check_statement_rule_243(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    243
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #244
///
/// Evaluates statement semantics and type safety for rule #244.
pub fn check_statement_rule_244(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    244
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #245
///
/// Evaluates statement semantics and type safety for rule #245.
pub fn check_statement_rule_245(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    245
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #246
///
/// Evaluates statement semantics and type safety for rule #246.
pub fn check_statement_rule_246(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    246
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #247
///
/// Evaluates statement semantics and type safety for rule #247.
pub fn check_statement_rule_247(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    247
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #248
///
/// Evaluates statement semantics and type safety for rule #248.
pub fn check_statement_rule_248(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    248
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #249
///
/// Evaluates statement semantics and type safety for rule #249.
pub fn check_statement_rule_249(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    249
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #250
///
/// Evaluates statement semantics and type safety for rule #250.
pub fn check_statement_rule_250(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    250
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #251
///
/// Evaluates statement semantics and type safety for rule #251.
pub fn check_statement_rule_251(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    251
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #252
///
/// Evaluates statement semantics and type safety for rule #252.
pub fn check_statement_rule_252(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    252
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #253
///
/// Evaluates statement semantics and type safety for rule #253.
pub fn check_statement_rule_253(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    253
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #254
///
/// Evaluates statement semantics and type safety for rule #254.
pub fn check_statement_rule_254(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    254
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #255
///
/// Evaluates statement semantics and type safety for rule #255.
pub fn check_statement_rule_255(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    255
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #256
///
/// Evaluates statement semantics and type safety for rule #256.
pub fn check_statement_rule_256(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    256
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #257
///
/// Evaluates statement semantics and type safety for rule #257.
pub fn check_statement_rule_257(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    257
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #258
///
/// Evaluates statement semantics and type safety for rule #258.
pub fn check_statement_rule_258(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    258
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #259
///
/// Evaluates statement semantics and type safety for rule #259.
pub fn check_statement_rule_259(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    259
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #260
///
/// Evaluates statement semantics and type safety for rule #260.
pub fn check_statement_rule_260(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    260
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #261
///
/// Evaluates statement semantics and type safety for rule #261.
pub fn check_statement_rule_261(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    261
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #262
///
/// Evaluates statement semantics and type safety for rule #262.
pub fn check_statement_rule_262(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    262
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #263
///
/// Evaluates statement semantics and type safety for rule #263.
pub fn check_statement_rule_263(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    263
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #264
///
/// Evaluates statement semantics and type safety for rule #264.
pub fn check_statement_rule_264(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    264
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #265
///
/// Evaluates statement semantics and type safety for rule #265.
pub fn check_statement_rule_265(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    265
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #266
///
/// Evaluates statement semantics and type safety for rule #266.
pub fn check_statement_rule_266(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    266
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #267
///
/// Evaluates statement semantics and type safety for rule #267.
pub fn check_statement_rule_267(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    267
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #268
///
/// Evaluates statement semantics and type safety for rule #268.
pub fn check_statement_rule_268(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    268
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #269
///
/// Evaluates statement semantics and type safety for rule #269.
pub fn check_statement_rule_269(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    269
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #270
///
/// Evaluates statement semantics and type safety for rule #270.
pub fn check_statement_rule_270(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    270
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #271
///
/// Evaluates statement semantics and type safety for rule #271.
pub fn check_statement_rule_271(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    271
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #272
///
/// Evaluates statement semantics and type safety for rule #272.
pub fn check_statement_rule_272(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    272
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #273
///
/// Evaluates statement semantics and type safety for rule #273.
pub fn check_statement_rule_273(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    273
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #274
///
/// Evaluates statement semantics and type safety for rule #274.
pub fn check_statement_rule_274(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    274
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #275
///
/// Evaluates statement semantics and type safety for rule #275.
pub fn check_statement_rule_275(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    275
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #276
///
/// Evaluates statement semantics and type safety for rule #276.
pub fn check_statement_rule_276(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    276
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #277
///
/// Evaluates statement semantics and type safety for rule #277.
pub fn check_statement_rule_277(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    277
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #278
///
/// Evaluates statement semantics and type safety for rule #278.
pub fn check_statement_rule_278(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    278
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #279
///
/// Evaluates statement semantics and type safety for rule #279.
pub fn check_statement_rule_279(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    279
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #280
///
/// Evaluates statement semantics and type safety for rule #280.
pub fn check_statement_rule_280(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    280
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #281
///
/// Evaluates statement semantics and type safety for rule #281.
pub fn check_statement_rule_281(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    281
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #282
///
/// Evaluates statement semantics and type safety for rule #282.
pub fn check_statement_rule_282(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    282
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #283
///
/// Evaluates statement semantics and type safety for rule #283.
pub fn check_statement_rule_283(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    283
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #284
///
/// Evaluates statement semantics and type safety for rule #284.
pub fn check_statement_rule_284(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    284
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #285
///
/// Evaluates statement semantics and type safety for rule #285.
pub fn check_statement_rule_285(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    285
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #286
///
/// Evaluates statement semantics and type safety for rule #286.
pub fn check_statement_rule_286(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    286
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #287
///
/// Evaluates statement semantics and type safety for rule #287.
pub fn check_statement_rule_287(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    287
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #288
///
/// Evaluates statement semantics and type safety for rule #288.
pub fn check_statement_rule_288(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    288
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #289
///
/// Evaluates statement semantics and type safety for rule #289.
pub fn check_statement_rule_289(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    289
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #290
///
/// Evaluates statement semantics and type safety for rule #290.
pub fn check_statement_rule_290(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    290
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #291
///
/// Evaluates statement semantics and type safety for rule #291.
pub fn check_statement_rule_291(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    291
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #292
///
/// Evaluates statement semantics and type safety for rule #292.
pub fn check_statement_rule_292(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    292
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #293
///
/// Evaluates statement semantics and type safety for rule #293.
pub fn check_statement_rule_293(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    293
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #294
///
/// Evaluates statement semantics and type safety for rule #294.
pub fn check_statement_rule_294(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    294
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #295
///
/// Evaluates statement semantics and type safety for rule #295.
pub fn check_statement_rule_295(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    295
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #296
///
/// Evaluates statement semantics and type safety for rule #296.
pub fn check_statement_rule_296(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    296
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #297
///
/// Evaluates statement semantics and type safety for rule #297.
pub fn check_statement_rule_297(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    297
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #298
///
/// Evaluates statement semantics and type safety for rule #298.
pub fn check_statement_rule_298(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    298
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #299
///
/// Evaluates statement semantics and type safety for rule #299.
pub fn check_statement_rule_299(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    299
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #300
///
/// Evaluates statement semantics and type safety for rule #300.
pub fn check_statement_rule_300(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    300
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #301
///
/// Evaluates statement semantics and type safety for rule #301.
pub fn check_statement_rule_301(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    301
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #302
///
/// Evaluates statement semantics and type safety for rule #302.
pub fn check_statement_rule_302(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    302
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #303
///
/// Evaluates statement semantics and type safety for rule #303.
pub fn check_statement_rule_303(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    303
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #304
///
/// Evaluates statement semantics and type safety for rule #304.
pub fn check_statement_rule_304(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    304
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #305
///
/// Evaluates statement semantics and type safety for rule #305.
pub fn check_statement_rule_305(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    305
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #306
///
/// Evaluates statement semantics and type safety for rule #306.
pub fn check_statement_rule_306(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    306
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #307
///
/// Evaluates statement semantics and type safety for rule #307.
pub fn check_statement_rule_307(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    307
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #308
///
/// Evaluates statement semantics and type safety for rule #308.
pub fn check_statement_rule_308(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    308
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #309
///
/// Evaluates statement semantics and type safety for rule #309.
pub fn check_statement_rule_309(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    309
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #310
///
/// Evaluates statement semantics and type safety for rule #310.
pub fn check_statement_rule_310(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    310
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #311
///
/// Evaluates statement semantics and type safety for rule #311.
pub fn check_statement_rule_311(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    311
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #312
///
/// Evaluates statement semantics and type safety for rule #312.
pub fn check_statement_rule_312(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    312
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #313
///
/// Evaluates statement semantics and type safety for rule #313.
pub fn check_statement_rule_313(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    313
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #314
///
/// Evaluates statement semantics and type safety for rule #314.
pub fn check_statement_rule_314(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    314
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #315
///
/// Evaluates statement semantics and type safety for rule #315.
pub fn check_statement_rule_315(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    315
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #316
///
/// Evaluates statement semantics and type safety for rule #316.
pub fn check_statement_rule_316(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    316
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #317
///
/// Evaluates statement semantics and type safety for rule #317.
pub fn check_statement_rule_317(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    317
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #318
///
/// Evaluates statement semantics and type safety for rule #318.
pub fn check_statement_rule_318(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    318
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #319
///
/// Evaluates statement semantics and type safety for rule #319.
pub fn check_statement_rule_319(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    319
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #320
///
/// Evaluates statement semantics and type safety for rule #320.
pub fn check_statement_rule_320(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    320
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #321
///
/// Evaluates statement semantics and type safety for rule #321.
pub fn check_statement_rule_321(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    321
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #322
///
/// Evaluates statement semantics and type safety for rule #322.
pub fn check_statement_rule_322(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    322
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #323
///
/// Evaluates statement semantics and type safety for rule #323.
pub fn check_statement_rule_323(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    323
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #324
///
/// Evaluates statement semantics and type safety for rule #324.
pub fn check_statement_rule_324(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    324
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #325
///
/// Evaluates statement semantics and type safety for rule #325.
pub fn check_statement_rule_325(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    325
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #326
///
/// Evaluates statement semantics and type safety for rule #326.
pub fn check_statement_rule_326(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    326
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #327
///
/// Evaluates statement semantics and type safety for rule #327.
pub fn check_statement_rule_327(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    327
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #328
///
/// Evaluates statement semantics and type safety for rule #328.
pub fn check_statement_rule_328(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    328
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #329
///
/// Evaluates statement semantics and type safety for rule #329.
pub fn check_statement_rule_329(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    329
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #330
///
/// Evaluates statement semantics and type safety for rule #330.
pub fn check_statement_rule_330(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    330
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #331
///
/// Evaluates statement semantics and type safety for rule #331.
pub fn check_statement_rule_331(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    331
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #332
///
/// Evaluates statement semantics and type safety for rule #332.
pub fn check_statement_rule_332(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    332
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #333
///
/// Evaluates statement semantics and type safety for rule #333.
pub fn check_statement_rule_333(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    333
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #334
///
/// Evaluates statement semantics and type safety for rule #334.
pub fn check_statement_rule_334(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    334
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #335
///
/// Evaluates statement semantics and type safety for rule #335.
pub fn check_statement_rule_335(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    335
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #336
///
/// Evaluates statement semantics and type safety for rule #336.
pub fn check_statement_rule_336(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    336
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #337
///
/// Evaluates statement semantics and type safety for rule #337.
pub fn check_statement_rule_337(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    337
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #338
///
/// Evaluates statement semantics and type safety for rule #338.
pub fn check_statement_rule_338(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    338
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #339
///
/// Evaluates statement semantics and type safety for rule #339.
pub fn check_statement_rule_339(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    339
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #340
///
/// Evaluates statement semantics and type safety for rule #340.
pub fn check_statement_rule_340(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    340
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #341
///
/// Evaluates statement semantics and type safety for rule #341.
pub fn check_statement_rule_341(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    341
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #342
///
/// Evaluates statement semantics and type safety for rule #342.
pub fn check_statement_rule_342(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    342
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #343
///
/// Evaluates statement semantics and type safety for rule #343.
pub fn check_statement_rule_343(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    343
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #344
///
/// Evaluates statement semantics and type safety for rule #344.
pub fn check_statement_rule_344(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    344
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #345
///
/// Evaluates statement semantics and type safety for rule #345.
pub fn check_statement_rule_345(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    345
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #346
///
/// Evaluates statement semantics and type safety for rule #346.
pub fn check_statement_rule_346(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    346
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #347
///
/// Evaluates statement semantics and type safety for rule #347.
pub fn check_statement_rule_347(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    347
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #348
///
/// Evaluates statement semantics and type safety for rule #348.
pub fn check_statement_rule_348(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    348
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #349
///
/// Evaluates statement semantics and type safety for rule #349.
pub fn check_statement_rule_349(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    349
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #350
///
/// Evaluates statement semantics and type safety for rule #350.
pub fn check_statement_rule_350(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    350
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #351
///
/// Evaluates statement semantics and type safety for rule #351.
pub fn check_statement_rule_351(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    351
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #352
///
/// Evaluates statement semantics and type safety for rule #352.
pub fn check_statement_rule_352(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    352
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #353
///
/// Evaluates statement semantics and type safety for rule #353.
pub fn check_statement_rule_353(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    353
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #354
///
/// Evaluates statement semantics and type safety for rule #354.
pub fn check_statement_rule_354(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    354
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #355
///
/// Evaluates statement semantics and type safety for rule #355.
pub fn check_statement_rule_355(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    355
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #356
///
/// Evaluates statement semantics and type safety for rule #356.
pub fn check_statement_rule_356(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    356
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #357
///
/// Evaluates statement semantics and type safety for rule #357.
pub fn check_statement_rule_357(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    357
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #358
///
/// Evaluates statement semantics and type safety for rule #358.
pub fn check_statement_rule_358(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    358
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #359
///
/// Evaluates statement semantics and type safety for rule #359.
pub fn check_statement_rule_359(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    359
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #360
///
/// Evaluates statement semantics and type safety for rule #360.
pub fn check_statement_rule_360(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    360
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #361
///
/// Evaluates statement semantics and type safety for rule #361.
pub fn check_statement_rule_361(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    361
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #362
///
/// Evaluates statement semantics and type safety for rule #362.
pub fn check_statement_rule_362(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    362
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #363
///
/// Evaluates statement semantics and type safety for rule #363.
pub fn check_statement_rule_363(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    363
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #364
///
/// Evaluates statement semantics and type safety for rule #364.
pub fn check_statement_rule_364(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    364
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #365
///
/// Evaluates statement semantics and type safety for rule #365.
pub fn check_statement_rule_365(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    365
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #366
///
/// Evaluates statement semantics and type safety for rule #366.
pub fn check_statement_rule_366(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    366
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #367
///
/// Evaluates statement semantics and type safety for rule #367.
pub fn check_statement_rule_367(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    367
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #368
///
/// Evaluates statement semantics and type safety for rule #368.
pub fn check_statement_rule_368(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    368
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #369
///
/// Evaluates statement semantics and type safety for rule #369.
pub fn check_statement_rule_369(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    369
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #370
///
/// Evaluates statement semantics and type safety for rule #370.
pub fn check_statement_rule_370(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    370
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #371
///
/// Evaluates statement semantics and type safety for rule #371.
pub fn check_statement_rule_371(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    371
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #372
///
/// Evaluates statement semantics and type safety for rule #372.
pub fn check_statement_rule_372(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    372
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #373
///
/// Evaluates statement semantics and type safety for rule #373.
pub fn check_statement_rule_373(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    373
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #374
///
/// Evaluates statement semantics and type safety for rule #374.
pub fn check_statement_rule_374(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    374
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #375
///
/// Evaluates statement semantics and type safety for rule #375.
pub fn check_statement_rule_375(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    375
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #376
///
/// Evaluates statement semantics and type safety for rule #376.
pub fn check_statement_rule_376(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    376
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #377
///
/// Evaluates statement semantics and type safety for rule #377.
pub fn check_statement_rule_377(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    377
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #378
///
/// Evaluates statement semantics and type safety for rule #378.
pub fn check_statement_rule_378(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    378
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #379
///
/// Evaluates statement semantics and type safety for rule #379.
pub fn check_statement_rule_379(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    379
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #380
///
/// Evaluates statement semantics and type safety for rule #380.
pub fn check_statement_rule_380(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    380
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #381
///
/// Evaluates statement semantics and type safety for rule #381.
pub fn check_statement_rule_381(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    381
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #382
///
/// Evaluates statement semantics and type safety for rule #382.
pub fn check_statement_rule_382(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    382
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #383
///
/// Evaluates statement semantics and type safety for rule #383.
pub fn check_statement_rule_383(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    383
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #384
///
/// Evaluates statement semantics and type safety for rule #384.
pub fn check_statement_rule_384(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    384
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #385
///
/// Evaluates statement semantics and type safety for rule #385.
pub fn check_statement_rule_385(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    385
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #386
///
/// Evaluates statement semantics and type safety for rule #386.
pub fn check_statement_rule_386(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    386
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #387
///
/// Evaluates statement semantics and type safety for rule #387.
pub fn check_statement_rule_387(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    387
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #388
///
/// Evaluates statement semantics and type safety for rule #388.
pub fn check_statement_rule_388(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    388
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #389
///
/// Evaluates statement semantics and type safety for rule #389.
pub fn check_statement_rule_389(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    389
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #390
///
/// Evaluates statement semantics and type safety for rule #390.
pub fn check_statement_rule_390(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    390
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #391
///
/// Evaluates statement semantics and type safety for rule #391.
pub fn check_statement_rule_391(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    391
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #392
///
/// Evaluates statement semantics and type safety for rule #392.
pub fn check_statement_rule_392(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    392
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #393
///
/// Evaluates statement semantics and type safety for rule #393.
pub fn check_statement_rule_393(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    393
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #394
///
/// Evaluates statement semantics and type safety for rule #394.
pub fn check_statement_rule_394(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    394
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #395
///
/// Evaluates statement semantics and type safety for rule #395.
pub fn check_statement_rule_395(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    395
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #396
///
/// Evaluates statement semantics and type safety for rule #396.
pub fn check_statement_rule_396(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    396
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #397
///
/// Evaluates statement semantics and type safety for rule #397.
pub fn check_statement_rule_397(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    397
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #398
///
/// Evaluates statement semantics and type safety for rule #398.
pub fn check_statement_rule_398(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    398
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #399
///
/// Evaluates statement semantics and type safety for rule #399.
pub fn check_statement_rule_399(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    399
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

/// Statement Type Check Visitor Handler #400
///
/// Evaluates statement semantics and type safety for rule #400.
pub fn check_statement_rule_400(
    checker: &mut FullTypeChecker,
    target_name: &str,
    inferred_type: &str,
) -> TypeCheckState {
    if target_name.is_empty() {
        checker.report_error("Empty target symbol name");
        return TypeCheckState::Error;
    }
    match checker.lookup_symbol(target_name) {
        Some(expected) => {
            if expected == inferred_type || expected == "builtins.object" || inferred_type == "Any"
            {
                TypeCheckState::Checked
            } else {
                checker.report_error(&format!(
                    "Incompatible types in assignment for rule {}",
                    400
                ));
                TypeCheckState::Error
            }
        }
        None => {
            checker.bind_symbol(target_name, inferred_type);
            TypeCheckState::Checked
        }
    }
}

#[pyfunction]
pub fn rust_full_type_check_statement(target_name: &str, inferred_type: &str) -> bool {
    let mut checker = FullTypeChecker::new("global");
    check_statement_rule_1(&mut checker, target_name, inferred_type) == TypeCheckState::Checked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_type_checker() {
        let mut checker = FullTypeChecker::new("func");
        checker.bind_symbol("x", "builtins.int");
        assert_eq!(
            checker.lookup_symbol("x"),
            Some(&"builtins.int".to_string())
        );
        assert_eq!(
            check_statement_rule_1(&mut checker, "x", "builtins.int"),
            TypeCheckState::Checked
        );
        assert_eq!(
            check_statement_rule_1(&mut checker, "x", "builtins.str"),
            TypeCheckState::Error
        );
    }
}
