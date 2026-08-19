//! Native port of the `try_parse_as_type_expression` classifier front
//! (`mypy/semanal.py`).
//!
//! `try_parse_as_type_expression` is called eagerly for EVERY value
//! expression in certain syntactic positions, so the bail-out decision
//! (which expressions cannot possibly be type expressions) is hot. This
//! module moves that decision tree to Rust. The Python shim gathers scalar
//! structural facts from the live AST node — node-kind tag, string value,
//! `isidentifier`/`isspace`/regex flags computed with Python `re`, and the
//! `IndexExpr` leftmost-component resolution — and Rust runs the same
//! branch-for-branch decision the Python classifier front would.
//!
//! Classification results:
//! - `Some(0)`: definitely not a type expression; Python sets `as_type =
//!   None` and returns.
//! - `Some(1)`: maybe a type expression; Python proceeds to the full parse
//!   tail (`expr_to_analyzed_type`).
//! - `None`: defer; Python runs the pure classifier front unchanged. This
//!   happens for identifier-string literals, whose classification needs
//!   `SemanticAnalyzer.lookup`, and for caller mismatches (wrong node-tag
//!   count or an unknown kind).
//!
//! The `_MULTIPLE_WORDS_NONTYPE_RE` regex stays in Python `re` (Unicode
//! semantics); its match result is passed in as a flag. `isidentifier` and
//! `isspace` are also Unicode-dependent Python string methods, so those
//! results are flags too. The quote / `[` / length checks are exact ASCII /
//! code-point scans, so Rust derives them from `str_value` itself.

use pyo3::prelude::*;

/// Not a type expression; Python sets `as_type = None` and returns.
const NOT_A_TYPE: i64 = 0;
/// Maybe a type expression; Python proceeds to the full parse tail.
const MAYBE_TYPE_EXPR: i64 = 1;

/// Node-kind tag for `StrExpr` in the `node_tags` argument.
const KIND_STR_EXPR: i64 = 0;
/// Node-kind tag for `IndexExpr` in the `node_tags` argument.
const KIND_INDEX_EXPR: i64 = 1;
/// Node-kind tag for `OpExpr` in the `node_tags` argument.
const KIND_OP_EXPR: i64 = 2;

/// `IndexExpr` base resolution tag: the base is a `NameExpr`.
const INDEX_BASE_NAME: i64 = 0;
/// `IndexExpr` base resolution tag: the base is a `MemberExpr`.
const INDEX_BASE_MEMBER: i64 = 1;
/// `IndexExpr` base resolution tag: the base is neither.
const INDEX_BASE_OTHER: i64 = 2;

/// Rust port of the `try_parse_as_type_expression` bail-out decision
/// (semanal.py:8929-9021).
///
/// The Python shim computes structural facts from the live AST node and
/// passes them as scalars; no node bodies cross the seam. Node kinds the
/// shim never routes here (`NameExpr`/`MemberExpr` immediate returns, and
/// anything outside `MaybeTypeExpression`) do not reach Rust.
///
/// `node_tags` is a single-element tuple holding the node kind
/// (`KIND_STR_EXPR` / `KIND_INDEX_EXPR` / `KIND_OP_EXPR`). A different
/// length or an unknown kind defers (return `None`), mirroring the
/// `assert_never` catch-all on the Python side.
///
/// Returns `Some(NOT_A_TYPE)`, `Some(MAYBE_TYPE_EXPR)`, or `None` (defer).
#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (
    node_tags,
    str_value = None,
    str_isidentifier = false,
    str_has_quotes = false,
    str_has_open_bracket = false,
    str_is_whitespace = false,
    str_nontype_regex_match = false,
    index_base_kind = 0,
    index_leftmost_is_name = false,
    index_node_is_var = false,
    index_var_is_special = false,
    op_is_pipe = false
))]
pub(crate) fn rust_classify_type_expression(
    node_tags: Vec<i64>,
    str_value: Option<String>,
    str_isidentifier: bool,
    str_has_quotes: bool,
    str_has_open_bracket: bool,
    str_is_whitespace: bool,
    str_nontype_regex_match: bool,
    index_base_kind: i64,
    index_leftmost_is_name: bool,
    index_node_is_var: bool,
    index_var_is_special: bool,
    op_is_pipe: bool,
) -> PyResult<Option<i64>> {
    // A caller mismatch on the tag shape is a seam violation; defer so the
    // pure-Python front runs unchanged rather than guessing.
    if node_tags.len() != 1 {
        return Ok(None);
    }
    match node_tags[0] {
        KIND_STR_EXPR => {
            // Identifier strings need `self.lookup` (symbol table resolution),
            // which cannot cross the seam; defer to Python.
            if str_isidentifier {
                return Ok(None);
            }
            match str_value.as_deref() {
                Some(s) => {
                    // Mirrors semanal.py:8966-8984: quote-only strings need
                    // Literal[...]/Annotated[..., ...] (`[`); short or
                    // whitespace-only strings and regex matches are not types.
                    if str_has_quotes {
                        if !str_has_open_bracket {
                            return Ok(Some(NOT_A_TYPE));
                        }
                    } else if s.chars().count() < 2 || str_is_whitespace {
                        return Ok(Some(NOT_A_TYPE));
                    }
                    if str_nontype_regex_match {
                        return Ok(Some(NOT_A_TYPE));
                    }
                    Ok(Some(MAYBE_TYPE_EXPR))
                }
                // StrExpr without a string value: cannot classify.
                None => Ok(None),
            }
        }
        KIND_INDEX_EXPR => {
            // Mirrors semanal.py:8985-9014: the leftmost base component is a
            // Var (not a typing special form) only when the shim resolved a
            // Name; other bases bail, non-Name leftmost bails, else proceed.
            if index_base_kind == INDEX_BASE_OTHER {
                return Ok(Some(NOT_A_TYPE));
            }
            if index_base_kind != INDEX_BASE_NAME && index_base_kind != INDEX_BASE_MEMBER {
                // Unknown resolution tag: defer.
                return Ok(None);
            }
            if !index_leftmost_is_name {
                // The leftmost part of the IndexExpr base is not a NameExpr.
                return Ok(Some(NOT_A_TYPE));
            }
            if index_node_is_var && !index_var_is_special {
                // The leftmost part refers to a Var; not a valid type.
                return Ok(Some(NOT_A_TYPE));
            }
            Ok(Some(MAYBE_TYPE_EXPR))
        }
        KIND_OP_EXPR => {
            // Mirrors semanal.py:9015-9019: binary operators other than '|'
            // never spell a valid type.
            if op_is_pipe {
                Ok(Some(MAYBE_TYPE_EXPR))
            } else {
                Ok(Some(NOT_A_TYPE))
            }
        }
        _ => Ok(None),
    }
}
