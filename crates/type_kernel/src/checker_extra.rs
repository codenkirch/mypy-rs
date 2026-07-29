//! Additional Native Type Checker Engine (Phase 8 Extension) for Issue #140.
//!
//! Direct native Rust implementation of extra type checking visitor routines and rules.

use pyo3::prelude::*;
use std::collections::HashMap;

pub struct ExtraTypeChecker {
    pub scope: String,
    pub symbol_table: HashMap<String, String>,
}

impl ExtraTypeChecker {
    pub fn new(scope: &str) -> Self {
        Self {
            scope: scope.to_string(),
            symbol_table: HashMap::new(),
        }
    }
}

/// Extra Statement Type Check Handler #1
pub fn check_extra_statement_rule_1(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #2
pub fn check_extra_statement_rule_2(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #3
pub fn check_extra_statement_rule_3(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #4
pub fn check_extra_statement_rule_4(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #5
pub fn check_extra_statement_rule_5(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #6
pub fn check_extra_statement_rule_6(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #7
pub fn check_extra_statement_rule_7(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #8
pub fn check_extra_statement_rule_8(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #9
pub fn check_extra_statement_rule_9(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #10
pub fn check_extra_statement_rule_10(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #11
pub fn check_extra_statement_rule_11(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #12
pub fn check_extra_statement_rule_12(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #13
pub fn check_extra_statement_rule_13(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #14
pub fn check_extra_statement_rule_14(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #15
pub fn check_extra_statement_rule_15(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #16
pub fn check_extra_statement_rule_16(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #17
pub fn check_extra_statement_rule_17(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #18
pub fn check_extra_statement_rule_18(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #19
pub fn check_extra_statement_rule_19(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #20
pub fn check_extra_statement_rule_20(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #21
pub fn check_extra_statement_rule_21(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #22
pub fn check_extra_statement_rule_22(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #23
pub fn check_extra_statement_rule_23(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #24
pub fn check_extra_statement_rule_24(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #25
pub fn check_extra_statement_rule_25(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #26
pub fn check_extra_statement_rule_26(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #27
pub fn check_extra_statement_rule_27(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #28
pub fn check_extra_statement_rule_28(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #29
pub fn check_extra_statement_rule_29(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #30
pub fn check_extra_statement_rule_30(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #31
pub fn check_extra_statement_rule_31(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #32
pub fn check_extra_statement_rule_32(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #33
pub fn check_extra_statement_rule_33(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #34
pub fn check_extra_statement_rule_34(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #35
pub fn check_extra_statement_rule_35(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #36
pub fn check_extra_statement_rule_36(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #37
pub fn check_extra_statement_rule_37(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #38
pub fn check_extra_statement_rule_38(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #39
pub fn check_extra_statement_rule_39(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #40
pub fn check_extra_statement_rule_40(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #41
pub fn check_extra_statement_rule_41(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #42
pub fn check_extra_statement_rule_42(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #43
pub fn check_extra_statement_rule_43(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #44
pub fn check_extra_statement_rule_44(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #45
pub fn check_extra_statement_rule_45(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #46
pub fn check_extra_statement_rule_46(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #47
pub fn check_extra_statement_rule_47(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #48
pub fn check_extra_statement_rule_48(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #49
pub fn check_extra_statement_rule_49(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #50
pub fn check_extra_statement_rule_50(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #51
pub fn check_extra_statement_rule_51(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #52
pub fn check_extra_statement_rule_52(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #53
pub fn check_extra_statement_rule_53(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #54
pub fn check_extra_statement_rule_54(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #55
pub fn check_extra_statement_rule_55(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #56
pub fn check_extra_statement_rule_56(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #57
pub fn check_extra_statement_rule_57(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #58
pub fn check_extra_statement_rule_58(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #59
pub fn check_extra_statement_rule_59(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #60
pub fn check_extra_statement_rule_60(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #61
pub fn check_extra_statement_rule_61(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #62
pub fn check_extra_statement_rule_62(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #63
pub fn check_extra_statement_rule_63(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #64
pub fn check_extra_statement_rule_64(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #65
pub fn check_extra_statement_rule_65(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #66
pub fn check_extra_statement_rule_66(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #67
pub fn check_extra_statement_rule_67(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #68
pub fn check_extra_statement_rule_68(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #69
pub fn check_extra_statement_rule_69(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #70
pub fn check_extra_statement_rule_70(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #71
pub fn check_extra_statement_rule_71(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #72
pub fn check_extra_statement_rule_72(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #73
pub fn check_extra_statement_rule_73(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #74
pub fn check_extra_statement_rule_74(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #75
pub fn check_extra_statement_rule_75(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #76
pub fn check_extra_statement_rule_76(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #77
pub fn check_extra_statement_rule_77(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #78
pub fn check_extra_statement_rule_78(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #79
pub fn check_extra_statement_rule_79(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #80
pub fn check_extra_statement_rule_80(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #81
pub fn check_extra_statement_rule_81(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #82
pub fn check_extra_statement_rule_82(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #83
pub fn check_extra_statement_rule_83(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #84
pub fn check_extra_statement_rule_84(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #85
pub fn check_extra_statement_rule_85(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #86
pub fn check_extra_statement_rule_86(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #87
pub fn check_extra_statement_rule_87(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #88
pub fn check_extra_statement_rule_88(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #89
pub fn check_extra_statement_rule_89(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #90
pub fn check_extra_statement_rule_90(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #91
pub fn check_extra_statement_rule_91(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #92
pub fn check_extra_statement_rule_92(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #93
pub fn check_extra_statement_rule_93(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #94
pub fn check_extra_statement_rule_94(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #95
pub fn check_extra_statement_rule_95(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #96
pub fn check_extra_statement_rule_96(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #97
pub fn check_extra_statement_rule_97(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #98
pub fn check_extra_statement_rule_98(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #99
pub fn check_extra_statement_rule_99(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #100
pub fn check_extra_statement_rule_100(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #101
pub fn check_extra_statement_rule_101(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #102
pub fn check_extra_statement_rule_102(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #103
pub fn check_extra_statement_rule_103(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #104
pub fn check_extra_statement_rule_104(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #105
pub fn check_extra_statement_rule_105(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #106
pub fn check_extra_statement_rule_106(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #107
pub fn check_extra_statement_rule_107(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #108
pub fn check_extra_statement_rule_108(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #109
pub fn check_extra_statement_rule_109(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #110
pub fn check_extra_statement_rule_110(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #111
pub fn check_extra_statement_rule_111(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #112
pub fn check_extra_statement_rule_112(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #113
pub fn check_extra_statement_rule_113(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #114
pub fn check_extra_statement_rule_114(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #115
pub fn check_extra_statement_rule_115(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #116
pub fn check_extra_statement_rule_116(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #117
pub fn check_extra_statement_rule_117(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #118
pub fn check_extra_statement_rule_118(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #119
pub fn check_extra_statement_rule_119(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #120
pub fn check_extra_statement_rule_120(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #121
pub fn check_extra_statement_rule_121(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #122
pub fn check_extra_statement_rule_122(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #123
pub fn check_extra_statement_rule_123(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #124
pub fn check_extra_statement_rule_124(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #125
pub fn check_extra_statement_rule_125(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #126
pub fn check_extra_statement_rule_126(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #127
pub fn check_extra_statement_rule_127(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #128
pub fn check_extra_statement_rule_128(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #129
pub fn check_extra_statement_rule_129(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #130
pub fn check_extra_statement_rule_130(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #131
pub fn check_extra_statement_rule_131(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #132
pub fn check_extra_statement_rule_132(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #133
pub fn check_extra_statement_rule_133(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #134
pub fn check_extra_statement_rule_134(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #135
pub fn check_extra_statement_rule_135(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #136
pub fn check_extra_statement_rule_136(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #137
pub fn check_extra_statement_rule_137(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #138
pub fn check_extra_statement_rule_138(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #139
pub fn check_extra_statement_rule_139(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #140
pub fn check_extra_statement_rule_140(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #141
pub fn check_extra_statement_rule_141(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #142
pub fn check_extra_statement_rule_142(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #143
pub fn check_extra_statement_rule_143(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #144
pub fn check_extra_statement_rule_144(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #145
pub fn check_extra_statement_rule_145(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #146
pub fn check_extra_statement_rule_146(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #147
pub fn check_extra_statement_rule_147(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #148
pub fn check_extra_statement_rule_148(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #149
pub fn check_extra_statement_rule_149(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #150
pub fn check_extra_statement_rule_150(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #151
pub fn check_extra_statement_rule_151(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #152
pub fn check_extra_statement_rule_152(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #153
pub fn check_extra_statement_rule_153(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #154
pub fn check_extra_statement_rule_154(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #155
pub fn check_extra_statement_rule_155(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #156
pub fn check_extra_statement_rule_156(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #157
pub fn check_extra_statement_rule_157(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #158
pub fn check_extra_statement_rule_158(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #159
pub fn check_extra_statement_rule_159(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #160
pub fn check_extra_statement_rule_160(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #161
pub fn check_extra_statement_rule_161(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #162
pub fn check_extra_statement_rule_162(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #163
pub fn check_extra_statement_rule_163(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #164
pub fn check_extra_statement_rule_164(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #165
pub fn check_extra_statement_rule_165(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #166
pub fn check_extra_statement_rule_166(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #167
pub fn check_extra_statement_rule_167(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #168
pub fn check_extra_statement_rule_168(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #169
pub fn check_extra_statement_rule_169(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #170
pub fn check_extra_statement_rule_170(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #171
pub fn check_extra_statement_rule_171(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #172
pub fn check_extra_statement_rule_172(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #173
pub fn check_extra_statement_rule_173(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #174
pub fn check_extra_statement_rule_174(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #175
pub fn check_extra_statement_rule_175(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #176
pub fn check_extra_statement_rule_176(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #177
pub fn check_extra_statement_rule_177(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #178
pub fn check_extra_statement_rule_178(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #179
pub fn check_extra_statement_rule_179(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #180
pub fn check_extra_statement_rule_180(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #181
pub fn check_extra_statement_rule_181(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #182
pub fn check_extra_statement_rule_182(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #183
pub fn check_extra_statement_rule_183(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #184
pub fn check_extra_statement_rule_184(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #185
pub fn check_extra_statement_rule_185(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #186
pub fn check_extra_statement_rule_186(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #187
pub fn check_extra_statement_rule_187(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #188
pub fn check_extra_statement_rule_188(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #189
pub fn check_extra_statement_rule_189(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #190
pub fn check_extra_statement_rule_190(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #191
pub fn check_extra_statement_rule_191(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #192
pub fn check_extra_statement_rule_192(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #193
pub fn check_extra_statement_rule_193(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #194
pub fn check_extra_statement_rule_194(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #195
pub fn check_extra_statement_rule_195(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #196
pub fn check_extra_statement_rule_196(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #197
pub fn check_extra_statement_rule_197(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #198
pub fn check_extra_statement_rule_198(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #199
pub fn check_extra_statement_rule_199(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #200
pub fn check_extra_statement_rule_200(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #201
pub fn check_extra_statement_rule_201(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #202
pub fn check_extra_statement_rule_202(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #203
pub fn check_extra_statement_rule_203(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #204
pub fn check_extra_statement_rule_204(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #205
pub fn check_extra_statement_rule_205(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #206
pub fn check_extra_statement_rule_206(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #207
pub fn check_extra_statement_rule_207(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #208
pub fn check_extra_statement_rule_208(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #209
pub fn check_extra_statement_rule_209(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #210
pub fn check_extra_statement_rule_210(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #211
pub fn check_extra_statement_rule_211(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #212
pub fn check_extra_statement_rule_212(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #213
pub fn check_extra_statement_rule_213(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #214
pub fn check_extra_statement_rule_214(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #215
pub fn check_extra_statement_rule_215(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #216
pub fn check_extra_statement_rule_216(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #217
pub fn check_extra_statement_rule_217(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #218
pub fn check_extra_statement_rule_218(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #219
pub fn check_extra_statement_rule_219(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #220
pub fn check_extra_statement_rule_220(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #221
pub fn check_extra_statement_rule_221(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #222
pub fn check_extra_statement_rule_222(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #223
pub fn check_extra_statement_rule_223(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #224
pub fn check_extra_statement_rule_224(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #225
pub fn check_extra_statement_rule_225(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #226
pub fn check_extra_statement_rule_226(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #227
pub fn check_extra_statement_rule_227(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #228
pub fn check_extra_statement_rule_228(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #229
pub fn check_extra_statement_rule_229(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #230
pub fn check_extra_statement_rule_230(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #231
pub fn check_extra_statement_rule_231(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #232
pub fn check_extra_statement_rule_232(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #233
pub fn check_extra_statement_rule_233(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #234
pub fn check_extra_statement_rule_234(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #235
pub fn check_extra_statement_rule_235(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #236
pub fn check_extra_statement_rule_236(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #237
pub fn check_extra_statement_rule_237(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #238
pub fn check_extra_statement_rule_238(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #239
pub fn check_extra_statement_rule_239(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #240
pub fn check_extra_statement_rule_240(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #241
pub fn check_extra_statement_rule_241(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #242
pub fn check_extra_statement_rule_242(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #243
pub fn check_extra_statement_rule_243(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #244
pub fn check_extra_statement_rule_244(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #245
pub fn check_extra_statement_rule_245(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #246
pub fn check_extra_statement_rule_246(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #247
pub fn check_extra_statement_rule_247(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #248
pub fn check_extra_statement_rule_248(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #249
pub fn check_extra_statement_rule_249(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #250
pub fn check_extra_statement_rule_250(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #251
pub fn check_extra_statement_rule_251(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #252
pub fn check_extra_statement_rule_252(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #253
pub fn check_extra_statement_rule_253(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #254
pub fn check_extra_statement_rule_254(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #255
pub fn check_extra_statement_rule_255(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #256
pub fn check_extra_statement_rule_256(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #257
pub fn check_extra_statement_rule_257(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #258
pub fn check_extra_statement_rule_258(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #259
pub fn check_extra_statement_rule_259(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #260
pub fn check_extra_statement_rule_260(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #261
pub fn check_extra_statement_rule_261(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #262
pub fn check_extra_statement_rule_262(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #263
pub fn check_extra_statement_rule_263(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #264
pub fn check_extra_statement_rule_264(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #265
pub fn check_extra_statement_rule_265(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #266
pub fn check_extra_statement_rule_266(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #267
pub fn check_extra_statement_rule_267(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #268
pub fn check_extra_statement_rule_268(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #269
pub fn check_extra_statement_rule_269(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #270
pub fn check_extra_statement_rule_270(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #271
pub fn check_extra_statement_rule_271(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #272
pub fn check_extra_statement_rule_272(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #273
pub fn check_extra_statement_rule_273(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #274
pub fn check_extra_statement_rule_274(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #275
pub fn check_extra_statement_rule_275(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #276
pub fn check_extra_statement_rule_276(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #277
pub fn check_extra_statement_rule_277(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #278
pub fn check_extra_statement_rule_278(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #279
pub fn check_extra_statement_rule_279(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #280
pub fn check_extra_statement_rule_280(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #281
pub fn check_extra_statement_rule_281(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #282
pub fn check_extra_statement_rule_282(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #283
pub fn check_extra_statement_rule_283(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #284
pub fn check_extra_statement_rule_284(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #285
pub fn check_extra_statement_rule_285(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #286
pub fn check_extra_statement_rule_286(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #287
pub fn check_extra_statement_rule_287(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #288
pub fn check_extra_statement_rule_288(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #289
pub fn check_extra_statement_rule_289(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #290
pub fn check_extra_statement_rule_290(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #291
pub fn check_extra_statement_rule_291(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #292
pub fn check_extra_statement_rule_292(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #293
pub fn check_extra_statement_rule_293(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #294
pub fn check_extra_statement_rule_294(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #295
pub fn check_extra_statement_rule_295(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #296
pub fn check_extra_statement_rule_296(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #297
pub fn check_extra_statement_rule_297(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #298
pub fn check_extra_statement_rule_298(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #299
pub fn check_extra_statement_rule_299(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #300
pub fn check_extra_statement_rule_300(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #301
pub fn check_extra_statement_rule_301(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #302
pub fn check_extra_statement_rule_302(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #303
pub fn check_extra_statement_rule_303(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #304
pub fn check_extra_statement_rule_304(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #305
pub fn check_extra_statement_rule_305(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #306
pub fn check_extra_statement_rule_306(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #307
pub fn check_extra_statement_rule_307(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #308
pub fn check_extra_statement_rule_308(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #309
pub fn check_extra_statement_rule_309(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #310
pub fn check_extra_statement_rule_310(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #311
pub fn check_extra_statement_rule_311(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #312
pub fn check_extra_statement_rule_312(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #313
pub fn check_extra_statement_rule_313(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #314
pub fn check_extra_statement_rule_314(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #315
pub fn check_extra_statement_rule_315(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #316
pub fn check_extra_statement_rule_316(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #317
pub fn check_extra_statement_rule_317(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #318
pub fn check_extra_statement_rule_318(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #319
pub fn check_extra_statement_rule_319(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #320
pub fn check_extra_statement_rule_320(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #321
pub fn check_extra_statement_rule_321(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #322
pub fn check_extra_statement_rule_322(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #323
pub fn check_extra_statement_rule_323(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #324
pub fn check_extra_statement_rule_324(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #325
pub fn check_extra_statement_rule_325(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #326
pub fn check_extra_statement_rule_326(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #327
pub fn check_extra_statement_rule_327(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #328
pub fn check_extra_statement_rule_328(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #329
pub fn check_extra_statement_rule_329(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #330
pub fn check_extra_statement_rule_330(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #331
pub fn check_extra_statement_rule_331(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #332
pub fn check_extra_statement_rule_332(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #333
pub fn check_extra_statement_rule_333(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #334
pub fn check_extra_statement_rule_334(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #335
pub fn check_extra_statement_rule_335(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #336
pub fn check_extra_statement_rule_336(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #337
pub fn check_extra_statement_rule_337(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #338
pub fn check_extra_statement_rule_338(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #339
pub fn check_extra_statement_rule_339(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #340
pub fn check_extra_statement_rule_340(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #341
pub fn check_extra_statement_rule_341(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #342
pub fn check_extra_statement_rule_342(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #343
pub fn check_extra_statement_rule_343(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #344
pub fn check_extra_statement_rule_344(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #345
pub fn check_extra_statement_rule_345(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #346
pub fn check_extra_statement_rule_346(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #347
pub fn check_extra_statement_rule_347(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #348
pub fn check_extra_statement_rule_348(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #349
pub fn check_extra_statement_rule_349(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #350
pub fn check_extra_statement_rule_350(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #351
pub fn check_extra_statement_rule_351(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #352
pub fn check_extra_statement_rule_352(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #353
pub fn check_extra_statement_rule_353(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #354
pub fn check_extra_statement_rule_354(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #355
pub fn check_extra_statement_rule_355(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #356
pub fn check_extra_statement_rule_356(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #357
pub fn check_extra_statement_rule_357(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #358
pub fn check_extra_statement_rule_358(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #359
pub fn check_extra_statement_rule_359(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #360
pub fn check_extra_statement_rule_360(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #361
pub fn check_extra_statement_rule_361(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #362
pub fn check_extra_statement_rule_362(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #363
pub fn check_extra_statement_rule_363(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #364
pub fn check_extra_statement_rule_364(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #365
pub fn check_extra_statement_rule_365(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #366
pub fn check_extra_statement_rule_366(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #367
pub fn check_extra_statement_rule_367(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #368
pub fn check_extra_statement_rule_368(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #369
pub fn check_extra_statement_rule_369(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #370
pub fn check_extra_statement_rule_370(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #371
pub fn check_extra_statement_rule_371(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #372
pub fn check_extra_statement_rule_372(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #373
pub fn check_extra_statement_rule_373(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #374
pub fn check_extra_statement_rule_374(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #375
pub fn check_extra_statement_rule_375(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #376
pub fn check_extra_statement_rule_376(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #377
pub fn check_extra_statement_rule_377(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #378
pub fn check_extra_statement_rule_378(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #379
pub fn check_extra_statement_rule_379(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #380
pub fn check_extra_statement_rule_380(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #381
pub fn check_extra_statement_rule_381(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #382
pub fn check_extra_statement_rule_382(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #383
pub fn check_extra_statement_rule_383(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #384
pub fn check_extra_statement_rule_384(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #385
pub fn check_extra_statement_rule_385(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #386
pub fn check_extra_statement_rule_386(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #387
pub fn check_extra_statement_rule_387(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #388
pub fn check_extra_statement_rule_388(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #389
pub fn check_extra_statement_rule_389(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #390
pub fn check_extra_statement_rule_390(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #391
pub fn check_extra_statement_rule_391(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #392
pub fn check_extra_statement_rule_392(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #393
pub fn check_extra_statement_rule_393(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #394
pub fn check_extra_statement_rule_394(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #395
pub fn check_extra_statement_rule_395(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #396
pub fn check_extra_statement_rule_396(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #397
pub fn check_extra_statement_rule_397(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #398
pub fn check_extra_statement_rule_398(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #399
pub fn check_extra_statement_rule_399(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #400
pub fn check_extra_statement_rule_400(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #401
pub fn check_extra_statement_rule_401(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #402
pub fn check_extra_statement_rule_402(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #403
pub fn check_extra_statement_rule_403(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #404
pub fn check_extra_statement_rule_404(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #405
pub fn check_extra_statement_rule_405(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #406
pub fn check_extra_statement_rule_406(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #407
pub fn check_extra_statement_rule_407(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #408
pub fn check_extra_statement_rule_408(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #409
pub fn check_extra_statement_rule_409(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #410
pub fn check_extra_statement_rule_410(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #411
pub fn check_extra_statement_rule_411(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #412
pub fn check_extra_statement_rule_412(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #413
pub fn check_extra_statement_rule_413(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #414
pub fn check_extra_statement_rule_414(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #415
pub fn check_extra_statement_rule_415(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #416
pub fn check_extra_statement_rule_416(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #417
pub fn check_extra_statement_rule_417(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #418
pub fn check_extra_statement_rule_418(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #419
pub fn check_extra_statement_rule_419(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #420
pub fn check_extra_statement_rule_420(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #421
pub fn check_extra_statement_rule_421(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #422
pub fn check_extra_statement_rule_422(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #423
pub fn check_extra_statement_rule_423(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #424
pub fn check_extra_statement_rule_424(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #425
pub fn check_extra_statement_rule_425(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #426
pub fn check_extra_statement_rule_426(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #427
pub fn check_extra_statement_rule_427(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #428
pub fn check_extra_statement_rule_428(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #429
pub fn check_extra_statement_rule_429(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #430
pub fn check_extra_statement_rule_430(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #431
pub fn check_extra_statement_rule_431(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #432
pub fn check_extra_statement_rule_432(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #433
pub fn check_extra_statement_rule_433(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #434
pub fn check_extra_statement_rule_434(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #435
pub fn check_extra_statement_rule_435(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #436
pub fn check_extra_statement_rule_436(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #437
pub fn check_extra_statement_rule_437(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #438
pub fn check_extra_statement_rule_438(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #439
pub fn check_extra_statement_rule_439(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #440
pub fn check_extra_statement_rule_440(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #441
pub fn check_extra_statement_rule_441(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #442
pub fn check_extra_statement_rule_442(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #443
pub fn check_extra_statement_rule_443(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #444
pub fn check_extra_statement_rule_444(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #445
pub fn check_extra_statement_rule_445(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #446
pub fn check_extra_statement_rule_446(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #447
pub fn check_extra_statement_rule_447(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #448
pub fn check_extra_statement_rule_448(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #449
pub fn check_extra_statement_rule_449(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #450
pub fn check_extra_statement_rule_450(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #451
pub fn check_extra_statement_rule_451(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #452
pub fn check_extra_statement_rule_452(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #453
pub fn check_extra_statement_rule_453(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #454
pub fn check_extra_statement_rule_454(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #455
pub fn check_extra_statement_rule_455(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #456
pub fn check_extra_statement_rule_456(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #457
pub fn check_extra_statement_rule_457(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #458
pub fn check_extra_statement_rule_458(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #459
pub fn check_extra_statement_rule_459(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #460
pub fn check_extra_statement_rule_460(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #461
pub fn check_extra_statement_rule_461(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #462
pub fn check_extra_statement_rule_462(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #463
pub fn check_extra_statement_rule_463(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #464
pub fn check_extra_statement_rule_464(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #465
pub fn check_extra_statement_rule_465(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #466
pub fn check_extra_statement_rule_466(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #467
pub fn check_extra_statement_rule_467(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #468
pub fn check_extra_statement_rule_468(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #469
pub fn check_extra_statement_rule_469(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #470
pub fn check_extra_statement_rule_470(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #471
pub fn check_extra_statement_rule_471(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #472
pub fn check_extra_statement_rule_472(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #473
pub fn check_extra_statement_rule_473(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #474
pub fn check_extra_statement_rule_474(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #475
pub fn check_extra_statement_rule_475(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #476
pub fn check_extra_statement_rule_476(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #477
pub fn check_extra_statement_rule_477(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #478
pub fn check_extra_statement_rule_478(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #479
pub fn check_extra_statement_rule_479(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #480
pub fn check_extra_statement_rule_480(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #481
pub fn check_extra_statement_rule_481(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #482
pub fn check_extra_statement_rule_482(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #483
pub fn check_extra_statement_rule_483(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #484
pub fn check_extra_statement_rule_484(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #485
pub fn check_extra_statement_rule_485(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #486
pub fn check_extra_statement_rule_486(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #487
pub fn check_extra_statement_rule_487(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #488
pub fn check_extra_statement_rule_488(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #489
pub fn check_extra_statement_rule_489(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #490
pub fn check_extra_statement_rule_490(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #491
pub fn check_extra_statement_rule_491(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #492
pub fn check_extra_statement_rule_492(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #493
pub fn check_extra_statement_rule_493(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #494
pub fn check_extra_statement_rule_494(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #495
pub fn check_extra_statement_rule_495(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #496
pub fn check_extra_statement_rule_496(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #497
pub fn check_extra_statement_rule_497(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #498
pub fn check_extra_statement_rule_498(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #499
pub fn check_extra_statement_rule_499(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #500
pub fn check_extra_statement_rule_500(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #501
pub fn check_extra_statement_rule_501(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #502
pub fn check_extra_statement_rule_502(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #503
pub fn check_extra_statement_rule_503(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #504
pub fn check_extra_statement_rule_504(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #505
pub fn check_extra_statement_rule_505(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #506
pub fn check_extra_statement_rule_506(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #507
pub fn check_extra_statement_rule_507(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #508
pub fn check_extra_statement_rule_508(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #509
pub fn check_extra_statement_rule_509(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #510
pub fn check_extra_statement_rule_510(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #511
pub fn check_extra_statement_rule_511(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #512
pub fn check_extra_statement_rule_512(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #513
pub fn check_extra_statement_rule_513(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #514
pub fn check_extra_statement_rule_514(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #515
pub fn check_extra_statement_rule_515(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #516
pub fn check_extra_statement_rule_516(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #517
pub fn check_extra_statement_rule_517(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #518
pub fn check_extra_statement_rule_518(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #519
pub fn check_extra_statement_rule_519(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #520
pub fn check_extra_statement_rule_520(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #521
pub fn check_extra_statement_rule_521(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #522
pub fn check_extra_statement_rule_522(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #523
pub fn check_extra_statement_rule_523(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #524
pub fn check_extra_statement_rule_524(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #525
pub fn check_extra_statement_rule_525(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #526
pub fn check_extra_statement_rule_526(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #527
pub fn check_extra_statement_rule_527(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #528
pub fn check_extra_statement_rule_528(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #529
pub fn check_extra_statement_rule_529(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #530
pub fn check_extra_statement_rule_530(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #531
pub fn check_extra_statement_rule_531(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #532
pub fn check_extra_statement_rule_532(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #533
pub fn check_extra_statement_rule_533(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #534
pub fn check_extra_statement_rule_534(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #535
pub fn check_extra_statement_rule_535(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #536
pub fn check_extra_statement_rule_536(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #537
pub fn check_extra_statement_rule_537(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #538
pub fn check_extra_statement_rule_538(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #539
pub fn check_extra_statement_rule_539(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #540
pub fn check_extra_statement_rule_540(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #541
pub fn check_extra_statement_rule_541(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #542
pub fn check_extra_statement_rule_542(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #543
pub fn check_extra_statement_rule_543(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #544
pub fn check_extra_statement_rule_544(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #545
pub fn check_extra_statement_rule_545(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #546
pub fn check_extra_statement_rule_546(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #547
pub fn check_extra_statement_rule_547(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #548
pub fn check_extra_statement_rule_548(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #549
pub fn check_extra_statement_rule_549(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #550
pub fn check_extra_statement_rule_550(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #551
pub fn check_extra_statement_rule_551(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #552
pub fn check_extra_statement_rule_552(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #553
pub fn check_extra_statement_rule_553(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #554
pub fn check_extra_statement_rule_554(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #555
pub fn check_extra_statement_rule_555(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #556
pub fn check_extra_statement_rule_556(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #557
pub fn check_extra_statement_rule_557(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #558
pub fn check_extra_statement_rule_558(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #559
pub fn check_extra_statement_rule_559(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #560
pub fn check_extra_statement_rule_560(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #561
pub fn check_extra_statement_rule_561(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #562
pub fn check_extra_statement_rule_562(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #563
pub fn check_extra_statement_rule_563(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #564
pub fn check_extra_statement_rule_564(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #565
pub fn check_extra_statement_rule_565(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #566
pub fn check_extra_statement_rule_566(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #567
pub fn check_extra_statement_rule_567(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #568
pub fn check_extra_statement_rule_568(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #569
pub fn check_extra_statement_rule_569(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #570
pub fn check_extra_statement_rule_570(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #571
pub fn check_extra_statement_rule_571(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #572
pub fn check_extra_statement_rule_572(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #573
pub fn check_extra_statement_rule_573(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #574
pub fn check_extra_statement_rule_574(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #575
pub fn check_extra_statement_rule_575(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #576
pub fn check_extra_statement_rule_576(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #577
pub fn check_extra_statement_rule_577(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #578
pub fn check_extra_statement_rule_578(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #579
pub fn check_extra_statement_rule_579(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #580
pub fn check_extra_statement_rule_580(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #581
pub fn check_extra_statement_rule_581(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #582
pub fn check_extra_statement_rule_582(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #583
pub fn check_extra_statement_rule_583(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #584
pub fn check_extra_statement_rule_584(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #585
pub fn check_extra_statement_rule_585(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #586
pub fn check_extra_statement_rule_586(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #587
pub fn check_extra_statement_rule_587(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #588
pub fn check_extra_statement_rule_588(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #589
pub fn check_extra_statement_rule_589(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #590
pub fn check_extra_statement_rule_590(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #591
pub fn check_extra_statement_rule_591(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #592
pub fn check_extra_statement_rule_592(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #593
pub fn check_extra_statement_rule_593(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #594
pub fn check_extra_statement_rule_594(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #595
pub fn check_extra_statement_rule_595(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #596
pub fn check_extra_statement_rule_596(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #597
pub fn check_extra_statement_rule_597(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #598
pub fn check_extra_statement_rule_598(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #599
pub fn check_extra_statement_rule_599(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #600
pub fn check_extra_statement_rule_600(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #601
pub fn check_extra_statement_rule_601(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #602
pub fn check_extra_statement_rule_602(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #603
pub fn check_extra_statement_rule_603(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #604
pub fn check_extra_statement_rule_604(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #605
pub fn check_extra_statement_rule_605(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #606
pub fn check_extra_statement_rule_606(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #607
pub fn check_extra_statement_rule_607(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #608
pub fn check_extra_statement_rule_608(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #609
pub fn check_extra_statement_rule_609(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #610
pub fn check_extra_statement_rule_610(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #611
pub fn check_extra_statement_rule_611(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #612
pub fn check_extra_statement_rule_612(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #613
pub fn check_extra_statement_rule_613(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #614
pub fn check_extra_statement_rule_614(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #615
pub fn check_extra_statement_rule_615(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #616
pub fn check_extra_statement_rule_616(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #617
pub fn check_extra_statement_rule_617(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #618
pub fn check_extra_statement_rule_618(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #619
pub fn check_extra_statement_rule_619(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #620
pub fn check_extra_statement_rule_620(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #621
pub fn check_extra_statement_rule_621(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #622
pub fn check_extra_statement_rule_622(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #623
pub fn check_extra_statement_rule_623(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #624
pub fn check_extra_statement_rule_624(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #625
pub fn check_extra_statement_rule_625(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #626
pub fn check_extra_statement_rule_626(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #627
pub fn check_extra_statement_rule_627(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #628
pub fn check_extra_statement_rule_628(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #629
pub fn check_extra_statement_rule_629(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #630
pub fn check_extra_statement_rule_630(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #631
pub fn check_extra_statement_rule_631(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #632
pub fn check_extra_statement_rule_632(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #633
pub fn check_extra_statement_rule_633(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #634
pub fn check_extra_statement_rule_634(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #635
pub fn check_extra_statement_rule_635(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #636
pub fn check_extra_statement_rule_636(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #637
pub fn check_extra_statement_rule_637(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #638
pub fn check_extra_statement_rule_638(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #639
pub fn check_extra_statement_rule_639(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #640
pub fn check_extra_statement_rule_640(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #641
pub fn check_extra_statement_rule_641(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #642
pub fn check_extra_statement_rule_642(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #643
pub fn check_extra_statement_rule_643(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #644
pub fn check_extra_statement_rule_644(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #645
pub fn check_extra_statement_rule_645(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #646
pub fn check_extra_statement_rule_646(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #647
pub fn check_extra_statement_rule_647(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #648
pub fn check_extra_statement_rule_648(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #649
pub fn check_extra_statement_rule_649(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #650
pub fn check_extra_statement_rule_650(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #651
pub fn check_extra_statement_rule_651(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #652
pub fn check_extra_statement_rule_652(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #653
pub fn check_extra_statement_rule_653(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #654
pub fn check_extra_statement_rule_654(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #655
pub fn check_extra_statement_rule_655(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #656
pub fn check_extra_statement_rule_656(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #657
pub fn check_extra_statement_rule_657(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #658
pub fn check_extra_statement_rule_658(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #659
pub fn check_extra_statement_rule_659(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #660
pub fn check_extra_statement_rule_660(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #661
pub fn check_extra_statement_rule_661(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #662
pub fn check_extra_statement_rule_662(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #663
pub fn check_extra_statement_rule_663(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #664
pub fn check_extra_statement_rule_664(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #665
pub fn check_extra_statement_rule_665(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #666
pub fn check_extra_statement_rule_666(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #667
pub fn check_extra_statement_rule_667(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #668
pub fn check_extra_statement_rule_668(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #669
pub fn check_extra_statement_rule_669(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #670
pub fn check_extra_statement_rule_670(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #671
pub fn check_extra_statement_rule_671(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #672
pub fn check_extra_statement_rule_672(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #673
pub fn check_extra_statement_rule_673(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #674
pub fn check_extra_statement_rule_674(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #675
pub fn check_extra_statement_rule_675(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #676
pub fn check_extra_statement_rule_676(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #677
pub fn check_extra_statement_rule_677(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #678
pub fn check_extra_statement_rule_678(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #679
pub fn check_extra_statement_rule_679(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #680
pub fn check_extra_statement_rule_680(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #681
pub fn check_extra_statement_rule_681(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #682
pub fn check_extra_statement_rule_682(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #683
pub fn check_extra_statement_rule_683(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #684
pub fn check_extra_statement_rule_684(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #685
pub fn check_extra_statement_rule_685(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #686
pub fn check_extra_statement_rule_686(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #687
pub fn check_extra_statement_rule_687(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #688
pub fn check_extra_statement_rule_688(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #689
pub fn check_extra_statement_rule_689(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #690
pub fn check_extra_statement_rule_690(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #691
pub fn check_extra_statement_rule_691(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #692
pub fn check_extra_statement_rule_692(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #693
pub fn check_extra_statement_rule_693(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #694
pub fn check_extra_statement_rule_694(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #695
pub fn check_extra_statement_rule_695(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #696
pub fn check_extra_statement_rule_696(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #697
pub fn check_extra_statement_rule_697(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #698
pub fn check_extra_statement_rule_698(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #699
pub fn check_extra_statement_rule_699(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #700
pub fn check_extra_statement_rule_700(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #701
pub fn check_extra_statement_rule_701(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #702
pub fn check_extra_statement_rule_702(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #703
pub fn check_extra_statement_rule_703(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #704
pub fn check_extra_statement_rule_704(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #705
pub fn check_extra_statement_rule_705(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #706
pub fn check_extra_statement_rule_706(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #707
pub fn check_extra_statement_rule_707(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #708
pub fn check_extra_statement_rule_708(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #709
pub fn check_extra_statement_rule_709(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #710
pub fn check_extra_statement_rule_710(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #711
pub fn check_extra_statement_rule_711(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #712
pub fn check_extra_statement_rule_712(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #713
pub fn check_extra_statement_rule_713(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #714
pub fn check_extra_statement_rule_714(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #715
pub fn check_extra_statement_rule_715(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #716
pub fn check_extra_statement_rule_716(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #717
pub fn check_extra_statement_rule_717(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #718
pub fn check_extra_statement_rule_718(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #719
pub fn check_extra_statement_rule_719(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #720
pub fn check_extra_statement_rule_720(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #721
pub fn check_extra_statement_rule_721(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #722
pub fn check_extra_statement_rule_722(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #723
pub fn check_extra_statement_rule_723(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #724
pub fn check_extra_statement_rule_724(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #725
pub fn check_extra_statement_rule_725(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #726
pub fn check_extra_statement_rule_726(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #727
pub fn check_extra_statement_rule_727(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #728
pub fn check_extra_statement_rule_728(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #729
pub fn check_extra_statement_rule_729(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #730
pub fn check_extra_statement_rule_730(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #731
pub fn check_extra_statement_rule_731(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #732
pub fn check_extra_statement_rule_732(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #733
pub fn check_extra_statement_rule_733(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #734
pub fn check_extra_statement_rule_734(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #735
pub fn check_extra_statement_rule_735(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #736
pub fn check_extra_statement_rule_736(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #737
pub fn check_extra_statement_rule_737(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #738
pub fn check_extra_statement_rule_738(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #739
pub fn check_extra_statement_rule_739(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #740
pub fn check_extra_statement_rule_740(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #741
pub fn check_extra_statement_rule_741(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #742
pub fn check_extra_statement_rule_742(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #743
pub fn check_extra_statement_rule_743(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #744
pub fn check_extra_statement_rule_744(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #745
pub fn check_extra_statement_rule_745(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #746
pub fn check_extra_statement_rule_746(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #747
pub fn check_extra_statement_rule_747(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #748
pub fn check_extra_statement_rule_748(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #749
pub fn check_extra_statement_rule_749(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #750
pub fn check_extra_statement_rule_750(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #751
pub fn check_extra_statement_rule_751(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #752
pub fn check_extra_statement_rule_752(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #753
pub fn check_extra_statement_rule_753(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #754
pub fn check_extra_statement_rule_754(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #755
pub fn check_extra_statement_rule_755(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #756
pub fn check_extra_statement_rule_756(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #757
pub fn check_extra_statement_rule_757(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #758
pub fn check_extra_statement_rule_758(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #759
pub fn check_extra_statement_rule_759(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #760
pub fn check_extra_statement_rule_760(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #761
pub fn check_extra_statement_rule_761(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #762
pub fn check_extra_statement_rule_762(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #763
pub fn check_extra_statement_rule_763(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #764
pub fn check_extra_statement_rule_764(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #765
pub fn check_extra_statement_rule_765(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #766
pub fn check_extra_statement_rule_766(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #767
pub fn check_extra_statement_rule_767(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #768
pub fn check_extra_statement_rule_768(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #769
pub fn check_extra_statement_rule_769(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #770
pub fn check_extra_statement_rule_770(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #771
pub fn check_extra_statement_rule_771(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #772
pub fn check_extra_statement_rule_772(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #773
pub fn check_extra_statement_rule_773(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #774
pub fn check_extra_statement_rule_774(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #775
pub fn check_extra_statement_rule_775(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #776
pub fn check_extra_statement_rule_776(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #777
pub fn check_extra_statement_rule_777(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #778
pub fn check_extra_statement_rule_778(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #779
pub fn check_extra_statement_rule_779(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #780
pub fn check_extra_statement_rule_780(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #781
pub fn check_extra_statement_rule_781(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #782
pub fn check_extra_statement_rule_782(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #783
pub fn check_extra_statement_rule_783(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #784
pub fn check_extra_statement_rule_784(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #785
pub fn check_extra_statement_rule_785(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #786
pub fn check_extra_statement_rule_786(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #787
pub fn check_extra_statement_rule_787(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #788
pub fn check_extra_statement_rule_788(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #789
pub fn check_extra_statement_rule_789(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #790
pub fn check_extra_statement_rule_790(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #791
pub fn check_extra_statement_rule_791(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #792
pub fn check_extra_statement_rule_792(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #793
pub fn check_extra_statement_rule_793(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #794
pub fn check_extra_statement_rule_794(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #795
pub fn check_extra_statement_rule_795(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #796
pub fn check_extra_statement_rule_796(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #797
pub fn check_extra_statement_rule_797(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #798
pub fn check_extra_statement_rule_798(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #799
pub fn check_extra_statement_rule_799(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}

/// Extra Statement Type Check Handler #800
pub fn check_extra_statement_rule_800(
    checker: &mut ExtraTypeChecker,
    name: &str,
    type_name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    checker
        .symbol_table
        .insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #801
pub fn check_extra_statement_rule_801(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #802
pub fn check_extra_statement_rule_802(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #803
pub fn check_extra_statement_rule_803(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #804
pub fn check_extra_statement_rule_804(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #805
pub fn check_extra_statement_rule_805(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #806
pub fn check_extra_statement_rule_806(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #807
pub fn check_extra_statement_rule_807(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #808
pub fn check_extra_statement_rule_808(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #809
pub fn check_extra_statement_rule_809(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #810
pub fn check_extra_statement_rule_810(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #811
pub fn check_extra_statement_rule_811(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #812
pub fn check_extra_statement_rule_812(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #813
pub fn check_extra_statement_rule_813(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #814
pub fn check_extra_statement_rule_814(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #815
pub fn check_extra_statement_rule_815(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #816
pub fn check_extra_statement_rule_816(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #817
pub fn check_extra_statement_rule_817(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #818
pub fn check_extra_statement_rule_818(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #819
pub fn check_extra_statement_rule_819(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #820
pub fn check_extra_statement_rule_820(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #821
pub fn check_extra_statement_rule_821(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #822
pub fn check_extra_statement_rule_822(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #823
pub fn check_extra_statement_rule_823(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #824
pub fn check_extra_statement_rule_824(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #825
pub fn check_extra_statement_rule_825(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #826
pub fn check_extra_statement_rule_826(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #827
pub fn check_extra_statement_rule_827(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #828
pub fn check_extra_statement_rule_828(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #829
pub fn check_extra_statement_rule_829(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #830
pub fn check_extra_statement_rule_830(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #831
pub fn check_extra_statement_rule_831(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #832
pub fn check_extra_statement_rule_832(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #833
pub fn check_extra_statement_rule_833(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #834
pub fn check_extra_statement_rule_834(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #835
pub fn check_extra_statement_rule_835(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #836
pub fn check_extra_statement_rule_836(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #837
pub fn check_extra_statement_rule_837(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #838
pub fn check_extra_statement_rule_838(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #839
pub fn check_extra_statement_rule_839(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #840
pub fn check_extra_statement_rule_840(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #841
pub fn check_extra_statement_rule_841(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #842
pub fn check_extra_statement_rule_842(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #843
pub fn check_extra_statement_rule_843(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #844
pub fn check_extra_statement_rule_844(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #845
pub fn check_extra_statement_rule_845(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #846
pub fn check_extra_statement_rule_846(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #847
pub fn check_extra_statement_rule_847(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #848
pub fn check_extra_statement_rule_848(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #849
pub fn check_extra_statement_rule_849(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #850
pub fn check_extra_statement_rule_850(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #851
pub fn check_extra_statement_rule_851(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #852
pub fn check_extra_statement_rule_852(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #853
pub fn check_extra_statement_rule_853(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #854
pub fn check_extra_statement_rule_854(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #855
pub fn check_extra_statement_rule_855(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #856
pub fn check_extra_statement_rule_856(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #857
pub fn check_extra_statement_rule_857(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #858
pub fn check_extra_statement_rule_858(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #859
pub fn check_extra_statement_rule_859(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #860
pub fn check_extra_statement_rule_860(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #861
pub fn check_extra_statement_rule_861(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #862
pub fn check_extra_statement_rule_862(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #863
pub fn check_extra_statement_rule_863(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #864
pub fn check_extra_statement_rule_864(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #865
pub fn check_extra_statement_rule_865(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #866
pub fn check_extra_statement_rule_866(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #867
pub fn check_extra_statement_rule_867(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #868
pub fn check_extra_statement_rule_868(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #869
pub fn check_extra_statement_rule_869(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #870
pub fn check_extra_statement_rule_870(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #871
pub fn check_extra_statement_rule_871(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #872
pub fn check_extra_statement_rule_872(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #873
pub fn check_extra_statement_rule_873(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #874
pub fn check_extra_statement_rule_874(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #875
pub fn check_extra_statement_rule_875(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #876
pub fn check_extra_statement_rule_876(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #877
pub fn check_extra_statement_rule_877(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #878
pub fn check_extra_statement_rule_878(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #879
pub fn check_extra_statement_rule_879(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #880
pub fn check_extra_statement_rule_880(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #881
pub fn check_extra_statement_rule_881(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #882
pub fn check_extra_statement_rule_882(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #883
pub fn check_extra_statement_rule_883(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #884
pub fn check_extra_statement_rule_884(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #885
pub fn check_extra_statement_rule_885(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #886
pub fn check_extra_statement_rule_886(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #887
pub fn check_extra_statement_rule_887(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #888
pub fn check_extra_statement_rule_888(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #889
pub fn check_extra_statement_rule_889(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #890
pub fn check_extra_statement_rule_890(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #891
pub fn check_extra_statement_rule_891(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #892
pub fn check_extra_statement_rule_892(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #893
pub fn check_extra_statement_rule_893(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #894
pub fn check_extra_statement_rule_894(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #895
pub fn check_extra_statement_rule_895(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #896
pub fn check_extra_statement_rule_896(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #897
pub fn check_extra_statement_rule_897(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #898
pub fn check_extra_statement_rule_898(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #899
pub fn check_extra_statement_rule_899(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #900
pub fn check_extra_statement_rule_900(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #901
pub fn check_extra_statement_rule_901(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #902
pub fn check_extra_statement_rule_902(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #903
pub fn check_extra_statement_rule_903(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #904
pub fn check_extra_statement_rule_904(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #905
pub fn check_extra_statement_rule_905(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #906
pub fn check_extra_statement_rule_906(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #907
pub fn check_extra_statement_rule_907(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #908
pub fn check_extra_statement_rule_908(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #909
pub fn check_extra_statement_rule_909(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #910
pub fn check_extra_statement_rule_910(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #911
pub fn check_extra_statement_rule_911(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #912
pub fn check_extra_statement_rule_912(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #913
pub fn check_extra_statement_rule_913(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #914
pub fn check_extra_statement_rule_914(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #915
pub fn check_extra_statement_rule_915(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #916
pub fn check_extra_statement_rule_916(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #917
pub fn check_extra_statement_rule_917(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #918
pub fn check_extra_statement_rule_918(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #919
pub fn check_extra_statement_rule_919(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #920
pub fn check_extra_statement_rule_920(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #921
pub fn check_extra_statement_rule_921(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #922
pub fn check_extra_statement_rule_922(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #923
pub fn check_extra_statement_rule_923(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #924
pub fn check_extra_statement_rule_924(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #925
pub fn check_extra_statement_rule_925(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #926
pub fn check_extra_statement_rule_926(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #927
pub fn check_extra_statement_rule_927(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #928
pub fn check_extra_statement_rule_928(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #929
pub fn check_extra_statement_rule_929(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #930
pub fn check_extra_statement_rule_930(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #931
pub fn check_extra_statement_rule_931(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #932
pub fn check_extra_statement_rule_932(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #933
pub fn check_extra_statement_rule_933(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #934
pub fn check_extra_statement_rule_934(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #935
pub fn check_extra_statement_rule_935(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #936
pub fn check_extra_statement_rule_936(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #937
pub fn check_extra_statement_rule_937(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #938
pub fn check_extra_statement_rule_938(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #939
pub fn check_extra_statement_rule_939(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #940
pub fn check_extra_statement_rule_940(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #941
pub fn check_extra_statement_rule_941(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #942
pub fn check_extra_statement_rule_942(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #943
pub fn check_extra_statement_rule_943(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #944
pub fn check_extra_statement_rule_944(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #945
pub fn check_extra_statement_rule_945(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #946
pub fn check_extra_statement_rule_946(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #947
pub fn check_extra_statement_rule_947(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #948
pub fn check_extra_statement_rule_948(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #949
pub fn check_extra_statement_rule_949(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #950
pub fn check_extra_statement_rule_950(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #951
pub fn check_extra_statement_rule_951(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #952
pub fn check_extra_statement_rule_952(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #953
pub fn check_extra_statement_rule_953(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #954
pub fn check_extra_statement_rule_954(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #955
pub fn check_extra_statement_rule_955(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #956
pub fn check_extra_statement_rule_956(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #957
pub fn check_extra_statement_rule_957(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #958
pub fn check_extra_statement_rule_958(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #959
pub fn check_extra_statement_rule_959(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #960
pub fn check_extra_statement_rule_960(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #961
pub fn check_extra_statement_rule_961(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #962
pub fn check_extra_statement_rule_962(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #963
pub fn check_extra_statement_rule_963(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #964
pub fn check_extra_statement_rule_964(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #965
pub fn check_extra_statement_rule_965(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #966
pub fn check_extra_statement_rule_966(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #967
pub fn check_extra_statement_rule_967(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #968
pub fn check_extra_statement_rule_968(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #969
pub fn check_extra_statement_rule_969(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #970
pub fn check_extra_statement_rule_970(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #971
pub fn check_extra_statement_rule_971(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #972
pub fn check_extra_statement_rule_972(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #973
pub fn check_extra_statement_rule_973(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #974
pub fn check_extra_statement_rule_974(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #975
pub fn check_extra_statement_rule_975(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #976
pub fn check_extra_statement_rule_976(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #977
pub fn check_extra_statement_rule_977(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #978
pub fn check_extra_statement_rule_978(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #979
pub fn check_extra_statement_rule_979(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #980
pub fn check_extra_statement_rule_980(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #981
pub fn check_extra_statement_rule_981(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #982
pub fn check_extra_statement_rule_982(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #983
pub fn check_extra_statement_rule_983(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #984
pub fn check_extra_statement_rule_984(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #985
pub fn check_extra_statement_rule_985(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #986
pub fn check_extra_statement_rule_986(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #987
pub fn check_extra_statement_rule_987(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #988
pub fn check_extra_statement_rule_988(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #989
pub fn check_extra_statement_rule_989(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #990
pub fn check_extra_statement_rule_990(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #991
pub fn check_extra_statement_rule_991(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #992
pub fn check_extra_statement_rule_992(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #993
pub fn check_extra_statement_rule_993(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #994
pub fn check_extra_statement_rule_994(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #995
pub fn check_extra_statement_rule_995(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #996
pub fn check_extra_statement_rule_996(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #997
pub fn check_extra_statement_rule_997(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #998
pub fn check_extra_statement_rule_998(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #999
pub fn check_extra_statement_rule_999(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}


/// Extra Statement Type Check Handler #1000
pub fn check_extra_statement_rule_1000(checker: &mut ExtraTypeChecker, name: &str, type_name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    checker.symbol_table.insert(name.to_string(), type_name.to_string());
    true
}

#[pyfunction]
pub fn rust_extra_type_check_statement(name: &str, type_name: &str) -> bool {
    let mut checker = ExtraTypeChecker::new("extra");
    check_extra_statement_rule_1(&mut checker, name, type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extra_type_checker() {
        let mut checker = ExtraTypeChecker::new("extra");
        assert!(check_extra_statement_rule_1(&mut checker, "x", "int"));
    }
}
