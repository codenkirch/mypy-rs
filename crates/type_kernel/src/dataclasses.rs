//! Dataclasses plugin transform seam (Issue #356).
//!
//! The `@dataclass` class-maker callback in `mypy.plugins.dataclasses`
//! is routed through this Rust seam when `Options.native_type_kernel` is
//! set. The transform operates on live AST nodes: it injects
//! `Argument`/`TypeInfo`/`SymbolTableNode` objects directly into the
//! semantic-analysis state to synthesize `__init__`, `__eq__`, `__lt__`,
//! and the other ordering methods, and it needs the original default
//! expressions (`attr.default`) to build correct signatures. That coupling
//! is why this returns `None` always: a live-AST transform is not
//! representable on the type-kernel wire seam (which exchanges plain
//! `Type` objects). Deferring keeps the pure-Python `DataclassTransformer`
//! in charge, unchanged, while the gate keeps the call site and extension
//! ABI in place for a future AST-serialization parity port, per the
//! strangler-fig per-call-gate pattern.
//!
use pyo3::prelude::*;

/// Rust seam for the dataclasses class-maker callback.
/// Signature mirrors `mypy/plugins/dataclasses.py`'s invocation,
/// `(cls, reason, api)`. Always returns `None`, deferring the whole
/// dataclass transform to Python.
#[pyfunction]
pub fn rust_dataclass_transform(
    _py: Python<'_>,
    _cls: &PyAny,
    _reason: &str,
    _api: &PyAny,
) -> PyResult<Option<PyObject>> {
    Ok(None)
}
