//! Port of `ExpressionChecker.always_returns_none` /
//! `defn_returns_none` (mypy/checkexpr.py:1712-1756): does the callee
//! refer to something explicitly annotated as only returning `None`?
//!
//! Live-PyO3-object seam in the `rust_is_magic_base` /
//! `rust_is_final_enum_value` shape: isinstance + attribute reads, zero
//! wire bytes. The recursion (`defn_returns_none` over
//! `OverloadedFuncDef.items` and `Var.__call__`) runs entirely in Rust;
//! the ret None-ness of `FuncDef.type` / `Var.type` is decided by calling
//! the real Python `get_proper_type`, never a bare attribute read, so a
//! partially-fixed wire object defers instead of corrupting the answer.
//! The MemberExpr owner-type resolution is checker state
//! (`chk.lookup_type`), so the shim pre-resolves it and passes the
//! resulting `TypeInfo`; Rust never touches checker state.
//!
//! Strangler-fig contract: any unreadable attribute or a failed
//! `get_proper_type` call defers (`None`), and the shim re-runs the
//! untouched pure-Python body.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyType};

/// Fetch a class from `mypy.nodes`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.nodes")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// The `OverloadedFuncDef` fold over per-item decisions
/// (checkexpr.py:1741): `all(self.defn_returns_none(item) ...)`. A
/// `false` item short-circuits before a later deferring item is
/// consulted, mirroring Python's `all()` laziness; a deferring item that
/// is reached defers the whole overload.
fn all_items_return_none<I: Iterator<Item = Option<bool>>>(items: I) -> Option<bool> {
    for item in items {
        match item {
            Some(true) => {}
            Some(false) => return Some(false),
            None => return None,
        }
    }
    Some(true)
}

/// The `Var` arm decision from pre-resolved facts (checkexpr.py:1742-1754).
/// `annotated_ret_none` is the `not is_inferred and CallableType and
/// ret-is-None` conjunction read in Python branch order; `call_returns_none`
/// is the `__call__` recursion result, where `None` defers and a missing
/// symbol or a non-Instance type folds into `Some(false)`.
fn var_returns_none_decision(
    is_inferred: bool,
    annotated_ret_none: bool,
    call_returns_none: Option<bool>,
) -> Option<bool> {
    if !is_inferred && annotated_ret_none {
        return Some(true);
    }
    call_returns_none
}

/// The recursive `defn_returns_none` walk. Errors propagate to the
/// `#[pyfunction]` boundary, which converts any unreadable fact into a
/// deferral.
fn defn_returns_none_inner(py: Python<'_>, defn: &PyAny) -> PyResult<Option<bool>> {
    let types = py.import("mypy.types")?;
    let func_def_cls = nodes_class(py, "FuncDef")?;
    let overloaded_cls = nodes_class(py, "OverloadedFuncDef")?;
    let var_cls = nodes_class(py, "Var")?;
    let callable_cls = types.getattr("CallableType")?.downcast::<PyType>()?;
    let instance_cls = types.getattr("Instance")?.downcast::<PyType>()?;
    let none_type_cls = types.getattr("NoneType")?.downcast::<PyType>()?;
    let get_proper = types.getattr("get_proper_type")?;

    if defn.is_instance(func_def_cls)? {
        let defn_type = defn.getattr("type")?;
        if !defn_type.is_instance(callable_cls)? {
            return Ok(Some(false));
        }
        let ret = get_proper.call1((defn_type.getattr("ret_type")?,))?;
        return Ok(Some(ret.is_instance(none_type_cls)?));
    }
    if defn.is_instance(overloaded_cls)? {
        let items = defn.getattr("items")?;
        let decisions = items.iter()?.map(|item| {
            item.and_then(|i| defn_returns_none_inner(py, i))
                .unwrap_or(None)
        });
        return Ok(all_items_return_none(decisions));
    }
    if defn.is_instance(var_cls)? {
        let typ = get_proper.call1((defn.getattr("type")?,))?;
        let is_inferred: bool = defn.getattr("is_inferred")?.extract()?;
        let annotated_ret_none = !is_inferred && typ.is_instance(callable_cls)? && {
            let ret = get_proper.call1((typ.getattr("ret_type")?,))?;
            ret.is_instance(none_type_cls)?
        };
        let call_returns_none = if typ.is_instance(instance_cls)? {
            let sym = typ.getattr("type")?.call_method1("get", ("__call__",))?;
            if sym.is_none() {
                Some(false)
            } else {
                defn_returns_none_inner(py, sym.getattr("node")?)?
            }
        } else {
            Some(false)
        };
        return Ok(var_returns_none_decision(
            is_inferred,
            annotated_ret_none,
            call_returns_none,
        ));
    }
    // Any other node kind (Decorator, TypeInfo, None, ...) is False,
    // mirroring the Python tail.
    Ok(Some(false))
}

