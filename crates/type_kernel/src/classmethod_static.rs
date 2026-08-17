//! Ports of the node-class predicates from `mypy/checker.py` used by the
//! complex statement visitors: `is_classmethod_node` and `is_node_static`.
//!
//! These walk live Python AST nodes via PyO3 rather than the wire format:
//! they are shallow structural helpers, so serializing the nodes would cost
//! more than it saves. Each mirrors the Python implementation and returns
//! `Ok(None)` for node classes it does not handle or when an attribute read
//! raises, so Python falls back gracefully (the strangler-fig per-call gate).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyType};

/// Fetch a class from `mypy.nodes`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.nodes")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// Read a boolean flag off a node, deferring (`Ok(None)`) if the attribute
/// read raises, mirroring the Python `try/except` seam around the Rust call.
fn get_flag(node: &PyAny, name: &str) -> PyResult<Option<bool>> {
    match node.getattr(name) {
        Ok(v) => Ok(Some(v.is_true()?)),
        Err(_) => Ok(None),
    }
}

/// Shared walk for both predicates: a `Decorator` unwraps to its `func`,
/// then a `FuncDef` reads `func_flag`, a `Var` reads `var_flag`, and any
/// other node type (including `None` input) yields `Ok(None)`.
fn node_flag(
    py: Python<'_>,
    node: &PyAny,
    func_flag: &str,
    var_flag: &str,
) -> PyResult<Option<bool>> {
    let decorator_cls = nodes_class(py, "Decorator")?;
    let resolved = if node.is_instance(decorator_cls)? {
        match node.getattr("func") {
            Ok(inner) => inner,
            Err(_) => return Ok(None),
        }
    } else {
        node
    };
    let func_def_cls = nodes_class(py, "FuncDef")?;
    if resolved.is_instance(func_def_cls)? {
        return get_flag(resolved, func_flag);
    }
    let var_cls = nodes_class(py, "Var")?;
    if resolved.is_instance(var_cls)? {
        return get_flag(resolved, var_flag);
    }
    Ok(None)
}

/// `mypy.checker.is_classmethod_node` — does the node describe a classmethod?
///
/// Mirrors checker.py:10098-10106.
#[pyfunction]
pub(crate) fn rust_is_classmethod_node(py: Python<'_>, node: &PyAny) -> PyResult<Option<bool>> {
    node_flag(py, node, "is_class", "is_classmethod")
}

/// `mypy.checker.is_node_static` — does the node describe a static function?
///
/// Mirrors checker.py:10109-10117.
#[pyfunction]
pub(crate) fn rust_is_node_static(py: Python<'_>, node: &PyAny) -> PyResult<Option<bool>> {
    node_flag(py, node, "is_static", "is_staticmethod")
}
