//! Native port of the 9-way dispatch head of
//! `mypy.typeanal.TypeAnalyser.analyze_literal_param`
//! (typeanal.py:2474-2557).
//!
//! The method dispatches each parameter of `Literal[...]` through a
//! fixed elif-chain. Every branch's *decision* is a pure function of
//! scalar facts (type-kind booleans, `original_str_expr`, `type_of_any`,
//! `literal_value`, `simple_name`, `last_known_value`). This module owns
//! that decision table and returns a single branch tag; the Python shim
//! applies the side effects (error messages, `LiteralType` construction,
//! `visit_unbound_type` recursion, union merge) exactly as the original
//! body does.
//!
//! The classifier never returns `None` (defer): every reachable path in
//! the original elif-chain maps to a tag. The Python shim owns the
//! two-phase structure (pre-`get_proper_type` check for branch (a), then
//! the unbound recursion (b) + `get_proper_type` side effects, then the
//! post-`get_proper_type` elif-chain). When branch (a) matches the shim
//! short-circuits before the recursion so the `api.defer()` side effect
//! of `visit_unbound_type` is never triggered.

use pyo3::prelude::*;

// Branch tags. Each maps to exactly one terminal branch of
// `analyze_literal_param`; the comment cites the typeanal.py line and
// the Python-side effect the shim must apply.
const TAG_STR_LITERAL: i64 = 1; // 2476-2489 string-Literal from original_str_expr
const TAG_ANY_FAIL: i64 = 2; // 2503-2523 AnyType, type_of_any not error/special
const TAG_ANY_SILENT: i64 = 3; // 2517-2523 AnyType, type_of_any is error/special
const TAG_RAW_NO_VALUE_FLOAT_COMPLEX: i64 = 4; // 2526-2535 literal_value None, float/complex
const TAG_RAW_NO_VALUE_ARBITRARY: i64 = 5; // 2526-2535 literal_value None, other expr
const TAG_RAW_WITH_VALUE: i64 = 6; // 2537-2540 RawExpressionType with value
const TAG_NONE_OR_LITERAL: i64 = 7; // 2541-2543 NoneType / LiteralType as-is
const TAG_INSTANCE_LKV: i64 = 8; // 2544-2546 Instance with last_known_value
const TAG_UNION_RECURSE: i64 = 9; // 2547-2554 UnionType per-item recursion
const TAG_INVALID: i64 = 10; // 2555-2557 else: fail + None

/// TypeOfAny int values from `mypy.types.TypeOfAny` (types.py:297-325).
const TYPE_OF_ANY_FROM_ERROR: i64 = 5;
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

