//! Native port of `mypy.constant_fold`.
//!
//! Mirrors `constant_fold_expr`, `constant_fold_binary_op`,
//! `constant_fold_binary_int_op`, `constant_fold_binary_float_op`, and
//! `constant_fold_unary_op` from `mypy/constant_fold.py`.
//!
//! Design: we walk live Python AST nodes the same way the Python code
//! does. Instead of re-implementing Python's arbitrary-precision int /
//! float / complex / str arithmetic in Rust (which would lose precision
//! parity), we construct the Python scalar results by calling back into
//! the Python interpreter via `PyAny` rich-comparison and `__add__` /
//! `__sub__` / etc. dunders. This keeps the semantics identical while
//! moving the *dispatch* (which node kinds fold, which ops apply, the
//! `cur_mod_id` final-var binding) into Rust.
//!
//! Entry point:
//! `rust_constant_fold_expr(expr, cur_mod_id) -> (bool, PyObject)`
//! returns a `(decided, value)` wire answer (Issue #1101): the port
//! mirrors the whole Python walk, so every call is decided — a foldable
//! expression yields `(true, scalar)` and an un-foldable one yields
//! `(true, None)` so the Python caller skips its chain. `(false, None)`
//! (defer) is currently unreachable; an exception propagates and the
//! caller falls through as before.
//!
//! Target: PyO3 0.20.x (`&PyAny`, not `Bound`).

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyComplex, PyFloat, PyInt, PyString, PyType};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// True if `v` is a Python `int` but NOT a `bool` (bool is an int subclass;
/// the Python code treats bool separately via the `True`/`False` NameExpr
/// path, so int ops should not fire on bools here). Uses the Python type
/// check (not `extract::<i64>`) so arbitrary-precision ints are accepted.
fn is_plain_int(v: &PyAny) -> bool {
    v.is_instance_of::<PyInt>() && !v.is_instance_of::<PyBool>()
}

fn is_float(v: &PyAny) -> bool {
    v.is_instance_of::<PyFloat>()
}

fn is_str(v: &PyAny) -> bool {
    v.is_instance_of::<PyString>()
}

fn is_complex(v: &PyAny) -> bool {
    v.is_instance_of::<PyComplex>()
}

fn is_const_type(v: &PyAny) -> bool {
    is_plain_int(v) || is_float(v) || is_complex(v) || is_str(v) || v.is_instance_of::<PyBool>()
}

/// True if `v` is a plain int (not bool) and `v >= 0`, using Python
/// comparison so arbitrary-precision ints are handled correctly.
fn is_nonneg_int(v: &PyAny) -> PyResult<bool> {
    if !is_plain_int(v) {
        return Ok(false);
    }
    let zero = v.py().import("builtins")?.getattr("int")?.call1((0,))?;
    let cmp = v.rich_compare(zero, pyo3::basic::CompareOp::Ge)?;
    cmp.is_true()
}

/// Call `left.__op__(right)` falling back to `right.__rop__(left)` on
/// `NotImplemented`, mirroring how Python itself dispatches binary ops.
/// Returns the result object, or None if the operation is not defined.
fn bin_op(py: Python<'_>, left: &PyAny, op: &str, right: &PyAny) -> PyResult<Option<PyObject>> {
    let left_dunder = format!("__{}__", op);
    let right_dunder = format!("__r{}__", op);
    let not_implemented: PyObject = py.NotImplemented();
    let call_dunder = |recv: &PyAny, other: &PyAny, name: &str| -> PyResult<PyObject> {
        let f = match recv.getattr(name) {
            Ok(f) => f,
            Err(_) => return Ok(not_implemented.clone_ref(py)),
        };
        Ok(f.call1((other,))?.into())
    };
    let l = call_dunder(left, right, &left_dunder)?;
    if !l.is(&not_implemented) {
        return Ok(Some(l));
    }
    let r = call_dunder(right, left, &right_dunder)?;
    if !r.is(&not_implemented) {
        return Ok(Some(r));
    }
    Ok(None)
}

/// `__neg__` / `__pos__` / `__invert__`.
fn unary_dunder(py: Python<'_>, recv: &PyAny, name: &str) -> PyResult<PyObject> {
    let not_implemented: PyObject = py.NotImplemented();
    let f = recv.getattr(format!("__{}__", name).as_str())?;
    let r: PyObject = f.call0()?.into();
    if r.is(&not_implemented) {
        Ok(py.None())
    } else {
        Ok(r)
    }
}

// ---------------------------------------------------------------------------
// the three typed dispatchers (mirror the Python functions)
// ---------------------------------------------------------------------------

