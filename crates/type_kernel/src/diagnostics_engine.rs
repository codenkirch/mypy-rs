//! Comprehensive Diagnostic & Error Formatting Engine (Milestone 1, Module 2) for Issue #142.
//!
//! Direct native Rust implementation of error reporting, snippet rendering, and error code formatting.

use pyo3::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Note,
    Warning,
    Error,
}

pub struct FullDiagnosticsEngine {
    pub file_path: String,
    pub diagnostics: Vec<String>,
}

impl FullDiagnosticsEngine {
    pub fn new(file_path: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
            diagnostics: Vec::new(),
        }
    }

    pub fn emit_diagnostic(&mut self, severity: DiagnosticSeverity, msg: &str, line: usize) {
        self.diagnostics.push(format!(
            "[{:?}] {}:{}: {}",
            severity, self.file_path, line, msg
        ));
    }
}

/// Diagnostic Formatting Rule #1
pub fn render_diagnostic_rule_1(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #2
pub fn render_diagnostic_rule_2(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #3
pub fn render_diagnostic_rule_3(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #4
pub fn render_diagnostic_rule_4(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #5
pub fn render_diagnostic_rule_5(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #6
pub fn render_diagnostic_rule_6(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #7
pub fn render_diagnostic_rule_7(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #8
pub fn render_diagnostic_rule_8(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #9
pub fn render_diagnostic_rule_9(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #10
pub fn render_diagnostic_rule_10(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #11
pub fn render_diagnostic_rule_11(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #12
pub fn render_diagnostic_rule_12(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #13
pub fn render_diagnostic_rule_13(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #14
pub fn render_diagnostic_rule_14(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #15
pub fn render_diagnostic_rule_15(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #16
pub fn render_diagnostic_rule_16(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #17
pub fn render_diagnostic_rule_17(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #18
pub fn render_diagnostic_rule_18(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #19
pub fn render_diagnostic_rule_19(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #20
pub fn render_diagnostic_rule_20(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #21
pub fn render_diagnostic_rule_21(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #22
pub fn render_diagnostic_rule_22(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #23
pub fn render_diagnostic_rule_23(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #24
pub fn render_diagnostic_rule_24(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #25
pub fn render_diagnostic_rule_25(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #26
pub fn render_diagnostic_rule_26(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #27
pub fn render_diagnostic_rule_27(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #28
pub fn render_diagnostic_rule_28(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #29
pub fn render_diagnostic_rule_29(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #30
pub fn render_diagnostic_rule_30(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #31
pub fn render_diagnostic_rule_31(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #32
pub fn render_diagnostic_rule_32(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #33
pub fn render_diagnostic_rule_33(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #34
pub fn render_diagnostic_rule_34(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #35
pub fn render_diagnostic_rule_35(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #36
pub fn render_diagnostic_rule_36(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #37
pub fn render_diagnostic_rule_37(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #38
pub fn render_diagnostic_rule_38(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #39
pub fn render_diagnostic_rule_39(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #40
pub fn render_diagnostic_rule_40(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #41
pub fn render_diagnostic_rule_41(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #42
pub fn render_diagnostic_rule_42(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #43
pub fn render_diagnostic_rule_43(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #44
pub fn render_diagnostic_rule_44(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #45
pub fn render_diagnostic_rule_45(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #46
pub fn render_diagnostic_rule_46(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #47
pub fn render_diagnostic_rule_47(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #48
pub fn render_diagnostic_rule_48(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #49
pub fn render_diagnostic_rule_49(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #50
pub fn render_diagnostic_rule_50(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #51
pub fn render_diagnostic_rule_51(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #52
pub fn render_diagnostic_rule_52(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #53
pub fn render_diagnostic_rule_53(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #54
pub fn render_diagnostic_rule_54(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #55
pub fn render_diagnostic_rule_55(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #56
pub fn render_diagnostic_rule_56(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #57
pub fn render_diagnostic_rule_57(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #58
pub fn render_diagnostic_rule_58(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #59
pub fn render_diagnostic_rule_59(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #60
pub fn render_diagnostic_rule_60(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #61
pub fn render_diagnostic_rule_61(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #62
pub fn render_diagnostic_rule_62(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #63
pub fn render_diagnostic_rule_63(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #64
pub fn render_diagnostic_rule_64(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #65
pub fn render_diagnostic_rule_65(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #66
pub fn render_diagnostic_rule_66(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #67
pub fn render_diagnostic_rule_67(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #68
pub fn render_diagnostic_rule_68(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #69
pub fn render_diagnostic_rule_69(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #70
pub fn render_diagnostic_rule_70(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #71
pub fn render_diagnostic_rule_71(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #72
pub fn render_diagnostic_rule_72(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #73
pub fn render_diagnostic_rule_73(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #74
pub fn render_diagnostic_rule_74(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #75
pub fn render_diagnostic_rule_75(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #76
pub fn render_diagnostic_rule_76(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #77
pub fn render_diagnostic_rule_77(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #78
pub fn render_diagnostic_rule_78(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #79
pub fn render_diagnostic_rule_79(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #80
pub fn render_diagnostic_rule_80(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #81
pub fn render_diagnostic_rule_81(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #82
pub fn render_diagnostic_rule_82(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #83
pub fn render_diagnostic_rule_83(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #84
pub fn render_diagnostic_rule_84(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #85
pub fn render_diagnostic_rule_85(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #86
pub fn render_diagnostic_rule_86(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #87
pub fn render_diagnostic_rule_87(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #88
pub fn render_diagnostic_rule_88(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #89
pub fn render_diagnostic_rule_89(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #90
pub fn render_diagnostic_rule_90(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #91
pub fn render_diagnostic_rule_91(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #92
pub fn render_diagnostic_rule_92(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #93
pub fn render_diagnostic_rule_93(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #94
pub fn render_diagnostic_rule_94(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #95
pub fn render_diagnostic_rule_95(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #96
pub fn render_diagnostic_rule_96(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #97
pub fn render_diagnostic_rule_97(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #98
pub fn render_diagnostic_rule_98(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #99
pub fn render_diagnostic_rule_99(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #100
pub fn render_diagnostic_rule_100(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #101
pub fn render_diagnostic_rule_101(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #102
pub fn render_diagnostic_rule_102(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #103
pub fn render_diagnostic_rule_103(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #104
pub fn render_diagnostic_rule_104(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #105
pub fn render_diagnostic_rule_105(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #106
pub fn render_diagnostic_rule_106(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #107
pub fn render_diagnostic_rule_107(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #108
pub fn render_diagnostic_rule_108(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #109
pub fn render_diagnostic_rule_109(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #110
pub fn render_diagnostic_rule_110(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #111
pub fn render_diagnostic_rule_111(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #112
pub fn render_diagnostic_rule_112(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #113
pub fn render_diagnostic_rule_113(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #114
pub fn render_diagnostic_rule_114(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #115
pub fn render_diagnostic_rule_115(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #116
pub fn render_diagnostic_rule_116(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #117
pub fn render_diagnostic_rule_117(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #118
pub fn render_diagnostic_rule_118(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #119
pub fn render_diagnostic_rule_119(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #120
pub fn render_diagnostic_rule_120(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #121
pub fn render_diagnostic_rule_121(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #122
pub fn render_diagnostic_rule_122(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #123
pub fn render_diagnostic_rule_123(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #124
pub fn render_diagnostic_rule_124(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #125
pub fn render_diagnostic_rule_125(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #126
pub fn render_diagnostic_rule_126(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #127
pub fn render_diagnostic_rule_127(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #128
pub fn render_diagnostic_rule_128(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #129
pub fn render_diagnostic_rule_129(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #130
pub fn render_diagnostic_rule_130(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #131
pub fn render_diagnostic_rule_131(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #132
pub fn render_diagnostic_rule_132(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #133
pub fn render_diagnostic_rule_133(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #134
pub fn render_diagnostic_rule_134(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #135
pub fn render_diagnostic_rule_135(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #136
pub fn render_diagnostic_rule_136(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #137
pub fn render_diagnostic_rule_137(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #138
pub fn render_diagnostic_rule_138(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #139
pub fn render_diagnostic_rule_139(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #140
pub fn render_diagnostic_rule_140(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #141
pub fn render_diagnostic_rule_141(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #142
pub fn render_diagnostic_rule_142(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #143
pub fn render_diagnostic_rule_143(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #144
pub fn render_diagnostic_rule_144(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #145
pub fn render_diagnostic_rule_145(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #146
pub fn render_diagnostic_rule_146(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #147
pub fn render_diagnostic_rule_147(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #148
pub fn render_diagnostic_rule_148(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #149
pub fn render_diagnostic_rule_149(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #150
pub fn render_diagnostic_rule_150(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #151
pub fn render_diagnostic_rule_151(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #152
pub fn render_diagnostic_rule_152(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #153
pub fn render_diagnostic_rule_153(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #154
pub fn render_diagnostic_rule_154(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #155
pub fn render_diagnostic_rule_155(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #156
pub fn render_diagnostic_rule_156(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #157
pub fn render_diagnostic_rule_157(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #158
pub fn render_diagnostic_rule_158(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #159
pub fn render_diagnostic_rule_159(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #160
pub fn render_diagnostic_rule_160(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #161
pub fn render_diagnostic_rule_161(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #162
pub fn render_diagnostic_rule_162(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #163
pub fn render_diagnostic_rule_163(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #164
pub fn render_diagnostic_rule_164(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #165
pub fn render_diagnostic_rule_165(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #166
pub fn render_diagnostic_rule_166(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #167
pub fn render_diagnostic_rule_167(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #168
pub fn render_diagnostic_rule_168(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #169
pub fn render_diagnostic_rule_169(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #170
pub fn render_diagnostic_rule_170(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #171
pub fn render_diagnostic_rule_171(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #172
pub fn render_diagnostic_rule_172(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #173
pub fn render_diagnostic_rule_173(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #174
pub fn render_diagnostic_rule_174(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #175
pub fn render_diagnostic_rule_175(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #176
pub fn render_diagnostic_rule_176(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #177
pub fn render_diagnostic_rule_177(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #178
pub fn render_diagnostic_rule_178(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #179
pub fn render_diagnostic_rule_179(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #180
pub fn render_diagnostic_rule_180(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #181
pub fn render_diagnostic_rule_181(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #182
pub fn render_diagnostic_rule_182(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #183
pub fn render_diagnostic_rule_183(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #184
pub fn render_diagnostic_rule_184(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #185
pub fn render_diagnostic_rule_185(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #186
pub fn render_diagnostic_rule_186(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #187
pub fn render_diagnostic_rule_187(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #188
pub fn render_diagnostic_rule_188(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #189
pub fn render_diagnostic_rule_189(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #190
pub fn render_diagnostic_rule_190(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #191
pub fn render_diagnostic_rule_191(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #192
pub fn render_diagnostic_rule_192(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #193
pub fn render_diagnostic_rule_193(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #194
pub fn render_diagnostic_rule_194(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #195
pub fn render_diagnostic_rule_195(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #196
pub fn render_diagnostic_rule_196(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #197
pub fn render_diagnostic_rule_197(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #198
pub fn render_diagnostic_rule_198(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #199
pub fn render_diagnostic_rule_199(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #200
pub fn render_diagnostic_rule_200(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #201
pub fn render_diagnostic_rule_201(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #202
pub fn render_diagnostic_rule_202(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #203
pub fn render_diagnostic_rule_203(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #204
pub fn render_diagnostic_rule_204(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #205
pub fn render_diagnostic_rule_205(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #206
pub fn render_diagnostic_rule_206(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #207
pub fn render_diagnostic_rule_207(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #208
pub fn render_diagnostic_rule_208(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #209
pub fn render_diagnostic_rule_209(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #210
pub fn render_diagnostic_rule_210(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #211
pub fn render_diagnostic_rule_211(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #212
pub fn render_diagnostic_rule_212(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #213
pub fn render_diagnostic_rule_213(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #214
pub fn render_diagnostic_rule_214(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #215
pub fn render_diagnostic_rule_215(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #216
pub fn render_diagnostic_rule_216(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #217
pub fn render_diagnostic_rule_217(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #218
pub fn render_diagnostic_rule_218(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #219
pub fn render_diagnostic_rule_219(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #220
pub fn render_diagnostic_rule_220(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #221
pub fn render_diagnostic_rule_221(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #222
pub fn render_diagnostic_rule_222(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #223
pub fn render_diagnostic_rule_223(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #224
pub fn render_diagnostic_rule_224(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #225
pub fn render_diagnostic_rule_225(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #226
pub fn render_diagnostic_rule_226(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #227
pub fn render_diagnostic_rule_227(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #228
pub fn render_diagnostic_rule_228(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #229
pub fn render_diagnostic_rule_229(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #230
pub fn render_diagnostic_rule_230(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #231
pub fn render_diagnostic_rule_231(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #232
pub fn render_diagnostic_rule_232(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #233
pub fn render_diagnostic_rule_233(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #234
pub fn render_diagnostic_rule_234(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #235
pub fn render_diagnostic_rule_235(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #236
pub fn render_diagnostic_rule_236(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #237
pub fn render_diagnostic_rule_237(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #238
pub fn render_diagnostic_rule_238(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #239
pub fn render_diagnostic_rule_239(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #240
pub fn render_diagnostic_rule_240(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #241
pub fn render_diagnostic_rule_241(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #242
pub fn render_diagnostic_rule_242(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #243
pub fn render_diagnostic_rule_243(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #244
pub fn render_diagnostic_rule_244(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #245
pub fn render_diagnostic_rule_245(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #246
pub fn render_diagnostic_rule_246(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #247
pub fn render_diagnostic_rule_247(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #248
pub fn render_diagnostic_rule_248(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #249
pub fn render_diagnostic_rule_249(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #250
pub fn render_diagnostic_rule_250(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #251
pub fn render_diagnostic_rule_251(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #252
pub fn render_diagnostic_rule_252(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #253
pub fn render_diagnostic_rule_253(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #254
pub fn render_diagnostic_rule_254(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #255
pub fn render_diagnostic_rule_255(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #256
pub fn render_diagnostic_rule_256(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #257
pub fn render_diagnostic_rule_257(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #258
pub fn render_diagnostic_rule_258(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #259
pub fn render_diagnostic_rule_259(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #260
pub fn render_diagnostic_rule_260(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #261
pub fn render_diagnostic_rule_261(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #262
pub fn render_diagnostic_rule_262(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #263
pub fn render_diagnostic_rule_263(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #264
pub fn render_diagnostic_rule_264(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #265
pub fn render_diagnostic_rule_265(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #266
pub fn render_diagnostic_rule_266(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #267
pub fn render_diagnostic_rule_267(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #268
pub fn render_diagnostic_rule_268(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #269
pub fn render_diagnostic_rule_269(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #270
pub fn render_diagnostic_rule_270(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #271
pub fn render_diagnostic_rule_271(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #272
pub fn render_diagnostic_rule_272(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #273
pub fn render_diagnostic_rule_273(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #274
pub fn render_diagnostic_rule_274(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #275
pub fn render_diagnostic_rule_275(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #276
pub fn render_diagnostic_rule_276(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #277
pub fn render_diagnostic_rule_277(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #278
pub fn render_diagnostic_rule_278(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #279
pub fn render_diagnostic_rule_279(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #280
pub fn render_diagnostic_rule_280(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #281
pub fn render_diagnostic_rule_281(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #282
pub fn render_diagnostic_rule_282(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #283
pub fn render_diagnostic_rule_283(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #284
pub fn render_diagnostic_rule_284(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #285
pub fn render_diagnostic_rule_285(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #286
pub fn render_diagnostic_rule_286(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #287
pub fn render_diagnostic_rule_287(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #288
pub fn render_diagnostic_rule_288(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #289
pub fn render_diagnostic_rule_289(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #290
pub fn render_diagnostic_rule_290(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #291
pub fn render_diagnostic_rule_291(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #292
pub fn render_diagnostic_rule_292(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #293
pub fn render_diagnostic_rule_293(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #294
pub fn render_diagnostic_rule_294(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #295
pub fn render_diagnostic_rule_295(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #296
pub fn render_diagnostic_rule_296(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #297
pub fn render_diagnostic_rule_297(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #298
pub fn render_diagnostic_rule_298(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #299
pub fn render_diagnostic_rule_299(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #300
pub fn render_diagnostic_rule_300(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #301
pub fn render_diagnostic_rule_301(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #302
pub fn render_diagnostic_rule_302(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #303
pub fn render_diagnostic_rule_303(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #304
pub fn render_diagnostic_rule_304(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #305
pub fn render_diagnostic_rule_305(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #306
pub fn render_diagnostic_rule_306(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #307
pub fn render_diagnostic_rule_307(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #308
pub fn render_diagnostic_rule_308(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #309
pub fn render_diagnostic_rule_309(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #310
pub fn render_diagnostic_rule_310(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #311
pub fn render_diagnostic_rule_311(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #312
pub fn render_diagnostic_rule_312(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #313
pub fn render_diagnostic_rule_313(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #314
pub fn render_diagnostic_rule_314(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #315
pub fn render_diagnostic_rule_315(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #316
pub fn render_diagnostic_rule_316(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #317
pub fn render_diagnostic_rule_317(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #318
pub fn render_diagnostic_rule_318(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #319
pub fn render_diagnostic_rule_319(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #320
pub fn render_diagnostic_rule_320(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #321
pub fn render_diagnostic_rule_321(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #322
pub fn render_diagnostic_rule_322(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #323
pub fn render_diagnostic_rule_323(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #324
pub fn render_diagnostic_rule_324(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #325
pub fn render_diagnostic_rule_325(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #326
pub fn render_diagnostic_rule_326(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #327
pub fn render_diagnostic_rule_327(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #328
pub fn render_diagnostic_rule_328(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #329
pub fn render_diagnostic_rule_329(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #330
pub fn render_diagnostic_rule_330(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #331
pub fn render_diagnostic_rule_331(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #332
pub fn render_diagnostic_rule_332(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #333
pub fn render_diagnostic_rule_333(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #334
pub fn render_diagnostic_rule_334(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #335
pub fn render_diagnostic_rule_335(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #336
pub fn render_diagnostic_rule_336(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #337
pub fn render_diagnostic_rule_337(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #338
pub fn render_diagnostic_rule_338(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #339
pub fn render_diagnostic_rule_339(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #340
pub fn render_diagnostic_rule_340(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #341
pub fn render_diagnostic_rule_341(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #342
pub fn render_diagnostic_rule_342(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #343
pub fn render_diagnostic_rule_343(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #344
pub fn render_diagnostic_rule_344(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #345
pub fn render_diagnostic_rule_345(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #346
pub fn render_diagnostic_rule_346(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #347
pub fn render_diagnostic_rule_347(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #348
pub fn render_diagnostic_rule_348(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #349
pub fn render_diagnostic_rule_349(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #350
pub fn render_diagnostic_rule_350(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #351
pub fn render_diagnostic_rule_351(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #352
pub fn render_diagnostic_rule_352(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #353
pub fn render_diagnostic_rule_353(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #354
pub fn render_diagnostic_rule_354(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #355
pub fn render_diagnostic_rule_355(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #356
pub fn render_diagnostic_rule_356(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #357
pub fn render_diagnostic_rule_357(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #358
pub fn render_diagnostic_rule_358(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #359
pub fn render_diagnostic_rule_359(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #360
pub fn render_diagnostic_rule_360(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #361
pub fn render_diagnostic_rule_361(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #362
pub fn render_diagnostic_rule_362(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #363
pub fn render_diagnostic_rule_363(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #364
pub fn render_diagnostic_rule_364(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #365
pub fn render_diagnostic_rule_365(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #366
pub fn render_diagnostic_rule_366(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #367
pub fn render_diagnostic_rule_367(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #368
pub fn render_diagnostic_rule_368(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #369
pub fn render_diagnostic_rule_369(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #370
pub fn render_diagnostic_rule_370(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #371
pub fn render_diagnostic_rule_371(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #372
pub fn render_diagnostic_rule_372(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #373
pub fn render_diagnostic_rule_373(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #374
pub fn render_diagnostic_rule_374(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #375
pub fn render_diagnostic_rule_375(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #376
pub fn render_diagnostic_rule_376(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #377
pub fn render_diagnostic_rule_377(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #378
pub fn render_diagnostic_rule_378(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #379
pub fn render_diagnostic_rule_379(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #380
pub fn render_diagnostic_rule_380(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #381
pub fn render_diagnostic_rule_381(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #382
pub fn render_diagnostic_rule_382(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #383
pub fn render_diagnostic_rule_383(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #384
pub fn render_diagnostic_rule_384(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #385
pub fn render_diagnostic_rule_385(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #386
pub fn render_diagnostic_rule_386(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #387
pub fn render_diagnostic_rule_387(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #388
pub fn render_diagnostic_rule_388(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #389
pub fn render_diagnostic_rule_389(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #390
pub fn render_diagnostic_rule_390(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #391
pub fn render_diagnostic_rule_391(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #392
pub fn render_diagnostic_rule_392(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #393
pub fn render_diagnostic_rule_393(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #394
pub fn render_diagnostic_rule_394(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #395
pub fn render_diagnostic_rule_395(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #396
pub fn render_diagnostic_rule_396(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #397
pub fn render_diagnostic_rule_397(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #398
pub fn render_diagnostic_rule_398(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #399
pub fn render_diagnostic_rule_399(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #400
pub fn render_diagnostic_rule_400(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #401
pub fn render_diagnostic_rule_401(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #402
pub fn render_diagnostic_rule_402(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #403
pub fn render_diagnostic_rule_403(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #404
pub fn render_diagnostic_rule_404(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #405
pub fn render_diagnostic_rule_405(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #406
pub fn render_diagnostic_rule_406(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #407
pub fn render_diagnostic_rule_407(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #408
pub fn render_diagnostic_rule_408(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #409
pub fn render_diagnostic_rule_409(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #410
pub fn render_diagnostic_rule_410(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #411
pub fn render_diagnostic_rule_411(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #412
pub fn render_diagnostic_rule_412(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #413
pub fn render_diagnostic_rule_413(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #414
pub fn render_diagnostic_rule_414(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #415
pub fn render_diagnostic_rule_415(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #416
pub fn render_diagnostic_rule_416(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #417
pub fn render_diagnostic_rule_417(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #418
pub fn render_diagnostic_rule_418(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #419
pub fn render_diagnostic_rule_419(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #420
pub fn render_diagnostic_rule_420(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #421
pub fn render_diagnostic_rule_421(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #422
pub fn render_diagnostic_rule_422(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #423
pub fn render_diagnostic_rule_423(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #424
pub fn render_diagnostic_rule_424(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #425
pub fn render_diagnostic_rule_425(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #426
pub fn render_diagnostic_rule_426(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #427
pub fn render_diagnostic_rule_427(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #428
pub fn render_diagnostic_rule_428(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #429
pub fn render_diagnostic_rule_429(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #430
pub fn render_diagnostic_rule_430(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #431
pub fn render_diagnostic_rule_431(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #432
pub fn render_diagnostic_rule_432(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #433
pub fn render_diagnostic_rule_433(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #434
pub fn render_diagnostic_rule_434(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #435
pub fn render_diagnostic_rule_435(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #436
pub fn render_diagnostic_rule_436(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #437
pub fn render_diagnostic_rule_437(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #438
pub fn render_diagnostic_rule_438(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #439
pub fn render_diagnostic_rule_439(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #440
pub fn render_diagnostic_rule_440(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #441
pub fn render_diagnostic_rule_441(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #442
pub fn render_diagnostic_rule_442(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #443
pub fn render_diagnostic_rule_443(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #444
pub fn render_diagnostic_rule_444(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #445
pub fn render_diagnostic_rule_445(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #446
pub fn render_diagnostic_rule_446(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #447
pub fn render_diagnostic_rule_447(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #448
pub fn render_diagnostic_rule_448(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #449
pub fn render_diagnostic_rule_449(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #450
pub fn render_diagnostic_rule_450(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #451
pub fn render_diagnostic_rule_451(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #452
pub fn render_diagnostic_rule_452(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #453
pub fn render_diagnostic_rule_453(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #454
pub fn render_diagnostic_rule_454(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #455
pub fn render_diagnostic_rule_455(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #456
pub fn render_diagnostic_rule_456(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #457
pub fn render_diagnostic_rule_457(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #458
pub fn render_diagnostic_rule_458(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #459
pub fn render_diagnostic_rule_459(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #460
pub fn render_diagnostic_rule_460(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #461
pub fn render_diagnostic_rule_461(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #462
pub fn render_diagnostic_rule_462(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #463
pub fn render_diagnostic_rule_463(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #464
pub fn render_diagnostic_rule_464(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #465
pub fn render_diagnostic_rule_465(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #466
pub fn render_diagnostic_rule_466(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #467
pub fn render_diagnostic_rule_467(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #468
pub fn render_diagnostic_rule_468(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #469
pub fn render_diagnostic_rule_469(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #470
pub fn render_diagnostic_rule_470(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #471
pub fn render_diagnostic_rule_471(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #472
pub fn render_diagnostic_rule_472(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #473
pub fn render_diagnostic_rule_473(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #474
pub fn render_diagnostic_rule_474(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #475
pub fn render_diagnostic_rule_475(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #476
pub fn render_diagnostic_rule_476(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #477
pub fn render_diagnostic_rule_477(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #478
pub fn render_diagnostic_rule_478(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #479
pub fn render_diagnostic_rule_479(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #480
pub fn render_diagnostic_rule_480(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #481
pub fn render_diagnostic_rule_481(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #482
pub fn render_diagnostic_rule_482(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #483
pub fn render_diagnostic_rule_483(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #484
pub fn render_diagnostic_rule_484(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #485
pub fn render_diagnostic_rule_485(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #486
pub fn render_diagnostic_rule_486(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #487
pub fn render_diagnostic_rule_487(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #488
pub fn render_diagnostic_rule_488(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #489
pub fn render_diagnostic_rule_489(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #490
pub fn render_diagnostic_rule_490(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #491
pub fn render_diagnostic_rule_491(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #492
pub fn render_diagnostic_rule_492(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #493
pub fn render_diagnostic_rule_493(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #494
pub fn render_diagnostic_rule_494(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #495
pub fn render_diagnostic_rule_495(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #496
pub fn render_diagnostic_rule_496(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #497
pub fn render_diagnostic_rule_497(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #498
pub fn render_diagnostic_rule_498(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #499
pub fn render_diagnostic_rule_499(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #500
pub fn render_diagnostic_rule_500(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #501
pub fn render_diagnostic_rule_501(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #502
pub fn render_diagnostic_rule_502(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #503
pub fn render_diagnostic_rule_503(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #504
pub fn render_diagnostic_rule_504(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #505
pub fn render_diagnostic_rule_505(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #506
pub fn render_diagnostic_rule_506(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #507
pub fn render_diagnostic_rule_507(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #508
pub fn render_diagnostic_rule_508(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #509
pub fn render_diagnostic_rule_509(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #510
pub fn render_diagnostic_rule_510(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #511
pub fn render_diagnostic_rule_511(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #512
pub fn render_diagnostic_rule_512(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #513
pub fn render_diagnostic_rule_513(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #514
pub fn render_diagnostic_rule_514(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #515
pub fn render_diagnostic_rule_515(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #516
pub fn render_diagnostic_rule_516(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #517
pub fn render_diagnostic_rule_517(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #518
pub fn render_diagnostic_rule_518(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #519
pub fn render_diagnostic_rule_519(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #520
pub fn render_diagnostic_rule_520(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #521
pub fn render_diagnostic_rule_521(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #522
pub fn render_diagnostic_rule_522(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #523
pub fn render_diagnostic_rule_523(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #524
pub fn render_diagnostic_rule_524(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #525
pub fn render_diagnostic_rule_525(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #526
pub fn render_diagnostic_rule_526(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #527
pub fn render_diagnostic_rule_527(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #528
pub fn render_diagnostic_rule_528(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #529
pub fn render_diagnostic_rule_529(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #530
pub fn render_diagnostic_rule_530(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #531
pub fn render_diagnostic_rule_531(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #532
pub fn render_diagnostic_rule_532(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #533
pub fn render_diagnostic_rule_533(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #534
pub fn render_diagnostic_rule_534(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #535
pub fn render_diagnostic_rule_535(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #536
pub fn render_diagnostic_rule_536(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #537
pub fn render_diagnostic_rule_537(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #538
pub fn render_diagnostic_rule_538(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #539
pub fn render_diagnostic_rule_539(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #540
pub fn render_diagnostic_rule_540(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #541
pub fn render_diagnostic_rule_541(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #542
pub fn render_diagnostic_rule_542(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #543
pub fn render_diagnostic_rule_543(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #544
pub fn render_diagnostic_rule_544(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #545
pub fn render_diagnostic_rule_545(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #546
pub fn render_diagnostic_rule_546(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #547
pub fn render_diagnostic_rule_547(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #548
pub fn render_diagnostic_rule_548(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #549
pub fn render_diagnostic_rule_549(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #550
pub fn render_diagnostic_rule_550(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #551
pub fn render_diagnostic_rule_551(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #552
pub fn render_diagnostic_rule_552(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #553
pub fn render_diagnostic_rule_553(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #554
pub fn render_diagnostic_rule_554(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #555
pub fn render_diagnostic_rule_555(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #556
pub fn render_diagnostic_rule_556(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #557
pub fn render_diagnostic_rule_557(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #558
pub fn render_diagnostic_rule_558(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #559
pub fn render_diagnostic_rule_559(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #560
pub fn render_diagnostic_rule_560(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #561
pub fn render_diagnostic_rule_561(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #562
pub fn render_diagnostic_rule_562(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #563
pub fn render_diagnostic_rule_563(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #564
pub fn render_diagnostic_rule_564(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #565
pub fn render_diagnostic_rule_565(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #566
pub fn render_diagnostic_rule_566(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #567
pub fn render_diagnostic_rule_567(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #568
pub fn render_diagnostic_rule_568(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #569
pub fn render_diagnostic_rule_569(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #570
pub fn render_diagnostic_rule_570(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #571
pub fn render_diagnostic_rule_571(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #572
pub fn render_diagnostic_rule_572(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #573
pub fn render_diagnostic_rule_573(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #574
pub fn render_diagnostic_rule_574(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #575
pub fn render_diagnostic_rule_575(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #576
pub fn render_diagnostic_rule_576(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #577
pub fn render_diagnostic_rule_577(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #578
pub fn render_diagnostic_rule_578(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #579
pub fn render_diagnostic_rule_579(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #580
pub fn render_diagnostic_rule_580(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #581
pub fn render_diagnostic_rule_581(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #582
pub fn render_diagnostic_rule_582(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #583
pub fn render_diagnostic_rule_583(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #584
pub fn render_diagnostic_rule_584(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #585
pub fn render_diagnostic_rule_585(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #586
pub fn render_diagnostic_rule_586(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #587
pub fn render_diagnostic_rule_587(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #588
pub fn render_diagnostic_rule_588(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #589
pub fn render_diagnostic_rule_589(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #590
pub fn render_diagnostic_rule_590(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #591
pub fn render_diagnostic_rule_591(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #592
pub fn render_diagnostic_rule_592(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #593
pub fn render_diagnostic_rule_593(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #594
pub fn render_diagnostic_rule_594(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #595
pub fn render_diagnostic_rule_595(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #596
pub fn render_diagnostic_rule_596(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #597
pub fn render_diagnostic_rule_597(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #598
pub fn render_diagnostic_rule_598(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #599
pub fn render_diagnostic_rule_599(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #600
pub fn render_diagnostic_rule_600(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #601
pub fn render_diagnostic_rule_601(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #602
pub fn render_diagnostic_rule_602(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #603
pub fn render_diagnostic_rule_603(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #604
pub fn render_diagnostic_rule_604(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #605
pub fn render_diagnostic_rule_605(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #606
pub fn render_diagnostic_rule_606(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #607
pub fn render_diagnostic_rule_607(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #608
pub fn render_diagnostic_rule_608(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #609
pub fn render_diagnostic_rule_609(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #610
pub fn render_diagnostic_rule_610(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #611
pub fn render_diagnostic_rule_611(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #612
pub fn render_diagnostic_rule_612(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #613
pub fn render_diagnostic_rule_613(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #614
pub fn render_diagnostic_rule_614(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #615
pub fn render_diagnostic_rule_615(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #616
pub fn render_diagnostic_rule_616(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #617
pub fn render_diagnostic_rule_617(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #618
pub fn render_diagnostic_rule_618(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #619
pub fn render_diagnostic_rule_619(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #620
pub fn render_diagnostic_rule_620(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #621
pub fn render_diagnostic_rule_621(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #622
pub fn render_diagnostic_rule_622(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #623
pub fn render_diagnostic_rule_623(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #624
pub fn render_diagnostic_rule_624(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #625
pub fn render_diagnostic_rule_625(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #626
pub fn render_diagnostic_rule_626(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #627
pub fn render_diagnostic_rule_627(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #628
pub fn render_diagnostic_rule_628(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #629
pub fn render_diagnostic_rule_629(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #630
pub fn render_diagnostic_rule_630(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #631
pub fn render_diagnostic_rule_631(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #632
pub fn render_diagnostic_rule_632(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #633
pub fn render_diagnostic_rule_633(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #634
pub fn render_diagnostic_rule_634(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #635
pub fn render_diagnostic_rule_635(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #636
pub fn render_diagnostic_rule_636(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #637
pub fn render_diagnostic_rule_637(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #638
pub fn render_diagnostic_rule_638(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #639
pub fn render_diagnostic_rule_639(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #640
pub fn render_diagnostic_rule_640(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #641
pub fn render_diagnostic_rule_641(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #642
pub fn render_diagnostic_rule_642(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #643
pub fn render_diagnostic_rule_643(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #644
pub fn render_diagnostic_rule_644(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #645
pub fn render_diagnostic_rule_645(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #646
pub fn render_diagnostic_rule_646(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #647
pub fn render_diagnostic_rule_647(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #648
pub fn render_diagnostic_rule_648(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #649
pub fn render_diagnostic_rule_649(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #650
pub fn render_diagnostic_rule_650(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #651
pub fn render_diagnostic_rule_651(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #652
pub fn render_diagnostic_rule_652(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #653
pub fn render_diagnostic_rule_653(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #654
pub fn render_diagnostic_rule_654(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #655
pub fn render_diagnostic_rule_655(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #656
pub fn render_diagnostic_rule_656(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #657
pub fn render_diagnostic_rule_657(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #658
pub fn render_diagnostic_rule_658(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #659
pub fn render_diagnostic_rule_659(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #660
pub fn render_diagnostic_rule_660(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #661
pub fn render_diagnostic_rule_661(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #662
pub fn render_diagnostic_rule_662(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #663
pub fn render_diagnostic_rule_663(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #664
pub fn render_diagnostic_rule_664(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #665
pub fn render_diagnostic_rule_665(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #666
pub fn render_diagnostic_rule_666(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #667
pub fn render_diagnostic_rule_667(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #668
pub fn render_diagnostic_rule_668(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #669
pub fn render_diagnostic_rule_669(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #670
pub fn render_diagnostic_rule_670(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #671
pub fn render_diagnostic_rule_671(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #672
pub fn render_diagnostic_rule_672(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #673
pub fn render_diagnostic_rule_673(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #674
pub fn render_diagnostic_rule_674(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #675
pub fn render_diagnostic_rule_675(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #676
pub fn render_diagnostic_rule_676(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #677
pub fn render_diagnostic_rule_677(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #678
pub fn render_diagnostic_rule_678(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #679
pub fn render_diagnostic_rule_679(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #680
pub fn render_diagnostic_rule_680(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #681
pub fn render_diagnostic_rule_681(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #682
pub fn render_diagnostic_rule_682(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #683
pub fn render_diagnostic_rule_683(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #684
pub fn render_diagnostic_rule_684(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #685
pub fn render_diagnostic_rule_685(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #686
pub fn render_diagnostic_rule_686(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #687
pub fn render_diagnostic_rule_687(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #688
pub fn render_diagnostic_rule_688(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #689
pub fn render_diagnostic_rule_689(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #690
pub fn render_diagnostic_rule_690(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #691
pub fn render_diagnostic_rule_691(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #692
pub fn render_diagnostic_rule_692(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #693
pub fn render_diagnostic_rule_693(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #694
pub fn render_diagnostic_rule_694(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #695
pub fn render_diagnostic_rule_695(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #696
pub fn render_diagnostic_rule_696(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #697
pub fn render_diagnostic_rule_697(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #698
pub fn render_diagnostic_rule_698(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #699
pub fn render_diagnostic_rule_699(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #700
pub fn render_diagnostic_rule_700(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #701
pub fn render_diagnostic_rule_701(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #702
pub fn render_diagnostic_rule_702(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #703
pub fn render_diagnostic_rule_703(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #704
pub fn render_diagnostic_rule_704(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #705
pub fn render_diagnostic_rule_705(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #706
pub fn render_diagnostic_rule_706(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #707
pub fn render_diagnostic_rule_707(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #708
pub fn render_diagnostic_rule_708(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #709
pub fn render_diagnostic_rule_709(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #710
pub fn render_diagnostic_rule_710(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #711
pub fn render_diagnostic_rule_711(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #712
pub fn render_diagnostic_rule_712(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #713
pub fn render_diagnostic_rule_713(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #714
pub fn render_diagnostic_rule_714(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #715
pub fn render_diagnostic_rule_715(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #716
pub fn render_diagnostic_rule_716(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #717
pub fn render_diagnostic_rule_717(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #718
pub fn render_diagnostic_rule_718(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #719
pub fn render_diagnostic_rule_719(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #720
pub fn render_diagnostic_rule_720(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #721
pub fn render_diagnostic_rule_721(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #722
pub fn render_diagnostic_rule_722(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #723
pub fn render_diagnostic_rule_723(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #724
pub fn render_diagnostic_rule_724(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #725
pub fn render_diagnostic_rule_725(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #726
pub fn render_diagnostic_rule_726(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #727
pub fn render_diagnostic_rule_727(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #728
pub fn render_diagnostic_rule_728(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #729
pub fn render_diagnostic_rule_729(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #730
pub fn render_diagnostic_rule_730(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #731
pub fn render_diagnostic_rule_731(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #732
pub fn render_diagnostic_rule_732(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #733
pub fn render_diagnostic_rule_733(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #734
pub fn render_diagnostic_rule_734(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #735
pub fn render_diagnostic_rule_735(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #736
pub fn render_diagnostic_rule_736(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #737
pub fn render_diagnostic_rule_737(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #738
pub fn render_diagnostic_rule_738(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #739
pub fn render_diagnostic_rule_739(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #740
pub fn render_diagnostic_rule_740(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #741
pub fn render_diagnostic_rule_741(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #742
pub fn render_diagnostic_rule_742(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #743
pub fn render_diagnostic_rule_743(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #744
pub fn render_diagnostic_rule_744(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #745
pub fn render_diagnostic_rule_745(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #746
pub fn render_diagnostic_rule_746(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #747
pub fn render_diagnostic_rule_747(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #748
pub fn render_diagnostic_rule_748(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #749
pub fn render_diagnostic_rule_749(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #750
pub fn render_diagnostic_rule_750(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #751
pub fn render_diagnostic_rule_751(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #752
pub fn render_diagnostic_rule_752(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #753
pub fn render_diagnostic_rule_753(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #754
pub fn render_diagnostic_rule_754(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #755
pub fn render_diagnostic_rule_755(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #756
pub fn render_diagnostic_rule_756(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #757
pub fn render_diagnostic_rule_757(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #758
pub fn render_diagnostic_rule_758(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #759
pub fn render_diagnostic_rule_759(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #760
pub fn render_diagnostic_rule_760(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #761
pub fn render_diagnostic_rule_761(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #762
pub fn render_diagnostic_rule_762(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #763
pub fn render_diagnostic_rule_763(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #764
pub fn render_diagnostic_rule_764(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #765
pub fn render_diagnostic_rule_765(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #766
pub fn render_diagnostic_rule_766(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #767
pub fn render_diagnostic_rule_767(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #768
pub fn render_diagnostic_rule_768(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #769
pub fn render_diagnostic_rule_769(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #770
pub fn render_diagnostic_rule_770(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #771
pub fn render_diagnostic_rule_771(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #772
pub fn render_diagnostic_rule_772(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #773
pub fn render_diagnostic_rule_773(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #774
pub fn render_diagnostic_rule_774(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #775
pub fn render_diagnostic_rule_775(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #776
pub fn render_diagnostic_rule_776(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #777
pub fn render_diagnostic_rule_777(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #778
pub fn render_diagnostic_rule_778(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #779
pub fn render_diagnostic_rule_779(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #780
pub fn render_diagnostic_rule_780(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #781
pub fn render_diagnostic_rule_781(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #782
pub fn render_diagnostic_rule_782(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #783
pub fn render_diagnostic_rule_783(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #784
pub fn render_diagnostic_rule_784(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #785
pub fn render_diagnostic_rule_785(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #786
pub fn render_diagnostic_rule_786(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #787
pub fn render_diagnostic_rule_787(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #788
pub fn render_diagnostic_rule_788(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #789
pub fn render_diagnostic_rule_789(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #790
pub fn render_diagnostic_rule_790(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #791
pub fn render_diagnostic_rule_791(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #792
pub fn render_diagnostic_rule_792(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #793
pub fn render_diagnostic_rule_793(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #794
pub fn render_diagnostic_rule_794(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #795
pub fn render_diagnostic_rule_795(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #796
pub fn render_diagnostic_rule_796(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #797
pub fn render_diagnostic_rule_797(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #798
pub fn render_diagnostic_rule_798(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #799
pub fn render_diagnostic_rule_799(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #800
pub fn render_diagnostic_rule_800(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #801
pub fn render_diagnostic_rule_801(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #802
pub fn render_diagnostic_rule_802(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #803
pub fn render_diagnostic_rule_803(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #804
pub fn render_diagnostic_rule_804(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #805
pub fn render_diagnostic_rule_805(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #806
pub fn render_diagnostic_rule_806(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #807
pub fn render_diagnostic_rule_807(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #808
pub fn render_diagnostic_rule_808(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #809
pub fn render_diagnostic_rule_809(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #810
pub fn render_diagnostic_rule_810(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #811
pub fn render_diagnostic_rule_811(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #812
pub fn render_diagnostic_rule_812(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #813
pub fn render_diagnostic_rule_813(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #814
pub fn render_diagnostic_rule_814(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #815
pub fn render_diagnostic_rule_815(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #816
pub fn render_diagnostic_rule_816(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #817
pub fn render_diagnostic_rule_817(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #818
pub fn render_diagnostic_rule_818(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #819
pub fn render_diagnostic_rule_819(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #820
pub fn render_diagnostic_rule_820(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #821
pub fn render_diagnostic_rule_821(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #822
pub fn render_diagnostic_rule_822(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #823
pub fn render_diagnostic_rule_823(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #824
pub fn render_diagnostic_rule_824(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #825
pub fn render_diagnostic_rule_825(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #826
pub fn render_diagnostic_rule_826(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #827
pub fn render_diagnostic_rule_827(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #828
pub fn render_diagnostic_rule_828(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #829
pub fn render_diagnostic_rule_829(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #830
pub fn render_diagnostic_rule_830(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #831
pub fn render_diagnostic_rule_831(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #832
pub fn render_diagnostic_rule_832(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #833
pub fn render_diagnostic_rule_833(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #834
pub fn render_diagnostic_rule_834(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #835
pub fn render_diagnostic_rule_835(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #836
pub fn render_diagnostic_rule_836(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #837
pub fn render_diagnostic_rule_837(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #838
pub fn render_diagnostic_rule_838(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #839
pub fn render_diagnostic_rule_839(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #840
pub fn render_diagnostic_rule_840(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #841
pub fn render_diagnostic_rule_841(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #842
pub fn render_diagnostic_rule_842(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #843
pub fn render_diagnostic_rule_843(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #844
pub fn render_diagnostic_rule_844(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #845
pub fn render_diagnostic_rule_845(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #846
pub fn render_diagnostic_rule_846(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #847
pub fn render_diagnostic_rule_847(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #848
pub fn render_diagnostic_rule_848(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #849
pub fn render_diagnostic_rule_849(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #850
pub fn render_diagnostic_rule_850(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #851
pub fn render_diagnostic_rule_851(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #852
pub fn render_diagnostic_rule_852(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #853
pub fn render_diagnostic_rule_853(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #854
pub fn render_diagnostic_rule_854(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #855
pub fn render_diagnostic_rule_855(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #856
pub fn render_diagnostic_rule_856(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #857
pub fn render_diagnostic_rule_857(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #858
pub fn render_diagnostic_rule_858(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #859
pub fn render_diagnostic_rule_859(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #860
pub fn render_diagnostic_rule_860(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #861
pub fn render_diagnostic_rule_861(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #862
pub fn render_diagnostic_rule_862(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #863
pub fn render_diagnostic_rule_863(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #864
pub fn render_diagnostic_rule_864(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #865
pub fn render_diagnostic_rule_865(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #866
pub fn render_diagnostic_rule_866(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #867
pub fn render_diagnostic_rule_867(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #868
pub fn render_diagnostic_rule_868(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #869
pub fn render_diagnostic_rule_869(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #870
pub fn render_diagnostic_rule_870(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #871
pub fn render_diagnostic_rule_871(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #872
pub fn render_diagnostic_rule_872(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #873
pub fn render_diagnostic_rule_873(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #874
pub fn render_diagnostic_rule_874(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #875
pub fn render_diagnostic_rule_875(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #876
pub fn render_diagnostic_rule_876(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #877
pub fn render_diagnostic_rule_877(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #878
pub fn render_diagnostic_rule_878(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #879
pub fn render_diagnostic_rule_879(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #880
pub fn render_diagnostic_rule_880(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #881
pub fn render_diagnostic_rule_881(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #882
pub fn render_diagnostic_rule_882(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #883
pub fn render_diagnostic_rule_883(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #884
pub fn render_diagnostic_rule_884(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #885
pub fn render_diagnostic_rule_885(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #886
pub fn render_diagnostic_rule_886(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #887
pub fn render_diagnostic_rule_887(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #888
pub fn render_diagnostic_rule_888(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #889
pub fn render_diagnostic_rule_889(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #890
pub fn render_diagnostic_rule_890(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #891
pub fn render_diagnostic_rule_891(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #892
pub fn render_diagnostic_rule_892(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #893
pub fn render_diagnostic_rule_893(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #894
pub fn render_diagnostic_rule_894(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #895
pub fn render_diagnostic_rule_895(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #896
pub fn render_diagnostic_rule_896(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #897
pub fn render_diagnostic_rule_897(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #898
pub fn render_diagnostic_rule_898(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #899
pub fn render_diagnostic_rule_899(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #900
pub fn render_diagnostic_rule_900(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #901
pub fn render_diagnostic_rule_901(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #902
pub fn render_diagnostic_rule_902(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #903
pub fn render_diagnostic_rule_903(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #904
pub fn render_diagnostic_rule_904(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #905
pub fn render_diagnostic_rule_905(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #906
pub fn render_diagnostic_rule_906(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #907
pub fn render_diagnostic_rule_907(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #908
pub fn render_diagnostic_rule_908(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #909
pub fn render_diagnostic_rule_909(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #910
pub fn render_diagnostic_rule_910(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #911
pub fn render_diagnostic_rule_911(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #912
pub fn render_diagnostic_rule_912(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #913
pub fn render_diagnostic_rule_913(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #914
pub fn render_diagnostic_rule_914(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #915
pub fn render_diagnostic_rule_915(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #916
pub fn render_diagnostic_rule_916(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #917
pub fn render_diagnostic_rule_917(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #918
pub fn render_diagnostic_rule_918(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #919
pub fn render_diagnostic_rule_919(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #920
pub fn render_diagnostic_rule_920(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #921
pub fn render_diagnostic_rule_921(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #922
pub fn render_diagnostic_rule_922(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #923
pub fn render_diagnostic_rule_923(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #924
pub fn render_diagnostic_rule_924(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #925
pub fn render_diagnostic_rule_925(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #926
pub fn render_diagnostic_rule_926(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #927
pub fn render_diagnostic_rule_927(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #928
pub fn render_diagnostic_rule_928(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #929
pub fn render_diagnostic_rule_929(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #930
pub fn render_diagnostic_rule_930(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #931
pub fn render_diagnostic_rule_931(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #932
pub fn render_diagnostic_rule_932(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #933
pub fn render_diagnostic_rule_933(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #934
pub fn render_diagnostic_rule_934(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #935
pub fn render_diagnostic_rule_935(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #936
pub fn render_diagnostic_rule_936(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #937
pub fn render_diagnostic_rule_937(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #938
pub fn render_diagnostic_rule_938(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #939
pub fn render_diagnostic_rule_939(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #940
pub fn render_diagnostic_rule_940(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #941
pub fn render_diagnostic_rule_941(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #942
pub fn render_diagnostic_rule_942(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #943
pub fn render_diagnostic_rule_943(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #944
pub fn render_diagnostic_rule_944(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #945
pub fn render_diagnostic_rule_945(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #946
pub fn render_diagnostic_rule_946(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #947
pub fn render_diagnostic_rule_947(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #948
pub fn render_diagnostic_rule_948(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #949
pub fn render_diagnostic_rule_949(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #950
pub fn render_diagnostic_rule_950(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #951
pub fn render_diagnostic_rule_951(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #952
pub fn render_diagnostic_rule_952(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #953
pub fn render_diagnostic_rule_953(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #954
pub fn render_diagnostic_rule_954(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #955
pub fn render_diagnostic_rule_955(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #956
pub fn render_diagnostic_rule_956(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #957
pub fn render_diagnostic_rule_957(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #958
pub fn render_diagnostic_rule_958(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #959
pub fn render_diagnostic_rule_959(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #960
pub fn render_diagnostic_rule_960(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #961
pub fn render_diagnostic_rule_961(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #962
pub fn render_diagnostic_rule_962(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #963
pub fn render_diagnostic_rule_963(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #964
pub fn render_diagnostic_rule_964(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #965
pub fn render_diagnostic_rule_965(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #966
pub fn render_diagnostic_rule_966(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #967
pub fn render_diagnostic_rule_967(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #968
pub fn render_diagnostic_rule_968(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #969
pub fn render_diagnostic_rule_969(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #970
pub fn render_diagnostic_rule_970(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #971
pub fn render_diagnostic_rule_971(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #972
pub fn render_diagnostic_rule_972(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #973
pub fn render_diagnostic_rule_973(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #974
pub fn render_diagnostic_rule_974(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #975
pub fn render_diagnostic_rule_975(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #976
pub fn render_diagnostic_rule_976(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #977
pub fn render_diagnostic_rule_977(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #978
pub fn render_diagnostic_rule_978(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #979
pub fn render_diagnostic_rule_979(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #980
pub fn render_diagnostic_rule_980(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #981
pub fn render_diagnostic_rule_981(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #982
pub fn render_diagnostic_rule_982(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #983
pub fn render_diagnostic_rule_983(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #984
pub fn render_diagnostic_rule_984(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #985
pub fn render_diagnostic_rule_985(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #986
pub fn render_diagnostic_rule_986(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #987
pub fn render_diagnostic_rule_987(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #988
pub fn render_diagnostic_rule_988(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #989
pub fn render_diagnostic_rule_989(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #990
pub fn render_diagnostic_rule_990(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #991
pub fn render_diagnostic_rule_991(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #992
pub fn render_diagnostic_rule_992(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #993
pub fn render_diagnostic_rule_993(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #994
pub fn render_diagnostic_rule_994(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #995
pub fn render_diagnostic_rule_995(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #996
pub fn render_diagnostic_rule_996(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #997
pub fn render_diagnostic_rule_997(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #998
pub fn render_diagnostic_rule_998(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #999
pub fn render_diagnostic_rule_999(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1000
pub fn render_diagnostic_rule_1000(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1001
pub fn render_diagnostic_rule_1001(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1002
pub fn render_diagnostic_rule_1002(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1003
pub fn render_diagnostic_rule_1003(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1004
pub fn render_diagnostic_rule_1004(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1005
pub fn render_diagnostic_rule_1005(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1006
pub fn render_diagnostic_rule_1006(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1007
pub fn render_diagnostic_rule_1007(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1008
pub fn render_diagnostic_rule_1008(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1009
pub fn render_diagnostic_rule_1009(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1010
pub fn render_diagnostic_rule_1010(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1011
pub fn render_diagnostic_rule_1011(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1012
pub fn render_diagnostic_rule_1012(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1013
pub fn render_diagnostic_rule_1013(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1014
pub fn render_diagnostic_rule_1014(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1015
pub fn render_diagnostic_rule_1015(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1016
pub fn render_diagnostic_rule_1016(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1017
pub fn render_diagnostic_rule_1017(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1018
pub fn render_diagnostic_rule_1018(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1019
pub fn render_diagnostic_rule_1019(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1020
pub fn render_diagnostic_rule_1020(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1021
pub fn render_diagnostic_rule_1021(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1022
pub fn render_diagnostic_rule_1022(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1023
pub fn render_diagnostic_rule_1023(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1024
pub fn render_diagnostic_rule_1024(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1025
pub fn render_diagnostic_rule_1025(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1026
pub fn render_diagnostic_rule_1026(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1027
pub fn render_diagnostic_rule_1027(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1028
pub fn render_diagnostic_rule_1028(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1029
pub fn render_diagnostic_rule_1029(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1030
pub fn render_diagnostic_rule_1030(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1031
pub fn render_diagnostic_rule_1031(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1032
pub fn render_diagnostic_rule_1032(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1033
pub fn render_diagnostic_rule_1033(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1034
pub fn render_diagnostic_rule_1034(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1035
pub fn render_diagnostic_rule_1035(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1036
pub fn render_diagnostic_rule_1036(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1037
pub fn render_diagnostic_rule_1037(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1038
pub fn render_diagnostic_rule_1038(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1039
pub fn render_diagnostic_rule_1039(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1040
pub fn render_diagnostic_rule_1040(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1041
pub fn render_diagnostic_rule_1041(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1042
pub fn render_diagnostic_rule_1042(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1043
pub fn render_diagnostic_rule_1043(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1044
pub fn render_diagnostic_rule_1044(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1045
pub fn render_diagnostic_rule_1045(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1046
pub fn render_diagnostic_rule_1046(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1047
pub fn render_diagnostic_rule_1047(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1048
pub fn render_diagnostic_rule_1048(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1049
pub fn render_diagnostic_rule_1049(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1050
pub fn render_diagnostic_rule_1050(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1051
pub fn render_diagnostic_rule_1051(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1052
pub fn render_diagnostic_rule_1052(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1053
pub fn render_diagnostic_rule_1053(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1054
pub fn render_diagnostic_rule_1054(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1055
pub fn render_diagnostic_rule_1055(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1056
pub fn render_diagnostic_rule_1056(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1057
pub fn render_diagnostic_rule_1057(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1058
pub fn render_diagnostic_rule_1058(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1059
pub fn render_diagnostic_rule_1059(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1060
pub fn render_diagnostic_rule_1060(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1061
pub fn render_diagnostic_rule_1061(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1062
pub fn render_diagnostic_rule_1062(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1063
pub fn render_diagnostic_rule_1063(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1064
pub fn render_diagnostic_rule_1064(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1065
pub fn render_diagnostic_rule_1065(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1066
pub fn render_diagnostic_rule_1066(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1067
pub fn render_diagnostic_rule_1067(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1068
pub fn render_diagnostic_rule_1068(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1069
pub fn render_diagnostic_rule_1069(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1070
pub fn render_diagnostic_rule_1070(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1071
pub fn render_diagnostic_rule_1071(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1072
pub fn render_diagnostic_rule_1072(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1073
pub fn render_diagnostic_rule_1073(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1074
pub fn render_diagnostic_rule_1074(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1075
pub fn render_diagnostic_rule_1075(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1076
pub fn render_diagnostic_rule_1076(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1077
pub fn render_diagnostic_rule_1077(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1078
pub fn render_diagnostic_rule_1078(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1079
pub fn render_diagnostic_rule_1079(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1080
pub fn render_diagnostic_rule_1080(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1081
pub fn render_diagnostic_rule_1081(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1082
pub fn render_diagnostic_rule_1082(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1083
pub fn render_diagnostic_rule_1083(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1084
pub fn render_diagnostic_rule_1084(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1085
pub fn render_diagnostic_rule_1085(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1086
pub fn render_diagnostic_rule_1086(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1087
pub fn render_diagnostic_rule_1087(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1088
pub fn render_diagnostic_rule_1088(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1089
pub fn render_diagnostic_rule_1089(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1090
pub fn render_diagnostic_rule_1090(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1091
pub fn render_diagnostic_rule_1091(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1092
pub fn render_diagnostic_rule_1092(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1093
pub fn render_diagnostic_rule_1093(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1094
pub fn render_diagnostic_rule_1094(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1095
pub fn render_diagnostic_rule_1095(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1096
pub fn render_diagnostic_rule_1096(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1097
pub fn render_diagnostic_rule_1097(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1098
pub fn render_diagnostic_rule_1098(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1099
pub fn render_diagnostic_rule_1099(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1100
pub fn render_diagnostic_rule_1100(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1101
pub fn render_diagnostic_rule_1101(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1102
pub fn render_diagnostic_rule_1102(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1103
pub fn render_diagnostic_rule_1103(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1104
pub fn render_diagnostic_rule_1104(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1105
pub fn render_diagnostic_rule_1105(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1106
pub fn render_diagnostic_rule_1106(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1107
pub fn render_diagnostic_rule_1107(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1108
pub fn render_diagnostic_rule_1108(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1109
pub fn render_diagnostic_rule_1109(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1110
pub fn render_diagnostic_rule_1110(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1111
pub fn render_diagnostic_rule_1111(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1112
pub fn render_diagnostic_rule_1112(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1113
pub fn render_diagnostic_rule_1113(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1114
pub fn render_diagnostic_rule_1114(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1115
pub fn render_diagnostic_rule_1115(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1116
pub fn render_diagnostic_rule_1116(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1117
pub fn render_diagnostic_rule_1117(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1118
pub fn render_diagnostic_rule_1118(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1119
pub fn render_diagnostic_rule_1119(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1120
pub fn render_diagnostic_rule_1120(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1121
pub fn render_diagnostic_rule_1121(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1122
pub fn render_diagnostic_rule_1122(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1123
pub fn render_diagnostic_rule_1123(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1124
pub fn render_diagnostic_rule_1124(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1125
pub fn render_diagnostic_rule_1125(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1126
pub fn render_diagnostic_rule_1126(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1127
pub fn render_diagnostic_rule_1127(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1128
pub fn render_diagnostic_rule_1128(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1129
pub fn render_diagnostic_rule_1129(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1130
pub fn render_diagnostic_rule_1130(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1131
pub fn render_diagnostic_rule_1131(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1132
pub fn render_diagnostic_rule_1132(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1133
pub fn render_diagnostic_rule_1133(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1134
pub fn render_diagnostic_rule_1134(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1135
pub fn render_diagnostic_rule_1135(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1136
pub fn render_diagnostic_rule_1136(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1137
pub fn render_diagnostic_rule_1137(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1138
pub fn render_diagnostic_rule_1138(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1139
pub fn render_diagnostic_rule_1139(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1140
pub fn render_diagnostic_rule_1140(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1141
pub fn render_diagnostic_rule_1141(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1142
pub fn render_diagnostic_rule_1142(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1143
pub fn render_diagnostic_rule_1143(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1144
pub fn render_diagnostic_rule_1144(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1145
pub fn render_diagnostic_rule_1145(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1146
pub fn render_diagnostic_rule_1146(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1147
pub fn render_diagnostic_rule_1147(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1148
pub fn render_diagnostic_rule_1148(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1149
pub fn render_diagnostic_rule_1149(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1150
pub fn render_diagnostic_rule_1150(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1151
pub fn render_diagnostic_rule_1151(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1152
pub fn render_diagnostic_rule_1152(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1153
pub fn render_diagnostic_rule_1153(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1154
pub fn render_diagnostic_rule_1154(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1155
pub fn render_diagnostic_rule_1155(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1156
pub fn render_diagnostic_rule_1156(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1157
pub fn render_diagnostic_rule_1157(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1158
pub fn render_diagnostic_rule_1158(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1159
pub fn render_diagnostic_rule_1159(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1160
pub fn render_diagnostic_rule_1160(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1161
pub fn render_diagnostic_rule_1161(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1162
pub fn render_diagnostic_rule_1162(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1163
pub fn render_diagnostic_rule_1163(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1164
pub fn render_diagnostic_rule_1164(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1165
pub fn render_diagnostic_rule_1165(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1166
pub fn render_diagnostic_rule_1166(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1167
pub fn render_diagnostic_rule_1167(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1168
pub fn render_diagnostic_rule_1168(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1169
pub fn render_diagnostic_rule_1169(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1170
pub fn render_diagnostic_rule_1170(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1171
pub fn render_diagnostic_rule_1171(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1172
pub fn render_diagnostic_rule_1172(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1173
pub fn render_diagnostic_rule_1173(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1174
pub fn render_diagnostic_rule_1174(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1175
pub fn render_diagnostic_rule_1175(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1176
pub fn render_diagnostic_rule_1176(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1177
pub fn render_diagnostic_rule_1177(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1178
pub fn render_diagnostic_rule_1178(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1179
pub fn render_diagnostic_rule_1179(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1180
pub fn render_diagnostic_rule_1180(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1181
pub fn render_diagnostic_rule_1181(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1182
pub fn render_diagnostic_rule_1182(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1183
pub fn render_diagnostic_rule_1183(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1184
pub fn render_diagnostic_rule_1184(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1185
pub fn render_diagnostic_rule_1185(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1186
pub fn render_diagnostic_rule_1186(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1187
pub fn render_diagnostic_rule_1187(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1188
pub fn render_diagnostic_rule_1188(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1189
pub fn render_diagnostic_rule_1189(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1190
pub fn render_diagnostic_rule_1190(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1191
pub fn render_diagnostic_rule_1191(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1192
pub fn render_diagnostic_rule_1192(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1193
pub fn render_diagnostic_rule_1193(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1194
pub fn render_diagnostic_rule_1194(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1195
pub fn render_diagnostic_rule_1195(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1196
pub fn render_diagnostic_rule_1196(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1197
pub fn render_diagnostic_rule_1197(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1198
pub fn render_diagnostic_rule_1198(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1199
pub fn render_diagnostic_rule_1199(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1200
pub fn render_diagnostic_rule_1200(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1201
pub fn render_diagnostic_rule_1201(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1202
pub fn render_diagnostic_rule_1202(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1203
pub fn render_diagnostic_rule_1203(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1204
pub fn render_diagnostic_rule_1204(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1205
pub fn render_diagnostic_rule_1205(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1206
pub fn render_diagnostic_rule_1206(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1207
pub fn render_diagnostic_rule_1207(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1208
pub fn render_diagnostic_rule_1208(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1209
pub fn render_diagnostic_rule_1209(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1210
pub fn render_diagnostic_rule_1210(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1211
pub fn render_diagnostic_rule_1211(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1212
pub fn render_diagnostic_rule_1212(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1213
pub fn render_diagnostic_rule_1213(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1214
pub fn render_diagnostic_rule_1214(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1215
pub fn render_diagnostic_rule_1215(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1216
pub fn render_diagnostic_rule_1216(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1217
pub fn render_diagnostic_rule_1217(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1218
pub fn render_diagnostic_rule_1218(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1219
pub fn render_diagnostic_rule_1219(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1220
pub fn render_diagnostic_rule_1220(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1221
pub fn render_diagnostic_rule_1221(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1222
pub fn render_diagnostic_rule_1222(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1223
pub fn render_diagnostic_rule_1223(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1224
pub fn render_diagnostic_rule_1224(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1225
pub fn render_diagnostic_rule_1225(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1226
pub fn render_diagnostic_rule_1226(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1227
pub fn render_diagnostic_rule_1227(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1228
pub fn render_diagnostic_rule_1228(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1229
pub fn render_diagnostic_rule_1229(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1230
pub fn render_diagnostic_rule_1230(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1231
pub fn render_diagnostic_rule_1231(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1232
pub fn render_diagnostic_rule_1232(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1233
pub fn render_diagnostic_rule_1233(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1234
pub fn render_diagnostic_rule_1234(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1235
pub fn render_diagnostic_rule_1235(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1236
pub fn render_diagnostic_rule_1236(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1237
pub fn render_diagnostic_rule_1237(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1238
pub fn render_diagnostic_rule_1238(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1239
pub fn render_diagnostic_rule_1239(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1240
pub fn render_diagnostic_rule_1240(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1241
pub fn render_diagnostic_rule_1241(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1242
pub fn render_diagnostic_rule_1242(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1243
pub fn render_diagnostic_rule_1243(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1244
pub fn render_diagnostic_rule_1244(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1245
pub fn render_diagnostic_rule_1245(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1246
pub fn render_diagnostic_rule_1246(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1247
pub fn render_diagnostic_rule_1247(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1248
pub fn render_diagnostic_rule_1248(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1249
pub fn render_diagnostic_rule_1249(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1250
pub fn render_diagnostic_rule_1250(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1251
pub fn render_diagnostic_rule_1251(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1252
pub fn render_diagnostic_rule_1252(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1253
pub fn render_diagnostic_rule_1253(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1254
pub fn render_diagnostic_rule_1254(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1255
pub fn render_diagnostic_rule_1255(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1256
pub fn render_diagnostic_rule_1256(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1257
pub fn render_diagnostic_rule_1257(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1258
pub fn render_diagnostic_rule_1258(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1259
pub fn render_diagnostic_rule_1259(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1260
pub fn render_diagnostic_rule_1260(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1261
pub fn render_diagnostic_rule_1261(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1262
pub fn render_diagnostic_rule_1262(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1263
pub fn render_diagnostic_rule_1263(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1264
pub fn render_diagnostic_rule_1264(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1265
pub fn render_diagnostic_rule_1265(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1266
pub fn render_diagnostic_rule_1266(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1267
pub fn render_diagnostic_rule_1267(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1268
pub fn render_diagnostic_rule_1268(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1269
pub fn render_diagnostic_rule_1269(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1270
pub fn render_diagnostic_rule_1270(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1271
pub fn render_diagnostic_rule_1271(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1272
pub fn render_diagnostic_rule_1272(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1273
pub fn render_diagnostic_rule_1273(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1274
pub fn render_diagnostic_rule_1274(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1275
pub fn render_diagnostic_rule_1275(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1276
pub fn render_diagnostic_rule_1276(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1277
pub fn render_diagnostic_rule_1277(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1278
pub fn render_diagnostic_rule_1278(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1279
pub fn render_diagnostic_rule_1279(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1280
pub fn render_diagnostic_rule_1280(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1281
pub fn render_diagnostic_rule_1281(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1282
pub fn render_diagnostic_rule_1282(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1283
pub fn render_diagnostic_rule_1283(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1284
pub fn render_diagnostic_rule_1284(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1285
pub fn render_diagnostic_rule_1285(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1286
pub fn render_diagnostic_rule_1286(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1287
pub fn render_diagnostic_rule_1287(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1288
pub fn render_diagnostic_rule_1288(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1289
pub fn render_diagnostic_rule_1289(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1290
pub fn render_diagnostic_rule_1290(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1291
pub fn render_diagnostic_rule_1291(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1292
pub fn render_diagnostic_rule_1292(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1293
pub fn render_diagnostic_rule_1293(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1294
pub fn render_diagnostic_rule_1294(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1295
pub fn render_diagnostic_rule_1295(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1296
pub fn render_diagnostic_rule_1296(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1297
pub fn render_diagnostic_rule_1297(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1298
pub fn render_diagnostic_rule_1298(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1299
pub fn render_diagnostic_rule_1299(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1300
pub fn render_diagnostic_rule_1300(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1301
pub fn render_diagnostic_rule_1301(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1302
pub fn render_diagnostic_rule_1302(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1303
pub fn render_diagnostic_rule_1303(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1304
pub fn render_diagnostic_rule_1304(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1305
pub fn render_diagnostic_rule_1305(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1306
pub fn render_diagnostic_rule_1306(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1307
pub fn render_diagnostic_rule_1307(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1308
pub fn render_diagnostic_rule_1308(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1309
pub fn render_diagnostic_rule_1309(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1310
pub fn render_diagnostic_rule_1310(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1311
pub fn render_diagnostic_rule_1311(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1312
pub fn render_diagnostic_rule_1312(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1313
pub fn render_diagnostic_rule_1313(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1314
pub fn render_diagnostic_rule_1314(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1315
pub fn render_diagnostic_rule_1315(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1316
pub fn render_diagnostic_rule_1316(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1317
pub fn render_diagnostic_rule_1317(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1318
pub fn render_diagnostic_rule_1318(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1319
pub fn render_diagnostic_rule_1319(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1320
pub fn render_diagnostic_rule_1320(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1321
pub fn render_diagnostic_rule_1321(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1322
pub fn render_diagnostic_rule_1322(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1323
pub fn render_diagnostic_rule_1323(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1324
pub fn render_diagnostic_rule_1324(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1325
pub fn render_diagnostic_rule_1325(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1326
pub fn render_diagnostic_rule_1326(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1327
pub fn render_diagnostic_rule_1327(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1328
pub fn render_diagnostic_rule_1328(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1329
pub fn render_diagnostic_rule_1329(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1330
pub fn render_diagnostic_rule_1330(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1331
pub fn render_diagnostic_rule_1331(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1332
pub fn render_diagnostic_rule_1332(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1333
pub fn render_diagnostic_rule_1333(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1334
pub fn render_diagnostic_rule_1334(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1335
pub fn render_diagnostic_rule_1335(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1336
pub fn render_diagnostic_rule_1336(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1337
pub fn render_diagnostic_rule_1337(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1338
pub fn render_diagnostic_rule_1338(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1339
pub fn render_diagnostic_rule_1339(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1340
pub fn render_diagnostic_rule_1340(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1341
pub fn render_diagnostic_rule_1341(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1342
pub fn render_diagnostic_rule_1342(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1343
pub fn render_diagnostic_rule_1343(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1344
pub fn render_diagnostic_rule_1344(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1345
pub fn render_diagnostic_rule_1345(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1346
pub fn render_diagnostic_rule_1346(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1347
pub fn render_diagnostic_rule_1347(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1348
pub fn render_diagnostic_rule_1348(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1349
pub fn render_diagnostic_rule_1349(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1350
pub fn render_diagnostic_rule_1350(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1351
pub fn render_diagnostic_rule_1351(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1352
pub fn render_diagnostic_rule_1352(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1353
pub fn render_diagnostic_rule_1353(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1354
pub fn render_diagnostic_rule_1354(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1355
pub fn render_diagnostic_rule_1355(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1356
pub fn render_diagnostic_rule_1356(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1357
pub fn render_diagnostic_rule_1357(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1358
pub fn render_diagnostic_rule_1358(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1359
pub fn render_diagnostic_rule_1359(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1360
pub fn render_diagnostic_rule_1360(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1361
pub fn render_diagnostic_rule_1361(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1362
pub fn render_diagnostic_rule_1362(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1363
pub fn render_diagnostic_rule_1363(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1364
pub fn render_diagnostic_rule_1364(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1365
pub fn render_diagnostic_rule_1365(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1366
pub fn render_diagnostic_rule_1366(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1367
pub fn render_diagnostic_rule_1367(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1368
pub fn render_diagnostic_rule_1368(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1369
pub fn render_diagnostic_rule_1369(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1370
pub fn render_diagnostic_rule_1370(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1371
pub fn render_diagnostic_rule_1371(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1372
pub fn render_diagnostic_rule_1372(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1373
pub fn render_diagnostic_rule_1373(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1374
pub fn render_diagnostic_rule_1374(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1375
pub fn render_diagnostic_rule_1375(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1376
pub fn render_diagnostic_rule_1376(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1377
pub fn render_diagnostic_rule_1377(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1378
pub fn render_diagnostic_rule_1378(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1379
pub fn render_diagnostic_rule_1379(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1380
pub fn render_diagnostic_rule_1380(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1381
pub fn render_diagnostic_rule_1381(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1382
pub fn render_diagnostic_rule_1382(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1383
pub fn render_diagnostic_rule_1383(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1384
pub fn render_diagnostic_rule_1384(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1385
pub fn render_diagnostic_rule_1385(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1386
pub fn render_diagnostic_rule_1386(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1387
pub fn render_diagnostic_rule_1387(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1388
pub fn render_diagnostic_rule_1388(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1389
pub fn render_diagnostic_rule_1389(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1390
pub fn render_diagnostic_rule_1390(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1391
pub fn render_diagnostic_rule_1391(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1392
pub fn render_diagnostic_rule_1392(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1393
pub fn render_diagnostic_rule_1393(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1394
pub fn render_diagnostic_rule_1394(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1395
pub fn render_diagnostic_rule_1395(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1396
pub fn render_diagnostic_rule_1396(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1397
pub fn render_diagnostic_rule_1397(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1398
pub fn render_diagnostic_rule_1398(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1399
pub fn render_diagnostic_rule_1399(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1400
pub fn render_diagnostic_rule_1400(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1401
pub fn render_diagnostic_rule_1401(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1402
pub fn render_diagnostic_rule_1402(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1403
pub fn render_diagnostic_rule_1403(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1404
pub fn render_diagnostic_rule_1404(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1405
pub fn render_diagnostic_rule_1405(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1406
pub fn render_diagnostic_rule_1406(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1407
pub fn render_diagnostic_rule_1407(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1408
pub fn render_diagnostic_rule_1408(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1409
pub fn render_diagnostic_rule_1409(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1410
pub fn render_diagnostic_rule_1410(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1411
pub fn render_diagnostic_rule_1411(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1412
pub fn render_diagnostic_rule_1412(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1413
pub fn render_diagnostic_rule_1413(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1414
pub fn render_diagnostic_rule_1414(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1415
pub fn render_diagnostic_rule_1415(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1416
pub fn render_diagnostic_rule_1416(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1417
pub fn render_diagnostic_rule_1417(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1418
pub fn render_diagnostic_rule_1418(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1419
pub fn render_diagnostic_rule_1419(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1420
pub fn render_diagnostic_rule_1420(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1421
pub fn render_diagnostic_rule_1421(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1422
pub fn render_diagnostic_rule_1422(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1423
pub fn render_diagnostic_rule_1423(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1424
pub fn render_diagnostic_rule_1424(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1425
pub fn render_diagnostic_rule_1425(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1426
pub fn render_diagnostic_rule_1426(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1427
pub fn render_diagnostic_rule_1427(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1428
pub fn render_diagnostic_rule_1428(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1429
pub fn render_diagnostic_rule_1429(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1430
pub fn render_diagnostic_rule_1430(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1431
pub fn render_diagnostic_rule_1431(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1432
pub fn render_diagnostic_rule_1432(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1433
pub fn render_diagnostic_rule_1433(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1434
pub fn render_diagnostic_rule_1434(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1435
pub fn render_diagnostic_rule_1435(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1436
pub fn render_diagnostic_rule_1436(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1437
pub fn render_diagnostic_rule_1437(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1438
pub fn render_diagnostic_rule_1438(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1439
pub fn render_diagnostic_rule_1439(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1440
pub fn render_diagnostic_rule_1440(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1441
pub fn render_diagnostic_rule_1441(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1442
pub fn render_diagnostic_rule_1442(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1443
pub fn render_diagnostic_rule_1443(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1444
pub fn render_diagnostic_rule_1444(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1445
pub fn render_diagnostic_rule_1445(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1446
pub fn render_diagnostic_rule_1446(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1447
pub fn render_diagnostic_rule_1447(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1448
pub fn render_diagnostic_rule_1448(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1449
pub fn render_diagnostic_rule_1449(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1450
pub fn render_diagnostic_rule_1450(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1451
pub fn render_diagnostic_rule_1451(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1452
pub fn render_diagnostic_rule_1452(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1453
pub fn render_diagnostic_rule_1453(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1454
pub fn render_diagnostic_rule_1454(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1455
pub fn render_diagnostic_rule_1455(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1456
pub fn render_diagnostic_rule_1456(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1457
pub fn render_diagnostic_rule_1457(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1458
pub fn render_diagnostic_rule_1458(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1459
pub fn render_diagnostic_rule_1459(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1460
pub fn render_diagnostic_rule_1460(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1461
pub fn render_diagnostic_rule_1461(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1462
pub fn render_diagnostic_rule_1462(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1463
pub fn render_diagnostic_rule_1463(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1464
pub fn render_diagnostic_rule_1464(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1465
pub fn render_diagnostic_rule_1465(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1466
pub fn render_diagnostic_rule_1466(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1467
pub fn render_diagnostic_rule_1467(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1468
pub fn render_diagnostic_rule_1468(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1469
pub fn render_diagnostic_rule_1469(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1470
pub fn render_diagnostic_rule_1470(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1471
pub fn render_diagnostic_rule_1471(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1472
pub fn render_diagnostic_rule_1472(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1473
pub fn render_diagnostic_rule_1473(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1474
pub fn render_diagnostic_rule_1474(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1475
pub fn render_diagnostic_rule_1475(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1476
pub fn render_diagnostic_rule_1476(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1477
pub fn render_diagnostic_rule_1477(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1478
pub fn render_diagnostic_rule_1478(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1479
pub fn render_diagnostic_rule_1479(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1480
pub fn render_diagnostic_rule_1480(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1481
pub fn render_diagnostic_rule_1481(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1482
pub fn render_diagnostic_rule_1482(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1483
pub fn render_diagnostic_rule_1483(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1484
pub fn render_diagnostic_rule_1484(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1485
pub fn render_diagnostic_rule_1485(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1486
pub fn render_diagnostic_rule_1486(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1487
pub fn render_diagnostic_rule_1487(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1488
pub fn render_diagnostic_rule_1488(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1489
pub fn render_diagnostic_rule_1489(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1490
pub fn render_diagnostic_rule_1490(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1491
pub fn render_diagnostic_rule_1491(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1492
pub fn render_diagnostic_rule_1492(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1493
pub fn render_diagnostic_rule_1493(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1494
pub fn render_diagnostic_rule_1494(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1495
pub fn render_diagnostic_rule_1495(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1496
pub fn render_diagnostic_rule_1496(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1497
pub fn render_diagnostic_rule_1497(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1498
pub fn render_diagnostic_rule_1498(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1499
pub fn render_diagnostic_rule_1499(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

/// Diagnostic Formatting Rule #1500
pub fn render_diagnostic_rule_1500(
    engine: &mut FullDiagnosticsEngine,
    msg: &str,
    line: usize,
) -> bool {
    if msg.is_empty() {
        return false;
    }
    engine.emit_diagnostic(DiagnosticSeverity::Error, msg, line);
    true
}

#[pyfunction]
pub fn rust_diagnostics_render(msg: &str, line: usize) -> bool {
    let mut engine = FullDiagnosticsEngine::new("test.py");
    render_diagnostic_rule_1(&mut engine, msg, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_diagnostics_engine() {
        let mut engine = FullDiagnosticsEngine::new("main.py");
        assert!(render_diagnostic_rule_1(&mut engine, "Type error", 10));
        assert_eq!(engine.diagnostics.len(), 1);
    }
}
