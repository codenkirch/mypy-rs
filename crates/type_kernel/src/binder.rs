//! Native port of `mypy/binder.py` helpers.
//!
//! `get_declaration` (Issue #527) is the state-free seam; the
//! `NativeBinder` store (H1b, wave 74) ports the frame-stack metadata
//! and the hot `is_unreachable` / `is_unreachable_warning_suppressed`
//! queries that Python answers by scanning every frame on every call.
//!
//! The type-merging core (`update_from_options`, `assign_type`,
//! `allow_jump`) stays in Python: it needs `Frame.types` dicts whose
//! keys are Python tuples containing live `Var` objects, which cannot
//! be hashed in Rust.  Break/continue/try frame tracking also stays
//! Python-side: those are simple list/set ops that don't benefit from
//! native code, and `allow_jump` doesn't modify `self.frames` at all.

#![allow(non_local_definitions)]

use std::cell::RefCell;

use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::refs::is_instance;

// ---------------------------------------------------------------------------
// get_declaration (Issue #527)
// ---------------------------------------------------------------------------

/// Port of `mypy/binder.py:get_declaration`.
///
/// Returns a `(decided, value)` wire answer (Issue #1101): `decided` is
/// true for every path this port handles, including the genuine
/// no-declaration answers — those come back as `(true, None)` so the
/// Python caller skips its walk.
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

// ---------------------------------------------------------------------------
// NativeBinder frame-stack store (H1b, wave 74)
// ---------------------------------------------------------------------------

/// Per-frame metadata.  `Frame.types` stays in Python (keys are Python
/// tuples with live `Var` objects); Rust only tracks the scalars needed
/// for the hot reachability queries.
struct RustFrame {
    unreachable: bool,
    suppress_unreachable_warnings: bool,
}

struct BinderStore {
    frames: Vec<RustFrame>,
    /// Cached count of frames with `unreachable == true` so
    /// `is_unreachable` is O(1) instead of Python's `any()` scan.
    unreachable_count: i64,
    /// Same cache for `suppress_unreachable_warnings`.
    suppressed_count: i64,
}

impl BinderStore {
    fn new() -> Self {
        BinderStore {
            frames: vec![RustFrame {
                unreachable: false,
                suppress_unreachable_warnings: false,
            }],
            unreachable_count: 0,
            suppressed_count: 0,
        }
    }

    fn push_frame(&mut self) {
        self.frames.push(RustFrame {
            unreachable: false,
            suppress_unreachable_warnings: false,
        });
    }

    fn pop_frame(&mut self) {
        if self.frames.len() <= 1 {
            return;
        }
        let f = self.frames.pop().unwrap();
        if f.unreachable {
            self.unreachable_count -= 1;
        }
        if f.suppress_unreachable_warnings {
            self.suppressed_count -= 1;
        }
    }

    fn is_unreachable(&self) -> bool {
        self.unreachable_count > 0
    }

    fn is_unreachable_warning_suppressed(&self) -> bool {
        self.suppressed_count > 0
    }

    fn set_unreachable(&mut self) {
        let f = self.frames.last_mut().unwrap();
        if !f.unreachable {
            f.unreachable = true;
            self.unreachable_count += 1;
        }
    }

    /// Sync the top frame's `unreachable` flag after `update_from_options`
    /// sets `self.frames[-1].unreachable = not frames`.  Can set or clear.
    fn set_top_unreachable(&mut self, v: bool) {
        let f = self.frames.last_mut().unwrap();
        if v != f.unreachable {
            f.unreachable = v;
            if v {
                self.unreachable_count += 1;
            } else {
                self.unreachable_count -= 1;
            }
        }
    }

    fn suppress_warnings(&mut self) {
        let f = self.frames.last_mut().unwrap();
        if !f.suppress_unreachable_warnings {
            f.suppress_unreachable_warnings = true;
            self.suppressed_count += 1;
        }
    }

    fn frame_count(&self) -> i64 {
        self.frames.len() as i64
    }
}

thread_local! {
    static STORE: RefCell<Option<BinderStore>> = const { RefCell::new(None) };
}

fn with_store<T>(f: impl FnOnce(&mut BinderStore) -> T) -> T {
    STORE.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(BinderStore::new());
        }
        f(guard.as_mut().unwrap())
    })
}

// --- pyfunctions ---

#[pyfunction]
pub fn rust_binder_new() {
    STORE.with(|cell| {
        *cell.borrow_mut() = Some(BinderStore::new());
    });
}

#[pyfunction]
pub fn rust_binder_reset() {
    STORE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

#[pyfunction]
pub fn rust_binder_push_frame() {
    with_store(|s| s.push_frame());
}

#[pyfunction]
pub fn rust_binder_pop_frame() {
    with_store(|s| s.pop_frame());
}

#[pyfunction]
pub fn rust_binder_is_unreachable() -> bool {
    with_store(|s| s.is_unreachable())
}

#[pyfunction]
pub fn rust_binder_is_unreachable_warning_suppressed() -> bool {
    with_store(|s| s.is_unreachable_warning_suppressed())
}

#[pyfunction]
pub fn rust_binder_set_unreachable() {
    with_store(|s| s.set_unreachable());
}

#[pyfunction]
pub fn rust_binder_set_top_unreachable(v: bool) {
    with_store(|s| s.set_top_unreachable(v));
}

#[pyfunction]
pub fn rust_binder_suppress_unreachable_warnings() {
    with_store(|s| s.suppress_warnings());
}

#[pyfunction]
pub fn rust_binder_frame_count() -> i64 {
    with_store(|s| s.frame_count())
}

/// Register this module's Python-facing seam surface (#1677).
pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    // Issue #527: binder.py pure helper (get_declaration).
    m.add_function(wrap_pyfunction!(rust_get_declaration, m)?)?;

    // H1b (wave 74): native binder frame-stack metadata store.
    m.add_function(wrap_pyfunction!(rust_binder_new, m)?)?;

    m.add_function(wrap_pyfunction!(rust_binder_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_binder_push_frame, m)?)?;

    m.add_function(wrap_pyfunction!(rust_binder_pop_frame, m)?)?;

    m.add_function(wrap_pyfunction!(rust_binder_is_unreachable, m)?)?;

    m.add_function(wrap_pyfunction!(
        rust_binder_is_unreachable_warning_suppressed,
        m
    )?)?;

    m.add_function(wrap_pyfunction!(rust_binder_set_unreachable, m)?)?;

    m.add_function(wrap_pyfunction!(rust_binder_set_top_unreachable, m)?)?;

    m.add_function(wrap_pyfunction!(
        rust_binder_suppress_unreachable_warnings,
        m
    )?)?;

    m.add_function(wrap_pyfunction!(rust_binder_frame_count, m)?)?;
    Ok(())
}
