//! Issue #560: Native port of `mypy/reachability.py`.
//!
//! Ports the pure functions from `mypy.reachability` that infer whether
//! conditions are always true/false (for unreachable-code elimination during
//! semantic analysis). These operate on live Python AST `Expression` objects
//! and `Options` objects via PyO3, following the same pattern as
//! `semanal_visitor.rs`.
//!
//! Ported functions:
//! - `infer_condition_value` — main entry: infer truth value of an expression.
//! - `infer_pattern_value` — infer truth value of a match pattern.
//! - `assert_will_always_fail` — whether an assert statement always fails.
//! - `consider_sys_version_info` — `sys.version_info` comparison analysis.
//! - `consider_sys_platform` — `sys.platform` comparison analysis.
//! - `contains_int_or_tuple_of_ints` — extract int or tuple-of-ints from expr.
//! - `contains_sys_version_info` — extract sys.version_info index/slice from expr.
//! - `is_sys_attr` — check whether expr is `sys.<name>`.
//! - `fixed_comparison` — generic comparison → truth-value constant.
//!
//! The AST-mutating functions (`infer_reachability_of_if_statement`,
//! `infer_reachability_of_match_statement`, `mark_block_unreachable`,
//! `mark_block_mypy_only`) stay in Python: they traverse and mutate the live
//! AST graph, which is not a pure computation.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple, PyType};

// ---------------------------------------------------------------------------
// Truth-value constants (mirror mypy/reachability.py)
// ---------------------------------------------------------------------------

pub(crate) const ALWAYS_TRUE: u8 = 1;
pub(crate) const MYPY_TRUE: u8 = 2;
pub(crate) const ALWAYS_FALSE: u8 = 3;
pub(crate) const MYPY_FALSE: u8 = 4;
pub(crate) const TRUTH_VALUE_UNKNOWN: u8 = 5;

/// `inverted_truth_mapping` — negate a truth value (for `not` expressions).
fn inverted_truth(value: u8) -> u8 {
    match value {
        ALWAYS_TRUE => ALWAYS_FALSE,
        ALWAYS_FALSE => ALWAYS_TRUE,
        TRUTH_VALUE_UNKNOWN => TRUTH_VALUE_UNKNOWN,
        MYPY_TRUE => MYPY_FALSE,
        MYPY_FALSE => MYPY_TRUE,
        _ => TRUTH_VALUE_UNKNOWN,
    }
}

/// `reverse_op` — swap the direction of a comparison operator.
fn reverse_op(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("=="),
        "!=" => Some("!="),
        "<" => Some(">"),
        ">" => Some("<"),
        "<=" => Some(">="),
        ">=" => Some("<="),
        _ => None,
    }
}

/// `fixed_comparison` — compare two values and return a truth-value constant.
///
/// Generic over any type that implements `PartialEq` + `PartialOrd`.
/// Works for `i64`, `String`, and `&[i64]` (lexicographic, matching Python
/// tuple comparison).
fn fixed_comparison<T: PartialEq + PartialOrd>(left: &T, op: &str, right: &T) -> u8 {
    let rmap = |b: bool| if b { ALWAYS_TRUE } else { ALWAYS_FALSE };
    match op {
        "==" => rmap(left == right),
        "!=" => rmap(left != right),
        "<=" => rmap(left <= right),
        ">=" => rmap(left >= right),
        "<" => rmap(left < right),
        ">" => rmap(left > right),
        _ => TRUTH_VALUE_UNKNOWN,
    }
}

// ---------------------------------------------------------------------------
// Return-type enums for helper functions
// ---------------------------------------------------------------------------

/// `None | int | tuple[int, ...]` from `contains_int_or_tuple_of_ints`.
enum IntOrTuple {
    Int(i64),
    Tuple(Vec<i64>),
}

/// `None | int | tuple[int | None, int | None]` from `contains_sys_version_info`.
enum VersionInfoIndex {
    Index(i64),
    Slice(Option<i64>, Option<i64>),
}