/// `ExpressionChecker.always_returns_none` (checkexpr.py:1712-1733) as a
/// thin wrapper over the recursive `defn_returns_none` walk. The shim
/// pre-resolves the MemberExpr owner type (checker state) and passes the
/// resulting `TypeInfo` in `info`; the RefExpr arm is decided from the
/// live `node.node`. Any unreadable fact defers (`None`), so the shim
/// re-runs the pure-Python body unchanged.
#[pyfunction]
#[pyo3(signature = (node, info))]
pub(crate) fn rust_always_returns_none(
    py: Python<'_>,
    node: &PyAny,
    info: Option<&PyAny>,
) -> PyResult<Option<bool>> {
    let result = always_returns_none_inner(py, node, info);
    Ok(result.unwrap_or(None))
}

fn always_returns_none_inner(
    py: Python<'_>,
    node: &PyAny,
    info: Option<&PyAny>,
) -> PyResult<Option<bool>> {
    // MemberExpr is a RefExpr subclass, so the arms are sequential ifs in
    // Python (checkexpr.py:1714-1732), not elifs: an analyzed MemberExpr
    // (node.node set) can still return True through the RefExpr arm.
    let ref_expr_cls = nodes_class(py, "RefExpr")?;
    if node.is_instance(ref_expr_cls)? {
        let defn = node.getattr("node")?;
        match defn_returns_none_inner(py, defn)? {
            Some(true) => return Ok(Some(true)),
            Some(false) => {}
            None => return Ok(None),
        }
    }
    let member_expr_cls = nodes_class(py, "MemberExpr")?;
    if node.is_instance(member_expr_cls)? && node.getattr("node")?.is_none() {
        // Only an unanalyzed attribute access consults the owner type,
        // mirroring the Python `node.node is None` gate.
        let info = match info {
            Some(i) => i,
            None => return Ok(None),
        };
        let sym = info.call_method1("get", (node.getattr("name")?,))?;
        if !sym.is_none() {
            let sym_node = sym.getattr("node")?;
            return defn_returns_none_inner(py, sym_node);
        }
    }
    Ok(Some(false))
}

#[cfg(test)]
mod returns_none_tests {
    use super::{all_items_return_none, var_returns_none_decision};

    #[test]
    fn test_overload_vacuous_is_true() {
        // `all(...)` over an empty item list is True in Python.
        assert_eq!(all_items_return_none(std::iter::empty()), Some(true));
    }

    #[test]
    fn test_overload_all_true() {
        let items = vec![Some(true), Some(true)];
        assert_eq!(all_items_return_none(items.into_iter()), Some(true));
    }

    #[test]
    fn test_overload_any_false_short_circuits() {
        let items = vec![Some(true), Some(false), Some(true)];
        assert_eq!(all_items_return_none(items.into_iter()), Some(false));
    }

    #[test]
    fn test_overload_false_beats_later_defer() {
        // Python `all()` short-circuits on the false item and never
        // consults the deferred tail.
        let items = vec![Some(false), None];
        assert_eq!(all_items_return_none(items.into_iter()), Some(false));
    }

    #[test]
    fn test_overload_reached_defer_propagates() {
        let items = vec![Some(true), None];
        assert_eq!(all_items_return_none(items.into_iter()), None);
    }

    #[test]
    fn test_var_annotated_none_returns_true() {
        assert_eq!(
            var_returns_none_decision(false, true, Some(false)),
            Some(true)
        );
    }

    #[test]
    fn test_var_inferred_annotated_ignored() {
        // An inferred Var never fires the annotated-None arm even when
        // the type is a None-returning callable.
        assert_eq!(
            var_returns_none_decision(true, true, Some(false)),
            Some(false)
        );
    }

    #[test]
    fn test_var_instance_call_recursion_true() {
        assert_eq!(
            var_returns_none_decision(true, false, Some(true)),
            Some(true)
        );
    }

    #[test]
    fn test_var_instance_call_false() {
        assert_eq!(
            var_returns_none_decision(false, false, Some(false)),
            Some(false)
        );
    }

    #[test]
    fn test_var_call_defer_propagates() {
        assert_eq!(var_returns_none_decision(true, false, None), None);
    }

    #[test]
    fn test_var_non_instance_is_false() {
        // A non-Instance, non-None-ret type folds into Some(false).
        assert_eq!(
            var_returns_none_decision(true, false, Some(false)),
            Some(false)
        );
    }
}
