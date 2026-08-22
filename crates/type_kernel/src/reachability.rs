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

use std::sync::OnceLock;

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
fn fixed_comparison<T: PartialEq + PartialOrd + ?Sized>(left: &T, op: &str, right: &T) -> u8 {
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
// Cached class handles (built once per interpreter)
// ---------------------------------------------------------------------------

/// Native Python class handles pinned for the interpreter's lifetime.
///
/// Built once per interpreter instead of per call: the previous per-call
/// construction did two module imports + 13 downcasts + a full `Options`
/// extraction on every condition inference. Handles are shareable because
/// `Py<T>: Send + Sync` (pyo3 `instance.rs`).
struct ReachabilityClasses {
    // Expression class types (mypy.nodes)
    name_expr_cls: Py<PyType>,
    member_expr_cls: Py<PyType>,
    op_expr_cls: Py<PyType>,
    unary_expr_cls: Py<PyType>,
    comparison_expr_cls: Py<PyType>,
    call_expr_cls: Py<PyType>,
    int_expr_cls: Py<PyType>,
    str_expr_cls: Py<PyType>,
    tuple_expr_cls: Py<PyType>,
    index_expr_cls: Py<PyType>,
    slice_expr_cls: Py<PyType>,
    // Pattern class types (mypy.patterns)
    as_pattern_cls: Py<PyType>,
    or_pattern_cls: Py<PyType>,
}

static CLASSES: OnceLock<ReachabilityClasses> = OnceLock::new();

/// Shared minimal `Options` for entry points without an options argument
/// (pattern inference, sys-attr / version-info extraction). A fresh
/// `Options()` per leaf call was a measurable allocation; this one is built
/// once and only read, never mutated.
static DUMMY_OPTIONS: OnceLock<Py<PyAny>> = OnceLock::new();

/// Fetch the cached class handles, building them on first use.
fn get_classes(py: Python<'_>) -> PyResult<&'static ReachabilityClasses> {
    if let Some(classes) = CLASSES.get() {
        return Ok(classes);
    }
    let nodes_mod = py.import("mypy.nodes")?;
    let patterns_mod = py.import("mypy.patterns")?;

    let nodes_cls = |name: &str| -> PyResult<Py<PyType>> {
        let cls: &PyType = nodes_mod.getattr(name)?.downcast()?;
        Ok(cls.into())
    };
    let pattern_cls = |name: &str| -> PyResult<Py<PyType>> {
        let cls: &PyType = patterns_mod.getattr(name)?.downcast()?;
        Ok(cls.into())
    };

    let built = ReachabilityClasses {
        name_expr_cls: nodes_cls("NameExpr")?,
        member_expr_cls: nodes_cls("MemberExpr")?,
        op_expr_cls: nodes_cls("OpExpr")?,
        unary_expr_cls: nodes_cls("UnaryExpr")?,
        comparison_expr_cls: nodes_cls("ComparisonExpr")?,
        call_expr_cls: nodes_cls("CallExpr")?,
        int_expr_cls: nodes_cls("IntExpr")?,
        str_expr_cls: nodes_cls("StrExpr")?,
        tuple_expr_cls: nodes_cls("TupleExpr")?,
        index_expr_cls: nodes_cls("IndexExpr")?,
        slice_expr_cls: nodes_cls("SliceExpr")?,
        as_pattern_cls: pattern_cls("AsPattern")?,
        or_pattern_cls: pattern_cls("OrPattern")?,
    };
    // A losing concurrent initializer drops its handles under the held GIL,
    // which is safe; the installed set is equivalent either way.
    let _ = CLASSES.set(built);
    Ok(CLASSES.get().unwrap())
}

/// Fetch the shared dummy `Options`, building it on first use.
fn dummy_options(py: Python<'_>) -> PyResult<&PyAny> {
    if let Some(options) = DUMMY_OPTIONS.get() {
        return Ok(options.as_ref(py));
    }
    let options = py.import("mypy.options")?.getattr("Options")?.call0()?;
    let _ = DUMMY_OPTIONS.set(options.into());
    Ok(DUMMY_OPTIONS.get().unwrap().as_ref(py))
}

// ---------------------------------------------------------------------------
// Per-call context
// ---------------------------------------------------------------------------

/// Cheap per-call context: the expensive pieces (class handles, dummy
/// `Options`) are cached statically, and live `Options` fields are read
/// lazily, only when a branch needs them (mirroring the Python code, which
/// reads `options.platform` / `options.always_true` / `options.always_false`
/// only inside the branch that uses them).
struct ReachabilityCtx<'py> {
    py: Python<'py>,
    classes: &'static ReachabilityClasses,
    options: &'py PyAny,
    pyversion: (i64, i64),
}

impl<'py> ReachabilityCtx<'py> {
    fn new(py: Python<'py>, options: &'py PyAny) -> PyResult<Self> {
        let classes = get_classes(py)?;
        let pyversion: (i64, i64) = options.getattr("python_version")?.extract()?;
        Ok(ReachabilityCtx {
            py,
            classes,
            options,
            pyversion,
        })
    }

    /// `options.platform` — the target platform string, read lazily.
    fn platform(&self) -> PyResult<&'py str> {
        let platform = self.options.getattr("platform")?;
        platform.downcast::<PyString>()?.to_str()
    }

    /// `options.always_true` / `options.always_false` — the name lists, read
    /// lazily.
    fn always_true(&self) -> PyResult<&'py PyAny> {
        self.options.getattr("always_true")
    }

    fn always_false(&self) -> PyResult<&'py PyAny> {
        self.options.getattr("always_false")
    }

    /// `is_sys_attr(expr, name)` — is `expr` a `sys.<name>` member expression?
    ///
    /// Mirrors reachability.py:323-332. Checks that `expr` is a `MemberExpr`
    /// with `.name == name` and `.expr` is a `NameExpr` with `.name == "sys"`.
    fn is_sys_attr(&self, expr: &PyAny, name: &str) -> PyResult<bool> {
        if !expr.is_instance(self.classes.member_expr_cls.as_ref(self.py))? {
            return Ok(false);
        }
        let member_name: String = expr.getattr("name")?.extract()?;
        if member_name != name {
            return Ok(false);
        }
        let base = expr.getattr("expr")?;
        if !base.is_instance(self.classes.name_expr_cls.as_ref(self.py))? {
            return Ok(false);
        }
        let base_name: String = base.getattr("name")?.extract()?;
        Ok(base_name == "sys")
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
        if expr.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
            let value: i64 = expr.getattr("value")?.extract()?;
            return Ok(Some(IntOrTuple::Int(value)));
        }
        if expr.is_instance(self.classes.tuple_expr_cls.as_ref(self.py))? {
            let items = expr.getattr("items")?;
            let items_list = items.downcast::<PyList>()?;
            let mut result = Vec::with_capacity(items_list.len());
            for item in items_list.iter() {
                if !item.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
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
        if !expr.is_instance(self.classes.index_expr_cls.as_ref(self.py))? {
            return Ok(None);
        }
        let base = expr.getattr("base")?;
        if !self.is_sys_attr(base, "version_info")? {
            return Ok(None);
        }
        let index = expr.getattr("index")?;
        if index.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
            let val: i64 = index.getattr("value")?.extract()?;
            return Ok(Some(VersionInfoIndex::Index(val)));
        }
        if index.is_instance(self.classes.slice_expr_cls.as_ref(self.py))? {
            let stride = index.getattr("stride")?;
            if !stride.is_none() {
                if !stride.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
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
                    if !begin_index.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
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
                    if !end_index.is_instance(self.classes.int_expr_cls.as_ref(self.py))? {
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
        if !expr.is_instance(self.classes.comparison_expr_cls.as_ref(self.py))? {
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
                if (0..=1).contains(&idx) {
                    let pyver_val = match idx {
                        0 => self.pyversion.0,
                        1 => self.pyversion.1,
                        _ => 0,
                    };
                    Ok(fixed_comparison(&pyver_val, &op, &thing_val))
                } else {
                    Ok(TRUTH_VALUE_UNKNOWN)
                }
            }
            (Some(VersionInfoIndex::Slice(lo, hi)), Some(IntOrTuple::Tuple(thing_tuple))) => {
                let lo = lo.unwrap_or(0);
                let hi = hi.unwrap_or(2);
                if 0 <= lo && lo < hi && hi <= 2 {
                    let vals = [self.pyversion.0, self.pyversion.1];
                    let val: Vec<i64> = vals[lo as usize..hi as usize].to_vec();
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
        if expr.is_instance(self.classes.comparison_expr_cls.as_ref(self.py))? {
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
            if !right.is_instance(self.classes.str_expr_cls.as_ref(self.py))? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let right_value = right.getattr("value")?;
            let right_str = right_value.downcast::<PyString>()?.to_str()?;
            Ok(fixed_comparison(self.platform()?, &op_str, right_str))
        } else if expr.is_instance(self.classes.call_expr_cls.as_ref(self.py))? {
            let callee = expr.getattr("callee")?;
            if !callee.is_instance(self.classes.member_expr_cls.as_ref(self.py))? {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let args = expr.getattr("args")?;
            let args_list = args.downcast::<PyList>()?;
            if args_list.len() != 1 {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let arg0 = args_list.get_item(0)?;
            if !arg0.is_instance(self.classes.str_expr_cls.as_ref(self.py))? {
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
            let arg_value = arg0.getattr("value")?;
            let arg_str = arg_value.downcast::<PyString>()?.to_str()?;
            if self.platform()?.starts_with(arg_str) {
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
        if pattern.is_instance(self.classes.as_pattern_cls.as_ref(self.py))? {
            let inner = pattern.getattr("pattern")?;
            if inner.is_none() {
                return Ok(ALWAYS_TRUE);
            }
        }
        if pattern.is_instance(self.classes.or_pattern_cls.as_ref(self.py))? {
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
        if expr.is_instance(self.classes.unary_expr_cls.as_ref(self.py))? {
            let op = expr.getattr("op")?;
            let op_str = op.downcast::<PyString>()?.to_str()?;
            if op_str == "not" {
                let inner = expr.getattr("expr")?;
                let positive = self.infer_condition_value(inner)?;
                return Ok(inverted_truth(positive));
            }
        }

        let mut result = TRUTH_VALUE_UNKNOWN;
        let name: Option<&str> = if expr.is_instance(self.classes.name_expr_cls.as_ref(self.py))?
            || expr.is_instance(self.classes.member_expr_cls.as_ref(self.py))?
        {
            let name_obj = expr.getattr("name")?;
            Some(name_obj.downcast::<PyString>()?.to_str()?)
        } else if expr.is_instance(self.classes.op_expr_cls.as_ref(self.py))? {
            let op = expr.getattr("op")?;
            let op_str = op.downcast::<PyString>()?.to_str()?;
            if op_str != "or" && op_str != "and" {
                return Ok(TRUTH_VALUE_UNKNOWN);
            }
            let left = self.infer_condition_value(expr.getattr("left")?)?;
            let right = self.infer_condition_value(expr.getattr("right")?)?;
            let in_set = |v: u8, set: [u8; 2]| v == set[0] || v == set[1];
            if op_str == "or" {
                if left == ALWAYS_TRUE || right == ALWAYS_TRUE {
                    return Ok(ALWAYS_TRUE);
                } else if left == MYPY_TRUE || right == MYPY_TRUE {
                    return Ok(MYPY_TRUE);
                } else if left == MYPY_FALSE && right == MYPY_FALSE {
                    return Ok(MYPY_FALSE);
                } else if in_set(left, [ALWAYS_FALSE, MYPY_FALSE])
                    && in_set(right, [ALWAYS_FALSE, MYPY_FALSE])
                {
                    return Ok(ALWAYS_FALSE);
                }
            } else if left == ALWAYS_FALSE || right == ALWAYS_FALSE {
                return Ok(ALWAYS_FALSE);
            } else if left == MYPY_FALSE || right == MYPY_FALSE {
                return Ok(MYPY_FALSE);
            } else if left == ALWAYS_TRUE && right == ALWAYS_TRUE {
                return Ok(ALWAYS_TRUE);
            } else if in_set(left, [ALWAYS_TRUE, MYPY_TRUE])
                && in_set(right, [ALWAYS_TRUE, MYPY_TRUE])
            {
                return Ok(MYPY_TRUE);
            }
            return Ok(TRUTH_VALUE_UNKNOWN);
        } else {
            // Not NameExpr, MemberExpr, or OpExpr — try sys checks
            result = self.consider_sys_version_info(expr)?;
            if result == TRUTH_VALUE_UNKNOWN {
                result = self.consider_sys_platform(expr)?;
            }
            None
        };

        if result == TRUTH_VALUE_UNKNOWN {
            if let Some(name) = name {
                if name == "PY2" {
                    result = ALWAYS_FALSE;
                } else if name == "PY3" {
                    result = ALWAYS_TRUE;
                } else if name == "MYPY" || name == "TYPE_CHECKING" {
                    result = MYPY_TRUE;
                } else if self.contains_name(self.always_true()?, name)? {
                    result = ALWAYS_TRUE;
                } else if self.contains_name(self.always_false()?, name)? {
                    result = ALWAYS_FALSE;
                }
            }
        }

        Ok(result)
    }

    /// `name in options.always_true/always_false` — membership over the
    /// options list. `always_true`/`always_false` are plain `list[str]`, so
    /// scan the `PyList` with borrowed string views instead of going through
    /// `PySequence_Contains` (which allocates a new Python `str` per call).
    /// Non-list containers fall back to the generic `in` operator.
    fn contains_name(&self, container: &PyAny, name: &str) -> PyResult<bool> {
        if let Ok(items) = container.downcast::<PyList>() {
            for item in items.iter() {
                if let Ok(s) = item.downcast::<PyString>() {
                    if s.to_str()? == name {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        } else {
            container.contains(name)
        }
    }
}

// ---------------------------------------------------------------------------
// Public #[pyfunction] entry points
// ---------------------------------------------------------------------------

/// `mypy.reachability.infer_condition_value` — infer whether a condition is
/// always true/false. Returns one of the truth-value constants
/// (ALWAYS_TRUE=1, MYPY_TRUE=2, ALWAYS_FALSE=3, MYPY_FALSE=4, UNKNOWN=5).
#[pyfunction]
pub(crate) fn rust_infer_condition_value<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
    options: &'py PyAny,
) -> PyResult<u8> {
    let ctx = ReachabilityCtx::new(py, options)?;
    ctx.infer_condition_value(expr)
}

/// `mypy.reachability.infer_pattern_value` — infer truth value of a match
/// pattern. Returns ALWAYS_TRUE=1 or TRUTH_VALUE_UNKNOWN=5.
#[pyfunction]
pub(crate) fn rust_infer_pattern_value<'py>(py: Python<'py>, pattern: &'py PyAny) -> PyResult<u8> {
    let dummy_options = dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
    ctx.infer_pattern_value(pattern)
}

/// `mypy.reachability.assert_will_always_fail` — whether an assert statement
/// always fails. Returns `true` if the condition is ALWAYS_FALSE or MYPY_FALSE.
#[pyfunction]
pub(crate) fn rust_assert_will_always_fail<'py>(
    py: Python<'py>,
    stmt: &'py PyAny,
    options: &'py PyAny,
) -> PyResult<bool> {
    let ctx = ReachabilityCtx::new(py, options)?;
    let expr = stmt.getattr("expr")?;
    let value = ctx.infer_condition_value(expr)?;
    Ok(value == ALWAYS_FALSE || value == MYPY_FALSE)
}

/// `mypy.reachability.consider_sys_version_info` — analyze a
/// `sys.version_info` comparison. Returns a truth-value constant.
#[pyfunction]
pub(crate) fn rust_consider_sys_version_info<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
    pyversion: (i64, i64),
) -> PyResult<u8> {
    let dummy_options = create_dummy_options_with_version(py, pyversion)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
    ctx.consider_sys_version_info(expr)
}

/// `mypy.reachability.consider_sys_platform` — analyze a `sys.platform`
/// comparison. Returns a truth-value constant.
#[pyfunction]
pub(crate) fn rust_consider_sys_platform<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
    platform: &str,
) -> PyResult<u8> {
    let dummy_options = create_dummy_options_with_platform(py, platform)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
    ctx.consider_sys_platform(expr)
}

/// `mypy.reachability.is_sys_attr` — is `expr` a `sys.<name>` member expr?
#[pyfunction]
pub(crate) fn rust_is_sys_attr<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
    name: &str,
) -> PyResult<bool> {
    let dummy_options = dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
    ctx.is_sys_attr(expr, name)
}

/// `mypy.reachability.contains_sys_version_info` — extract the index/slice
/// from a `sys.version_info` expression. Returns `None`, an int, or a
/// `(begin, end)` tuple of ints-or-None.
#[pyfunction]
pub(crate) fn rust_contains_sys_version_info<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
) -> PyResult<Option<PyObject>> {
    let dummy_options = dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
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
pub(crate) fn rust_contains_int_or_tuple_of_ints<'py>(
    py: Python<'py>,
    expr: &'py PyAny,
) -> PyResult<Option<PyObject>> {
    let dummy_options = dummy_options(py)?;
    let ctx = ReachabilityCtx::new(py, dummy_options)?;
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