// ---------------------------------------------------------------------------
// Cached context for reachability analysis
// ---------------------------------------------------------------------------

/// Holds cached PyO3 references to avoid repeated imports and attribute
/// lookups during recursive condition inference.
struct ReachabilityCtx<'py> {
    #[allow(dead_code)]
    py: Python<'py>,
    nodes_mod: &'py PyModule,
    // Expression class types
    name_expr_cls: &'py PyType,
    member_expr_cls: &'py PyType,
    op_expr_cls: &'py PyType,
    unary_expr_cls: &'py PyType,
    comparison_expr_cls: &'py PyType,
    call_expr_cls: &'py PyType,
    int_expr_cls: &'py PyType,
    str_expr_cls: &'py PyType,
    tuple_expr_cls: &'py PyType,
    index_expr_cls: &'py PyType,
    slice_expr_cls: &'py PyType,
    as_pattern_cls: &'py PyType,
    or_pattern_cls: &'py PyType,
    // Options data
    pyversion: Vec<i64>,
    platform: String,
    always_true: HashSet<String>,
    always_false: HashSet<String>,
}

impl<'py> ReachabilityCtx<'py> {
    fn new(py: Python<'py>, options: &PyAny) -> PyResult<Self> {
        let nodes_mod = py.import("mypy.nodes")?;
        let patterns_mod = py.import("mypy.patterns")?;

        let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
        let member_expr_cls: &PyType = nodes_mod.getattr("MemberExpr")?.downcast()?;
        let op_expr_cls: &PyType = nodes_mod.getattr("OpExpr")?.downcast()?;
        let unary_expr_cls: &PyType = nodes_mod.getattr("UnaryExpr")?.downcast()?;
        let comparison_expr_cls: &PyType = nodes_mod.getattr("ComparisonExpr")?.downcast()?;
        let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
        let int_expr_cls: &PyType = nodes_mod.getattr("IntExpr")?.downcast()?;
        let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;
        let tuple_expr_cls: &PyType = nodes_mod.getattr("TupleExpr")?.downcast()?;
        let index_expr_cls: &PyType = nodes_mod.getattr("IndexExpr")?.downcast()?;
        let slice_expr_cls: &PyType = nodes_mod.getattr("SliceExpr")?.downcast()?;
        let as_pattern_cls: &PyType = patterns_mod.getattr("AsPattern")?.downcast()?;
        let or_pattern_cls: &PyType = patterns_mod.getattr("OrPattern")?.downcast()?;

        // Extract options fields
        let pyversion: Vec<i64> = options.getattr("python_version")?.extract()?;
        let platform: String = options.getattr("platform")?.extract()?;
        let always_true: HashSet<String> = options.getattr("always_true")?.extract()?;
        let always_false: HashSet<String> = options.getattr("always_false")?.extract()?;

        Ok(ReachabilityCtx {
            py,
            nodes_mod,
            name_expr_cls,
            member_expr_cls,
            op_expr_cls,
            unary_expr_cls,
            comparison_expr_cls,
            call_expr_cls,
            int_expr_cls,
            str_expr_cls,
            tuple_expr_cls,
            index_expr_cls,
            slice_expr_cls,
            as_pattern_cls,
            or_pattern_cls,
            pyversion,
            platform,
            always_true,
            always_false,
        })
    }

    /// `is_sys_attr(expr, name)` — is `expr` a `sys.<name>` member expression?
    ///
    /// Mirrors reachability.py:323-332. Checks that `expr` is a `MemberExpr`
    /// with `.name == name` and `.expr` is a `NameExpr` with `.name == "sys"`.
    fn is_sys_attr(&self, expr: &PyAny, name: &str) -> PyResult<bool> {
        if !expr.is_instance(self.member_expr_cls)? {
            return Ok(false);
        }
        let member_name = expr.getattr("name")?;
        let member_name_str: &str = member_name.downcast::<PyString>()?.to_str()?;
        if member_name_str != name {
            return Ok(false);
        }
        let base = expr.getattr("expr")?;
        if !base.is_instance(self.name_expr_cls)? {
            return Ok(false);
        }
        let base_name = base.getattr("name")?;
        let base_name_str: &str = base_name.downcast::<PyString>()?.to_str()?;
        Ok(base_name_str == "sys")
    }

