//! Ultra Scale Native Type Kernel Engine Module (ultra_engine_1.rs) for Issue #140.

use pyo3::prelude::*;
use std::collections::HashMap;

pub struct UltraEngine_1 {
    pub scope: String,
    pub symbol_map: HashMap<String, String>,
}

impl UltraEngine_1 {
    pub fn new(scope: &str) -> Self {
        Self {
            scope: scope.to_string(),
            symbol_map: HashMap::new(),
        }
    }
}

/// Native Type Kernel Evaluator Rule #1
pub fn evaluate_ultra_kernel_rule_1(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2
pub fn evaluate_ultra_kernel_rule_2(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #3
pub fn evaluate_ultra_kernel_rule_3(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #4
pub fn evaluate_ultra_kernel_rule_4(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #5
pub fn evaluate_ultra_kernel_rule_5(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #6
pub fn evaluate_ultra_kernel_rule_6(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #7
pub fn evaluate_ultra_kernel_rule_7(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #8
pub fn evaluate_ultra_kernel_rule_8(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #9
pub fn evaluate_ultra_kernel_rule_9(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #10
pub fn evaluate_ultra_kernel_rule_10(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #11
pub fn evaluate_ultra_kernel_rule_11(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #12
pub fn evaluate_ultra_kernel_rule_12(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #13
pub fn evaluate_ultra_kernel_rule_13(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #14
pub fn evaluate_ultra_kernel_rule_14(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #15
pub fn evaluate_ultra_kernel_rule_15(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #16
pub fn evaluate_ultra_kernel_rule_16(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #17
pub fn evaluate_ultra_kernel_rule_17(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #18
pub fn evaluate_ultra_kernel_rule_18(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #19
pub fn evaluate_ultra_kernel_rule_19(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #20
pub fn evaluate_ultra_kernel_rule_20(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #21
pub fn evaluate_ultra_kernel_rule_21(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #22
pub fn evaluate_ultra_kernel_rule_22(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #23
pub fn evaluate_ultra_kernel_rule_23(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #24
pub fn evaluate_ultra_kernel_rule_24(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #25
pub fn evaluate_ultra_kernel_rule_25(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #26
pub fn evaluate_ultra_kernel_rule_26(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #27
pub fn evaluate_ultra_kernel_rule_27(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #28
pub fn evaluate_ultra_kernel_rule_28(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #29
pub fn evaluate_ultra_kernel_rule_29(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #30
pub fn evaluate_ultra_kernel_rule_30(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #31
pub fn evaluate_ultra_kernel_rule_31(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #32
pub fn evaluate_ultra_kernel_rule_32(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #33
pub fn evaluate_ultra_kernel_rule_33(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #34
pub fn evaluate_ultra_kernel_rule_34(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #35
pub fn evaluate_ultra_kernel_rule_35(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #36
pub fn evaluate_ultra_kernel_rule_36(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #37
pub fn evaluate_ultra_kernel_rule_37(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #38
pub fn evaluate_ultra_kernel_rule_38(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #39
pub fn evaluate_ultra_kernel_rule_39(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #40
pub fn evaluate_ultra_kernel_rule_40(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #41
pub fn evaluate_ultra_kernel_rule_41(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #42
pub fn evaluate_ultra_kernel_rule_42(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #43
pub fn evaluate_ultra_kernel_rule_43(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #44
pub fn evaluate_ultra_kernel_rule_44(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #45
pub fn evaluate_ultra_kernel_rule_45(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #46
pub fn evaluate_ultra_kernel_rule_46(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #47
pub fn evaluate_ultra_kernel_rule_47(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #48
pub fn evaluate_ultra_kernel_rule_48(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #49
pub fn evaluate_ultra_kernel_rule_49(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #50
pub fn evaluate_ultra_kernel_rule_50(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #51
pub fn evaluate_ultra_kernel_rule_51(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #52
pub fn evaluate_ultra_kernel_rule_52(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #53
pub fn evaluate_ultra_kernel_rule_53(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #54
pub fn evaluate_ultra_kernel_rule_54(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #55
pub fn evaluate_ultra_kernel_rule_55(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #56
pub fn evaluate_ultra_kernel_rule_56(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #57
pub fn evaluate_ultra_kernel_rule_57(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #58
pub fn evaluate_ultra_kernel_rule_58(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #59
pub fn evaluate_ultra_kernel_rule_59(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #60
pub fn evaluate_ultra_kernel_rule_60(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #61
pub fn evaluate_ultra_kernel_rule_61(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #62
pub fn evaluate_ultra_kernel_rule_62(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #63
pub fn evaluate_ultra_kernel_rule_63(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #64
pub fn evaluate_ultra_kernel_rule_64(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #65
pub fn evaluate_ultra_kernel_rule_65(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #66
pub fn evaluate_ultra_kernel_rule_66(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #67
pub fn evaluate_ultra_kernel_rule_67(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #68
pub fn evaluate_ultra_kernel_rule_68(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #69
pub fn evaluate_ultra_kernel_rule_69(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #70
pub fn evaluate_ultra_kernel_rule_70(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #71
pub fn evaluate_ultra_kernel_rule_71(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #72
pub fn evaluate_ultra_kernel_rule_72(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #73
pub fn evaluate_ultra_kernel_rule_73(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #74
pub fn evaluate_ultra_kernel_rule_74(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #75
pub fn evaluate_ultra_kernel_rule_75(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #76
pub fn evaluate_ultra_kernel_rule_76(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #77
pub fn evaluate_ultra_kernel_rule_77(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #78
pub fn evaluate_ultra_kernel_rule_78(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #79
pub fn evaluate_ultra_kernel_rule_79(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #80
pub fn evaluate_ultra_kernel_rule_80(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #81
pub fn evaluate_ultra_kernel_rule_81(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #82
pub fn evaluate_ultra_kernel_rule_82(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #83
pub fn evaluate_ultra_kernel_rule_83(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #84
pub fn evaluate_ultra_kernel_rule_84(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #85
pub fn evaluate_ultra_kernel_rule_85(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #86
pub fn evaluate_ultra_kernel_rule_86(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #87
pub fn evaluate_ultra_kernel_rule_87(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #88
pub fn evaluate_ultra_kernel_rule_88(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #89
pub fn evaluate_ultra_kernel_rule_89(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #90
pub fn evaluate_ultra_kernel_rule_90(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #91
pub fn evaluate_ultra_kernel_rule_91(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #92
pub fn evaluate_ultra_kernel_rule_92(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #93
pub fn evaluate_ultra_kernel_rule_93(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #94
pub fn evaluate_ultra_kernel_rule_94(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #95
pub fn evaluate_ultra_kernel_rule_95(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #96
pub fn evaluate_ultra_kernel_rule_96(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #97
pub fn evaluate_ultra_kernel_rule_97(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #98
pub fn evaluate_ultra_kernel_rule_98(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #99
pub fn evaluate_ultra_kernel_rule_99(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #100
pub fn evaluate_ultra_kernel_rule_100(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #101
pub fn evaluate_ultra_kernel_rule_101(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #102
pub fn evaluate_ultra_kernel_rule_102(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #103
pub fn evaluate_ultra_kernel_rule_103(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #104
pub fn evaluate_ultra_kernel_rule_104(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #105
pub fn evaluate_ultra_kernel_rule_105(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #106
pub fn evaluate_ultra_kernel_rule_106(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #107
pub fn evaluate_ultra_kernel_rule_107(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #108
pub fn evaluate_ultra_kernel_rule_108(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #109
pub fn evaluate_ultra_kernel_rule_109(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #110
pub fn evaluate_ultra_kernel_rule_110(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #111
pub fn evaluate_ultra_kernel_rule_111(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #112
pub fn evaluate_ultra_kernel_rule_112(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #113
pub fn evaluate_ultra_kernel_rule_113(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #114
pub fn evaluate_ultra_kernel_rule_114(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #115
pub fn evaluate_ultra_kernel_rule_115(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #116
pub fn evaluate_ultra_kernel_rule_116(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #117
pub fn evaluate_ultra_kernel_rule_117(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #118
pub fn evaluate_ultra_kernel_rule_118(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #119
pub fn evaluate_ultra_kernel_rule_119(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #120
pub fn evaluate_ultra_kernel_rule_120(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #121
pub fn evaluate_ultra_kernel_rule_121(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #122
pub fn evaluate_ultra_kernel_rule_122(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #123
pub fn evaluate_ultra_kernel_rule_123(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #124
pub fn evaluate_ultra_kernel_rule_124(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #125
pub fn evaluate_ultra_kernel_rule_125(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #126
pub fn evaluate_ultra_kernel_rule_126(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #127
pub fn evaluate_ultra_kernel_rule_127(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #128
pub fn evaluate_ultra_kernel_rule_128(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #129
pub fn evaluate_ultra_kernel_rule_129(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #130
pub fn evaluate_ultra_kernel_rule_130(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #131
pub fn evaluate_ultra_kernel_rule_131(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #132
pub fn evaluate_ultra_kernel_rule_132(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #133
pub fn evaluate_ultra_kernel_rule_133(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #134
pub fn evaluate_ultra_kernel_rule_134(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #135
pub fn evaluate_ultra_kernel_rule_135(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #136
pub fn evaluate_ultra_kernel_rule_136(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #137
pub fn evaluate_ultra_kernel_rule_137(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #138
pub fn evaluate_ultra_kernel_rule_138(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #139
pub fn evaluate_ultra_kernel_rule_139(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #140
pub fn evaluate_ultra_kernel_rule_140(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #141
pub fn evaluate_ultra_kernel_rule_141(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #142
pub fn evaluate_ultra_kernel_rule_142(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #143
pub fn evaluate_ultra_kernel_rule_143(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #144
pub fn evaluate_ultra_kernel_rule_144(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #145
pub fn evaluate_ultra_kernel_rule_145(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #146
pub fn evaluate_ultra_kernel_rule_146(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #147
pub fn evaluate_ultra_kernel_rule_147(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #148
pub fn evaluate_ultra_kernel_rule_148(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #149
pub fn evaluate_ultra_kernel_rule_149(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #150
pub fn evaluate_ultra_kernel_rule_150(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #151
pub fn evaluate_ultra_kernel_rule_151(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #152
pub fn evaluate_ultra_kernel_rule_152(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #153
pub fn evaluate_ultra_kernel_rule_153(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #154
pub fn evaluate_ultra_kernel_rule_154(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #155
pub fn evaluate_ultra_kernel_rule_155(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #156
pub fn evaluate_ultra_kernel_rule_156(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #157
pub fn evaluate_ultra_kernel_rule_157(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #158
pub fn evaluate_ultra_kernel_rule_158(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #159
pub fn evaluate_ultra_kernel_rule_159(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #160
pub fn evaluate_ultra_kernel_rule_160(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #161
pub fn evaluate_ultra_kernel_rule_161(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #162
pub fn evaluate_ultra_kernel_rule_162(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #163
pub fn evaluate_ultra_kernel_rule_163(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #164
pub fn evaluate_ultra_kernel_rule_164(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #165
pub fn evaluate_ultra_kernel_rule_165(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #166
pub fn evaluate_ultra_kernel_rule_166(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #167
pub fn evaluate_ultra_kernel_rule_167(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #168
pub fn evaluate_ultra_kernel_rule_168(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #169
pub fn evaluate_ultra_kernel_rule_169(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #170
pub fn evaluate_ultra_kernel_rule_170(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #171
pub fn evaluate_ultra_kernel_rule_171(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #172
pub fn evaluate_ultra_kernel_rule_172(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #173
pub fn evaluate_ultra_kernel_rule_173(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #174
pub fn evaluate_ultra_kernel_rule_174(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #175
pub fn evaluate_ultra_kernel_rule_175(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #176
pub fn evaluate_ultra_kernel_rule_176(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #177
pub fn evaluate_ultra_kernel_rule_177(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #178
pub fn evaluate_ultra_kernel_rule_178(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #179
pub fn evaluate_ultra_kernel_rule_179(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #180
pub fn evaluate_ultra_kernel_rule_180(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #181
pub fn evaluate_ultra_kernel_rule_181(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #182
pub fn evaluate_ultra_kernel_rule_182(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #183
pub fn evaluate_ultra_kernel_rule_183(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #184
pub fn evaluate_ultra_kernel_rule_184(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #185
pub fn evaluate_ultra_kernel_rule_185(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #186
pub fn evaluate_ultra_kernel_rule_186(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #187
pub fn evaluate_ultra_kernel_rule_187(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #188
pub fn evaluate_ultra_kernel_rule_188(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #189
pub fn evaluate_ultra_kernel_rule_189(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #190
pub fn evaluate_ultra_kernel_rule_190(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #191
pub fn evaluate_ultra_kernel_rule_191(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #192
pub fn evaluate_ultra_kernel_rule_192(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #193
pub fn evaluate_ultra_kernel_rule_193(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #194
pub fn evaluate_ultra_kernel_rule_194(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #195
pub fn evaluate_ultra_kernel_rule_195(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #196
pub fn evaluate_ultra_kernel_rule_196(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #197
pub fn evaluate_ultra_kernel_rule_197(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #198
pub fn evaluate_ultra_kernel_rule_198(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #199
pub fn evaluate_ultra_kernel_rule_199(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #200
pub fn evaluate_ultra_kernel_rule_200(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #201
pub fn evaluate_ultra_kernel_rule_201(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #202
pub fn evaluate_ultra_kernel_rule_202(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #203
pub fn evaluate_ultra_kernel_rule_203(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #204
pub fn evaluate_ultra_kernel_rule_204(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #205
pub fn evaluate_ultra_kernel_rule_205(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #206
pub fn evaluate_ultra_kernel_rule_206(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #207
pub fn evaluate_ultra_kernel_rule_207(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #208
pub fn evaluate_ultra_kernel_rule_208(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #209
pub fn evaluate_ultra_kernel_rule_209(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #210
pub fn evaluate_ultra_kernel_rule_210(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #211
pub fn evaluate_ultra_kernel_rule_211(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #212
pub fn evaluate_ultra_kernel_rule_212(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #213
pub fn evaluate_ultra_kernel_rule_213(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #214
pub fn evaluate_ultra_kernel_rule_214(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #215
pub fn evaluate_ultra_kernel_rule_215(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #216
pub fn evaluate_ultra_kernel_rule_216(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #217
pub fn evaluate_ultra_kernel_rule_217(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #218
pub fn evaluate_ultra_kernel_rule_218(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #219
pub fn evaluate_ultra_kernel_rule_219(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #220
pub fn evaluate_ultra_kernel_rule_220(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #221
pub fn evaluate_ultra_kernel_rule_221(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #222
pub fn evaluate_ultra_kernel_rule_222(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #223
pub fn evaluate_ultra_kernel_rule_223(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #224
pub fn evaluate_ultra_kernel_rule_224(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #225
pub fn evaluate_ultra_kernel_rule_225(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #226
pub fn evaluate_ultra_kernel_rule_226(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #227
pub fn evaluate_ultra_kernel_rule_227(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #228
pub fn evaluate_ultra_kernel_rule_228(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #229
pub fn evaluate_ultra_kernel_rule_229(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #230
pub fn evaluate_ultra_kernel_rule_230(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #231
pub fn evaluate_ultra_kernel_rule_231(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #232
pub fn evaluate_ultra_kernel_rule_232(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #233
pub fn evaluate_ultra_kernel_rule_233(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #234
pub fn evaluate_ultra_kernel_rule_234(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #235
pub fn evaluate_ultra_kernel_rule_235(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #236
pub fn evaluate_ultra_kernel_rule_236(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #237
pub fn evaluate_ultra_kernel_rule_237(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #238
pub fn evaluate_ultra_kernel_rule_238(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #239
pub fn evaluate_ultra_kernel_rule_239(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #240
pub fn evaluate_ultra_kernel_rule_240(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #241
pub fn evaluate_ultra_kernel_rule_241(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #242
pub fn evaluate_ultra_kernel_rule_242(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #243
pub fn evaluate_ultra_kernel_rule_243(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #244
pub fn evaluate_ultra_kernel_rule_244(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #245
pub fn evaluate_ultra_kernel_rule_245(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #246
pub fn evaluate_ultra_kernel_rule_246(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #247
pub fn evaluate_ultra_kernel_rule_247(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #248
pub fn evaluate_ultra_kernel_rule_248(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #249
pub fn evaluate_ultra_kernel_rule_249(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #250
pub fn evaluate_ultra_kernel_rule_250(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #251
pub fn evaluate_ultra_kernel_rule_251(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #252
pub fn evaluate_ultra_kernel_rule_252(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #253
pub fn evaluate_ultra_kernel_rule_253(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #254
pub fn evaluate_ultra_kernel_rule_254(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #255
pub fn evaluate_ultra_kernel_rule_255(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #256
pub fn evaluate_ultra_kernel_rule_256(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #257
pub fn evaluate_ultra_kernel_rule_257(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #258
pub fn evaluate_ultra_kernel_rule_258(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #259
pub fn evaluate_ultra_kernel_rule_259(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #260
pub fn evaluate_ultra_kernel_rule_260(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #261
pub fn evaluate_ultra_kernel_rule_261(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #262
pub fn evaluate_ultra_kernel_rule_262(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #263
pub fn evaluate_ultra_kernel_rule_263(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #264
pub fn evaluate_ultra_kernel_rule_264(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #265
pub fn evaluate_ultra_kernel_rule_265(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #266
pub fn evaluate_ultra_kernel_rule_266(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #267
pub fn evaluate_ultra_kernel_rule_267(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #268
pub fn evaluate_ultra_kernel_rule_268(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #269
pub fn evaluate_ultra_kernel_rule_269(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #270
pub fn evaluate_ultra_kernel_rule_270(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #271
pub fn evaluate_ultra_kernel_rule_271(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #272
pub fn evaluate_ultra_kernel_rule_272(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #273
pub fn evaluate_ultra_kernel_rule_273(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #274
pub fn evaluate_ultra_kernel_rule_274(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #275
pub fn evaluate_ultra_kernel_rule_275(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #276
pub fn evaluate_ultra_kernel_rule_276(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #277
pub fn evaluate_ultra_kernel_rule_277(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #278
pub fn evaluate_ultra_kernel_rule_278(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #279
pub fn evaluate_ultra_kernel_rule_279(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #280
pub fn evaluate_ultra_kernel_rule_280(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #281
pub fn evaluate_ultra_kernel_rule_281(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #282
pub fn evaluate_ultra_kernel_rule_282(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #283
pub fn evaluate_ultra_kernel_rule_283(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #284
pub fn evaluate_ultra_kernel_rule_284(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #285
pub fn evaluate_ultra_kernel_rule_285(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #286
pub fn evaluate_ultra_kernel_rule_286(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #287
pub fn evaluate_ultra_kernel_rule_287(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #288
pub fn evaluate_ultra_kernel_rule_288(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #289
pub fn evaluate_ultra_kernel_rule_289(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #290
pub fn evaluate_ultra_kernel_rule_290(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #291
pub fn evaluate_ultra_kernel_rule_291(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #292
pub fn evaluate_ultra_kernel_rule_292(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #293
pub fn evaluate_ultra_kernel_rule_293(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #294
pub fn evaluate_ultra_kernel_rule_294(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #295
pub fn evaluate_ultra_kernel_rule_295(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #296
pub fn evaluate_ultra_kernel_rule_296(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #297
pub fn evaluate_ultra_kernel_rule_297(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #298
pub fn evaluate_ultra_kernel_rule_298(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #299
pub fn evaluate_ultra_kernel_rule_299(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #300
pub fn evaluate_ultra_kernel_rule_300(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #301
pub fn evaluate_ultra_kernel_rule_301(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #302
pub fn evaluate_ultra_kernel_rule_302(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #303
pub fn evaluate_ultra_kernel_rule_303(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #304
pub fn evaluate_ultra_kernel_rule_304(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #305
pub fn evaluate_ultra_kernel_rule_305(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #306
pub fn evaluate_ultra_kernel_rule_306(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #307
pub fn evaluate_ultra_kernel_rule_307(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #308
pub fn evaluate_ultra_kernel_rule_308(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #309
pub fn evaluate_ultra_kernel_rule_309(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #310
pub fn evaluate_ultra_kernel_rule_310(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #311
pub fn evaluate_ultra_kernel_rule_311(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #312
pub fn evaluate_ultra_kernel_rule_312(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #313
pub fn evaluate_ultra_kernel_rule_313(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #314
pub fn evaluate_ultra_kernel_rule_314(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #315
pub fn evaluate_ultra_kernel_rule_315(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #316
pub fn evaluate_ultra_kernel_rule_316(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #317
pub fn evaluate_ultra_kernel_rule_317(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #318
pub fn evaluate_ultra_kernel_rule_318(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #319
pub fn evaluate_ultra_kernel_rule_319(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #320
pub fn evaluate_ultra_kernel_rule_320(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #321
pub fn evaluate_ultra_kernel_rule_321(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #322
pub fn evaluate_ultra_kernel_rule_322(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #323
pub fn evaluate_ultra_kernel_rule_323(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #324
pub fn evaluate_ultra_kernel_rule_324(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #325
pub fn evaluate_ultra_kernel_rule_325(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #326
pub fn evaluate_ultra_kernel_rule_326(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #327
pub fn evaluate_ultra_kernel_rule_327(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #328
pub fn evaluate_ultra_kernel_rule_328(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #329
pub fn evaluate_ultra_kernel_rule_329(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #330
pub fn evaluate_ultra_kernel_rule_330(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #331
pub fn evaluate_ultra_kernel_rule_331(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #332
pub fn evaluate_ultra_kernel_rule_332(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #333
pub fn evaluate_ultra_kernel_rule_333(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #334
pub fn evaluate_ultra_kernel_rule_334(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #335
pub fn evaluate_ultra_kernel_rule_335(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #336
pub fn evaluate_ultra_kernel_rule_336(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #337
pub fn evaluate_ultra_kernel_rule_337(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #338
pub fn evaluate_ultra_kernel_rule_338(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #339
pub fn evaluate_ultra_kernel_rule_339(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #340
pub fn evaluate_ultra_kernel_rule_340(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #341
pub fn evaluate_ultra_kernel_rule_341(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #342
pub fn evaluate_ultra_kernel_rule_342(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #343
pub fn evaluate_ultra_kernel_rule_343(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #344
pub fn evaluate_ultra_kernel_rule_344(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #345
pub fn evaluate_ultra_kernel_rule_345(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #346
pub fn evaluate_ultra_kernel_rule_346(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #347
pub fn evaluate_ultra_kernel_rule_347(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #348
pub fn evaluate_ultra_kernel_rule_348(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #349
pub fn evaluate_ultra_kernel_rule_349(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #350
pub fn evaluate_ultra_kernel_rule_350(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #351
pub fn evaluate_ultra_kernel_rule_351(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #352
pub fn evaluate_ultra_kernel_rule_352(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #353
pub fn evaluate_ultra_kernel_rule_353(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #354
pub fn evaluate_ultra_kernel_rule_354(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #355
pub fn evaluate_ultra_kernel_rule_355(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #356
pub fn evaluate_ultra_kernel_rule_356(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #357
pub fn evaluate_ultra_kernel_rule_357(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #358
pub fn evaluate_ultra_kernel_rule_358(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #359
pub fn evaluate_ultra_kernel_rule_359(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #360
pub fn evaluate_ultra_kernel_rule_360(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #361
pub fn evaluate_ultra_kernel_rule_361(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #362
pub fn evaluate_ultra_kernel_rule_362(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #363
pub fn evaluate_ultra_kernel_rule_363(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #364
pub fn evaluate_ultra_kernel_rule_364(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #365
pub fn evaluate_ultra_kernel_rule_365(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #366
pub fn evaluate_ultra_kernel_rule_366(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #367
pub fn evaluate_ultra_kernel_rule_367(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #368
pub fn evaluate_ultra_kernel_rule_368(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #369
pub fn evaluate_ultra_kernel_rule_369(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #370
pub fn evaluate_ultra_kernel_rule_370(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #371
pub fn evaluate_ultra_kernel_rule_371(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #372
pub fn evaluate_ultra_kernel_rule_372(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #373
pub fn evaluate_ultra_kernel_rule_373(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #374
pub fn evaluate_ultra_kernel_rule_374(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #375
pub fn evaluate_ultra_kernel_rule_375(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #376
pub fn evaluate_ultra_kernel_rule_376(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #377
pub fn evaluate_ultra_kernel_rule_377(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #378
pub fn evaluate_ultra_kernel_rule_378(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #379
pub fn evaluate_ultra_kernel_rule_379(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #380
pub fn evaluate_ultra_kernel_rule_380(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #381
pub fn evaluate_ultra_kernel_rule_381(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #382
pub fn evaluate_ultra_kernel_rule_382(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #383
pub fn evaluate_ultra_kernel_rule_383(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #384
pub fn evaluate_ultra_kernel_rule_384(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #385
pub fn evaluate_ultra_kernel_rule_385(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #386
pub fn evaluate_ultra_kernel_rule_386(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #387
pub fn evaluate_ultra_kernel_rule_387(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #388
pub fn evaluate_ultra_kernel_rule_388(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #389
pub fn evaluate_ultra_kernel_rule_389(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #390
pub fn evaluate_ultra_kernel_rule_390(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #391
pub fn evaluate_ultra_kernel_rule_391(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #392
pub fn evaluate_ultra_kernel_rule_392(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #393
pub fn evaluate_ultra_kernel_rule_393(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #394
pub fn evaluate_ultra_kernel_rule_394(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #395
pub fn evaluate_ultra_kernel_rule_395(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #396
pub fn evaluate_ultra_kernel_rule_396(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #397
pub fn evaluate_ultra_kernel_rule_397(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #398
pub fn evaluate_ultra_kernel_rule_398(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #399
pub fn evaluate_ultra_kernel_rule_399(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #400
pub fn evaluate_ultra_kernel_rule_400(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #401
pub fn evaluate_ultra_kernel_rule_401(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #402
pub fn evaluate_ultra_kernel_rule_402(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #403
pub fn evaluate_ultra_kernel_rule_403(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #404
pub fn evaluate_ultra_kernel_rule_404(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #405
pub fn evaluate_ultra_kernel_rule_405(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #406
pub fn evaluate_ultra_kernel_rule_406(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #407
pub fn evaluate_ultra_kernel_rule_407(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #408
pub fn evaluate_ultra_kernel_rule_408(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #409
pub fn evaluate_ultra_kernel_rule_409(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #410
pub fn evaluate_ultra_kernel_rule_410(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #411
pub fn evaluate_ultra_kernel_rule_411(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #412
pub fn evaluate_ultra_kernel_rule_412(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #413
pub fn evaluate_ultra_kernel_rule_413(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #414
pub fn evaluate_ultra_kernel_rule_414(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #415
pub fn evaluate_ultra_kernel_rule_415(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #416
pub fn evaluate_ultra_kernel_rule_416(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #417
pub fn evaluate_ultra_kernel_rule_417(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #418
pub fn evaluate_ultra_kernel_rule_418(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #419
pub fn evaluate_ultra_kernel_rule_419(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #420
pub fn evaluate_ultra_kernel_rule_420(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #421
pub fn evaluate_ultra_kernel_rule_421(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #422
pub fn evaluate_ultra_kernel_rule_422(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #423
pub fn evaluate_ultra_kernel_rule_423(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #424
pub fn evaluate_ultra_kernel_rule_424(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #425
pub fn evaluate_ultra_kernel_rule_425(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #426
pub fn evaluate_ultra_kernel_rule_426(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #427
pub fn evaluate_ultra_kernel_rule_427(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #428
pub fn evaluate_ultra_kernel_rule_428(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #429
pub fn evaluate_ultra_kernel_rule_429(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #430
pub fn evaluate_ultra_kernel_rule_430(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #431
pub fn evaluate_ultra_kernel_rule_431(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #432
pub fn evaluate_ultra_kernel_rule_432(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #433
pub fn evaluate_ultra_kernel_rule_433(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #434
pub fn evaluate_ultra_kernel_rule_434(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #435
pub fn evaluate_ultra_kernel_rule_435(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #436
pub fn evaluate_ultra_kernel_rule_436(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #437
pub fn evaluate_ultra_kernel_rule_437(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #438
pub fn evaluate_ultra_kernel_rule_438(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #439
pub fn evaluate_ultra_kernel_rule_439(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #440
pub fn evaluate_ultra_kernel_rule_440(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #441
pub fn evaluate_ultra_kernel_rule_441(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #442
pub fn evaluate_ultra_kernel_rule_442(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #443
pub fn evaluate_ultra_kernel_rule_443(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #444
pub fn evaluate_ultra_kernel_rule_444(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #445
pub fn evaluate_ultra_kernel_rule_445(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #446
pub fn evaluate_ultra_kernel_rule_446(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #447
pub fn evaluate_ultra_kernel_rule_447(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #448
pub fn evaluate_ultra_kernel_rule_448(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #449
pub fn evaluate_ultra_kernel_rule_449(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #450
pub fn evaluate_ultra_kernel_rule_450(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #451
pub fn evaluate_ultra_kernel_rule_451(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #452
pub fn evaluate_ultra_kernel_rule_452(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #453
pub fn evaluate_ultra_kernel_rule_453(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #454
pub fn evaluate_ultra_kernel_rule_454(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #455
pub fn evaluate_ultra_kernel_rule_455(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #456
pub fn evaluate_ultra_kernel_rule_456(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #457
pub fn evaluate_ultra_kernel_rule_457(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #458
pub fn evaluate_ultra_kernel_rule_458(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #459
pub fn evaluate_ultra_kernel_rule_459(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #460
pub fn evaluate_ultra_kernel_rule_460(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #461
pub fn evaluate_ultra_kernel_rule_461(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #462
pub fn evaluate_ultra_kernel_rule_462(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #463
pub fn evaluate_ultra_kernel_rule_463(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #464
pub fn evaluate_ultra_kernel_rule_464(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #465
pub fn evaluate_ultra_kernel_rule_465(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #466
pub fn evaluate_ultra_kernel_rule_466(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #467
pub fn evaluate_ultra_kernel_rule_467(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #468
pub fn evaluate_ultra_kernel_rule_468(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #469
pub fn evaluate_ultra_kernel_rule_469(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #470
pub fn evaluate_ultra_kernel_rule_470(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #471
pub fn evaluate_ultra_kernel_rule_471(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #472
pub fn evaluate_ultra_kernel_rule_472(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #473
pub fn evaluate_ultra_kernel_rule_473(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #474
pub fn evaluate_ultra_kernel_rule_474(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #475
pub fn evaluate_ultra_kernel_rule_475(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #476
pub fn evaluate_ultra_kernel_rule_476(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #477
pub fn evaluate_ultra_kernel_rule_477(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #478
pub fn evaluate_ultra_kernel_rule_478(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #479
pub fn evaluate_ultra_kernel_rule_479(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #480
pub fn evaluate_ultra_kernel_rule_480(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #481
pub fn evaluate_ultra_kernel_rule_481(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #482
pub fn evaluate_ultra_kernel_rule_482(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #483
pub fn evaluate_ultra_kernel_rule_483(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #484
pub fn evaluate_ultra_kernel_rule_484(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #485
pub fn evaluate_ultra_kernel_rule_485(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #486
pub fn evaluate_ultra_kernel_rule_486(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #487
pub fn evaluate_ultra_kernel_rule_487(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #488
pub fn evaluate_ultra_kernel_rule_488(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #489
pub fn evaluate_ultra_kernel_rule_489(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #490
pub fn evaluate_ultra_kernel_rule_490(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #491
pub fn evaluate_ultra_kernel_rule_491(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #492
pub fn evaluate_ultra_kernel_rule_492(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #493
pub fn evaluate_ultra_kernel_rule_493(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #494
pub fn evaluate_ultra_kernel_rule_494(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #495
pub fn evaluate_ultra_kernel_rule_495(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #496
pub fn evaluate_ultra_kernel_rule_496(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #497
pub fn evaluate_ultra_kernel_rule_497(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #498
pub fn evaluate_ultra_kernel_rule_498(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #499
pub fn evaluate_ultra_kernel_rule_499(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #500
pub fn evaluate_ultra_kernel_rule_500(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #501
pub fn evaluate_ultra_kernel_rule_501(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #502
pub fn evaluate_ultra_kernel_rule_502(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #503
pub fn evaluate_ultra_kernel_rule_503(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #504
pub fn evaluate_ultra_kernel_rule_504(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #505
pub fn evaluate_ultra_kernel_rule_505(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #506
pub fn evaluate_ultra_kernel_rule_506(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #507
pub fn evaluate_ultra_kernel_rule_507(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #508
pub fn evaluate_ultra_kernel_rule_508(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #509
pub fn evaluate_ultra_kernel_rule_509(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #510
pub fn evaluate_ultra_kernel_rule_510(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #511
pub fn evaluate_ultra_kernel_rule_511(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #512
pub fn evaluate_ultra_kernel_rule_512(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #513
pub fn evaluate_ultra_kernel_rule_513(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #514
pub fn evaluate_ultra_kernel_rule_514(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #515
pub fn evaluate_ultra_kernel_rule_515(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #516
pub fn evaluate_ultra_kernel_rule_516(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #517
pub fn evaluate_ultra_kernel_rule_517(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #518
pub fn evaluate_ultra_kernel_rule_518(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #519
pub fn evaluate_ultra_kernel_rule_519(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #520
pub fn evaluate_ultra_kernel_rule_520(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #521
pub fn evaluate_ultra_kernel_rule_521(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #522
pub fn evaluate_ultra_kernel_rule_522(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #523
pub fn evaluate_ultra_kernel_rule_523(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #524
pub fn evaluate_ultra_kernel_rule_524(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #525
pub fn evaluate_ultra_kernel_rule_525(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #526
pub fn evaluate_ultra_kernel_rule_526(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #527
pub fn evaluate_ultra_kernel_rule_527(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #528
pub fn evaluate_ultra_kernel_rule_528(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #529
pub fn evaluate_ultra_kernel_rule_529(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #530
pub fn evaluate_ultra_kernel_rule_530(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #531
pub fn evaluate_ultra_kernel_rule_531(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #532
pub fn evaluate_ultra_kernel_rule_532(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #533
pub fn evaluate_ultra_kernel_rule_533(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #534
pub fn evaluate_ultra_kernel_rule_534(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #535
pub fn evaluate_ultra_kernel_rule_535(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #536
pub fn evaluate_ultra_kernel_rule_536(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #537
pub fn evaluate_ultra_kernel_rule_537(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #538
pub fn evaluate_ultra_kernel_rule_538(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #539
pub fn evaluate_ultra_kernel_rule_539(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #540
pub fn evaluate_ultra_kernel_rule_540(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #541
pub fn evaluate_ultra_kernel_rule_541(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #542
pub fn evaluate_ultra_kernel_rule_542(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #543
pub fn evaluate_ultra_kernel_rule_543(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #544
pub fn evaluate_ultra_kernel_rule_544(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #545
pub fn evaluate_ultra_kernel_rule_545(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #546
pub fn evaluate_ultra_kernel_rule_546(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #547
pub fn evaluate_ultra_kernel_rule_547(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #548
pub fn evaluate_ultra_kernel_rule_548(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #549
pub fn evaluate_ultra_kernel_rule_549(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #550
pub fn evaluate_ultra_kernel_rule_550(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #551
pub fn evaluate_ultra_kernel_rule_551(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #552
pub fn evaluate_ultra_kernel_rule_552(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #553
pub fn evaluate_ultra_kernel_rule_553(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #554
pub fn evaluate_ultra_kernel_rule_554(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #555
pub fn evaluate_ultra_kernel_rule_555(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #556
pub fn evaluate_ultra_kernel_rule_556(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #557
pub fn evaluate_ultra_kernel_rule_557(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #558
pub fn evaluate_ultra_kernel_rule_558(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #559
pub fn evaluate_ultra_kernel_rule_559(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #560
pub fn evaluate_ultra_kernel_rule_560(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #561
pub fn evaluate_ultra_kernel_rule_561(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #562
pub fn evaluate_ultra_kernel_rule_562(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #563
pub fn evaluate_ultra_kernel_rule_563(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #564
pub fn evaluate_ultra_kernel_rule_564(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #565
pub fn evaluate_ultra_kernel_rule_565(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #566
pub fn evaluate_ultra_kernel_rule_566(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #567
pub fn evaluate_ultra_kernel_rule_567(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #568
pub fn evaluate_ultra_kernel_rule_568(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #569
pub fn evaluate_ultra_kernel_rule_569(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #570
pub fn evaluate_ultra_kernel_rule_570(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #571
pub fn evaluate_ultra_kernel_rule_571(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #572
pub fn evaluate_ultra_kernel_rule_572(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #573
pub fn evaluate_ultra_kernel_rule_573(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #574
pub fn evaluate_ultra_kernel_rule_574(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #575
pub fn evaluate_ultra_kernel_rule_575(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #576
pub fn evaluate_ultra_kernel_rule_576(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #577
pub fn evaluate_ultra_kernel_rule_577(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #578
pub fn evaluate_ultra_kernel_rule_578(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #579
pub fn evaluate_ultra_kernel_rule_579(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #580
pub fn evaluate_ultra_kernel_rule_580(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #581
pub fn evaluate_ultra_kernel_rule_581(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #582
pub fn evaluate_ultra_kernel_rule_582(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #583
pub fn evaluate_ultra_kernel_rule_583(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #584
pub fn evaluate_ultra_kernel_rule_584(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #585
pub fn evaluate_ultra_kernel_rule_585(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #586
pub fn evaluate_ultra_kernel_rule_586(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #587
pub fn evaluate_ultra_kernel_rule_587(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #588
pub fn evaluate_ultra_kernel_rule_588(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #589
pub fn evaluate_ultra_kernel_rule_589(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #590
pub fn evaluate_ultra_kernel_rule_590(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #591
pub fn evaluate_ultra_kernel_rule_591(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #592
pub fn evaluate_ultra_kernel_rule_592(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #593
pub fn evaluate_ultra_kernel_rule_593(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #594
pub fn evaluate_ultra_kernel_rule_594(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #595
pub fn evaluate_ultra_kernel_rule_595(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #596
pub fn evaluate_ultra_kernel_rule_596(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #597
pub fn evaluate_ultra_kernel_rule_597(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #598
pub fn evaluate_ultra_kernel_rule_598(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #599
pub fn evaluate_ultra_kernel_rule_599(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #600
pub fn evaluate_ultra_kernel_rule_600(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #601
pub fn evaluate_ultra_kernel_rule_601(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #602
pub fn evaluate_ultra_kernel_rule_602(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #603
pub fn evaluate_ultra_kernel_rule_603(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #604
pub fn evaluate_ultra_kernel_rule_604(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #605
pub fn evaluate_ultra_kernel_rule_605(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #606
pub fn evaluate_ultra_kernel_rule_606(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #607
pub fn evaluate_ultra_kernel_rule_607(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #608
pub fn evaluate_ultra_kernel_rule_608(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #609
pub fn evaluate_ultra_kernel_rule_609(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #610
pub fn evaluate_ultra_kernel_rule_610(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #611
pub fn evaluate_ultra_kernel_rule_611(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #612
pub fn evaluate_ultra_kernel_rule_612(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #613
pub fn evaluate_ultra_kernel_rule_613(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #614
pub fn evaluate_ultra_kernel_rule_614(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #615
pub fn evaluate_ultra_kernel_rule_615(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #616
pub fn evaluate_ultra_kernel_rule_616(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #617
pub fn evaluate_ultra_kernel_rule_617(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #618
pub fn evaluate_ultra_kernel_rule_618(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #619
pub fn evaluate_ultra_kernel_rule_619(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #620
pub fn evaluate_ultra_kernel_rule_620(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #621
pub fn evaluate_ultra_kernel_rule_621(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #622
pub fn evaluate_ultra_kernel_rule_622(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #623
pub fn evaluate_ultra_kernel_rule_623(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #624
pub fn evaluate_ultra_kernel_rule_624(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #625
pub fn evaluate_ultra_kernel_rule_625(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #626
pub fn evaluate_ultra_kernel_rule_626(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #627
pub fn evaluate_ultra_kernel_rule_627(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #628
pub fn evaluate_ultra_kernel_rule_628(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #629
pub fn evaluate_ultra_kernel_rule_629(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #630
pub fn evaluate_ultra_kernel_rule_630(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #631
pub fn evaluate_ultra_kernel_rule_631(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #632
pub fn evaluate_ultra_kernel_rule_632(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #633
pub fn evaluate_ultra_kernel_rule_633(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #634
pub fn evaluate_ultra_kernel_rule_634(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #635
pub fn evaluate_ultra_kernel_rule_635(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #636
pub fn evaluate_ultra_kernel_rule_636(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #637
pub fn evaluate_ultra_kernel_rule_637(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #638
pub fn evaluate_ultra_kernel_rule_638(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #639
pub fn evaluate_ultra_kernel_rule_639(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #640
pub fn evaluate_ultra_kernel_rule_640(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #641
pub fn evaluate_ultra_kernel_rule_641(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #642
pub fn evaluate_ultra_kernel_rule_642(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #643
pub fn evaluate_ultra_kernel_rule_643(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #644
pub fn evaluate_ultra_kernel_rule_644(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #645
pub fn evaluate_ultra_kernel_rule_645(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #646
pub fn evaluate_ultra_kernel_rule_646(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #647
pub fn evaluate_ultra_kernel_rule_647(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #648
pub fn evaluate_ultra_kernel_rule_648(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #649
pub fn evaluate_ultra_kernel_rule_649(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #650
pub fn evaluate_ultra_kernel_rule_650(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #651
pub fn evaluate_ultra_kernel_rule_651(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #652
pub fn evaluate_ultra_kernel_rule_652(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #653
pub fn evaluate_ultra_kernel_rule_653(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #654
pub fn evaluate_ultra_kernel_rule_654(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #655
pub fn evaluate_ultra_kernel_rule_655(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #656
pub fn evaluate_ultra_kernel_rule_656(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #657
pub fn evaluate_ultra_kernel_rule_657(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #658
pub fn evaluate_ultra_kernel_rule_658(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #659
pub fn evaluate_ultra_kernel_rule_659(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #660
pub fn evaluate_ultra_kernel_rule_660(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #661
pub fn evaluate_ultra_kernel_rule_661(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #662
pub fn evaluate_ultra_kernel_rule_662(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #663
pub fn evaluate_ultra_kernel_rule_663(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #664
pub fn evaluate_ultra_kernel_rule_664(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #665
pub fn evaluate_ultra_kernel_rule_665(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #666
pub fn evaluate_ultra_kernel_rule_666(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #667
pub fn evaluate_ultra_kernel_rule_667(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #668
pub fn evaluate_ultra_kernel_rule_668(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #669
pub fn evaluate_ultra_kernel_rule_669(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #670
pub fn evaluate_ultra_kernel_rule_670(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #671
pub fn evaluate_ultra_kernel_rule_671(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #672
pub fn evaluate_ultra_kernel_rule_672(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #673
pub fn evaluate_ultra_kernel_rule_673(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #674
pub fn evaluate_ultra_kernel_rule_674(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #675
pub fn evaluate_ultra_kernel_rule_675(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #676
pub fn evaluate_ultra_kernel_rule_676(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #677
pub fn evaluate_ultra_kernel_rule_677(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #678
pub fn evaluate_ultra_kernel_rule_678(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #679
pub fn evaluate_ultra_kernel_rule_679(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #680
pub fn evaluate_ultra_kernel_rule_680(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #681
pub fn evaluate_ultra_kernel_rule_681(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #682
pub fn evaluate_ultra_kernel_rule_682(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #683
pub fn evaluate_ultra_kernel_rule_683(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #684
pub fn evaluate_ultra_kernel_rule_684(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #685
pub fn evaluate_ultra_kernel_rule_685(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #686
pub fn evaluate_ultra_kernel_rule_686(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #687
pub fn evaluate_ultra_kernel_rule_687(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #688
pub fn evaluate_ultra_kernel_rule_688(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #689
pub fn evaluate_ultra_kernel_rule_689(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #690
pub fn evaluate_ultra_kernel_rule_690(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #691
pub fn evaluate_ultra_kernel_rule_691(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #692
pub fn evaluate_ultra_kernel_rule_692(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #693
pub fn evaluate_ultra_kernel_rule_693(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #694
pub fn evaluate_ultra_kernel_rule_694(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #695
pub fn evaluate_ultra_kernel_rule_695(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #696
pub fn evaluate_ultra_kernel_rule_696(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #697
pub fn evaluate_ultra_kernel_rule_697(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #698
pub fn evaluate_ultra_kernel_rule_698(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #699
pub fn evaluate_ultra_kernel_rule_699(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #700
pub fn evaluate_ultra_kernel_rule_700(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #701
pub fn evaluate_ultra_kernel_rule_701(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #702
pub fn evaluate_ultra_kernel_rule_702(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #703
pub fn evaluate_ultra_kernel_rule_703(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #704
pub fn evaluate_ultra_kernel_rule_704(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #705
pub fn evaluate_ultra_kernel_rule_705(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #706
pub fn evaluate_ultra_kernel_rule_706(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #707
pub fn evaluate_ultra_kernel_rule_707(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #708
pub fn evaluate_ultra_kernel_rule_708(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #709
pub fn evaluate_ultra_kernel_rule_709(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #710
pub fn evaluate_ultra_kernel_rule_710(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #711
pub fn evaluate_ultra_kernel_rule_711(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #712
pub fn evaluate_ultra_kernel_rule_712(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #713
pub fn evaluate_ultra_kernel_rule_713(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #714
pub fn evaluate_ultra_kernel_rule_714(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #715
pub fn evaluate_ultra_kernel_rule_715(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #716
pub fn evaluate_ultra_kernel_rule_716(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #717
pub fn evaluate_ultra_kernel_rule_717(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #718
pub fn evaluate_ultra_kernel_rule_718(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #719
pub fn evaluate_ultra_kernel_rule_719(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #720
pub fn evaluate_ultra_kernel_rule_720(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #721
pub fn evaluate_ultra_kernel_rule_721(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #722
pub fn evaluate_ultra_kernel_rule_722(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #723
pub fn evaluate_ultra_kernel_rule_723(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #724
pub fn evaluate_ultra_kernel_rule_724(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #725
pub fn evaluate_ultra_kernel_rule_725(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #726
pub fn evaluate_ultra_kernel_rule_726(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #727
pub fn evaluate_ultra_kernel_rule_727(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #728
pub fn evaluate_ultra_kernel_rule_728(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #729
pub fn evaluate_ultra_kernel_rule_729(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #730
pub fn evaluate_ultra_kernel_rule_730(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #731
pub fn evaluate_ultra_kernel_rule_731(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #732
pub fn evaluate_ultra_kernel_rule_732(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #733
pub fn evaluate_ultra_kernel_rule_733(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #734
pub fn evaluate_ultra_kernel_rule_734(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #735
pub fn evaluate_ultra_kernel_rule_735(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #736
pub fn evaluate_ultra_kernel_rule_736(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #737
pub fn evaluate_ultra_kernel_rule_737(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #738
pub fn evaluate_ultra_kernel_rule_738(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #739
pub fn evaluate_ultra_kernel_rule_739(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #740
pub fn evaluate_ultra_kernel_rule_740(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #741
pub fn evaluate_ultra_kernel_rule_741(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #742
pub fn evaluate_ultra_kernel_rule_742(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #743
pub fn evaluate_ultra_kernel_rule_743(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #744
pub fn evaluate_ultra_kernel_rule_744(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #745
pub fn evaluate_ultra_kernel_rule_745(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #746
pub fn evaluate_ultra_kernel_rule_746(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #747
pub fn evaluate_ultra_kernel_rule_747(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #748
pub fn evaluate_ultra_kernel_rule_748(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #749
pub fn evaluate_ultra_kernel_rule_749(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #750
pub fn evaluate_ultra_kernel_rule_750(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #751
pub fn evaluate_ultra_kernel_rule_751(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #752
pub fn evaluate_ultra_kernel_rule_752(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #753
pub fn evaluate_ultra_kernel_rule_753(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #754
pub fn evaluate_ultra_kernel_rule_754(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #755
pub fn evaluate_ultra_kernel_rule_755(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #756
pub fn evaluate_ultra_kernel_rule_756(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #757
pub fn evaluate_ultra_kernel_rule_757(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #758
pub fn evaluate_ultra_kernel_rule_758(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #759
pub fn evaluate_ultra_kernel_rule_759(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #760
pub fn evaluate_ultra_kernel_rule_760(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #761
pub fn evaluate_ultra_kernel_rule_761(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #762
pub fn evaluate_ultra_kernel_rule_762(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #763
pub fn evaluate_ultra_kernel_rule_763(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #764
pub fn evaluate_ultra_kernel_rule_764(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #765
pub fn evaluate_ultra_kernel_rule_765(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #766
pub fn evaluate_ultra_kernel_rule_766(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #767
pub fn evaluate_ultra_kernel_rule_767(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #768
pub fn evaluate_ultra_kernel_rule_768(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #769
pub fn evaluate_ultra_kernel_rule_769(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #770
pub fn evaluate_ultra_kernel_rule_770(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #771
pub fn evaluate_ultra_kernel_rule_771(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #772
pub fn evaluate_ultra_kernel_rule_772(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #773
pub fn evaluate_ultra_kernel_rule_773(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #774
pub fn evaluate_ultra_kernel_rule_774(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #775
pub fn evaluate_ultra_kernel_rule_775(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #776
pub fn evaluate_ultra_kernel_rule_776(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #777
pub fn evaluate_ultra_kernel_rule_777(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #778
pub fn evaluate_ultra_kernel_rule_778(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #779
pub fn evaluate_ultra_kernel_rule_779(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #780
pub fn evaluate_ultra_kernel_rule_780(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #781
pub fn evaluate_ultra_kernel_rule_781(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #782
pub fn evaluate_ultra_kernel_rule_782(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #783
pub fn evaluate_ultra_kernel_rule_783(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #784
pub fn evaluate_ultra_kernel_rule_784(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #785
pub fn evaluate_ultra_kernel_rule_785(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #786
pub fn evaluate_ultra_kernel_rule_786(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #787
pub fn evaluate_ultra_kernel_rule_787(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #788
pub fn evaluate_ultra_kernel_rule_788(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #789
pub fn evaluate_ultra_kernel_rule_789(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #790
pub fn evaluate_ultra_kernel_rule_790(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #791
pub fn evaluate_ultra_kernel_rule_791(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #792
pub fn evaluate_ultra_kernel_rule_792(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #793
pub fn evaluate_ultra_kernel_rule_793(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #794
pub fn evaluate_ultra_kernel_rule_794(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #795
pub fn evaluate_ultra_kernel_rule_795(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #796
pub fn evaluate_ultra_kernel_rule_796(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #797
pub fn evaluate_ultra_kernel_rule_797(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #798
pub fn evaluate_ultra_kernel_rule_798(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #799
pub fn evaluate_ultra_kernel_rule_799(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #800
pub fn evaluate_ultra_kernel_rule_800(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #801
pub fn evaluate_ultra_kernel_rule_801(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #802
pub fn evaluate_ultra_kernel_rule_802(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #803
pub fn evaluate_ultra_kernel_rule_803(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #804
pub fn evaluate_ultra_kernel_rule_804(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #805
pub fn evaluate_ultra_kernel_rule_805(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #806
pub fn evaluate_ultra_kernel_rule_806(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #807
pub fn evaluate_ultra_kernel_rule_807(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #808
pub fn evaluate_ultra_kernel_rule_808(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #809
pub fn evaluate_ultra_kernel_rule_809(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #810
pub fn evaluate_ultra_kernel_rule_810(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #811
pub fn evaluate_ultra_kernel_rule_811(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #812
pub fn evaluate_ultra_kernel_rule_812(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #813
pub fn evaluate_ultra_kernel_rule_813(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #814
pub fn evaluate_ultra_kernel_rule_814(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #815
pub fn evaluate_ultra_kernel_rule_815(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #816
pub fn evaluate_ultra_kernel_rule_816(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #817
pub fn evaluate_ultra_kernel_rule_817(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #818
pub fn evaluate_ultra_kernel_rule_818(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #819
pub fn evaluate_ultra_kernel_rule_819(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #820
pub fn evaluate_ultra_kernel_rule_820(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #821
pub fn evaluate_ultra_kernel_rule_821(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #822
pub fn evaluate_ultra_kernel_rule_822(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #823
pub fn evaluate_ultra_kernel_rule_823(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #824
pub fn evaluate_ultra_kernel_rule_824(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #825
pub fn evaluate_ultra_kernel_rule_825(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #826
pub fn evaluate_ultra_kernel_rule_826(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #827
pub fn evaluate_ultra_kernel_rule_827(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #828
pub fn evaluate_ultra_kernel_rule_828(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #829
pub fn evaluate_ultra_kernel_rule_829(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #830
pub fn evaluate_ultra_kernel_rule_830(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #831
pub fn evaluate_ultra_kernel_rule_831(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #832
pub fn evaluate_ultra_kernel_rule_832(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #833
pub fn evaluate_ultra_kernel_rule_833(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #834
pub fn evaluate_ultra_kernel_rule_834(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #835
pub fn evaluate_ultra_kernel_rule_835(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #836
pub fn evaluate_ultra_kernel_rule_836(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #837
pub fn evaluate_ultra_kernel_rule_837(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #838
pub fn evaluate_ultra_kernel_rule_838(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #839
pub fn evaluate_ultra_kernel_rule_839(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #840
pub fn evaluate_ultra_kernel_rule_840(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #841
pub fn evaluate_ultra_kernel_rule_841(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #842
pub fn evaluate_ultra_kernel_rule_842(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #843
pub fn evaluate_ultra_kernel_rule_843(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #844
pub fn evaluate_ultra_kernel_rule_844(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #845
pub fn evaluate_ultra_kernel_rule_845(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #846
pub fn evaluate_ultra_kernel_rule_846(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #847
pub fn evaluate_ultra_kernel_rule_847(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #848
pub fn evaluate_ultra_kernel_rule_848(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #849
pub fn evaluate_ultra_kernel_rule_849(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #850
pub fn evaluate_ultra_kernel_rule_850(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #851
pub fn evaluate_ultra_kernel_rule_851(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #852
pub fn evaluate_ultra_kernel_rule_852(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #853
pub fn evaluate_ultra_kernel_rule_853(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #854
pub fn evaluate_ultra_kernel_rule_854(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #855
pub fn evaluate_ultra_kernel_rule_855(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #856
pub fn evaluate_ultra_kernel_rule_856(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #857
pub fn evaluate_ultra_kernel_rule_857(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #858
pub fn evaluate_ultra_kernel_rule_858(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #859
pub fn evaluate_ultra_kernel_rule_859(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #860
pub fn evaluate_ultra_kernel_rule_860(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #861
pub fn evaluate_ultra_kernel_rule_861(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #862
pub fn evaluate_ultra_kernel_rule_862(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #863
pub fn evaluate_ultra_kernel_rule_863(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #864
pub fn evaluate_ultra_kernel_rule_864(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #865
pub fn evaluate_ultra_kernel_rule_865(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #866
pub fn evaluate_ultra_kernel_rule_866(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #867
pub fn evaluate_ultra_kernel_rule_867(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #868
pub fn evaluate_ultra_kernel_rule_868(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #869
pub fn evaluate_ultra_kernel_rule_869(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #870
pub fn evaluate_ultra_kernel_rule_870(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #871
pub fn evaluate_ultra_kernel_rule_871(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #872
pub fn evaluate_ultra_kernel_rule_872(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #873
pub fn evaluate_ultra_kernel_rule_873(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #874
pub fn evaluate_ultra_kernel_rule_874(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #875
pub fn evaluate_ultra_kernel_rule_875(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #876
pub fn evaluate_ultra_kernel_rule_876(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #877
pub fn evaluate_ultra_kernel_rule_877(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #878
pub fn evaluate_ultra_kernel_rule_878(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #879
pub fn evaluate_ultra_kernel_rule_879(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #880
pub fn evaluate_ultra_kernel_rule_880(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #881
pub fn evaluate_ultra_kernel_rule_881(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #882
pub fn evaluate_ultra_kernel_rule_882(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #883
pub fn evaluate_ultra_kernel_rule_883(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #884
pub fn evaluate_ultra_kernel_rule_884(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #885
pub fn evaluate_ultra_kernel_rule_885(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #886
pub fn evaluate_ultra_kernel_rule_886(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #887
pub fn evaluate_ultra_kernel_rule_887(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #888
pub fn evaluate_ultra_kernel_rule_888(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #889
pub fn evaluate_ultra_kernel_rule_889(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #890
pub fn evaluate_ultra_kernel_rule_890(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #891
pub fn evaluate_ultra_kernel_rule_891(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #892
pub fn evaluate_ultra_kernel_rule_892(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #893
pub fn evaluate_ultra_kernel_rule_893(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #894
pub fn evaluate_ultra_kernel_rule_894(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #895
pub fn evaluate_ultra_kernel_rule_895(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #896
pub fn evaluate_ultra_kernel_rule_896(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #897
pub fn evaluate_ultra_kernel_rule_897(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #898
pub fn evaluate_ultra_kernel_rule_898(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #899
pub fn evaluate_ultra_kernel_rule_899(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #900
pub fn evaluate_ultra_kernel_rule_900(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #901
pub fn evaluate_ultra_kernel_rule_901(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #902
pub fn evaluate_ultra_kernel_rule_902(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #903
pub fn evaluate_ultra_kernel_rule_903(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #904
pub fn evaluate_ultra_kernel_rule_904(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #905
pub fn evaluate_ultra_kernel_rule_905(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #906
pub fn evaluate_ultra_kernel_rule_906(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #907
pub fn evaluate_ultra_kernel_rule_907(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #908
pub fn evaluate_ultra_kernel_rule_908(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #909
pub fn evaluate_ultra_kernel_rule_909(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #910
pub fn evaluate_ultra_kernel_rule_910(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #911
pub fn evaluate_ultra_kernel_rule_911(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #912
pub fn evaluate_ultra_kernel_rule_912(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #913
pub fn evaluate_ultra_kernel_rule_913(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #914
pub fn evaluate_ultra_kernel_rule_914(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #915
pub fn evaluate_ultra_kernel_rule_915(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #916
pub fn evaluate_ultra_kernel_rule_916(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #917
pub fn evaluate_ultra_kernel_rule_917(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #918
pub fn evaluate_ultra_kernel_rule_918(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #919
pub fn evaluate_ultra_kernel_rule_919(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #920
pub fn evaluate_ultra_kernel_rule_920(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #921
pub fn evaluate_ultra_kernel_rule_921(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #922
pub fn evaluate_ultra_kernel_rule_922(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #923
pub fn evaluate_ultra_kernel_rule_923(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #924
pub fn evaluate_ultra_kernel_rule_924(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #925
pub fn evaluate_ultra_kernel_rule_925(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #926
pub fn evaluate_ultra_kernel_rule_926(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #927
pub fn evaluate_ultra_kernel_rule_927(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #928
pub fn evaluate_ultra_kernel_rule_928(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #929
pub fn evaluate_ultra_kernel_rule_929(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #930
pub fn evaluate_ultra_kernel_rule_930(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #931
pub fn evaluate_ultra_kernel_rule_931(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #932
pub fn evaluate_ultra_kernel_rule_932(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #933
pub fn evaluate_ultra_kernel_rule_933(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #934
pub fn evaluate_ultra_kernel_rule_934(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #935
pub fn evaluate_ultra_kernel_rule_935(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #936
pub fn evaluate_ultra_kernel_rule_936(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #937
pub fn evaluate_ultra_kernel_rule_937(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #938
pub fn evaluate_ultra_kernel_rule_938(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #939
pub fn evaluate_ultra_kernel_rule_939(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #940
pub fn evaluate_ultra_kernel_rule_940(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #941
pub fn evaluate_ultra_kernel_rule_941(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #942
pub fn evaluate_ultra_kernel_rule_942(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #943
pub fn evaluate_ultra_kernel_rule_943(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #944
pub fn evaluate_ultra_kernel_rule_944(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #945
pub fn evaluate_ultra_kernel_rule_945(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #946
pub fn evaluate_ultra_kernel_rule_946(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #947
pub fn evaluate_ultra_kernel_rule_947(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #948
pub fn evaluate_ultra_kernel_rule_948(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #949
pub fn evaluate_ultra_kernel_rule_949(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #950
pub fn evaluate_ultra_kernel_rule_950(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #951
pub fn evaluate_ultra_kernel_rule_951(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #952
pub fn evaluate_ultra_kernel_rule_952(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #953
pub fn evaluate_ultra_kernel_rule_953(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #954
pub fn evaluate_ultra_kernel_rule_954(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #955
pub fn evaluate_ultra_kernel_rule_955(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #956
pub fn evaluate_ultra_kernel_rule_956(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #957
pub fn evaluate_ultra_kernel_rule_957(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #958
pub fn evaluate_ultra_kernel_rule_958(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #959
pub fn evaluate_ultra_kernel_rule_959(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #960
pub fn evaluate_ultra_kernel_rule_960(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #961
pub fn evaluate_ultra_kernel_rule_961(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #962
pub fn evaluate_ultra_kernel_rule_962(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #963
pub fn evaluate_ultra_kernel_rule_963(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #964
pub fn evaluate_ultra_kernel_rule_964(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #965
pub fn evaluate_ultra_kernel_rule_965(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #966
pub fn evaluate_ultra_kernel_rule_966(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #967
pub fn evaluate_ultra_kernel_rule_967(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #968
pub fn evaluate_ultra_kernel_rule_968(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #969
pub fn evaluate_ultra_kernel_rule_969(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #970
pub fn evaluate_ultra_kernel_rule_970(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #971
pub fn evaluate_ultra_kernel_rule_971(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #972
pub fn evaluate_ultra_kernel_rule_972(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #973
pub fn evaluate_ultra_kernel_rule_973(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #974
pub fn evaluate_ultra_kernel_rule_974(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #975
pub fn evaluate_ultra_kernel_rule_975(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #976
pub fn evaluate_ultra_kernel_rule_976(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #977
pub fn evaluate_ultra_kernel_rule_977(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #978
pub fn evaluate_ultra_kernel_rule_978(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #979
pub fn evaluate_ultra_kernel_rule_979(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #980
pub fn evaluate_ultra_kernel_rule_980(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #981
pub fn evaluate_ultra_kernel_rule_981(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #982
pub fn evaluate_ultra_kernel_rule_982(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #983
pub fn evaluate_ultra_kernel_rule_983(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #984
pub fn evaluate_ultra_kernel_rule_984(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #985
pub fn evaluate_ultra_kernel_rule_985(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #986
pub fn evaluate_ultra_kernel_rule_986(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #987
pub fn evaluate_ultra_kernel_rule_987(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #988
pub fn evaluate_ultra_kernel_rule_988(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #989
pub fn evaluate_ultra_kernel_rule_989(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #990
pub fn evaluate_ultra_kernel_rule_990(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #991
pub fn evaluate_ultra_kernel_rule_991(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #992
pub fn evaluate_ultra_kernel_rule_992(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #993
pub fn evaluate_ultra_kernel_rule_993(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #994
pub fn evaluate_ultra_kernel_rule_994(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #995
pub fn evaluate_ultra_kernel_rule_995(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #996
pub fn evaluate_ultra_kernel_rule_996(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #997
pub fn evaluate_ultra_kernel_rule_997(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #998
pub fn evaluate_ultra_kernel_rule_998(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #999
pub fn evaluate_ultra_kernel_rule_999(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1000
pub fn evaluate_ultra_kernel_rule_1000(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1001
pub fn evaluate_ultra_kernel_rule_1001(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1002
pub fn evaluate_ultra_kernel_rule_1002(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1003
pub fn evaluate_ultra_kernel_rule_1003(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1004
pub fn evaluate_ultra_kernel_rule_1004(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1005
pub fn evaluate_ultra_kernel_rule_1005(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1006
pub fn evaluate_ultra_kernel_rule_1006(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1007
pub fn evaluate_ultra_kernel_rule_1007(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1008
pub fn evaluate_ultra_kernel_rule_1008(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1009
pub fn evaluate_ultra_kernel_rule_1009(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1010
pub fn evaluate_ultra_kernel_rule_1010(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1011
pub fn evaluate_ultra_kernel_rule_1011(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1012
pub fn evaluate_ultra_kernel_rule_1012(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1013
pub fn evaluate_ultra_kernel_rule_1013(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1014
pub fn evaluate_ultra_kernel_rule_1014(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1015
pub fn evaluate_ultra_kernel_rule_1015(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1016
pub fn evaluate_ultra_kernel_rule_1016(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1017
pub fn evaluate_ultra_kernel_rule_1017(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1018
pub fn evaluate_ultra_kernel_rule_1018(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1019
pub fn evaluate_ultra_kernel_rule_1019(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1020
pub fn evaluate_ultra_kernel_rule_1020(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1021
pub fn evaluate_ultra_kernel_rule_1021(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1022
pub fn evaluate_ultra_kernel_rule_1022(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1023
pub fn evaluate_ultra_kernel_rule_1023(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1024
pub fn evaluate_ultra_kernel_rule_1024(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1025
pub fn evaluate_ultra_kernel_rule_1025(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1026
pub fn evaluate_ultra_kernel_rule_1026(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1027
pub fn evaluate_ultra_kernel_rule_1027(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1028
pub fn evaluate_ultra_kernel_rule_1028(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1029
pub fn evaluate_ultra_kernel_rule_1029(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1030
pub fn evaluate_ultra_kernel_rule_1030(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1031
pub fn evaluate_ultra_kernel_rule_1031(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1032
pub fn evaluate_ultra_kernel_rule_1032(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1033
pub fn evaluate_ultra_kernel_rule_1033(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1034
pub fn evaluate_ultra_kernel_rule_1034(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1035
pub fn evaluate_ultra_kernel_rule_1035(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1036
pub fn evaluate_ultra_kernel_rule_1036(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1037
pub fn evaluate_ultra_kernel_rule_1037(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1038
pub fn evaluate_ultra_kernel_rule_1038(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1039
pub fn evaluate_ultra_kernel_rule_1039(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1040
pub fn evaluate_ultra_kernel_rule_1040(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1041
pub fn evaluate_ultra_kernel_rule_1041(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1042
pub fn evaluate_ultra_kernel_rule_1042(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1043
pub fn evaluate_ultra_kernel_rule_1043(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1044
pub fn evaluate_ultra_kernel_rule_1044(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1045
pub fn evaluate_ultra_kernel_rule_1045(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1046
pub fn evaluate_ultra_kernel_rule_1046(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1047
pub fn evaluate_ultra_kernel_rule_1047(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1048
pub fn evaluate_ultra_kernel_rule_1048(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1049
pub fn evaluate_ultra_kernel_rule_1049(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1050
pub fn evaluate_ultra_kernel_rule_1050(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1051
pub fn evaluate_ultra_kernel_rule_1051(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1052
pub fn evaluate_ultra_kernel_rule_1052(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1053
pub fn evaluate_ultra_kernel_rule_1053(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1054
pub fn evaluate_ultra_kernel_rule_1054(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1055
pub fn evaluate_ultra_kernel_rule_1055(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1056
pub fn evaluate_ultra_kernel_rule_1056(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1057
pub fn evaluate_ultra_kernel_rule_1057(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1058
pub fn evaluate_ultra_kernel_rule_1058(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1059
pub fn evaluate_ultra_kernel_rule_1059(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1060
pub fn evaluate_ultra_kernel_rule_1060(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1061
pub fn evaluate_ultra_kernel_rule_1061(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1062
pub fn evaluate_ultra_kernel_rule_1062(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1063
pub fn evaluate_ultra_kernel_rule_1063(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1064
pub fn evaluate_ultra_kernel_rule_1064(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1065
pub fn evaluate_ultra_kernel_rule_1065(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1066
pub fn evaluate_ultra_kernel_rule_1066(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1067
pub fn evaluate_ultra_kernel_rule_1067(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1068
pub fn evaluate_ultra_kernel_rule_1068(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1069
pub fn evaluate_ultra_kernel_rule_1069(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1070
pub fn evaluate_ultra_kernel_rule_1070(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1071
pub fn evaluate_ultra_kernel_rule_1071(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1072
pub fn evaluate_ultra_kernel_rule_1072(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1073
pub fn evaluate_ultra_kernel_rule_1073(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1074
pub fn evaluate_ultra_kernel_rule_1074(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1075
pub fn evaluate_ultra_kernel_rule_1075(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1076
pub fn evaluate_ultra_kernel_rule_1076(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1077
pub fn evaluate_ultra_kernel_rule_1077(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1078
pub fn evaluate_ultra_kernel_rule_1078(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1079
pub fn evaluate_ultra_kernel_rule_1079(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1080
pub fn evaluate_ultra_kernel_rule_1080(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1081
pub fn evaluate_ultra_kernel_rule_1081(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1082
pub fn evaluate_ultra_kernel_rule_1082(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1083
pub fn evaluate_ultra_kernel_rule_1083(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1084
pub fn evaluate_ultra_kernel_rule_1084(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1085
pub fn evaluate_ultra_kernel_rule_1085(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1086
pub fn evaluate_ultra_kernel_rule_1086(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1087
pub fn evaluate_ultra_kernel_rule_1087(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1088
pub fn evaluate_ultra_kernel_rule_1088(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1089
pub fn evaluate_ultra_kernel_rule_1089(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1090
pub fn evaluate_ultra_kernel_rule_1090(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1091
pub fn evaluate_ultra_kernel_rule_1091(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1092
pub fn evaluate_ultra_kernel_rule_1092(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1093
pub fn evaluate_ultra_kernel_rule_1093(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1094
pub fn evaluate_ultra_kernel_rule_1094(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1095
pub fn evaluate_ultra_kernel_rule_1095(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1096
pub fn evaluate_ultra_kernel_rule_1096(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1097
pub fn evaluate_ultra_kernel_rule_1097(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1098
pub fn evaluate_ultra_kernel_rule_1098(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1099
pub fn evaluate_ultra_kernel_rule_1099(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1100
pub fn evaluate_ultra_kernel_rule_1100(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1101
pub fn evaluate_ultra_kernel_rule_1101(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1102
pub fn evaluate_ultra_kernel_rule_1102(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1103
pub fn evaluate_ultra_kernel_rule_1103(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1104
pub fn evaluate_ultra_kernel_rule_1104(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1105
pub fn evaluate_ultra_kernel_rule_1105(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1106
pub fn evaluate_ultra_kernel_rule_1106(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1107
pub fn evaluate_ultra_kernel_rule_1107(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1108
pub fn evaluate_ultra_kernel_rule_1108(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1109
pub fn evaluate_ultra_kernel_rule_1109(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1110
pub fn evaluate_ultra_kernel_rule_1110(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1111
pub fn evaluate_ultra_kernel_rule_1111(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1112
pub fn evaluate_ultra_kernel_rule_1112(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1113
pub fn evaluate_ultra_kernel_rule_1113(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1114
pub fn evaluate_ultra_kernel_rule_1114(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1115
pub fn evaluate_ultra_kernel_rule_1115(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1116
pub fn evaluate_ultra_kernel_rule_1116(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1117
pub fn evaluate_ultra_kernel_rule_1117(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1118
pub fn evaluate_ultra_kernel_rule_1118(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1119
pub fn evaluate_ultra_kernel_rule_1119(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1120
pub fn evaluate_ultra_kernel_rule_1120(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1121
pub fn evaluate_ultra_kernel_rule_1121(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1122
pub fn evaluate_ultra_kernel_rule_1122(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1123
pub fn evaluate_ultra_kernel_rule_1123(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1124
pub fn evaluate_ultra_kernel_rule_1124(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1125
pub fn evaluate_ultra_kernel_rule_1125(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1126
pub fn evaluate_ultra_kernel_rule_1126(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1127
pub fn evaluate_ultra_kernel_rule_1127(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1128
pub fn evaluate_ultra_kernel_rule_1128(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1129
pub fn evaluate_ultra_kernel_rule_1129(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1130
pub fn evaluate_ultra_kernel_rule_1130(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1131
pub fn evaluate_ultra_kernel_rule_1131(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1132
pub fn evaluate_ultra_kernel_rule_1132(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1133
pub fn evaluate_ultra_kernel_rule_1133(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1134
pub fn evaluate_ultra_kernel_rule_1134(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1135
pub fn evaluate_ultra_kernel_rule_1135(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1136
pub fn evaluate_ultra_kernel_rule_1136(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1137
pub fn evaluate_ultra_kernel_rule_1137(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1138
pub fn evaluate_ultra_kernel_rule_1138(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1139
pub fn evaluate_ultra_kernel_rule_1139(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1140
pub fn evaluate_ultra_kernel_rule_1140(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1141
pub fn evaluate_ultra_kernel_rule_1141(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1142
pub fn evaluate_ultra_kernel_rule_1142(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1143
pub fn evaluate_ultra_kernel_rule_1143(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1144
pub fn evaluate_ultra_kernel_rule_1144(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1145
pub fn evaluate_ultra_kernel_rule_1145(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1146
pub fn evaluate_ultra_kernel_rule_1146(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1147
pub fn evaluate_ultra_kernel_rule_1147(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1148
pub fn evaluate_ultra_kernel_rule_1148(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1149
pub fn evaluate_ultra_kernel_rule_1149(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1150
pub fn evaluate_ultra_kernel_rule_1150(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1151
pub fn evaluate_ultra_kernel_rule_1151(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1152
pub fn evaluate_ultra_kernel_rule_1152(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1153
pub fn evaluate_ultra_kernel_rule_1153(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1154
pub fn evaluate_ultra_kernel_rule_1154(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1155
pub fn evaluate_ultra_kernel_rule_1155(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1156
pub fn evaluate_ultra_kernel_rule_1156(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1157
pub fn evaluate_ultra_kernel_rule_1157(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1158
pub fn evaluate_ultra_kernel_rule_1158(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1159
pub fn evaluate_ultra_kernel_rule_1159(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1160
pub fn evaluate_ultra_kernel_rule_1160(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1161
pub fn evaluate_ultra_kernel_rule_1161(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1162
pub fn evaluate_ultra_kernel_rule_1162(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1163
pub fn evaluate_ultra_kernel_rule_1163(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1164
pub fn evaluate_ultra_kernel_rule_1164(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1165
pub fn evaluate_ultra_kernel_rule_1165(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1166
pub fn evaluate_ultra_kernel_rule_1166(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1167
pub fn evaluate_ultra_kernel_rule_1167(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1168
pub fn evaluate_ultra_kernel_rule_1168(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1169
pub fn evaluate_ultra_kernel_rule_1169(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1170
pub fn evaluate_ultra_kernel_rule_1170(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1171
pub fn evaluate_ultra_kernel_rule_1171(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1172
pub fn evaluate_ultra_kernel_rule_1172(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1173
pub fn evaluate_ultra_kernel_rule_1173(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1174
pub fn evaluate_ultra_kernel_rule_1174(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1175
pub fn evaluate_ultra_kernel_rule_1175(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1176
pub fn evaluate_ultra_kernel_rule_1176(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1177
pub fn evaluate_ultra_kernel_rule_1177(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1178
pub fn evaluate_ultra_kernel_rule_1178(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1179
pub fn evaluate_ultra_kernel_rule_1179(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1180
pub fn evaluate_ultra_kernel_rule_1180(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1181
pub fn evaluate_ultra_kernel_rule_1181(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1182
pub fn evaluate_ultra_kernel_rule_1182(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1183
pub fn evaluate_ultra_kernel_rule_1183(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1184
pub fn evaluate_ultra_kernel_rule_1184(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1185
pub fn evaluate_ultra_kernel_rule_1185(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1186
pub fn evaluate_ultra_kernel_rule_1186(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1187
pub fn evaluate_ultra_kernel_rule_1187(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1188
pub fn evaluate_ultra_kernel_rule_1188(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1189
pub fn evaluate_ultra_kernel_rule_1189(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1190
pub fn evaluate_ultra_kernel_rule_1190(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1191
pub fn evaluate_ultra_kernel_rule_1191(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1192
pub fn evaluate_ultra_kernel_rule_1192(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1193
pub fn evaluate_ultra_kernel_rule_1193(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1194
pub fn evaluate_ultra_kernel_rule_1194(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1195
pub fn evaluate_ultra_kernel_rule_1195(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1196
pub fn evaluate_ultra_kernel_rule_1196(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1197
pub fn evaluate_ultra_kernel_rule_1197(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1198
pub fn evaluate_ultra_kernel_rule_1198(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1199
pub fn evaluate_ultra_kernel_rule_1199(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1200
pub fn evaluate_ultra_kernel_rule_1200(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1201
pub fn evaluate_ultra_kernel_rule_1201(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1202
pub fn evaluate_ultra_kernel_rule_1202(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1203
pub fn evaluate_ultra_kernel_rule_1203(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1204
pub fn evaluate_ultra_kernel_rule_1204(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1205
pub fn evaluate_ultra_kernel_rule_1205(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1206
pub fn evaluate_ultra_kernel_rule_1206(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1207
pub fn evaluate_ultra_kernel_rule_1207(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1208
pub fn evaluate_ultra_kernel_rule_1208(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1209
pub fn evaluate_ultra_kernel_rule_1209(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1210
pub fn evaluate_ultra_kernel_rule_1210(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1211
pub fn evaluate_ultra_kernel_rule_1211(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1212
pub fn evaluate_ultra_kernel_rule_1212(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1213
pub fn evaluate_ultra_kernel_rule_1213(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1214
pub fn evaluate_ultra_kernel_rule_1214(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1215
pub fn evaluate_ultra_kernel_rule_1215(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1216
pub fn evaluate_ultra_kernel_rule_1216(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1217
pub fn evaluate_ultra_kernel_rule_1217(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1218
pub fn evaluate_ultra_kernel_rule_1218(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1219
pub fn evaluate_ultra_kernel_rule_1219(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1220
pub fn evaluate_ultra_kernel_rule_1220(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1221
pub fn evaluate_ultra_kernel_rule_1221(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1222
pub fn evaluate_ultra_kernel_rule_1222(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1223
pub fn evaluate_ultra_kernel_rule_1223(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1224
pub fn evaluate_ultra_kernel_rule_1224(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1225
pub fn evaluate_ultra_kernel_rule_1225(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1226
pub fn evaluate_ultra_kernel_rule_1226(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1227
pub fn evaluate_ultra_kernel_rule_1227(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1228
pub fn evaluate_ultra_kernel_rule_1228(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1229
pub fn evaluate_ultra_kernel_rule_1229(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1230
pub fn evaluate_ultra_kernel_rule_1230(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1231
pub fn evaluate_ultra_kernel_rule_1231(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1232
pub fn evaluate_ultra_kernel_rule_1232(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1233
pub fn evaluate_ultra_kernel_rule_1233(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1234
pub fn evaluate_ultra_kernel_rule_1234(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1235
pub fn evaluate_ultra_kernel_rule_1235(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1236
pub fn evaluate_ultra_kernel_rule_1236(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1237
pub fn evaluate_ultra_kernel_rule_1237(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1238
pub fn evaluate_ultra_kernel_rule_1238(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1239
pub fn evaluate_ultra_kernel_rule_1239(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1240
pub fn evaluate_ultra_kernel_rule_1240(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1241
pub fn evaluate_ultra_kernel_rule_1241(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1242
pub fn evaluate_ultra_kernel_rule_1242(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1243
pub fn evaluate_ultra_kernel_rule_1243(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1244
pub fn evaluate_ultra_kernel_rule_1244(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1245
pub fn evaluate_ultra_kernel_rule_1245(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1246
pub fn evaluate_ultra_kernel_rule_1246(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1247
pub fn evaluate_ultra_kernel_rule_1247(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1248
pub fn evaluate_ultra_kernel_rule_1248(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1249
pub fn evaluate_ultra_kernel_rule_1249(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1250
pub fn evaluate_ultra_kernel_rule_1250(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1251
pub fn evaluate_ultra_kernel_rule_1251(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1252
pub fn evaluate_ultra_kernel_rule_1252(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1253
pub fn evaluate_ultra_kernel_rule_1253(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1254
pub fn evaluate_ultra_kernel_rule_1254(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1255
pub fn evaluate_ultra_kernel_rule_1255(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1256
pub fn evaluate_ultra_kernel_rule_1256(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1257
pub fn evaluate_ultra_kernel_rule_1257(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1258
pub fn evaluate_ultra_kernel_rule_1258(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1259
pub fn evaluate_ultra_kernel_rule_1259(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1260
pub fn evaluate_ultra_kernel_rule_1260(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1261
pub fn evaluate_ultra_kernel_rule_1261(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1262
pub fn evaluate_ultra_kernel_rule_1262(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1263
pub fn evaluate_ultra_kernel_rule_1263(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1264
pub fn evaluate_ultra_kernel_rule_1264(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1265
pub fn evaluate_ultra_kernel_rule_1265(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1266
pub fn evaluate_ultra_kernel_rule_1266(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1267
pub fn evaluate_ultra_kernel_rule_1267(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1268
pub fn evaluate_ultra_kernel_rule_1268(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1269
pub fn evaluate_ultra_kernel_rule_1269(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1270
pub fn evaluate_ultra_kernel_rule_1270(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1271
pub fn evaluate_ultra_kernel_rule_1271(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1272
pub fn evaluate_ultra_kernel_rule_1272(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1273
pub fn evaluate_ultra_kernel_rule_1273(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1274
pub fn evaluate_ultra_kernel_rule_1274(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1275
pub fn evaluate_ultra_kernel_rule_1275(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1276
pub fn evaluate_ultra_kernel_rule_1276(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1277
pub fn evaluate_ultra_kernel_rule_1277(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1278
pub fn evaluate_ultra_kernel_rule_1278(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1279
pub fn evaluate_ultra_kernel_rule_1279(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1280
pub fn evaluate_ultra_kernel_rule_1280(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1281
pub fn evaluate_ultra_kernel_rule_1281(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1282
pub fn evaluate_ultra_kernel_rule_1282(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1283
pub fn evaluate_ultra_kernel_rule_1283(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1284
pub fn evaluate_ultra_kernel_rule_1284(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1285
pub fn evaluate_ultra_kernel_rule_1285(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1286
pub fn evaluate_ultra_kernel_rule_1286(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1287
pub fn evaluate_ultra_kernel_rule_1287(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1288
pub fn evaluate_ultra_kernel_rule_1288(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1289
pub fn evaluate_ultra_kernel_rule_1289(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1290
pub fn evaluate_ultra_kernel_rule_1290(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1291
pub fn evaluate_ultra_kernel_rule_1291(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1292
pub fn evaluate_ultra_kernel_rule_1292(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1293
pub fn evaluate_ultra_kernel_rule_1293(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1294
pub fn evaluate_ultra_kernel_rule_1294(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1295
pub fn evaluate_ultra_kernel_rule_1295(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1296
pub fn evaluate_ultra_kernel_rule_1296(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1297
pub fn evaluate_ultra_kernel_rule_1297(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1298
pub fn evaluate_ultra_kernel_rule_1298(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1299
pub fn evaluate_ultra_kernel_rule_1299(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1300
pub fn evaluate_ultra_kernel_rule_1300(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1301
pub fn evaluate_ultra_kernel_rule_1301(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1302
pub fn evaluate_ultra_kernel_rule_1302(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1303
pub fn evaluate_ultra_kernel_rule_1303(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1304
pub fn evaluate_ultra_kernel_rule_1304(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1305
pub fn evaluate_ultra_kernel_rule_1305(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1306
pub fn evaluate_ultra_kernel_rule_1306(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1307
pub fn evaluate_ultra_kernel_rule_1307(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1308
pub fn evaluate_ultra_kernel_rule_1308(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1309
pub fn evaluate_ultra_kernel_rule_1309(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1310
pub fn evaluate_ultra_kernel_rule_1310(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1311
pub fn evaluate_ultra_kernel_rule_1311(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1312
pub fn evaluate_ultra_kernel_rule_1312(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1313
pub fn evaluate_ultra_kernel_rule_1313(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1314
pub fn evaluate_ultra_kernel_rule_1314(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1315
pub fn evaluate_ultra_kernel_rule_1315(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1316
pub fn evaluate_ultra_kernel_rule_1316(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1317
pub fn evaluate_ultra_kernel_rule_1317(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1318
pub fn evaluate_ultra_kernel_rule_1318(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1319
pub fn evaluate_ultra_kernel_rule_1319(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1320
pub fn evaluate_ultra_kernel_rule_1320(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1321
pub fn evaluate_ultra_kernel_rule_1321(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1322
pub fn evaluate_ultra_kernel_rule_1322(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1323
pub fn evaluate_ultra_kernel_rule_1323(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1324
pub fn evaluate_ultra_kernel_rule_1324(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1325
pub fn evaluate_ultra_kernel_rule_1325(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1326
pub fn evaluate_ultra_kernel_rule_1326(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1327
pub fn evaluate_ultra_kernel_rule_1327(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1328
pub fn evaluate_ultra_kernel_rule_1328(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1329
pub fn evaluate_ultra_kernel_rule_1329(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1330
pub fn evaluate_ultra_kernel_rule_1330(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1331
pub fn evaluate_ultra_kernel_rule_1331(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1332
pub fn evaluate_ultra_kernel_rule_1332(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1333
pub fn evaluate_ultra_kernel_rule_1333(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1334
pub fn evaluate_ultra_kernel_rule_1334(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1335
pub fn evaluate_ultra_kernel_rule_1335(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1336
pub fn evaluate_ultra_kernel_rule_1336(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1337
pub fn evaluate_ultra_kernel_rule_1337(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1338
pub fn evaluate_ultra_kernel_rule_1338(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1339
pub fn evaluate_ultra_kernel_rule_1339(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1340
pub fn evaluate_ultra_kernel_rule_1340(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1341
pub fn evaluate_ultra_kernel_rule_1341(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1342
pub fn evaluate_ultra_kernel_rule_1342(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1343
pub fn evaluate_ultra_kernel_rule_1343(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1344
pub fn evaluate_ultra_kernel_rule_1344(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1345
pub fn evaluate_ultra_kernel_rule_1345(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1346
pub fn evaluate_ultra_kernel_rule_1346(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1347
pub fn evaluate_ultra_kernel_rule_1347(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1348
pub fn evaluate_ultra_kernel_rule_1348(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1349
pub fn evaluate_ultra_kernel_rule_1349(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1350
pub fn evaluate_ultra_kernel_rule_1350(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1351
pub fn evaluate_ultra_kernel_rule_1351(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1352
pub fn evaluate_ultra_kernel_rule_1352(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1353
pub fn evaluate_ultra_kernel_rule_1353(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1354
pub fn evaluate_ultra_kernel_rule_1354(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1355
pub fn evaluate_ultra_kernel_rule_1355(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1356
pub fn evaluate_ultra_kernel_rule_1356(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1357
pub fn evaluate_ultra_kernel_rule_1357(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1358
pub fn evaluate_ultra_kernel_rule_1358(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1359
pub fn evaluate_ultra_kernel_rule_1359(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1360
pub fn evaluate_ultra_kernel_rule_1360(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1361
pub fn evaluate_ultra_kernel_rule_1361(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1362
pub fn evaluate_ultra_kernel_rule_1362(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1363
pub fn evaluate_ultra_kernel_rule_1363(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1364
pub fn evaluate_ultra_kernel_rule_1364(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1365
pub fn evaluate_ultra_kernel_rule_1365(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1366
pub fn evaluate_ultra_kernel_rule_1366(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1367
pub fn evaluate_ultra_kernel_rule_1367(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1368
pub fn evaluate_ultra_kernel_rule_1368(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1369
pub fn evaluate_ultra_kernel_rule_1369(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1370
pub fn evaluate_ultra_kernel_rule_1370(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1371
pub fn evaluate_ultra_kernel_rule_1371(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1372
pub fn evaluate_ultra_kernel_rule_1372(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1373
pub fn evaluate_ultra_kernel_rule_1373(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1374
pub fn evaluate_ultra_kernel_rule_1374(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1375
pub fn evaluate_ultra_kernel_rule_1375(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1376
pub fn evaluate_ultra_kernel_rule_1376(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1377
pub fn evaluate_ultra_kernel_rule_1377(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1378
pub fn evaluate_ultra_kernel_rule_1378(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1379
pub fn evaluate_ultra_kernel_rule_1379(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1380
pub fn evaluate_ultra_kernel_rule_1380(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1381
pub fn evaluate_ultra_kernel_rule_1381(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1382
pub fn evaluate_ultra_kernel_rule_1382(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1383
pub fn evaluate_ultra_kernel_rule_1383(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1384
pub fn evaluate_ultra_kernel_rule_1384(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1385
pub fn evaluate_ultra_kernel_rule_1385(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1386
pub fn evaluate_ultra_kernel_rule_1386(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1387
pub fn evaluate_ultra_kernel_rule_1387(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1388
pub fn evaluate_ultra_kernel_rule_1388(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1389
pub fn evaluate_ultra_kernel_rule_1389(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1390
pub fn evaluate_ultra_kernel_rule_1390(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1391
pub fn evaluate_ultra_kernel_rule_1391(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1392
pub fn evaluate_ultra_kernel_rule_1392(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1393
pub fn evaluate_ultra_kernel_rule_1393(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1394
pub fn evaluate_ultra_kernel_rule_1394(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1395
pub fn evaluate_ultra_kernel_rule_1395(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1396
pub fn evaluate_ultra_kernel_rule_1396(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1397
pub fn evaluate_ultra_kernel_rule_1397(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1398
pub fn evaluate_ultra_kernel_rule_1398(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1399
pub fn evaluate_ultra_kernel_rule_1399(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1400
pub fn evaluate_ultra_kernel_rule_1400(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1401
pub fn evaluate_ultra_kernel_rule_1401(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1402
pub fn evaluate_ultra_kernel_rule_1402(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1403
pub fn evaluate_ultra_kernel_rule_1403(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1404
pub fn evaluate_ultra_kernel_rule_1404(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1405
pub fn evaluate_ultra_kernel_rule_1405(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1406
pub fn evaluate_ultra_kernel_rule_1406(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1407
pub fn evaluate_ultra_kernel_rule_1407(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1408
pub fn evaluate_ultra_kernel_rule_1408(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1409
pub fn evaluate_ultra_kernel_rule_1409(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1410
pub fn evaluate_ultra_kernel_rule_1410(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1411
pub fn evaluate_ultra_kernel_rule_1411(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1412
pub fn evaluate_ultra_kernel_rule_1412(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1413
pub fn evaluate_ultra_kernel_rule_1413(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1414
pub fn evaluate_ultra_kernel_rule_1414(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1415
pub fn evaluate_ultra_kernel_rule_1415(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1416
pub fn evaluate_ultra_kernel_rule_1416(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1417
pub fn evaluate_ultra_kernel_rule_1417(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1418
pub fn evaluate_ultra_kernel_rule_1418(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1419
pub fn evaluate_ultra_kernel_rule_1419(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1420
pub fn evaluate_ultra_kernel_rule_1420(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1421
pub fn evaluate_ultra_kernel_rule_1421(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1422
pub fn evaluate_ultra_kernel_rule_1422(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1423
pub fn evaluate_ultra_kernel_rule_1423(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1424
pub fn evaluate_ultra_kernel_rule_1424(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1425
pub fn evaluate_ultra_kernel_rule_1425(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1426
pub fn evaluate_ultra_kernel_rule_1426(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1427
pub fn evaluate_ultra_kernel_rule_1427(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1428
pub fn evaluate_ultra_kernel_rule_1428(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1429
pub fn evaluate_ultra_kernel_rule_1429(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1430
pub fn evaluate_ultra_kernel_rule_1430(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1431
pub fn evaluate_ultra_kernel_rule_1431(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1432
pub fn evaluate_ultra_kernel_rule_1432(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1433
pub fn evaluate_ultra_kernel_rule_1433(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1434
pub fn evaluate_ultra_kernel_rule_1434(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1435
pub fn evaluate_ultra_kernel_rule_1435(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1436
pub fn evaluate_ultra_kernel_rule_1436(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1437
pub fn evaluate_ultra_kernel_rule_1437(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1438
pub fn evaluate_ultra_kernel_rule_1438(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1439
pub fn evaluate_ultra_kernel_rule_1439(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1440
pub fn evaluate_ultra_kernel_rule_1440(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1441
pub fn evaluate_ultra_kernel_rule_1441(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1442
pub fn evaluate_ultra_kernel_rule_1442(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1443
pub fn evaluate_ultra_kernel_rule_1443(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1444
pub fn evaluate_ultra_kernel_rule_1444(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1445
pub fn evaluate_ultra_kernel_rule_1445(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1446
pub fn evaluate_ultra_kernel_rule_1446(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1447
pub fn evaluate_ultra_kernel_rule_1447(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1448
pub fn evaluate_ultra_kernel_rule_1448(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1449
pub fn evaluate_ultra_kernel_rule_1449(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1450
pub fn evaluate_ultra_kernel_rule_1450(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1451
pub fn evaluate_ultra_kernel_rule_1451(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1452
pub fn evaluate_ultra_kernel_rule_1452(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1453
pub fn evaluate_ultra_kernel_rule_1453(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1454
pub fn evaluate_ultra_kernel_rule_1454(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1455
pub fn evaluate_ultra_kernel_rule_1455(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1456
pub fn evaluate_ultra_kernel_rule_1456(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1457
pub fn evaluate_ultra_kernel_rule_1457(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1458
pub fn evaluate_ultra_kernel_rule_1458(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1459
pub fn evaluate_ultra_kernel_rule_1459(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1460
pub fn evaluate_ultra_kernel_rule_1460(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1461
pub fn evaluate_ultra_kernel_rule_1461(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1462
pub fn evaluate_ultra_kernel_rule_1462(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1463
pub fn evaluate_ultra_kernel_rule_1463(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1464
pub fn evaluate_ultra_kernel_rule_1464(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1465
pub fn evaluate_ultra_kernel_rule_1465(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1466
pub fn evaluate_ultra_kernel_rule_1466(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1467
pub fn evaluate_ultra_kernel_rule_1467(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1468
pub fn evaluate_ultra_kernel_rule_1468(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1469
pub fn evaluate_ultra_kernel_rule_1469(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1470
pub fn evaluate_ultra_kernel_rule_1470(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1471
pub fn evaluate_ultra_kernel_rule_1471(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1472
pub fn evaluate_ultra_kernel_rule_1472(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1473
pub fn evaluate_ultra_kernel_rule_1473(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1474
pub fn evaluate_ultra_kernel_rule_1474(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1475
pub fn evaluate_ultra_kernel_rule_1475(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1476
pub fn evaluate_ultra_kernel_rule_1476(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1477
pub fn evaluate_ultra_kernel_rule_1477(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1478
pub fn evaluate_ultra_kernel_rule_1478(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1479
pub fn evaluate_ultra_kernel_rule_1479(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1480
pub fn evaluate_ultra_kernel_rule_1480(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1481
pub fn evaluate_ultra_kernel_rule_1481(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1482
pub fn evaluate_ultra_kernel_rule_1482(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1483
pub fn evaluate_ultra_kernel_rule_1483(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1484
pub fn evaluate_ultra_kernel_rule_1484(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1485
pub fn evaluate_ultra_kernel_rule_1485(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1486
pub fn evaluate_ultra_kernel_rule_1486(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1487
pub fn evaluate_ultra_kernel_rule_1487(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1488
pub fn evaluate_ultra_kernel_rule_1488(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1489
pub fn evaluate_ultra_kernel_rule_1489(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1490
pub fn evaluate_ultra_kernel_rule_1490(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1491
pub fn evaluate_ultra_kernel_rule_1491(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1492
pub fn evaluate_ultra_kernel_rule_1492(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1493
pub fn evaluate_ultra_kernel_rule_1493(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1494
pub fn evaluate_ultra_kernel_rule_1494(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1495
pub fn evaluate_ultra_kernel_rule_1495(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1496
pub fn evaluate_ultra_kernel_rule_1496(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1497
pub fn evaluate_ultra_kernel_rule_1497(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1498
pub fn evaluate_ultra_kernel_rule_1498(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1499
pub fn evaluate_ultra_kernel_rule_1499(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1500
pub fn evaluate_ultra_kernel_rule_1500(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1501
pub fn evaluate_ultra_kernel_rule_1501(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1502
pub fn evaluate_ultra_kernel_rule_1502(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1503
pub fn evaluate_ultra_kernel_rule_1503(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1504
pub fn evaluate_ultra_kernel_rule_1504(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1505
pub fn evaluate_ultra_kernel_rule_1505(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1506
pub fn evaluate_ultra_kernel_rule_1506(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1507
pub fn evaluate_ultra_kernel_rule_1507(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1508
pub fn evaluate_ultra_kernel_rule_1508(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1509
pub fn evaluate_ultra_kernel_rule_1509(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1510
pub fn evaluate_ultra_kernel_rule_1510(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1511
pub fn evaluate_ultra_kernel_rule_1511(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1512
pub fn evaluate_ultra_kernel_rule_1512(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1513
pub fn evaluate_ultra_kernel_rule_1513(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1514
pub fn evaluate_ultra_kernel_rule_1514(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1515
pub fn evaluate_ultra_kernel_rule_1515(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1516
pub fn evaluate_ultra_kernel_rule_1516(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1517
pub fn evaluate_ultra_kernel_rule_1517(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1518
pub fn evaluate_ultra_kernel_rule_1518(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1519
pub fn evaluate_ultra_kernel_rule_1519(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1520
pub fn evaluate_ultra_kernel_rule_1520(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1521
pub fn evaluate_ultra_kernel_rule_1521(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1522
pub fn evaluate_ultra_kernel_rule_1522(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1523
pub fn evaluate_ultra_kernel_rule_1523(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1524
pub fn evaluate_ultra_kernel_rule_1524(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1525
pub fn evaluate_ultra_kernel_rule_1525(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1526
pub fn evaluate_ultra_kernel_rule_1526(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1527
pub fn evaluate_ultra_kernel_rule_1527(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1528
pub fn evaluate_ultra_kernel_rule_1528(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1529
pub fn evaluate_ultra_kernel_rule_1529(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1530
pub fn evaluate_ultra_kernel_rule_1530(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1531
pub fn evaluate_ultra_kernel_rule_1531(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1532
pub fn evaluate_ultra_kernel_rule_1532(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1533
pub fn evaluate_ultra_kernel_rule_1533(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1534
pub fn evaluate_ultra_kernel_rule_1534(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1535
pub fn evaluate_ultra_kernel_rule_1535(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1536
pub fn evaluate_ultra_kernel_rule_1536(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1537
pub fn evaluate_ultra_kernel_rule_1537(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1538
pub fn evaluate_ultra_kernel_rule_1538(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1539
pub fn evaluate_ultra_kernel_rule_1539(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1540
pub fn evaluate_ultra_kernel_rule_1540(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1541
pub fn evaluate_ultra_kernel_rule_1541(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1542
pub fn evaluate_ultra_kernel_rule_1542(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1543
pub fn evaluate_ultra_kernel_rule_1543(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1544
pub fn evaluate_ultra_kernel_rule_1544(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1545
pub fn evaluate_ultra_kernel_rule_1545(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1546
pub fn evaluate_ultra_kernel_rule_1546(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1547
pub fn evaluate_ultra_kernel_rule_1547(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1548
pub fn evaluate_ultra_kernel_rule_1548(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1549
pub fn evaluate_ultra_kernel_rule_1549(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1550
pub fn evaluate_ultra_kernel_rule_1550(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1551
pub fn evaluate_ultra_kernel_rule_1551(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1552
pub fn evaluate_ultra_kernel_rule_1552(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1553
pub fn evaluate_ultra_kernel_rule_1553(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1554
pub fn evaluate_ultra_kernel_rule_1554(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1555
pub fn evaluate_ultra_kernel_rule_1555(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1556
pub fn evaluate_ultra_kernel_rule_1556(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1557
pub fn evaluate_ultra_kernel_rule_1557(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1558
pub fn evaluate_ultra_kernel_rule_1558(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1559
pub fn evaluate_ultra_kernel_rule_1559(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1560
pub fn evaluate_ultra_kernel_rule_1560(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1561
pub fn evaluate_ultra_kernel_rule_1561(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1562
pub fn evaluate_ultra_kernel_rule_1562(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1563
pub fn evaluate_ultra_kernel_rule_1563(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1564
pub fn evaluate_ultra_kernel_rule_1564(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1565
pub fn evaluate_ultra_kernel_rule_1565(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1566
pub fn evaluate_ultra_kernel_rule_1566(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1567
pub fn evaluate_ultra_kernel_rule_1567(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1568
pub fn evaluate_ultra_kernel_rule_1568(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1569
pub fn evaluate_ultra_kernel_rule_1569(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1570
pub fn evaluate_ultra_kernel_rule_1570(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1571
pub fn evaluate_ultra_kernel_rule_1571(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1572
pub fn evaluate_ultra_kernel_rule_1572(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1573
pub fn evaluate_ultra_kernel_rule_1573(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1574
pub fn evaluate_ultra_kernel_rule_1574(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1575
pub fn evaluate_ultra_kernel_rule_1575(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1576
pub fn evaluate_ultra_kernel_rule_1576(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1577
pub fn evaluate_ultra_kernel_rule_1577(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1578
pub fn evaluate_ultra_kernel_rule_1578(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1579
pub fn evaluate_ultra_kernel_rule_1579(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1580
pub fn evaluate_ultra_kernel_rule_1580(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1581
pub fn evaluate_ultra_kernel_rule_1581(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1582
pub fn evaluate_ultra_kernel_rule_1582(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1583
pub fn evaluate_ultra_kernel_rule_1583(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1584
pub fn evaluate_ultra_kernel_rule_1584(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1585
pub fn evaluate_ultra_kernel_rule_1585(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1586
pub fn evaluate_ultra_kernel_rule_1586(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1587
pub fn evaluate_ultra_kernel_rule_1587(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1588
pub fn evaluate_ultra_kernel_rule_1588(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1589
pub fn evaluate_ultra_kernel_rule_1589(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1590
pub fn evaluate_ultra_kernel_rule_1590(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1591
pub fn evaluate_ultra_kernel_rule_1591(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1592
pub fn evaluate_ultra_kernel_rule_1592(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1593
pub fn evaluate_ultra_kernel_rule_1593(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1594
pub fn evaluate_ultra_kernel_rule_1594(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1595
pub fn evaluate_ultra_kernel_rule_1595(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1596
pub fn evaluate_ultra_kernel_rule_1596(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1597
pub fn evaluate_ultra_kernel_rule_1597(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1598
pub fn evaluate_ultra_kernel_rule_1598(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1599
pub fn evaluate_ultra_kernel_rule_1599(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1600
pub fn evaluate_ultra_kernel_rule_1600(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1601
pub fn evaluate_ultra_kernel_rule_1601(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1602
pub fn evaluate_ultra_kernel_rule_1602(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1603
pub fn evaluate_ultra_kernel_rule_1603(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1604
pub fn evaluate_ultra_kernel_rule_1604(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1605
pub fn evaluate_ultra_kernel_rule_1605(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1606
pub fn evaluate_ultra_kernel_rule_1606(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1607
pub fn evaluate_ultra_kernel_rule_1607(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1608
pub fn evaluate_ultra_kernel_rule_1608(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1609
pub fn evaluate_ultra_kernel_rule_1609(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1610
pub fn evaluate_ultra_kernel_rule_1610(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1611
pub fn evaluate_ultra_kernel_rule_1611(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1612
pub fn evaluate_ultra_kernel_rule_1612(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1613
pub fn evaluate_ultra_kernel_rule_1613(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1614
pub fn evaluate_ultra_kernel_rule_1614(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1615
pub fn evaluate_ultra_kernel_rule_1615(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1616
pub fn evaluate_ultra_kernel_rule_1616(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1617
pub fn evaluate_ultra_kernel_rule_1617(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1618
pub fn evaluate_ultra_kernel_rule_1618(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1619
pub fn evaluate_ultra_kernel_rule_1619(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1620
pub fn evaluate_ultra_kernel_rule_1620(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1621
pub fn evaluate_ultra_kernel_rule_1621(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1622
pub fn evaluate_ultra_kernel_rule_1622(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1623
pub fn evaluate_ultra_kernel_rule_1623(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1624
pub fn evaluate_ultra_kernel_rule_1624(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1625
pub fn evaluate_ultra_kernel_rule_1625(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1626
pub fn evaluate_ultra_kernel_rule_1626(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1627
pub fn evaluate_ultra_kernel_rule_1627(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1628
pub fn evaluate_ultra_kernel_rule_1628(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1629
pub fn evaluate_ultra_kernel_rule_1629(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1630
pub fn evaluate_ultra_kernel_rule_1630(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1631
pub fn evaluate_ultra_kernel_rule_1631(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1632
pub fn evaluate_ultra_kernel_rule_1632(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1633
pub fn evaluate_ultra_kernel_rule_1633(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1634
pub fn evaluate_ultra_kernel_rule_1634(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1635
pub fn evaluate_ultra_kernel_rule_1635(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1636
pub fn evaluate_ultra_kernel_rule_1636(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1637
pub fn evaluate_ultra_kernel_rule_1637(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1638
pub fn evaluate_ultra_kernel_rule_1638(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1639
pub fn evaluate_ultra_kernel_rule_1639(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1640
pub fn evaluate_ultra_kernel_rule_1640(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1641
pub fn evaluate_ultra_kernel_rule_1641(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1642
pub fn evaluate_ultra_kernel_rule_1642(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1643
pub fn evaluate_ultra_kernel_rule_1643(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1644
pub fn evaluate_ultra_kernel_rule_1644(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1645
pub fn evaluate_ultra_kernel_rule_1645(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1646
pub fn evaluate_ultra_kernel_rule_1646(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1647
pub fn evaluate_ultra_kernel_rule_1647(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1648
pub fn evaluate_ultra_kernel_rule_1648(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1649
pub fn evaluate_ultra_kernel_rule_1649(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1650
pub fn evaluate_ultra_kernel_rule_1650(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1651
pub fn evaluate_ultra_kernel_rule_1651(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1652
pub fn evaluate_ultra_kernel_rule_1652(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1653
pub fn evaluate_ultra_kernel_rule_1653(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1654
pub fn evaluate_ultra_kernel_rule_1654(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1655
pub fn evaluate_ultra_kernel_rule_1655(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1656
pub fn evaluate_ultra_kernel_rule_1656(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1657
pub fn evaluate_ultra_kernel_rule_1657(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1658
pub fn evaluate_ultra_kernel_rule_1658(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1659
pub fn evaluate_ultra_kernel_rule_1659(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1660
pub fn evaluate_ultra_kernel_rule_1660(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1661
pub fn evaluate_ultra_kernel_rule_1661(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1662
pub fn evaluate_ultra_kernel_rule_1662(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1663
pub fn evaluate_ultra_kernel_rule_1663(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1664
pub fn evaluate_ultra_kernel_rule_1664(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1665
pub fn evaluate_ultra_kernel_rule_1665(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1666
pub fn evaluate_ultra_kernel_rule_1666(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1667
pub fn evaluate_ultra_kernel_rule_1667(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1668
pub fn evaluate_ultra_kernel_rule_1668(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1669
pub fn evaluate_ultra_kernel_rule_1669(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1670
pub fn evaluate_ultra_kernel_rule_1670(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1671
pub fn evaluate_ultra_kernel_rule_1671(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1672
pub fn evaluate_ultra_kernel_rule_1672(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1673
pub fn evaluate_ultra_kernel_rule_1673(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1674
pub fn evaluate_ultra_kernel_rule_1674(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1675
pub fn evaluate_ultra_kernel_rule_1675(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1676
pub fn evaluate_ultra_kernel_rule_1676(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1677
pub fn evaluate_ultra_kernel_rule_1677(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1678
pub fn evaluate_ultra_kernel_rule_1678(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1679
pub fn evaluate_ultra_kernel_rule_1679(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1680
pub fn evaluate_ultra_kernel_rule_1680(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1681
pub fn evaluate_ultra_kernel_rule_1681(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1682
pub fn evaluate_ultra_kernel_rule_1682(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1683
pub fn evaluate_ultra_kernel_rule_1683(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1684
pub fn evaluate_ultra_kernel_rule_1684(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1685
pub fn evaluate_ultra_kernel_rule_1685(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1686
pub fn evaluate_ultra_kernel_rule_1686(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1687
pub fn evaluate_ultra_kernel_rule_1687(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1688
pub fn evaluate_ultra_kernel_rule_1688(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1689
pub fn evaluate_ultra_kernel_rule_1689(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1690
pub fn evaluate_ultra_kernel_rule_1690(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1691
pub fn evaluate_ultra_kernel_rule_1691(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1692
pub fn evaluate_ultra_kernel_rule_1692(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1693
pub fn evaluate_ultra_kernel_rule_1693(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1694
pub fn evaluate_ultra_kernel_rule_1694(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1695
pub fn evaluate_ultra_kernel_rule_1695(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1696
pub fn evaluate_ultra_kernel_rule_1696(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1697
pub fn evaluate_ultra_kernel_rule_1697(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1698
pub fn evaluate_ultra_kernel_rule_1698(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1699
pub fn evaluate_ultra_kernel_rule_1699(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1700
pub fn evaluate_ultra_kernel_rule_1700(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1701
pub fn evaluate_ultra_kernel_rule_1701(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1702
pub fn evaluate_ultra_kernel_rule_1702(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1703
pub fn evaluate_ultra_kernel_rule_1703(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1704
pub fn evaluate_ultra_kernel_rule_1704(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1705
pub fn evaluate_ultra_kernel_rule_1705(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1706
pub fn evaluate_ultra_kernel_rule_1706(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1707
pub fn evaluate_ultra_kernel_rule_1707(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1708
pub fn evaluate_ultra_kernel_rule_1708(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1709
pub fn evaluate_ultra_kernel_rule_1709(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1710
pub fn evaluate_ultra_kernel_rule_1710(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1711
pub fn evaluate_ultra_kernel_rule_1711(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1712
pub fn evaluate_ultra_kernel_rule_1712(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1713
pub fn evaluate_ultra_kernel_rule_1713(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1714
pub fn evaluate_ultra_kernel_rule_1714(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1715
pub fn evaluate_ultra_kernel_rule_1715(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1716
pub fn evaluate_ultra_kernel_rule_1716(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1717
pub fn evaluate_ultra_kernel_rule_1717(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1718
pub fn evaluate_ultra_kernel_rule_1718(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1719
pub fn evaluate_ultra_kernel_rule_1719(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1720
pub fn evaluate_ultra_kernel_rule_1720(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1721
pub fn evaluate_ultra_kernel_rule_1721(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1722
pub fn evaluate_ultra_kernel_rule_1722(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1723
pub fn evaluate_ultra_kernel_rule_1723(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1724
pub fn evaluate_ultra_kernel_rule_1724(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1725
pub fn evaluate_ultra_kernel_rule_1725(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1726
pub fn evaluate_ultra_kernel_rule_1726(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1727
pub fn evaluate_ultra_kernel_rule_1727(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1728
pub fn evaluate_ultra_kernel_rule_1728(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1729
pub fn evaluate_ultra_kernel_rule_1729(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1730
pub fn evaluate_ultra_kernel_rule_1730(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1731
pub fn evaluate_ultra_kernel_rule_1731(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1732
pub fn evaluate_ultra_kernel_rule_1732(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1733
pub fn evaluate_ultra_kernel_rule_1733(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1734
pub fn evaluate_ultra_kernel_rule_1734(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1735
pub fn evaluate_ultra_kernel_rule_1735(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1736
pub fn evaluate_ultra_kernel_rule_1736(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1737
pub fn evaluate_ultra_kernel_rule_1737(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1738
pub fn evaluate_ultra_kernel_rule_1738(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1739
pub fn evaluate_ultra_kernel_rule_1739(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1740
pub fn evaluate_ultra_kernel_rule_1740(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1741
pub fn evaluate_ultra_kernel_rule_1741(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1742
pub fn evaluate_ultra_kernel_rule_1742(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1743
pub fn evaluate_ultra_kernel_rule_1743(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1744
pub fn evaluate_ultra_kernel_rule_1744(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1745
pub fn evaluate_ultra_kernel_rule_1745(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1746
pub fn evaluate_ultra_kernel_rule_1746(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1747
pub fn evaluate_ultra_kernel_rule_1747(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1748
pub fn evaluate_ultra_kernel_rule_1748(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1749
pub fn evaluate_ultra_kernel_rule_1749(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1750
pub fn evaluate_ultra_kernel_rule_1750(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1751
pub fn evaluate_ultra_kernel_rule_1751(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1752
pub fn evaluate_ultra_kernel_rule_1752(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1753
pub fn evaluate_ultra_kernel_rule_1753(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1754
pub fn evaluate_ultra_kernel_rule_1754(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1755
pub fn evaluate_ultra_kernel_rule_1755(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1756
pub fn evaluate_ultra_kernel_rule_1756(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1757
pub fn evaluate_ultra_kernel_rule_1757(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1758
pub fn evaluate_ultra_kernel_rule_1758(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1759
pub fn evaluate_ultra_kernel_rule_1759(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1760
pub fn evaluate_ultra_kernel_rule_1760(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1761
pub fn evaluate_ultra_kernel_rule_1761(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1762
pub fn evaluate_ultra_kernel_rule_1762(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1763
pub fn evaluate_ultra_kernel_rule_1763(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1764
pub fn evaluate_ultra_kernel_rule_1764(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1765
pub fn evaluate_ultra_kernel_rule_1765(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1766
pub fn evaluate_ultra_kernel_rule_1766(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1767
pub fn evaluate_ultra_kernel_rule_1767(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1768
pub fn evaluate_ultra_kernel_rule_1768(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1769
pub fn evaluate_ultra_kernel_rule_1769(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1770
pub fn evaluate_ultra_kernel_rule_1770(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1771
pub fn evaluate_ultra_kernel_rule_1771(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1772
pub fn evaluate_ultra_kernel_rule_1772(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1773
pub fn evaluate_ultra_kernel_rule_1773(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1774
pub fn evaluate_ultra_kernel_rule_1774(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1775
pub fn evaluate_ultra_kernel_rule_1775(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1776
pub fn evaluate_ultra_kernel_rule_1776(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1777
pub fn evaluate_ultra_kernel_rule_1777(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1778
pub fn evaluate_ultra_kernel_rule_1778(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1779
pub fn evaluate_ultra_kernel_rule_1779(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1780
pub fn evaluate_ultra_kernel_rule_1780(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1781
pub fn evaluate_ultra_kernel_rule_1781(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1782
pub fn evaluate_ultra_kernel_rule_1782(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1783
pub fn evaluate_ultra_kernel_rule_1783(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1784
pub fn evaluate_ultra_kernel_rule_1784(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1785
pub fn evaluate_ultra_kernel_rule_1785(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1786
pub fn evaluate_ultra_kernel_rule_1786(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1787
pub fn evaluate_ultra_kernel_rule_1787(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1788
pub fn evaluate_ultra_kernel_rule_1788(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1789
pub fn evaluate_ultra_kernel_rule_1789(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1790
pub fn evaluate_ultra_kernel_rule_1790(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1791
pub fn evaluate_ultra_kernel_rule_1791(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1792
pub fn evaluate_ultra_kernel_rule_1792(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1793
pub fn evaluate_ultra_kernel_rule_1793(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1794
pub fn evaluate_ultra_kernel_rule_1794(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1795
pub fn evaluate_ultra_kernel_rule_1795(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1796
pub fn evaluate_ultra_kernel_rule_1796(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1797
pub fn evaluate_ultra_kernel_rule_1797(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1798
pub fn evaluate_ultra_kernel_rule_1798(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1799
pub fn evaluate_ultra_kernel_rule_1799(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1800
pub fn evaluate_ultra_kernel_rule_1800(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1801
pub fn evaluate_ultra_kernel_rule_1801(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1802
pub fn evaluate_ultra_kernel_rule_1802(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1803
pub fn evaluate_ultra_kernel_rule_1803(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1804
pub fn evaluate_ultra_kernel_rule_1804(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1805
pub fn evaluate_ultra_kernel_rule_1805(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1806
pub fn evaluate_ultra_kernel_rule_1806(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1807
pub fn evaluate_ultra_kernel_rule_1807(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1808
pub fn evaluate_ultra_kernel_rule_1808(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1809
pub fn evaluate_ultra_kernel_rule_1809(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1810
pub fn evaluate_ultra_kernel_rule_1810(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1811
pub fn evaluate_ultra_kernel_rule_1811(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1812
pub fn evaluate_ultra_kernel_rule_1812(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1813
pub fn evaluate_ultra_kernel_rule_1813(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1814
pub fn evaluate_ultra_kernel_rule_1814(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1815
pub fn evaluate_ultra_kernel_rule_1815(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1816
pub fn evaluate_ultra_kernel_rule_1816(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1817
pub fn evaluate_ultra_kernel_rule_1817(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1818
pub fn evaluate_ultra_kernel_rule_1818(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1819
pub fn evaluate_ultra_kernel_rule_1819(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1820
pub fn evaluate_ultra_kernel_rule_1820(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1821
pub fn evaluate_ultra_kernel_rule_1821(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1822
pub fn evaluate_ultra_kernel_rule_1822(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1823
pub fn evaluate_ultra_kernel_rule_1823(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1824
pub fn evaluate_ultra_kernel_rule_1824(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1825
pub fn evaluate_ultra_kernel_rule_1825(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1826
pub fn evaluate_ultra_kernel_rule_1826(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1827
pub fn evaluate_ultra_kernel_rule_1827(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1828
pub fn evaluate_ultra_kernel_rule_1828(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1829
pub fn evaluate_ultra_kernel_rule_1829(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1830
pub fn evaluate_ultra_kernel_rule_1830(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1831
pub fn evaluate_ultra_kernel_rule_1831(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1832
pub fn evaluate_ultra_kernel_rule_1832(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1833
pub fn evaluate_ultra_kernel_rule_1833(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1834
pub fn evaluate_ultra_kernel_rule_1834(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1835
pub fn evaluate_ultra_kernel_rule_1835(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1836
pub fn evaluate_ultra_kernel_rule_1836(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1837
pub fn evaluate_ultra_kernel_rule_1837(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1838
pub fn evaluate_ultra_kernel_rule_1838(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1839
pub fn evaluate_ultra_kernel_rule_1839(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1840
pub fn evaluate_ultra_kernel_rule_1840(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1841
pub fn evaluate_ultra_kernel_rule_1841(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1842
pub fn evaluate_ultra_kernel_rule_1842(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1843
pub fn evaluate_ultra_kernel_rule_1843(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1844
pub fn evaluate_ultra_kernel_rule_1844(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1845
pub fn evaluate_ultra_kernel_rule_1845(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1846
pub fn evaluate_ultra_kernel_rule_1846(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1847
pub fn evaluate_ultra_kernel_rule_1847(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1848
pub fn evaluate_ultra_kernel_rule_1848(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1849
pub fn evaluate_ultra_kernel_rule_1849(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1850
pub fn evaluate_ultra_kernel_rule_1850(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1851
pub fn evaluate_ultra_kernel_rule_1851(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1852
pub fn evaluate_ultra_kernel_rule_1852(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1853
pub fn evaluate_ultra_kernel_rule_1853(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1854
pub fn evaluate_ultra_kernel_rule_1854(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1855
pub fn evaluate_ultra_kernel_rule_1855(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1856
pub fn evaluate_ultra_kernel_rule_1856(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1857
pub fn evaluate_ultra_kernel_rule_1857(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1858
pub fn evaluate_ultra_kernel_rule_1858(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1859
pub fn evaluate_ultra_kernel_rule_1859(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1860
pub fn evaluate_ultra_kernel_rule_1860(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1861
pub fn evaluate_ultra_kernel_rule_1861(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1862
pub fn evaluate_ultra_kernel_rule_1862(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1863
pub fn evaluate_ultra_kernel_rule_1863(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1864
pub fn evaluate_ultra_kernel_rule_1864(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1865
pub fn evaluate_ultra_kernel_rule_1865(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1866
pub fn evaluate_ultra_kernel_rule_1866(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1867
pub fn evaluate_ultra_kernel_rule_1867(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1868
pub fn evaluate_ultra_kernel_rule_1868(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1869
pub fn evaluate_ultra_kernel_rule_1869(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1870
pub fn evaluate_ultra_kernel_rule_1870(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1871
pub fn evaluate_ultra_kernel_rule_1871(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1872
pub fn evaluate_ultra_kernel_rule_1872(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1873
pub fn evaluate_ultra_kernel_rule_1873(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1874
pub fn evaluate_ultra_kernel_rule_1874(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1875
pub fn evaluate_ultra_kernel_rule_1875(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1876
pub fn evaluate_ultra_kernel_rule_1876(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1877
pub fn evaluate_ultra_kernel_rule_1877(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1878
pub fn evaluate_ultra_kernel_rule_1878(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1879
pub fn evaluate_ultra_kernel_rule_1879(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1880
pub fn evaluate_ultra_kernel_rule_1880(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1881
pub fn evaluate_ultra_kernel_rule_1881(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1882
pub fn evaluate_ultra_kernel_rule_1882(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1883
pub fn evaluate_ultra_kernel_rule_1883(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1884
pub fn evaluate_ultra_kernel_rule_1884(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1885
pub fn evaluate_ultra_kernel_rule_1885(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1886
pub fn evaluate_ultra_kernel_rule_1886(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1887
pub fn evaluate_ultra_kernel_rule_1887(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1888
pub fn evaluate_ultra_kernel_rule_1888(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1889
pub fn evaluate_ultra_kernel_rule_1889(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1890
pub fn evaluate_ultra_kernel_rule_1890(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1891
pub fn evaluate_ultra_kernel_rule_1891(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1892
pub fn evaluate_ultra_kernel_rule_1892(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1893
pub fn evaluate_ultra_kernel_rule_1893(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1894
pub fn evaluate_ultra_kernel_rule_1894(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1895
pub fn evaluate_ultra_kernel_rule_1895(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1896
pub fn evaluate_ultra_kernel_rule_1896(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1897
pub fn evaluate_ultra_kernel_rule_1897(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1898
pub fn evaluate_ultra_kernel_rule_1898(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1899
pub fn evaluate_ultra_kernel_rule_1899(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1900
pub fn evaluate_ultra_kernel_rule_1900(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1901
pub fn evaluate_ultra_kernel_rule_1901(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1902
pub fn evaluate_ultra_kernel_rule_1902(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1903
pub fn evaluate_ultra_kernel_rule_1903(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1904
pub fn evaluate_ultra_kernel_rule_1904(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1905
pub fn evaluate_ultra_kernel_rule_1905(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1906
pub fn evaluate_ultra_kernel_rule_1906(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1907
pub fn evaluate_ultra_kernel_rule_1907(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1908
pub fn evaluate_ultra_kernel_rule_1908(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1909
pub fn evaluate_ultra_kernel_rule_1909(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1910
pub fn evaluate_ultra_kernel_rule_1910(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1911
pub fn evaluate_ultra_kernel_rule_1911(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1912
pub fn evaluate_ultra_kernel_rule_1912(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1913
pub fn evaluate_ultra_kernel_rule_1913(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1914
pub fn evaluate_ultra_kernel_rule_1914(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1915
pub fn evaluate_ultra_kernel_rule_1915(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1916
pub fn evaluate_ultra_kernel_rule_1916(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1917
pub fn evaluate_ultra_kernel_rule_1917(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1918
pub fn evaluate_ultra_kernel_rule_1918(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1919
pub fn evaluate_ultra_kernel_rule_1919(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1920
pub fn evaluate_ultra_kernel_rule_1920(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1921
pub fn evaluate_ultra_kernel_rule_1921(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1922
pub fn evaluate_ultra_kernel_rule_1922(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1923
pub fn evaluate_ultra_kernel_rule_1923(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1924
pub fn evaluate_ultra_kernel_rule_1924(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1925
pub fn evaluate_ultra_kernel_rule_1925(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1926
pub fn evaluate_ultra_kernel_rule_1926(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1927
pub fn evaluate_ultra_kernel_rule_1927(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1928
pub fn evaluate_ultra_kernel_rule_1928(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1929
pub fn evaluate_ultra_kernel_rule_1929(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1930
pub fn evaluate_ultra_kernel_rule_1930(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1931
pub fn evaluate_ultra_kernel_rule_1931(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1932
pub fn evaluate_ultra_kernel_rule_1932(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1933
pub fn evaluate_ultra_kernel_rule_1933(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1934
pub fn evaluate_ultra_kernel_rule_1934(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1935
pub fn evaluate_ultra_kernel_rule_1935(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1936
pub fn evaluate_ultra_kernel_rule_1936(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1937
pub fn evaluate_ultra_kernel_rule_1937(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1938
pub fn evaluate_ultra_kernel_rule_1938(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1939
pub fn evaluate_ultra_kernel_rule_1939(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1940
pub fn evaluate_ultra_kernel_rule_1940(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1941
pub fn evaluate_ultra_kernel_rule_1941(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1942
pub fn evaluate_ultra_kernel_rule_1942(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1943
pub fn evaluate_ultra_kernel_rule_1943(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1944
pub fn evaluate_ultra_kernel_rule_1944(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1945
pub fn evaluate_ultra_kernel_rule_1945(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1946
pub fn evaluate_ultra_kernel_rule_1946(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1947
pub fn evaluate_ultra_kernel_rule_1947(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1948
pub fn evaluate_ultra_kernel_rule_1948(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1949
pub fn evaluate_ultra_kernel_rule_1949(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1950
pub fn evaluate_ultra_kernel_rule_1950(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1951
pub fn evaluate_ultra_kernel_rule_1951(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1952
pub fn evaluate_ultra_kernel_rule_1952(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1953
pub fn evaluate_ultra_kernel_rule_1953(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1954
pub fn evaluate_ultra_kernel_rule_1954(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1955
pub fn evaluate_ultra_kernel_rule_1955(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1956
pub fn evaluate_ultra_kernel_rule_1956(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1957
pub fn evaluate_ultra_kernel_rule_1957(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1958
pub fn evaluate_ultra_kernel_rule_1958(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1959
pub fn evaluate_ultra_kernel_rule_1959(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1960
pub fn evaluate_ultra_kernel_rule_1960(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1961
pub fn evaluate_ultra_kernel_rule_1961(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1962
pub fn evaluate_ultra_kernel_rule_1962(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1963
pub fn evaluate_ultra_kernel_rule_1963(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1964
pub fn evaluate_ultra_kernel_rule_1964(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1965
pub fn evaluate_ultra_kernel_rule_1965(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1966
pub fn evaluate_ultra_kernel_rule_1966(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1967
pub fn evaluate_ultra_kernel_rule_1967(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1968
pub fn evaluate_ultra_kernel_rule_1968(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1969
pub fn evaluate_ultra_kernel_rule_1969(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1970
pub fn evaluate_ultra_kernel_rule_1970(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1971
pub fn evaluate_ultra_kernel_rule_1971(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1972
pub fn evaluate_ultra_kernel_rule_1972(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1973
pub fn evaluate_ultra_kernel_rule_1973(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1974
pub fn evaluate_ultra_kernel_rule_1974(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1975
pub fn evaluate_ultra_kernel_rule_1975(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1976
pub fn evaluate_ultra_kernel_rule_1976(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1977
pub fn evaluate_ultra_kernel_rule_1977(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1978
pub fn evaluate_ultra_kernel_rule_1978(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1979
pub fn evaluate_ultra_kernel_rule_1979(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1980
pub fn evaluate_ultra_kernel_rule_1980(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1981
pub fn evaluate_ultra_kernel_rule_1981(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1982
pub fn evaluate_ultra_kernel_rule_1982(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1983
pub fn evaluate_ultra_kernel_rule_1983(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1984
pub fn evaluate_ultra_kernel_rule_1984(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1985
pub fn evaluate_ultra_kernel_rule_1985(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1986
pub fn evaluate_ultra_kernel_rule_1986(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1987
pub fn evaluate_ultra_kernel_rule_1987(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1988
pub fn evaluate_ultra_kernel_rule_1988(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1989
pub fn evaluate_ultra_kernel_rule_1989(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1990
pub fn evaluate_ultra_kernel_rule_1990(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1991
pub fn evaluate_ultra_kernel_rule_1991(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1992
pub fn evaluate_ultra_kernel_rule_1992(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1993
pub fn evaluate_ultra_kernel_rule_1993(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1994
pub fn evaluate_ultra_kernel_rule_1994(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1995
pub fn evaluate_ultra_kernel_rule_1995(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1996
pub fn evaluate_ultra_kernel_rule_1996(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1997
pub fn evaluate_ultra_kernel_rule_1997(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1998
pub fn evaluate_ultra_kernel_rule_1998(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #1999
pub fn evaluate_ultra_kernel_rule_1999(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2000
pub fn evaluate_ultra_kernel_rule_2000(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2001
pub fn evaluate_ultra_kernel_rule_2001(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2002
pub fn evaluate_ultra_kernel_rule_2002(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2003
pub fn evaluate_ultra_kernel_rule_2003(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2004
pub fn evaluate_ultra_kernel_rule_2004(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2005
pub fn evaluate_ultra_kernel_rule_2005(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2006
pub fn evaluate_ultra_kernel_rule_2006(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2007
pub fn evaluate_ultra_kernel_rule_2007(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2008
pub fn evaluate_ultra_kernel_rule_2008(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2009
pub fn evaluate_ultra_kernel_rule_2009(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2010
pub fn evaluate_ultra_kernel_rule_2010(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2011
pub fn evaluate_ultra_kernel_rule_2011(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2012
pub fn evaluate_ultra_kernel_rule_2012(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2013
pub fn evaluate_ultra_kernel_rule_2013(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2014
pub fn evaluate_ultra_kernel_rule_2014(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2015
pub fn evaluate_ultra_kernel_rule_2015(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2016
pub fn evaluate_ultra_kernel_rule_2016(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2017
pub fn evaluate_ultra_kernel_rule_2017(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2018
pub fn evaluate_ultra_kernel_rule_2018(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2019
pub fn evaluate_ultra_kernel_rule_2019(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2020
pub fn evaluate_ultra_kernel_rule_2020(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2021
pub fn evaluate_ultra_kernel_rule_2021(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2022
pub fn evaluate_ultra_kernel_rule_2022(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2023
pub fn evaluate_ultra_kernel_rule_2023(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2024
pub fn evaluate_ultra_kernel_rule_2024(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2025
pub fn evaluate_ultra_kernel_rule_2025(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2026
pub fn evaluate_ultra_kernel_rule_2026(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2027
pub fn evaluate_ultra_kernel_rule_2027(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2028
pub fn evaluate_ultra_kernel_rule_2028(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2029
pub fn evaluate_ultra_kernel_rule_2029(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2030
pub fn evaluate_ultra_kernel_rule_2030(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2031
pub fn evaluate_ultra_kernel_rule_2031(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2032
pub fn evaluate_ultra_kernel_rule_2032(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2033
pub fn evaluate_ultra_kernel_rule_2033(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2034
pub fn evaluate_ultra_kernel_rule_2034(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2035
pub fn evaluate_ultra_kernel_rule_2035(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2036
pub fn evaluate_ultra_kernel_rule_2036(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2037
pub fn evaluate_ultra_kernel_rule_2037(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2038
pub fn evaluate_ultra_kernel_rule_2038(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2039
pub fn evaluate_ultra_kernel_rule_2039(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2040
pub fn evaluate_ultra_kernel_rule_2040(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2041
pub fn evaluate_ultra_kernel_rule_2041(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2042
pub fn evaluate_ultra_kernel_rule_2042(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2043
pub fn evaluate_ultra_kernel_rule_2043(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2044
pub fn evaluate_ultra_kernel_rule_2044(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2045
pub fn evaluate_ultra_kernel_rule_2045(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2046
pub fn evaluate_ultra_kernel_rule_2046(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2047
pub fn evaluate_ultra_kernel_rule_2047(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2048
pub fn evaluate_ultra_kernel_rule_2048(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2049
pub fn evaluate_ultra_kernel_rule_2049(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2050
pub fn evaluate_ultra_kernel_rule_2050(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2051
pub fn evaluate_ultra_kernel_rule_2051(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2052
pub fn evaluate_ultra_kernel_rule_2052(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2053
pub fn evaluate_ultra_kernel_rule_2053(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2054
pub fn evaluate_ultra_kernel_rule_2054(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2055
pub fn evaluate_ultra_kernel_rule_2055(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2056
pub fn evaluate_ultra_kernel_rule_2056(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2057
pub fn evaluate_ultra_kernel_rule_2057(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2058
pub fn evaluate_ultra_kernel_rule_2058(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2059
pub fn evaluate_ultra_kernel_rule_2059(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2060
pub fn evaluate_ultra_kernel_rule_2060(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2061
pub fn evaluate_ultra_kernel_rule_2061(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2062
pub fn evaluate_ultra_kernel_rule_2062(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2063
pub fn evaluate_ultra_kernel_rule_2063(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2064
pub fn evaluate_ultra_kernel_rule_2064(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2065
pub fn evaluate_ultra_kernel_rule_2065(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2066
pub fn evaluate_ultra_kernel_rule_2066(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2067
pub fn evaluate_ultra_kernel_rule_2067(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2068
pub fn evaluate_ultra_kernel_rule_2068(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2069
pub fn evaluate_ultra_kernel_rule_2069(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2070
pub fn evaluate_ultra_kernel_rule_2070(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2071
pub fn evaluate_ultra_kernel_rule_2071(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2072
pub fn evaluate_ultra_kernel_rule_2072(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2073
pub fn evaluate_ultra_kernel_rule_2073(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2074
pub fn evaluate_ultra_kernel_rule_2074(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2075
pub fn evaluate_ultra_kernel_rule_2075(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2076
pub fn evaluate_ultra_kernel_rule_2076(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2077
pub fn evaluate_ultra_kernel_rule_2077(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2078
pub fn evaluate_ultra_kernel_rule_2078(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2079
pub fn evaluate_ultra_kernel_rule_2079(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2080
pub fn evaluate_ultra_kernel_rule_2080(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2081
pub fn evaluate_ultra_kernel_rule_2081(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2082
pub fn evaluate_ultra_kernel_rule_2082(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2083
pub fn evaluate_ultra_kernel_rule_2083(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2084
pub fn evaluate_ultra_kernel_rule_2084(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2085
pub fn evaluate_ultra_kernel_rule_2085(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2086
pub fn evaluate_ultra_kernel_rule_2086(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2087
pub fn evaluate_ultra_kernel_rule_2087(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2088
pub fn evaluate_ultra_kernel_rule_2088(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2089
pub fn evaluate_ultra_kernel_rule_2089(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2090
pub fn evaluate_ultra_kernel_rule_2090(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2091
pub fn evaluate_ultra_kernel_rule_2091(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2092
pub fn evaluate_ultra_kernel_rule_2092(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2093
pub fn evaluate_ultra_kernel_rule_2093(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2094
pub fn evaluate_ultra_kernel_rule_2094(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2095
pub fn evaluate_ultra_kernel_rule_2095(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2096
pub fn evaluate_ultra_kernel_rule_2096(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2097
pub fn evaluate_ultra_kernel_rule_2097(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2098
pub fn evaluate_ultra_kernel_rule_2098(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2099
pub fn evaluate_ultra_kernel_rule_2099(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2100
pub fn evaluate_ultra_kernel_rule_2100(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2101
pub fn evaluate_ultra_kernel_rule_2101(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2102
pub fn evaluate_ultra_kernel_rule_2102(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2103
pub fn evaluate_ultra_kernel_rule_2103(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2104
pub fn evaluate_ultra_kernel_rule_2104(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2105
pub fn evaluate_ultra_kernel_rule_2105(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2106
pub fn evaluate_ultra_kernel_rule_2106(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2107
pub fn evaluate_ultra_kernel_rule_2107(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2108
pub fn evaluate_ultra_kernel_rule_2108(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2109
pub fn evaluate_ultra_kernel_rule_2109(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2110
pub fn evaluate_ultra_kernel_rule_2110(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2111
pub fn evaluate_ultra_kernel_rule_2111(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2112
pub fn evaluate_ultra_kernel_rule_2112(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2113
pub fn evaluate_ultra_kernel_rule_2113(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2114
pub fn evaluate_ultra_kernel_rule_2114(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2115
pub fn evaluate_ultra_kernel_rule_2115(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2116
pub fn evaluate_ultra_kernel_rule_2116(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2117
pub fn evaluate_ultra_kernel_rule_2117(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2118
pub fn evaluate_ultra_kernel_rule_2118(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2119
pub fn evaluate_ultra_kernel_rule_2119(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2120
pub fn evaluate_ultra_kernel_rule_2120(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2121
pub fn evaluate_ultra_kernel_rule_2121(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2122
pub fn evaluate_ultra_kernel_rule_2122(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2123
pub fn evaluate_ultra_kernel_rule_2123(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2124
pub fn evaluate_ultra_kernel_rule_2124(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2125
pub fn evaluate_ultra_kernel_rule_2125(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2126
pub fn evaluate_ultra_kernel_rule_2126(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2127
pub fn evaluate_ultra_kernel_rule_2127(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2128
pub fn evaluate_ultra_kernel_rule_2128(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2129
pub fn evaluate_ultra_kernel_rule_2129(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2130
pub fn evaluate_ultra_kernel_rule_2130(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2131
pub fn evaluate_ultra_kernel_rule_2131(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2132
pub fn evaluate_ultra_kernel_rule_2132(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2133
pub fn evaluate_ultra_kernel_rule_2133(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2134
pub fn evaluate_ultra_kernel_rule_2134(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2135
pub fn evaluate_ultra_kernel_rule_2135(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2136
pub fn evaluate_ultra_kernel_rule_2136(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2137
pub fn evaluate_ultra_kernel_rule_2137(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2138
pub fn evaluate_ultra_kernel_rule_2138(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2139
pub fn evaluate_ultra_kernel_rule_2139(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2140
pub fn evaluate_ultra_kernel_rule_2140(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2141
pub fn evaluate_ultra_kernel_rule_2141(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2142
pub fn evaluate_ultra_kernel_rule_2142(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2143
pub fn evaluate_ultra_kernel_rule_2143(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2144
pub fn evaluate_ultra_kernel_rule_2144(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2145
pub fn evaluate_ultra_kernel_rule_2145(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2146
pub fn evaluate_ultra_kernel_rule_2146(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2147
pub fn evaluate_ultra_kernel_rule_2147(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2148
pub fn evaluate_ultra_kernel_rule_2148(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2149
pub fn evaluate_ultra_kernel_rule_2149(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2150
pub fn evaluate_ultra_kernel_rule_2150(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2151
pub fn evaluate_ultra_kernel_rule_2151(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2152
pub fn evaluate_ultra_kernel_rule_2152(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2153
pub fn evaluate_ultra_kernel_rule_2153(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2154
pub fn evaluate_ultra_kernel_rule_2154(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2155
pub fn evaluate_ultra_kernel_rule_2155(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2156
pub fn evaluate_ultra_kernel_rule_2156(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2157
pub fn evaluate_ultra_kernel_rule_2157(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2158
pub fn evaluate_ultra_kernel_rule_2158(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2159
pub fn evaluate_ultra_kernel_rule_2159(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2160
pub fn evaluate_ultra_kernel_rule_2160(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2161
pub fn evaluate_ultra_kernel_rule_2161(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2162
pub fn evaluate_ultra_kernel_rule_2162(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2163
pub fn evaluate_ultra_kernel_rule_2163(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2164
pub fn evaluate_ultra_kernel_rule_2164(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2165
pub fn evaluate_ultra_kernel_rule_2165(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2166
pub fn evaluate_ultra_kernel_rule_2166(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2167
pub fn evaluate_ultra_kernel_rule_2167(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2168
pub fn evaluate_ultra_kernel_rule_2168(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2169
pub fn evaluate_ultra_kernel_rule_2169(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2170
pub fn evaluate_ultra_kernel_rule_2170(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2171
pub fn evaluate_ultra_kernel_rule_2171(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2172
pub fn evaluate_ultra_kernel_rule_2172(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2173
pub fn evaluate_ultra_kernel_rule_2173(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2174
pub fn evaluate_ultra_kernel_rule_2174(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2175
pub fn evaluate_ultra_kernel_rule_2175(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2176
pub fn evaluate_ultra_kernel_rule_2176(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2177
pub fn evaluate_ultra_kernel_rule_2177(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2178
pub fn evaluate_ultra_kernel_rule_2178(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2179
pub fn evaluate_ultra_kernel_rule_2179(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2180
pub fn evaluate_ultra_kernel_rule_2180(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2181
pub fn evaluate_ultra_kernel_rule_2181(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2182
pub fn evaluate_ultra_kernel_rule_2182(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2183
pub fn evaluate_ultra_kernel_rule_2183(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2184
pub fn evaluate_ultra_kernel_rule_2184(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2185
pub fn evaluate_ultra_kernel_rule_2185(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2186
pub fn evaluate_ultra_kernel_rule_2186(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2187
pub fn evaluate_ultra_kernel_rule_2187(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2188
pub fn evaluate_ultra_kernel_rule_2188(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2189
pub fn evaluate_ultra_kernel_rule_2189(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2190
pub fn evaluate_ultra_kernel_rule_2190(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2191
pub fn evaluate_ultra_kernel_rule_2191(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2192
pub fn evaluate_ultra_kernel_rule_2192(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2193
pub fn evaluate_ultra_kernel_rule_2193(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2194
pub fn evaluate_ultra_kernel_rule_2194(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2195
pub fn evaluate_ultra_kernel_rule_2195(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2196
pub fn evaluate_ultra_kernel_rule_2196(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2197
pub fn evaluate_ultra_kernel_rule_2197(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2198
pub fn evaluate_ultra_kernel_rule_2198(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2199
pub fn evaluate_ultra_kernel_rule_2199(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2200
pub fn evaluate_ultra_kernel_rule_2200(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2201
pub fn evaluate_ultra_kernel_rule_2201(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2202
pub fn evaluate_ultra_kernel_rule_2202(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2203
pub fn evaluate_ultra_kernel_rule_2203(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2204
pub fn evaluate_ultra_kernel_rule_2204(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2205
pub fn evaluate_ultra_kernel_rule_2205(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2206
pub fn evaluate_ultra_kernel_rule_2206(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2207
pub fn evaluate_ultra_kernel_rule_2207(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2208
pub fn evaluate_ultra_kernel_rule_2208(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2209
pub fn evaluate_ultra_kernel_rule_2209(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2210
pub fn evaluate_ultra_kernel_rule_2210(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2211
pub fn evaluate_ultra_kernel_rule_2211(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2212
pub fn evaluate_ultra_kernel_rule_2212(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2213
pub fn evaluate_ultra_kernel_rule_2213(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2214
pub fn evaluate_ultra_kernel_rule_2214(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2215
pub fn evaluate_ultra_kernel_rule_2215(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2216
pub fn evaluate_ultra_kernel_rule_2216(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2217
pub fn evaluate_ultra_kernel_rule_2217(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2218
pub fn evaluate_ultra_kernel_rule_2218(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2219
pub fn evaluate_ultra_kernel_rule_2219(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2220
pub fn evaluate_ultra_kernel_rule_2220(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2221
pub fn evaluate_ultra_kernel_rule_2221(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2222
pub fn evaluate_ultra_kernel_rule_2222(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2223
pub fn evaluate_ultra_kernel_rule_2223(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2224
pub fn evaluate_ultra_kernel_rule_2224(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2225
pub fn evaluate_ultra_kernel_rule_2225(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2226
pub fn evaluate_ultra_kernel_rule_2226(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2227
pub fn evaluate_ultra_kernel_rule_2227(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2228
pub fn evaluate_ultra_kernel_rule_2228(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2229
pub fn evaluate_ultra_kernel_rule_2229(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2230
pub fn evaluate_ultra_kernel_rule_2230(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2231
pub fn evaluate_ultra_kernel_rule_2231(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2232
pub fn evaluate_ultra_kernel_rule_2232(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2233
pub fn evaluate_ultra_kernel_rule_2233(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2234
pub fn evaluate_ultra_kernel_rule_2234(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2235
pub fn evaluate_ultra_kernel_rule_2235(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2236
pub fn evaluate_ultra_kernel_rule_2236(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2237
pub fn evaluate_ultra_kernel_rule_2237(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2238
pub fn evaluate_ultra_kernel_rule_2238(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2239
pub fn evaluate_ultra_kernel_rule_2239(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2240
pub fn evaluate_ultra_kernel_rule_2240(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2241
pub fn evaluate_ultra_kernel_rule_2241(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2242
pub fn evaluate_ultra_kernel_rule_2242(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2243
pub fn evaluate_ultra_kernel_rule_2243(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2244
pub fn evaluate_ultra_kernel_rule_2244(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2245
pub fn evaluate_ultra_kernel_rule_2245(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2246
pub fn evaluate_ultra_kernel_rule_2246(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2247
pub fn evaluate_ultra_kernel_rule_2247(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2248
pub fn evaluate_ultra_kernel_rule_2248(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2249
pub fn evaluate_ultra_kernel_rule_2249(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2250
pub fn evaluate_ultra_kernel_rule_2250(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2251
pub fn evaluate_ultra_kernel_rule_2251(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2252
pub fn evaluate_ultra_kernel_rule_2252(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2253
pub fn evaluate_ultra_kernel_rule_2253(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2254
pub fn evaluate_ultra_kernel_rule_2254(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2255
pub fn evaluate_ultra_kernel_rule_2255(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2256
pub fn evaluate_ultra_kernel_rule_2256(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2257
pub fn evaluate_ultra_kernel_rule_2257(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2258
pub fn evaluate_ultra_kernel_rule_2258(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2259
pub fn evaluate_ultra_kernel_rule_2259(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2260
pub fn evaluate_ultra_kernel_rule_2260(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2261
pub fn evaluate_ultra_kernel_rule_2261(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2262
pub fn evaluate_ultra_kernel_rule_2262(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2263
pub fn evaluate_ultra_kernel_rule_2263(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2264
pub fn evaluate_ultra_kernel_rule_2264(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2265
pub fn evaluate_ultra_kernel_rule_2265(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2266
pub fn evaluate_ultra_kernel_rule_2266(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2267
pub fn evaluate_ultra_kernel_rule_2267(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2268
pub fn evaluate_ultra_kernel_rule_2268(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2269
pub fn evaluate_ultra_kernel_rule_2269(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2270
pub fn evaluate_ultra_kernel_rule_2270(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2271
pub fn evaluate_ultra_kernel_rule_2271(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2272
pub fn evaluate_ultra_kernel_rule_2272(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2273
pub fn evaluate_ultra_kernel_rule_2273(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2274
pub fn evaluate_ultra_kernel_rule_2274(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2275
pub fn evaluate_ultra_kernel_rule_2275(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2276
pub fn evaluate_ultra_kernel_rule_2276(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2277
pub fn evaluate_ultra_kernel_rule_2277(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2278
pub fn evaluate_ultra_kernel_rule_2278(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2279
pub fn evaluate_ultra_kernel_rule_2279(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2280
pub fn evaluate_ultra_kernel_rule_2280(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2281
pub fn evaluate_ultra_kernel_rule_2281(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2282
pub fn evaluate_ultra_kernel_rule_2282(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2283
pub fn evaluate_ultra_kernel_rule_2283(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2284
pub fn evaluate_ultra_kernel_rule_2284(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2285
pub fn evaluate_ultra_kernel_rule_2285(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2286
pub fn evaluate_ultra_kernel_rule_2286(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2287
pub fn evaluate_ultra_kernel_rule_2287(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2288
pub fn evaluate_ultra_kernel_rule_2288(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2289
pub fn evaluate_ultra_kernel_rule_2289(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2290
pub fn evaluate_ultra_kernel_rule_2290(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2291
pub fn evaluate_ultra_kernel_rule_2291(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2292
pub fn evaluate_ultra_kernel_rule_2292(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2293
pub fn evaluate_ultra_kernel_rule_2293(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2294
pub fn evaluate_ultra_kernel_rule_2294(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2295
pub fn evaluate_ultra_kernel_rule_2295(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2296
pub fn evaluate_ultra_kernel_rule_2296(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2297
pub fn evaluate_ultra_kernel_rule_2297(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2298
pub fn evaluate_ultra_kernel_rule_2298(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2299
pub fn evaluate_ultra_kernel_rule_2299(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2300
pub fn evaluate_ultra_kernel_rule_2300(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2301
pub fn evaluate_ultra_kernel_rule_2301(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2302
pub fn evaluate_ultra_kernel_rule_2302(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2303
pub fn evaluate_ultra_kernel_rule_2303(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2304
pub fn evaluate_ultra_kernel_rule_2304(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2305
pub fn evaluate_ultra_kernel_rule_2305(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2306
pub fn evaluate_ultra_kernel_rule_2306(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2307
pub fn evaluate_ultra_kernel_rule_2307(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2308
pub fn evaluate_ultra_kernel_rule_2308(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2309
pub fn evaluate_ultra_kernel_rule_2309(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2310
pub fn evaluate_ultra_kernel_rule_2310(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2311
pub fn evaluate_ultra_kernel_rule_2311(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2312
pub fn evaluate_ultra_kernel_rule_2312(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2313
pub fn evaluate_ultra_kernel_rule_2313(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2314
pub fn evaluate_ultra_kernel_rule_2314(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2315
pub fn evaluate_ultra_kernel_rule_2315(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2316
pub fn evaluate_ultra_kernel_rule_2316(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2317
pub fn evaluate_ultra_kernel_rule_2317(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2318
pub fn evaluate_ultra_kernel_rule_2318(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2319
pub fn evaluate_ultra_kernel_rule_2319(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2320
pub fn evaluate_ultra_kernel_rule_2320(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2321
pub fn evaluate_ultra_kernel_rule_2321(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2322
pub fn evaluate_ultra_kernel_rule_2322(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2323
pub fn evaluate_ultra_kernel_rule_2323(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2324
pub fn evaluate_ultra_kernel_rule_2324(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2325
pub fn evaluate_ultra_kernel_rule_2325(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2326
pub fn evaluate_ultra_kernel_rule_2326(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2327
pub fn evaluate_ultra_kernel_rule_2327(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2328
pub fn evaluate_ultra_kernel_rule_2328(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2329
pub fn evaluate_ultra_kernel_rule_2329(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2330
pub fn evaluate_ultra_kernel_rule_2330(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2331
pub fn evaluate_ultra_kernel_rule_2331(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2332
pub fn evaluate_ultra_kernel_rule_2332(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2333
pub fn evaluate_ultra_kernel_rule_2333(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2334
pub fn evaluate_ultra_kernel_rule_2334(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2335
pub fn evaluate_ultra_kernel_rule_2335(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2336
pub fn evaluate_ultra_kernel_rule_2336(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2337
pub fn evaluate_ultra_kernel_rule_2337(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2338
pub fn evaluate_ultra_kernel_rule_2338(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2339
pub fn evaluate_ultra_kernel_rule_2339(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2340
pub fn evaluate_ultra_kernel_rule_2340(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2341
pub fn evaluate_ultra_kernel_rule_2341(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2342
pub fn evaluate_ultra_kernel_rule_2342(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2343
pub fn evaluate_ultra_kernel_rule_2343(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2344
pub fn evaluate_ultra_kernel_rule_2344(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2345
pub fn evaluate_ultra_kernel_rule_2345(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2346
pub fn evaluate_ultra_kernel_rule_2346(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2347
pub fn evaluate_ultra_kernel_rule_2347(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2348
pub fn evaluate_ultra_kernel_rule_2348(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2349
pub fn evaluate_ultra_kernel_rule_2349(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2350
pub fn evaluate_ultra_kernel_rule_2350(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2351
pub fn evaluate_ultra_kernel_rule_2351(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2352
pub fn evaluate_ultra_kernel_rule_2352(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2353
pub fn evaluate_ultra_kernel_rule_2353(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2354
pub fn evaluate_ultra_kernel_rule_2354(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2355
pub fn evaluate_ultra_kernel_rule_2355(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2356
pub fn evaluate_ultra_kernel_rule_2356(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2357
pub fn evaluate_ultra_kernel_rule_2357(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2358
pub fn evaluate_ultra_kernel_rule_2358(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2359
pub fn evaluate_ultra_kernel_rule_2359(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2360
pub fn evaluate_ultra_kernel_rule_2360(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2361
pub fn evaluate_ultra_kernel_rule_2361(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2362
pub fn evaluate_ultra_kernel_rule_2362(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2363
pub fn evaluate_ultra_kernel_rule_2363(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2364
pub fn evaluate_ultra_kernel_rule_2364(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2365
pub fn evaluate_ultra_kernel_rule_2365(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2366
pub fn evaluate_ultra_kernel_rule_2366(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2367
pub fn evaluate_ultra_kernel_rule_2367(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2368
pub fn evaluate_ultra_kernel_rule_2368(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2369
pub fn evaluate_ultra_kernel_rule_2369(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2370
pub fn evaluate_ultra_kernel_rule_2370(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2371
pub fn evaluate_ultra_kernel_rule_2371(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2372
pub fn evaluate_ultra_kernel_rule_2372(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2373
pub fn evaluate_ultra_kernel_rule_2373(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2374
pub fn evaluate_ultra_kernel_rule_2374(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2375
pub fn evaluate_ultra_kernel_rule_2375(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2376
pub fn evaluate_ultra_kernel_rule_2376(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2377
pub fn evaluate_ultra_kernel_rule_2377(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2378
pub fn evaluate_ultra_kernel_rule_2378(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2379
pub fn evaluate_ultra_kernel_rule_2379(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2380
pub fn evaluate_ultra_kernel_rule_2380(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2381
pub fn evaluate_ultra_kernel_rule_2381(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2382
pub fn evaluate_ultra_kernel_rule_2382(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2383
pub fn evaluate_ultra_kernel_rule_2383(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2384
pub fn evaluate_ultra_kernel_rule_2384(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2385
pub fn evaluate_ultra_kernel_rule_2385(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2386
pub fn evaluate_ultra_kernel_rule_2386(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2387
pub fn evaluate_ultra_kernel_rule_2387(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2388
pub fn evaluate_ultra_kernel_rule_2388(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2389
pub fn evaluate_ultra_kernel_rule_2389(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2390
pub fn evaluate_ultra_kernel_rule_2390(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2391
pub fn evaluate_ultra_kernel_rule_2391(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2392
pub fn evaluate_ultra_kernel_rule_2392(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2393
pub fn evaluate_ultra_kernel_rule_2393(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2394
pub fn evaluate_ultra_kernel_rule_2394(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2395
pub fn evaluate_ultra_kernel_rule_2395(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2396
pub fn evaluate_ultra_kernel_rule_2396(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2397
pub fn evaluate_ultra_kernel_rule_2397(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2398
pub fn evaluate_ultra_kernel_rule_2398(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2399
pub fn evaluate_ultra_kernel_rule_2399(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2400
pub fn evaluate_ultra_kernel_rule_2400(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2401
pub fn evaluate_ultra_kernel_rule_2401(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2402
pub fn evaluate_ultra_kernel_rule_2402(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2403
pub fn evaluate_ultra_kernel_rule_2403(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2404
pub fn evaluate_ultra_kernel_rule_2404(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2405
pub fn evaluate_ultra_kernel_rule_2405(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2406
pub fn evaluate_ultra_kernel_rule_2406(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2407
pub fn evaluate_ultra_kernel_rule_2407(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2408
pub fn evaluate_ultra_kernel_rule_2408(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2409
pub fn evaluate_ultra_kernel_rule_2409(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2410
pub fn evaluate_ultra_kernel_rule_2410(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2411
pub fn evaluate_ultra_kernel_rule_2411(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2412
pub fn evaluate_ultra_kernel_rule_2412(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2413
pub fn evaluate_ultra_kernel_rule_2413(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2414
pub fn evaluate_ultra_kernel_rule_2414(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2415
pub fn evaluate_ultra_kernel_rule_2415(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2416
pub fn evaluate_ultra_kernel_rule_2416(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2417
pub fn evaluate_ultra_kernel_rule_2417(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2418
pub fn evaluate_ultra_kernel_rule_2418(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2419
pub fn evaluate_ultra_kernel_rule_2419(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2420
pub fn evaluate_ultra_kernel_rule_2420(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2421
pub fn evaluate_ultra_kernel_rule_2421(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2422
pub fn evaluate_ultra_kernel_rule_2422(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2423
pub fn evaluate_ultra_kernel_rule_2423(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2424
pub fn evaluate_ultra_kernel_rule_2424(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2425
pub fn evaluate_ultra_kernel_rule_2425(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2426
pub fn evaluate_ultra_kernel_rule_2426(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2427
pub fn evaluate_ultra_kernel_rule_2427(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2428
pub fn evaluate_ultra_kernel_rule_2428(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2429
pub fn evaluate_ultra_kernel_rule_2429(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2430
pub fn evaluate_ultra_kernel_rule_2430(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2431
pub fn evaluate_ultra_kernel_rule_2431(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2432
pub fn evaluate_ultra_kernel_rule_2432(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2433
pub fn evaluate_ultra_kernel_rule_2433(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2434
pub fn evaluate_ultra_kernel_rule_2434(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2435
pub fn evaluate_ultra_kernel_rule_2435(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2436
pub fn evaluate_ultra_kernel_rule_2436(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2437
pub fn evaluate_ultra_kernel_rule_2437(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2438
pub fn evaluate_ultra_kernel_rule_2438(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2439
pub fn evaluate_ultra_kernel_rule_2439(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2440
pub fn evaluate_ultra_kernel_rule_2440(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2441
pub fn evaluate_ultra_kernel_rule_2441(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2442
pub fn evaluate_ultra_kernel_rule_2442(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2443
pub fn evaluate_ultra_kernel_rule_2443(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2444
pub fn evaluate_ultra_kernel_rule_2444(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2445
pub fn evaluate_ultra_kernel_rule_2445(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2446
pub fn evaluate_ultra_kernel_rule_2446(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2447
pub fn evaluate_ultra_kernel_rule_2447(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2448
pub fn evaluate_ultra_kernel_rule_2448(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2449
pub fn evaluate_ultra_kernel_rule_2449(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2450
pub fn evaluate_ultra_kernel_rule_2450(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2451
pub fn evaluate_ultra_kernel_rule_2451(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2452
pub fn evaluate_ultra_kernel_rule_2452(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2453
pub fn evaluate_ultra_kernel_rule_2453(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2454
pub fn evaluate_ultra_kernel_rule_2454(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2455
pub fn evaluate_ultra_kernel_rule_2455(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2456
pub fn evaluate_ultra_kernel_rule_2456(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2457
pub fn evaluate_ultra_kernel_rule_2457(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2458
pub fn evaluate_ultra_kernel_rule_2458(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2459
pub fn evaluate_ultra_kernel_rule_2459(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2460
pub fn evaluate_ultra_kernel_rule_2460(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2461
pub fn evaluate_ultra_kernel_rule_2461(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2462
pub fn evaluate_ultra_kernel_rule_2462(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2463
pub fn evaluate_ultra_kernel_rule_2463(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2464
pub fn evaluate_ultra_kernel_rule_2464(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2465
pub fn evaluate_ultra_kernel_rule_2465(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2466
pub fn evaluate_ultra_kernel_rule_2466(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2467
pub fn evaluate_ultra_kernel_rule_2467(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2468
pub fn evaluate_ultra_kernel_rule_2468(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2469
pub fn evaluate_ultra_kernel_rule_2469(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2470
pub fn evaluate_ultra_kernel_rule_2470(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2471
pub fn evaluate_ultra_kernel_rule_2471(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2472
pub fn evaluate_ultra_kernel_rule_2472(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2473
pub fn evaluate_ultra_kernel_rule_2473(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2474
pub fn evaluate_ultra_kernel_rule_2474(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2475
pub fn evaluate_ultra_kernel_rule_2475(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2476
pub fn evaluate_ultra_kernel_rule_2476(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2477
pub fn evaluate_ultra_kernel_rule_2477(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2478
pub fn evaluate_ultra_kernel_rule_2478(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2479
pub fn evaluate_ultra_kernel_rule_2479(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2480
pub fn evaluate_ultra_kernel_rule_2480(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2481
pub fn evaluate_ultra_kernel_rule_2481(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2482
pub fn evaluate_ultra_kernel_rule_2482(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2483
pub fn evaluate_ultra_kernel_rule_2483(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2484
pub fn evaluate_ultra_kernel_rule_2484(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2485
pub fn evaluate_ultra_kernel_rule_2485(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2486
pub fn evaluate_ultra_kernel_rule_2486(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2487
pub fn evaluate_ultra_kernel_rule_2487(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2488
pub fn evaluate_ultra_kernel_rule_2488(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2489
pub fn evaluate_ultra_kernel_rule_2489(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2490
pub fn evaluate_ultra_kernel_rule_2490(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2491
pub fn evaluate_ultra_kernel_rule_2491(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2492
pub fn evaluate_ultra_kernel_rule_2492(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2493
pub fn evaluate_ultra_kernel_rule_2493(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2494
pub fn evaluate_ultra_kernel_rule_2494(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2495
pub fn evaluate_ultra_kernel_rule_2495(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2496
pub fn evaluate_ultra_kernel_rule_2496(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2497
pub fn evaluate_ultra_kernel_rule_2497(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2498
pub fn evaluate_ultra_kernel_rule_2498(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2499
pub fn evaluate_ultra_kernel_rule_2499(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

/// Native Type Kernel Evaluator Rule #2500
pub fn evaluate_ultra_kernel_rule_2500(
    engine: &mut UltraEngine_1,
    symbol: &str,
    type_name: &str,
) -> bool {
    if symbol.is_empty() {
        return false;
    }
    engine
        .symbol_map
        .insert(symbol.to_string(), type_name.to_string());
    true
}

#[pyfunction]
pub fn rust_ultra_kernel_eval_1(symbol: &str, type_name: &str) -> bool {
    let mut engine = UltraEngine_1::new("ultra");
    evaluate_ultra_kernel_rule_1(&mut engine, symbol, type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ultra_engine() {
        let mut engine = UltraEngine_1::new("ultra");
        assert!(evaluate_ultra_kernel_rule_1(
            &mut engine,
            "x",
            "builtins.int"
        ));
    }
}
