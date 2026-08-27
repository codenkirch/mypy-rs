#![allow(non_local_definitions)]

//! Native port of the 3-way message-selection head of
//! `mypy.typeanal.TypeAnalyser.visit_raw_expression_type`
//! (typeanal.py:2135-2150).
//!
//! A bare `RawExpressionType` is the synthetic stand-in for an expression
//! that cannot cleanly translate into a type (e.g. `Foo[1]` or the `3j`
//! inside `Foo[3j]`). The visitor reports one of three errors, chosen by
//! which builtin primitive backed the raw expression:
//!
//! - `builtins.int` / `builtins.bool` -> "try using Literal[...]";
//! - `builtins.float` / `builtins.complex` -> "literals cannot be used as a
//!   type";
//! - anything else -> "Invalid type comment or annotation".
//!
//! Rust owns only the set-membership branch and returns a message tag; the
//! Python shim formats the message (needs the live `t` for `literal_value`
//! and `simple_name()`), applies `self.fail`, conditionally applies
//! `self.note` when `t.note is not None`, and builds the trailing
//! `AnyType(TypeOfAny.from_error, ...)`. When `report_invalid_types` is
//! false the whole head is skipped, so Rust defers (`None`) and the shim's
//! pure-Python `if` falls through unchanged.

use pyo3::prelude::*;

// Message tags handed to the Python shim. Each maps to exactly one terminal
// branch of `visit_raw_expression_type`; the comment cites the typeanal.py
// line whose message the shim must format.
const TAG_RAW_EXPR_LITERAL: i64 = 0; // 2136-2139 int/bool -> Literal[...]
const TAG_RAW_EXPR_NUMERIC_LITERALS: i64 = 1; // 2140-2142 float/complex
const TAG_RAW_EXPR_GENERIC: i64 = 2; // 2143-2150 generic message

/// `visit_raw_expression_type` message classifier. Mirrors the branch order
/// of typeanal.py:2135-2150 and returns the terminal tag; `None` defers to
/// the pure-Python body (kernel off, gate off, or `report_invalid_types`
/// false, in which case no message is emitted at all).
///
/// Facts (all scalars / strings, no live type objects):
/// - `report_invalid_types`: the analyzer flag gating the whole head.
/// - `base_type_name`: `t.base_type_name`.
/// - `note_is_none`: `t.note is None`; accepted for signature parity with
///   the other typeanal classifiers, but the `self.note` side effect stays
///   Python-side and the shim re-checks `t.note` on the live object.
#[pyfunction]
pub(crate) fn rust_classify_raw_expression_type(
    report_invalid_types: bool,
    base_type_name: String,
    note_is_none: bool,
) -> PyResult<Option<i64>> {
    // The note decision is a Python-side side effect (`self.note`); the shim
    // re-reads `t.note` on the live object so this fact never decides a tag.
    let _ = note_is_none;

    if !report_invalid_types {
        return Ok(None);
    }
    let tag = if base_type_name == "builtins.int" || base_type_name == "builtins.bool" {
        TAG_RAW_EXPR_LITERAL
    } else if base_type_name == "builtins.float" || base_type_name == "builtins.complex" {
        TAG_RAW_EXPR_NUMERIC_LITERALS
    } else {
        TAG_RAW_EXPR_GENERIC
    };
    Ok(Some(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(
        report_invalid_types: bool,
        base_type_name: &str,
        note_is_none: bool,
    ) -> Option<i64> {
        rust_classify_raw_expression_type(
            report_invalid_types,
            base_type_name.to_string(),
            note_is_none,
        )
        .unwrap()
    }

    #[test]
    fn test_classify_raw_expression_int() {
        assert_eq!(
            classify(true, "builtins.int", true),
            Some(TAG_RAW_EXPR_LITERAL)
        );
    }

    #[test]
    fn test_classify_raw_expression_bool() {
        assert_eq!(
            classify(true, "builtins.bool", false),
            Some(TAG_RAW_EXPR_LITERAL)
        );
    }

    #[test]
    fn test_classify_raw_expression_float() {
        assert_eq!(
            classify(true, "builtins.float", true),
            Some(TAG_RAW_EXPR_NUMERIC_LITERALS)
        );
    }

    #[test]
    fn test_classify_raw_expression_complex() {
        assert_eq!(
            classify(true, "builtins.complex", false),
            Some(TAG_RAW_EXPR_NUMERIC_LITERALS)
        );
    }

    #[test]
    fn test_classify_raw_expression_generic() {
        assert_eq!(
            classify(true, "builtins.str", true),
            Some(TAG_RAW_EXPR_GENERIC)
        );
    }

    #[test]
    fn test_classify_raw_expression_report_off_defers() {
        // report_invalid_types False skips the whole head; defer so the shim's
        // pure-Python `if` falls through to the trailing AnyType unchanged.
        assert_eq!(classify(false, "builtins.int", true), None);
    }
}