    /// `contains_int_or_tuple_of_ints(expr)` — extract int or tuple-of-ints.
    ///
    /// Mirrors reachability.py:285-296. Returns `Some(Int(value))` for
    /// `IntExpr`, `Some(Tuple(items))` for `TupleExpr` of all `IntExpr`
    /// items, `None` otherwise.
    ///
    /// The Python `literal(expr) == LITERAL_YES` guard is redundant here:
    /// the loop that follows already returns `None` if any item is not an
    /// `IntExpr`, which is the only case `literal()` would reject.
    fn contains_int_or_tuple_of_ints(&self, expr: &PyAny) -> PyResult<Option<IntOrTuple>> {
        if expr.is_instance(self.int_expr_cls)? {
            let value: i64 = expr.getattr("value")?.extract()?;
            return Ok(Some(IntOrTuple::Int(value)));
        }
        if expr.is_instance(self.tuple_expr_cls)? {
            let items = expr.getattr("items")?;
            let items_list = items.downcast::<PyList>()?;
            let mut result = Vec::with_capacity(items_list.len());
            for item in items_list.iter() {
                if !item.is_instance(self.int_expr_cls)? {
                    return Ok(None);
                }
                let val: i64 = item.getattr("value")?.extract()?;
                result.push(val);
            }
            return Ok(Some(IntOrTuple::Tuple(result)));
        }
        Ok(None)
    }

    /// `contains_sys_version_info(expr)` — extract sys.version_info index/slice.
    ///
    /// Mirrors reachability.py:299-320. Returns:
    /// - `Some(Index(i))` for `sys.version_info[i]`
    /// - `Some(Slice(begin, end))` for `sys.version_info[begin:end]`
    /// - `Some(Slice(None, None))` for bare `sys.version_info`
    /// - `None` otherwise
    fn contains_sys_version_info(&self, expr: &PyAny) -> PyResult<Option<VersionInfoIndex>> {
        // Bare sys.version_info (same as sys.version_info[:])
        if self.is_sys_attr(expr, "version_info")? {
            return Ok(Some(VersionInfoIndex::Slice(None, None)));
        }
        if !expr.is_instance(self.index_expr_cls)? {
            return Ok(None);
        }
        let base = expr.getattr("base")?;
        if !self.is_sys_attr(base, "version_info")? {
            return Ok(None);
        }
        let index = expr.getattr("index")?;
        if index.is_instance(self.int_expr_cls)? {
            let val: i64 = index.getattr("value")?.extract()?;
            return Ok(Some(VersionInfoIndex::Index(val)));
        }
        if index.is_instance(self.slice_expr_cls)? {
            let stride = index.getattr("stride")?;
            if !stride.is_none() {
                if !stride.is_instance(self.int_expr_cls)? {
                    return Ok(None);
                }
                let stride_val: i64 = stride.getattr("value")?.extract()?;
                if stride_val != 1 {
                    return Ok(None);
                }
            }
            let begin = {
                let begin_index = index.getattr("begin_index")?;
                if begin_index.is_none() {
                    None
                } else {
                    if !begin_index.is_instance(self.int_expr_cls)? {
                        return Ok(None);
                    }
                    Some(begin_index.getattr("value")?.extract::<i64>()?)
                }
            };
            let end = {
                let end_index = index.getattr("end_index")?;
                if end_index.is_none() {
                    None
                } else {
                    if !end_index.is_instance(self.int_expr_cls)? {
                        return Ok(None);
                    }
                    Some(end_index.getattr("value")?.extract::<i64>()?)
                }
            };
            return Ok(Some(VersionInfoIndex::Slice(begin, end)));
        }
        Ok(None)
    }