/// `analyze_literal_param` classifier. Mirrors the branch order of
/// typeanal.py:2474-2557 and returns the terminal branch tag.
///
/// Facts (all scalars / strings, no live type objects):
/// - `is_proper_type`, `is_unbound`, `is_union_pre`: pre-
///   `get_proper_type` isinstance checks on the original arg.
/// - `original_str_expr_is_not_none`: `arg.original_str_expr is not
///   None` (only meaningful when `is_proper_type` and the arg is an
///   `UnboundType` or `UnionType`).
/// - `is_any`, `type_of_any`: post-`get_proper_type` `AnyType` check
///   and the `TypeOfAny` int value.
/// - `is_raw_expr`, `literal_value_is_none`, `simple_name`: the
///   `RawExpressionType` check, its `literal_value` nullity, and
///   `simple_name()` result.
/// - `is_none_type`, `is_literal`, `is_instance`,
///   `last_known_value_is_none`, `is_union_post`: the remaining
///   post-`get_proper_type` isinstance checks.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub(crate) fn rust_classify_literal_param(
    is_proper_type: bool,
    is_unbound: bool,
    is_union_pre: bool,
    original_str_expr_is_not_none: bool,
    is_any: bool,
    type_of_any: i64,
    is_raw_expr: bool,
    literal_value_is_none: bool,
    simple_name: String,
    is_none_type: bool,
    is_literal: bool,
    is_instance: bool,
    last_known_value_is_none: bool,
    is_union_post: bool,
) -> PyResult<i64> {
    // (a) String-Literal from original_str_expr (typeanal.py:2476-2489).
    if is_proper_type && (is_unbound || is_union_pre) && original_str_expr_is_not_none {
        return Ok(TAG_STR_LITERAL);
    }

    // (c) AnyType (typeanal.py:2503-2523). Branch (b) (unbound recursion)
    // is a Python-side pre-step already applied; the classifier jumps to
    // the post-get_proper_type chain.
    if is_any {
        if type_of_any != TYPE_OF_ANY_FROM_ERROR && type_of_any != TYPE_OF_ANY_SPECIAL_FORM {
            return Ok(TAG_ANY_FAIL);
        }
        return Ok(TAG_ANY_SILENT);
    }

    // (d) / (e) RawExpressionType (typeanal.py:2524-2540).
    if is_raw_expr {
        if literal_value_is_none {
            // (d) literal_value is None: split by simple_name.
            if simple_name == "float" || simple_name == "complex" {
                return Ok(TAG_RAW_NO_VALUE_FLOAT_COMPLEX);
            }
            return Ok(TAG_RAW_NO_VALUE_ARBITRARY);
        }
        // (e) RawExpressionType with a value.
        return Ok(TAG_RAW_WITH_VALUE);
    }

    // (f) NoneType / LiteralType (typeanal.py:2541-2543).
    if is_none_type || is_literal {
        return Ok(TAG_NONE_OR_LITERAL);
    }

    // (g) Instance with last_known_value (typeanal.py:2544-2546).
    if is_instance && !last_known_value_is_none {
        return Ok(TAG_INSTANCE_LKV);
    }

    // (h) UnionType (typeanal.py:2547-2554).
    if is_union_post {
        return Ok(TAG_UNION_RECURSE);
    }

    // (i) else: invalid (typeanal.py:2555-2557).
    Ok(TAG_INVALID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Facts {
        is_proper_type: bool,
        is_unbound: bool,
        is_union_pre: bool,
        original_str_expr_is_not_none: bool,
        is_any: bool,
        type_of_any: i64,
        is_raw_expr: bool,
        literal_value_is_none: bool,
        simple_name: String,
        is_none_type: bool,
        is_literal: bool,
        is_instance: bool,
        last_known_value_is_none: bool,
        is_union_post: bool,
    }

    impl Default for Facts {
        fn default() -> Self {
            Facts {
                is_proper_type: false,
                is_unbound: false,
                is_union_pre: false,
                original_str_expr_is_not_none: false,
                is_any: false,
                type_of_any: 0,
                is_raw_expr: false,
                literal_value_is_none: true,
                simple_name: String::new(),
                is_none_type: false,
                is_literal: false,
                is_instance: false,
                last_known_value_is_none: true,
                is_union_post: false,
            }
        }
    }

    fn classify(f: &Facts) -> i64 {
        rust_classify_literal_param(
            f.is_proper_type,
            f.is_unbound,
            f.is_union_pre,
            f.original_str_expr_is_not_none,
            f.is_any,
            f.type_of_any,
            f.is_raw_expr,
            f.literal_value_is_none,
            f.simple_name.clone(),
            f.is_none_type,
            f.is_literal,
            f.is_instance,
            f.last_known_value_is_none,
            f.is_union_post,
        )
        .unwrap()
    }

    #[test]
    fn test_classify_literal_param_str_literal_unbound() {
        let f = Facts {
            is_proper_type: true,
            is_unbound: true,
            original_str_expr_is_not_none: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_STR_LITERAL);
    }

    #[test]
    fn test_classify_literal_param_str_literal_union() {
        let f = Facts {
            is_proper_type: true,
            is_union_pre: true,
            original_str_expr_is_not_none: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_STR_LITERAL);
    }

    #[test]
    fn test_classify_literal_param_str_literal_needs_proper_type() {
        let f = Facts {
            is_unbound: true,
            original_str_expr_is_not_none: true,
            ..Default::default()
        };
        assert_ne!(classify(&f), TAG_STR_LITERAL);
    }

    #[test]
    fn test_classify_literal_param_any_fail() {
        let f = Facts {
            is_any: true,
            type_of_any: 2, // explicit
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_ANY_FAIL);
    }

    #[test]
    fn test_classify_literal_param_any_silent_from_error() {
        let f = Facts {
            is_any: true,
            type_of_any: TYPE_OF_ANY_FROM_ERROR,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_ANY_SILENT);
    }

    #[test]
    fn test_classify_literal_param_any_silent_special_form() {
        let f = Facts {
            is_any: true,
            type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_ANY_SILENT);
    }

    #[test]
    fn test_classify_literal_param_raw_no_value_float() {
        let f = Facts {
            is_raw_expr: true,
            literal_value_is_none: true,
            simple_name: "float".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_RAW_NO_VALUE_FLOAT_COMPLEX);
    }

    #[test]
    fn test_classify_literal_param_raw_no_value_complex() {
        let f = Facts {
            is_raw_expr: true,
            literal_value_is_none: true,
            simple_name: "complex".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_RAW_NO_VALUE_FLOAT_COMPLEX);
    }

    #[test]
    fn test_classify_literal_param_raw_no_value_arbitrary() {
        let f = Facts {
            is_raw_expr: true,
            literal_value_is_none: true,
            simple_name: "something".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_RAW_NO_VALUE_ARBITRARY);
    }

    #[test]
    fn test_classify_literal_param_raw_with_value() {
        let f = Facts {
            is_raw_expr: true,
            literal_value_is_none: false,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_RAW_WITH_VALUE);
    }

    #[test]
    fn test_classify_literal_param_none_type() {
        let f = Facts {
            is_none_type: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_NONE_OR_LITERAL);
    }

    #[test]
    fn test_classify_literal_param_literal() {
        let f = Facts {
            is_literal: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_NONE_OR_LITERAL);
    }

    #[test]
    fn test_classify_literal_param_instance_lkv() {
        let f = Facts {
            is_instance: true,
            last_known_value_is_none: false,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_INSTANCE_LKV);
    }

    #[test]
    fn test_classify_literal_param_instance_no_lkv_is_invalid() {
        let f = Facts {
            is_instance: true,
            last_known_value_is_none: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_INVALID);
    }

    #[test]
    fn test_classify_literal_param_union_recurse() {
        let f = Facts {
            is_union_post: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_UNION_RECURSE);
    }

    #[test]
    fn test_classify_literal_param_invalid() {
        assert_eq!(classify(&Facts::default()), TAG_INVALID);
    }

    #[test]
    fn test_classify_literal_param_any_beats_raw() {
        // Branch order: AnyType is checked before RawExpressionType.
        let f = Facts {
            is_any: true,
            is_raw_expr: true,
            type_of_any: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_ANY_FAIL);
    }

    #[test]
    fn test_classify_literal_param_str_beats_any() {
        // Branch (a) is checked first, before the post chain.
        let f = Facts {
            is_proper_type: true,
            is_unbound: true,
            original_str_expr_is_not_none: true,
            is_any: true,
            type_of_any: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), TAG_STR_LITERAL);
    }
}