fn binary_int_op(
    py: Python<'_>,
    op: &str,
    left: &PyAny,
    right: &PyAny,
) -> PyResult<Option<PyObject>> {
    match op {
        "+" => bin_op(py, left, "add", right),
        "-" => bin_op(py, left, "sub", right),
        "*" => bin_op(py, left, "mul", right),
        "/" => {
            if !right.is_true()? {
                return Ok(None);
            }
            bin_op(py, left, "truediv", right)
        }
        "//" => {
            if !right.is_true()? {
                return Ok(None);
            }
            bin_op(py, left, "floordiv", right)
        }
        "%" => {
            if !right.is_true()? {
                return Ok(None);
            }
            bin_op(py, left, "mod", right)
        }
        "&" => bin_op(py, left, "and", right),
        "|" => bin_op(py, left, "or", right),
        "^" => bin_op(py, left, "xor", right),
        "<<" => {
            if !is_nonneg_int(right)? {
                return Ok(None);
            }
            bin_op(py, left, "lshift", right)
        }
        ">>" => {
            if !is_nonneg_int(right)? {
                return Ok(None);
            }
            bin_op(py, left, "rshift", right)
        }
        "**" => {
            if !is_nonneg_int(right)? {
                return Ok(None);
            }
            // Python: `left ** right`; for ints this stays exact. We must
            // guard against OverflowError (Python's ints are arbitrary
            // precision but PyO3/CPython may still raise on huge exponents

            // in some builds) by catching and returning None.
            let pow = py.import("operator")?.getattr("pow")?;
            match pow.call1((left, right)) {
                Ok(v) => Ok(Some(v.into())),
                Err(_) => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn binary_float_op(
    py: Python<'_>,
    op: &str,
    left: &PyAny,
    right: &PyAny,
) -> PyResult<Option<PyObject>> {
    match op {
        "+" => bin_op(py, left, "add", right),
        "-" => bin_op(py, left, "sub", right),
        "*" => bin_op(py, left, "mul", right),
        "/" => {
            // right != 0 check. Use Python truthiness via `operator.truth`.
            let truth = right.is_true()?;
            if !truth {
                return Ok(None);
            }
            bin_op(py, left, "truediv", right)
        }
        "//" => {
            let truth = right.is_true()?;
            if !truth {
                return Ok(None);
            }
            bin_op(py, left, "floordiv", right)
        }
        "%" => {
            let truth = right.is_true()?;
            if !truth {
                return Ok(None);
            }
            bin_op(py, left, "mod", right)
        }
        "**" => {
            // Mirror: `(left < 0 and isinstance(right, int)) or left > 0`.
            // If the condition is false, return None (Python falls through).
            let left_lt0 = left
                .rich_compare(right, pyo3::basic::CompareOp::Lt)?
                .is_true()?;
            let right_is_int = is_plain_int(right);
            let left_gt0 = {
                let zero = py.import("builtins")?.getattr("int")?.call1((0,))?;
                left.rich_compare(zero, pyo3::basic::CompareOp::Gt)?
                    .is_true()?
            };
            let cond = (left_lt0 && right_is_int) || left_gt0;
            if !cond {
                return Ok(None);
            }
            let pow = py.import("operator")?.getattr("pow")?;
            match pow.call1((left, right)) {
                Ok(v) => Ok(Some(v.into())),
                Err(_) => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn unary_op(py: Python<'_>, op: &str, value: &PyAny) -> PyResult<Option<PyObject>> {
    match op {
        "-" if is_plain_int(value) || is_float(value) => Ok(Some(unary_dunder(py, value, "neg")?)),
        "~" if is_plain_int(value) => Ok(Some(unary_dunder(py, value, "invert")?)),
        "+" if is_plain_int(value) || is_float(value) => Ok(Some(unary_dunder(py, value, "pos")?)),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// binary_op dispatch (mirrors constant_fold_binary_op)
// ---------------------------------------------------------------------------

fn binary_op(py: Python<'_>, op: &str, left: &PyAny, right: &PyAny) -> PyResult<Option<PyObject>> {
    // int + int
    if is_plain_int(left) && is_plain_int(right) {
        return binary_int_op(py, op, left, right);
    }
    // float paths (float+float, float+int, int+float)
    let l_float = is_float(left);
    let r_float = is_float(right);
    if (l_float && r_float) || (l_float && is_plain_int(right)) || (is_plain_int(left) && r_float) {
        return binary_float_op(py, op, left, right);
    }
    // string concat / multiply
    if op == "+" && is_str(left) && is_str(right) {
        return bin_op(py, left, "add", right);
    }
    if op == "*" && is_str(left) && is_plain_int(right) {
        return bin_op(py, left, "mul", right);
    }
    if op == "*" && is_plain_int(left) && is_str(right) {
        return bin_op(py, right, "mul", left);
    }
    // complex construction (+/-)
    let l_num = is_plain_int(left) || is_float(left);
    let r_num = is_plain_int(right) || is_float(right);
    if op == "+" && l_num && is_complex(right) {
        return bin_op(py, left, "add", right);
    }
    if op == "+" && is_complex(left) && r_num {
        return bin_op(py, left, "add", right);
    }
    if op == "-" && l_num && is_complex(right) {
        return bin_op(py, left, "sub", right);
    }
    if op == "-" && is_complex(left) && r_num {
        return bin_op(py, left, "sub", right);
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// recursive AST walk (mirrors constant_fold_expr)
// ---------------------------------------------------------------------------

fn fold_expr(
    py: Python<'_>,
    expr: &PyAny,
    cur_mod_id: &str,
    nodes_mod: &PyAny,
) -> PyResult<Option<PyObject>> {
    // Leaf types: IntExpr / StrExpr / FloatExpr / ComplexExpr have a
    // `.value` attribute that is already the folded constant.
    macro_rules! try_class {
        ($name:literal) => {
            nodes_mod.getattr($name)?.downcast::<PyType>().ok()
        };
    }
    let int_cls = try_class!("IntExpr");
    let str_cls = try_class!("StrExpr");
    let float_cls = try_class!("FloatExpr");
    let complex_cls = try_class!("ComplexExpr");
    let name_cls = try_class!("NameExpr");
    let op_cls = try_class!("OpExpr");
    let unary_cls = try_class!("UnaryExpr");
    let var_cls = try_class!("Var");

    if let Some(cls) = int_cls {
        if expr.is_instance(cls)? {
            return Ok(Some(expr.getattr("value")?.into()));
        }
    }
    if let Some(cls) = str_cls {
        if expr.is_instance(cls)? {
            return Ok(Some(expr.getattr("value")?.into()));
        }
    }
    if let Some(cls) = float_cls {
        if expr.is_instance(cls)? {
            return Ok(Some(expr.getattr("value")?.into()));
        }
    }
    if let Some(cls) = complex_cls {
        if expr.is_instance(cls)? {
            return Ok(Some(expr.getattr("value")?.into()));
        }
    }
    if let Some(cls) = name_cls {
        if expr.is_instance(cls)? {
            let name: String = expr.getattr("name")?.extract()?;
            if name == "True" {
                return Ok(Some(true.to_object(py)));
            }
            if name == "False" {
                return Ok(Some(false.to_object(py)));
            }
            let node = expr.getattr("node")?;
            if let Some(var_cls) = var_cls {
                if node.is_instance(var_cls)? {
                    let is_final: bool = node.getattr("is_final")?.extract()?;
                    if is_final {
                        let fullname: String = node.getattr("fullname")?.extract()?;
                        let parent = fullname.rsplit('.').next().unwrap_or("");
                        // rsplit('.', 1)[0] in Python: everything before the
                        // last dot, or the whole string if no dot.
                        let prefix = if let Some(idx) = fullname.rfind('.') {
                            &fullname[..idx]
                        } else {
                            fullname.as_str()
                        };
                        if prefix == cur_mod_id {
                            let value = node.getattr("final_value")?;
                            if is_const_type(value) {
                                return Ok(Some(value.into()));
                            }
                        }
                        let _ = parent; // suppress unused
                    }
                }
            }
            return Ok(None);
        }
    }
    if let Some(cls) = op_cls {
        if expr.is_instance(cls)? {
            let op: String = expr.getattr("op")?.extract()?;
            let left = expr.getattr("left")?;
            let right = expr.getattr("right")?;
            let l = fold_expr(py, left, cur_mod_id, nodes_mod)?;
            let r = fold_expr(py, right, cur_mod_id, nodes_mod)?;
            match (l, r) {
                (Some(lv), Some(rv)) => {
                    return binary_op(py, &op, lv.into_ref(py), rv.into_ref(py));
                }
                _ => return Ok(None),
            }
        }
    }
    if let Some(cls) = unary_cls {
        if expr.is_instance(cls)? {
            let op: String = expr.getattr("op")?.extract()?;
            let inner = expr.getattr("expr")?;
            let v = fold_expr(py, inner, cur_mod_id, nodes_mod)?;
            if let Some(vv) = v {
                return unary_op(py, &op, vv.into_ref(py));
            }
            return Ok(None);
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// pyfunction
// ---------------------------------------------------------------------------

/// `mypy.constant_fold.constant_fold_expr` — fold a constant expression
/// AST node into its scalar value, or None (as a `(decided, value)`
/// wire answer; see the module doc).
#[pyfunction]
pub(crate) fn rust_constant_fold_expr(
    py: Python<'_>,
    expr: &PyAny,
    cur_mod_id: &str,
) -> PyResult<(bool, PyObject)> {
    let nodes_mod = py.import("mypy.nodes")?;
    let value = fold_expr(py, expr, cur_mod_id, nodes_mod)?;
    Ok((true, value.unwrap_or_else(|| py.None())))
}