    /// `consider_sys_version_info(expr, pyversion)` — analyze a
    /// `sys.version_info` comparison expression.
    ///
    /// Mirrors reachability.py:182-223.
    fn consider_sys_version_info(&self, expr: &PyAny) -> PyResult<u8> {
        if !expr.is_instance(self.comparison_expr_cls)? {
            return Ok(TRUTH_VALUE_UNKNOWN);
        }
        let operators = expr.getattr("operators")?;
        let operators_list = operators.downcast::<PyList>()?;
        if operators_list.len() > 1 {
            return Ok(TRUTH_VALUE_UNKNOWN);
        }
        let op_str: String = operators_list.get_item(0)?.extract()?;
        if !matches!(op_str.as_str(), "==" | "!=" | "<=" | ">=" | "<" | ">") {
            return Ok(TRUTH_VALUE_UNKNOWN);
        }

        let operands = expr.getattr("operands")?;
        let operands_list = operands.downcast::<PyList>()?;
        let operand0 = operands_list.get_item(0)?;
        let operand1 = operands_list.get_item(1)?;

        let mut index = self.contains_sys_version_info(operand0)?;
        let mut thing = self.contains_int_or_tuple_of_ints(operand1)?;
        let mut op = op_str.clone();

        if index.is_none() || thing.is_none() {
            index = self.contains_sys_version_info(operand1)?;
            thing = self.contains_int_or_tuple_of_ints(operand0)?;
            op = reverse_op(&op_str).unwrap_or(&op_str).to_string();
        }

        match (index, thing) {
            (Some(VersionInfoIndex::Index(idx)), Some(IntOrTuple::Int(thing_val))) => {
                if 0 <= idx && idx <= 1 {
                    let pyver_val = self.pyversion.get(idx as usize).copied().unwrap_or(0);
                    Ok(fixed_comparison(&pyver_val, &op, &thing_val))
                } else {
                    Ok(TRUTH_VALUE_UNKNOWN)
                }
            }
            (Some(VersionInfoIndex::Slice(lo, hi)), Some(IntOrTuple::Tuple(thing_tuple))) => {
                let lo = lo.unwrap_or(0);
                let hi = hi.unwrap_or(2);
                if 0 <= lo && lo < hi && hi <= 2 {
                    let lo_us = lo as usize;
                    let hi_us = hi as usize;
                    let val: Vec<i64> =
                        self.pyversion[lo_us..hi_us.min(self.pyversion.len())].to_vec();
                    if val.len() == thing_tuple.len()
                        || (val.len() > thing_tuple.len() && op != "==" && op != "!=")
                    {
                        Ok(fixed_comparison(&val, &op, &thing_tuple))
                    } else {
                        Ok(TRUTH_VALUE_UNKNOWN)
                    }
                } else {
                    Ok(TRUTH_VALUE_UNKNOWN)
                }
            }
            _ => Ok(TRUTH_VALUE_UNKNOWN),
        }
    }

