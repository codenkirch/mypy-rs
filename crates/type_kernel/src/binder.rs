//! Native port of `mypy/binder.py` pure helper: `get_declaration`
//! (Issue #527).
//!
//! `get_declaration` resolves the declared or inferred type of a `RefExpr`
//! (`NameExpr`/`MemberExpr`). It is the one self-contained, state-free
//! function in the binder module with real Python call sites (2 in
//! `mypy/checker.py`), so it is a clean strangler-fig seam.
//!
//! The stateful core (`ConditionalTypeBinder`, `Frame`, `FrameContext`,
//! `update_from_options`, `assign_type`) stays in Python: it is a
//! mypyc-optimized, heap-heavy class with direct external mutation of
//! `frames`/`Frame.{id,types,conditional_frame}`/`declarations`/`version`,
//! which does not cross a narrow seam cleanly. `can_put_directly` stays in
//! Python too: it requires a full port of `literals.literal` (a recursive
//! `_Hasher` visitor over many expression classes) for a single call site,
//! which is not a net win. `collapse_variadic_union` is internal-only
//! (called solely from `update_from_options`), so it offers no test
//! surface until that method moves.
//!
//! This port follows the parity-only pattern of the recent pure-module
//! ports (e.g. reachability.py, Issue #560): the Rust function is
//! registered in `lib.rs` and exercised by an opt-in parity suite
//! (`NativeBinderSuite` in `mypy/test/testtypes.py`), with no Python
//! production call-site change.

#![allow(non_local_definitions)]

use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::refs::is_instance;

/// Port of `mypy/binder.py:get_declaration`.
///
/// ```python
/// def get_declaration(expr: BindableExpression) -> Type | None:
///     if isinstance(expr, RefExpr):
///         if isinstance(expr.node, Var):
///             type = expr.node.type
///             if not isinstance(get_proper_type(type), PartialType):
///                 return type
///         elif isinstance(expr.node, TypeInfo):
///             return TypeType(fill_typevars_with_any(expr.node))
///     return None
/// ```
///
/// Returns a `(decided, value)` wire answer (Issue #1101): `decided` is
/// true for every path this port handles, including the genuine
/// no-declaration answers (non-`RefExpr`, `Var` without an inferred/declared
/// type yet, a `PartialType`, or a non-`Var`/`TypeInfo` node) — those come
/// back as `(true, None)` so the Python caller skips its walk. The port
/// mirrors the whole Python walk, so `(false, None)` (defer) is currently
/// unreachable; an exception still propagates and the caller falls back.
#[pyfunction]
pub fn rust_get_declaration(py: Python<'_>, expr: &PyAny) -> PyResult<(bool, PyObject)> {
    Ok((true, get_declaration_inner(py, expr)?))
}

fn get_declaration_inner(py: Python<'_>, expr: &PyAny) -> PyResult<PyObject> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !expr.is_instance(ref_expr_cls)? {
        return Ok(py.None());
    }

    let node = expr.getattr("node")?;
    if node.is_none() {
        return Ok(py.None());
    }

    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
    let type_info_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;

    if node.is_instance(var_cls)? {
        let typ = node.getattr("type")?;
        if typ.is_none() {
            // `Var.type` can be None (not inferred yet) — mirror the Python
            // fall-through (returns None at the end).
            return Ok(py.None());
        }
        let types_mod = py.import("mypy.types")?;
        let proper = types_mod.getattr("get_proper_type")?.call1((typ,))?;
        let partial_type_cls: &PyType = types_mod.getattr("PartialType")?.downcast()?;
        if is_instance(proper, partial_type_cls) {
            return Ok(py.None());
        }
        return Ok(typ.into());
    }

    if node.is_instance(type_info_cls)? {
        let typevars_mod = py.import("mypy.typevars")?;
        let fill_typevars_with_any = typevars_mod.getattr("fill_typevars_with_any")?;
        let filled = fill_typevars_with_any.call1((node,))?;
        let types_mod = py.import("mypy.types")?;
        let type_type_cls = types_mod.getattr("TypeType")?;
        return Ok(type_type_cls.call1((filled,))?.into());
    }

    Ok(py.None())
}