    /// `consider_sys_platform(expr, platform)` — analyze a `sys.platform`
    /// comparison expression.
    ///
    /// Mirrors reachability.py:226-262.
    fn consider_sys_platform(&self, expr: &PyAny) -> PyResult<u8> {
        if expr.is_instance(self.comparison_expr_cls)? {
            let operators = expr.getattr("operators")?;
            let operators_list = operators.downcast::<PyList>()?;
            if operators_list.len() > 1 {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let op_str: String = operators_list.get_item(0)?.extract()?;
            if op_str != "==" && op_str != "!=" {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let operands = expr.getattr("operands")?;
            let operands_list = operands.downcast::<PyList>()?;
            let operand0 = operands_list.get_item(0)?;
            if !self.is_sys_attr(operand0, "platform")? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let right = operands_list.get_item(1)?;
            if !right.is_instance(self.str_expr_cls)? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let right_value: String = right.getattr("value")?.extract()?;
            Ok(fixed_comparison(&self.platform, &op_str, &right_value))
        } else if expr.is_instance(self.call_expr_cls)? {
            let callee = expr.getattr("callee")?;
            if !callee.is_instance(self.member_expr_cls)? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let args = expr.getattr("args")?;
            let args_list = args.downcast::<PyList>()?;
            if args_list.len() != 1 {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let arg0 = args_list.get_item(0)?;
            if !arg0.is_instance(self.str_expr_cls)? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let callee_expr = callee.getattr("expr")?;
            if !self.is_sys_attr(callee_expr, "platform")? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let callee_name: String = callee.getattr("name")?.extract()?;
            if callee_name != "startswith" {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let arg_value: String = arg0.getattr("value")?.extract()?;
            if self.platform.starts_with(&arg_value) {
                Ok(ALWAYS_TRUE)
            } else {
                Ok(ALWAYS_FALSE)
            }
        } else {
            Ok(TRUTH_VALUE_UNKNOWN)
        }
    }

    /// `infer_pattern_value(pattern)` — infer truth value of a match pattern.
    ///
    /// Mirrors reachability.py:171-179.
    fn infer_pattern_value(&self, pattern: &PyAny) -> PyResult<u8> {
        if pattern.is_instance(self.as_pattern_cls)? {
            let inner = pattern.getattr("pattern")?;
            if inner.is_none() {
                return Ok(ALWAYS_TRUE);
            }
        }
        if pattern.is_instance(self.or_pattern_cls)? {
            let patterns = pattern.getattr("patterns")?;
            let patterns_list = patterns.downcast::<PyList>()?;
            for p in patterns_list.iter() {
                if self.infer_pattern_value(p)? == ALWAYS_TRUE {
                    return Ok(ALWAYS_TRUE);
                }
            }
        }
        Ok(TRUTH_VALUE_UNKNOWN)
    }

    /// `infer_condition_value(expr, options)` — infer truth value of an expr.
    ///
    /// Mirrors reachability.py:108-168. Recursive for `not`, `and`, `or`.
    fn infer_condition_value(&self, expr: &PyAny) -> PyResult<u8> {
        // UnaryExpr with "not"
        if expr.is_instance(self.unary_expr_cls)? {
            let op: String = expr.getattr("op")?.extract()?;
            if op == "not" {
                let inner = expr.getattr("expr")?;
                let positive = self.infer_condition_value(inner)?;
                return Ok(inverted_truth(positive));
            }
        }

        let mut name = String::new();
        let mut result = TRUTH_VALUE_UNKNOWN;

        if expr.is_instance(self.name_expr_cls)? {
            name = expr.getattr("name")?.extract()?;
        } else if expr.is_instance(self.member_expr_cls)? {
            name = expr.getattr("name")?.extract()?;
        } else if expr.is_instance(self.op_expr_cls)? {
            let op: String = expr.getattr("op")?.extract()?;
            if op != "or" && op != "and" {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let left = self.infer_condition_value(expr.getattr("left")?)?;
            let right = self.infer_condition_value(expr.getattr("right")?)?;
            let results = [left, right];
            let results_set: HashSet<u8> = results.iter().copied().collect();
            if op == "or" {
                if results_set.contains(&ALWAYS_TRUE) {
                    return Ok(ALWAYS_TRUE);
                } else if results_set.contains(&MYPY_TRUE) {
                    return Ok(MYPY_TRUE);
                } else if left == MYPY_FALSE && right == MYPY_FALSE {
                    return Ok(MYPY_FALSE);
                } else if results_set
                    .iter()
                    .all(|&v| v == ALWAYS_FALSE || v == MYPY_FALSE)
                {
                    return Ok(ALWAYS_FALSE);
                }
            } else if op == "and" {
                if results_set.contains(&ALWAYS_FALSE) {
                    return Ok(ALWAYS_FALSE);
                } else if results_set.contains(&MYPY_FALSE) {
                    return Ok(MYPY_FALSE);
                } else if left == ALWAYS_TRUE && right == ALWAYS_TRUE {
                    return Ok(ALWAYS_TRUE);
                } else if results_set
                    .iter()
                    .all(|&v| v == ALWAYS_TRUE || v == MYPY_TRUE)
                {
                    return Ok(MYPY_TRUE);
                }
            }
            return Ok(TRUTH_VALUE_UNKNOWN);
        } else {
            // Not NameExpr, MemberExpr, or OpExpr — try sys checks
            result = self.consider_sys_version_info(expr)?;
            if result == TRUTH_VALUE_UNKNOWN {
                result = self.consider_sys_platform(expr)?;
            }
        }

        if result == TRUTH_VALUE_UNKNOWN {
            if name == "PY2" {
                result = ALWAYS_FALSE;
            } else if name == "PY3" {
                result = ALWAYS_TRUE;
            } else if name == "MYPY" || name == "TYPE_CHECKING" {
                result = MYPY_TRUE;
            } else if self.always_true.contains(&name) {
                result = ALWAYS_TRUE;
            } else if self.always_false.contains(&name) {
                result = ALWAYS_FALSE;
            }
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Public #[pyfunction] entry points
// ---------------------------------------------------------------------------

/// `mypy.reachability.infer_condition_value` — infer whether a condition is
/// always true/false. Returns one of the truth-value constants
/// (ALWAYS_TRUE=1, MYPY_TRUE=2, ALWAYS_FALSE=3, MYPY_FALSE=4, UNKNOWN=5).
#[pyfunction]
pub(crate) fn rust_infer_condition_value(
    py: Python<'_>,
    expr: &PyAny,
    options: &PyAny,
) -> PyResult<u8> {
    let ctx = ReachabilityCtx::new(py, options)?;
    ctx.infer_condition_value(expr)
}

/// `mypy.reachability.infer_pattern_value` — infer truth value of a match
/// pattern. Returns ALWAYS_TRUE=1 or TRUTH_VALUE_UNKNOWN=5.
#[pyfunction]
pub(crate) fn rust_infer_pattern_value(py: Python<'_>, pattern: &PyAny) -> PyResult<u8> {
    let dummy_options = create_dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    ctx.infer_pattern_value(pattern)
}

/// `mypy.reachability.assert_will_always_fail` — whether an assert statement
/// always fails. Returns `true` if the condition is ALWAYS_FALSE or MYPY_FALSE.
#[pyfunction]
pub(crate) fn rust_assert_will_always_fail(
    py: Python<'_>,
    stmt: &PyAny,
    options: &PyAny,
) -> PyResult<bool> {
    let ctx = ReachabilityCtx::new(py, options)?;
    let expr = stmt.getattr("expr")?;
    let value = ctx.infer_condition_value(expr)?;
    Ok(value == ALWAYS_FALSE || value == MYPY_FALSE)
}

/// `mypy.reachability.consider_sys_version_info` — analyze a
/// `sys.version_info` comparison. Returns a truth-value constant.
#[pyfunction]
pub(crate) fn rust_consider_sys_version_info(
    py: Python<'_>,
    expr: &PyAny,
    pyversion: (i64, i64),
) -> PyResult<u8> {
    let dummy_options = create_dummy_options_with_version(py, pyversion)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    ctx.consider_sys_version_info(expr)
}

/// `mypy.reachability.consider_sys_platform` — analyze a `sys.platform`
/// comparison. Returns a truth-value constant.
#[pyfunction]
pub(crate) fn rust_consider_sys_platform(
    py: Python<'_>,
    expr: &PyAny,
    platform: &str,
) -> PyResult<u8> {
    let dummy_options = create_dummy_options_with_platform(py, platform)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    ctx.consider_sys_platform(expr)
}

/// `mypy.reachability.is_sys_attr` — is `expr` a `sys.<name>` member expr?
#[pyfunction]
pub(crate) fn rust_is_sys_attr(py: Python<'_>, expr: &PyAny, name: &str) -> PyResult<bool> {
    let dummy_options = create_dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    ctx.is_sys_attr(expr, name)
}

/// `mypy.reachability.contains_sys_version_info` — extract the index/slice
/// from a `sys.version_info` expression. Returns `None`, an int, or a
/// `(begin, end)` tuple of ints-or-None.
#[pyfunction]
pub(crate) fn rust_contains_sys_version_info(
    py: Python<'_>,
    expr: &PyAny,
) -> PyResult<Option<PyObject>> {
    let dummy_options = create_dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    match ctx.contains_sys_version_info(expr)? {
        Some(VersionInfoIndex::Index(idx)) => Ok(Some(idx.into_py(py))),
        Some(VersionInfoIndex::Slice(begin, end)) => {
            let begin_obj: PyObject = match begin {
                Some(v) => v.into_py(py),
                None => py.None(),
            };
            let end_obj: PyObject = match end {
                Some(v) => v.into_py(py),
                None => py.None(),
            };
            let tuple = PyTuple::new(py, [begin_obj, end_obj]);
            Ok(Some(tuple.into()))
        }
        None => Ok(None),
    }
}

/// `mypy.reachability.contains_int_or_tuple_of_ints` — extract int or
/// tuple-of-ints from an expression. Returns `None`, an int, or a tuple of
/// ints.
#[pyfunction]
pub(crate) fn rust_contains_int_or_tuple_of_ints(
    py: Python<'_>,
    expr: &PyAny,
) -> PyResult<Option<PyObject>> {
    let dummy_options = create_dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, &dummy_options)?;
    match ctx.contains_int_or_tuple_of_ints(expr)? {
        Some(IntOrTuple::Int(val)) => Ok(Some(val.into_py(py))),
        Some(IntOrTuple::Tuple(items)) => {
            let tuple = PyTuple::new(py, items.iter().map(|v| v.into_py(py)).collect::<Vec<_>>());
            Ok(Some(tuple.into()))
        }
        None => Ok(None),
    }
}

/// `mypy.reachability.fixed_comparison` — generic comparison → truth value.
///
/// Takes Python objects and compares them using Python comparison operators,
/// returning a truth-value constant.
#[pyfunction]
pub(crate) fn rust_fixed_comparison(left: &PyAny, op: &str, right: &PyAny) -> PyResult<u8> {
    use pyo3::basic::CompareOp;
    let rmap = |b: bool| if b { ALWAYS_TRUE } else { ALWAYS_FALSE };
    let op_enum = match op {
        "==" => CompareOp::Eq,
        "!=" => CompareOp::Ne,
        "<=" => CompareOp::Le,
        ">=" => CompareOp::Ge,
        "<" => CompareOp::Lt,
        ">" => CompareOp::Gt,
        _ => return Ok(TRUTH_VALUE_UNKNOWN),
    };
    let result = left.rich_compare(right, op_enum)?;
    Ok(rmap(result.is_true()?))
}

// ---------------------------------------------------------------------------
// Dummy Options helpers
// ---------------------------------------------------------------------------

/// Create a minimal `Options` object for helper functions that only need
/// a subset of fields (pattern analysis, sys_attr checks, etc.).
fn create_dummy_options(py: Python<'_>) -> PyResult<&PyAny> {
    let options_cls = py.import("mypy.options")?.getattr("Options")?;
    let options = options_cls.call0()?;
    Ok(options)
}

fn create_dummy_options_with_version(py: Python<'_>, pyversion: (i64, i64)) -> PyResult<&PyAny> {
    let options_cls = py.import("mypy.options")?.getattr("Options")?;
    let options = options_cls.call0()?;
    options.setattr("python_version", pyversion)?;
    Ok(options)
}

fn create_dummy_options_with_platform<'a>(
    py: Python<'a>,
    platform: &'a str,
) -> PyResult<&'a PyAny> {
    let options_cls = py.import("mypy.options")?.getattr("Options")?;
    let options = options_cls.call0()?;
    options.setattr("platform", platform)?;
    Ok(options)
}
