//! Stable-ID service for live Python `Type`-graph objects (Phase F0, #1349).
//!
//! The wire format carries live-object references as stable IDs (the
//! `type_ref`-style fields in `doc/f0_coverage.md`). As Phase F moves graph
//! ownership into Rust, seams will need to hand Rust a handle for a live
//! Python object that is stable across calls, without copying the object or
//! leaking `id()` collisions after GC. This module is that handle mint.
//!
//! Guarantees:
//! 1. **Thread-local.** The registry lives in a `thread_local!` cell: handles
//!    are only meaningful on the thread that minted them, so no locking is
//!    needed and the GIL remains the only cross-thread synchronization point.
//! 2. **Idempotent per live object.** `handle_for` returns the same handle
//!    for the same object until the entry is retired or the registry resets.
//! 3. **Bounded lifetime by contract.** Keys are raw `id()`s: a freed object
//!    can have its id recycled, so the enclosing seam MUST reset the
//!    registry at the same lifecycle boundary it clears the per-build
//!    resolvers (the `_clear_native_*` discipline in `mypy/build.py`).
//!    `reset` bumps a generation counter so stale handles are detectable;
//!    handles minted before a reset are never re-issued, but ids may be
//!    re-registered with fresh handles after one.
//! 4. **No production callers in this issue.** Reserved for the #1344
//!    pair-identity registry and the Phase F graph-ownership stages; the
//!    `allow(dead_code)` here is deliberate, matching `wire.rs`'s Stage-3a
//!    posture (parity-tested, not yet wired to production paths).

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::prelude::*;

/// Reserved "no identity" value so `Option<u64>` stays unambiguous against
/// a legitimate handle.
#[allow(dead_code)]
const NO_IDENTITY: u64 = 0;

struct Registry {
    /// Monotonic handle counter; starts at 1 so 0 stays reserved.
    next_handle: u64,
    /// Raw object pointer (`id()`-equivalent) -> minted handle.
    by_id: HashMap<usize, u64>,
    /// Bumped by `reset` so callers can detect stale handles.
    generation: u64,
}

impl Registry {
    fn new() -> Self {
        Registry {
            next_handle: 1,
            by_id: HashMap::new(),
            generation: 1,
        }
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::new());
}

fn with_registry<T>(f: impl FnOnce(&mut Registry) -> T) -> T {
    REGISTRY.with(|cell| f(&mut cell.borrow_mut()))
}

/// Mint (or return the existing) stable handle for a live Python object.
pub(crate) fn handle_for(obj: &PyAny) -> Option<u64> {
    with_registry(|reg| {
        let key = obj.as_ptr() as usize;
        Some(*reg.by_id.entry(key).or_insert_with(|| {
            let h = reg.next_handle;
            reg.next_handle += 1;
            h
        }))
    })
}

/// Look up an existing handle without minting one. `None` means the object
/// has never been registered on this thread (or the registry was reset).
#[allow(dead_code)]
pub(crate) fn handle_of(obj: &PyAny) -> Option<u64> {
    with_registry(|reg| reg.by_id.get(&(obj.as_ptr() as usize)).copied())
}

/// Drop the mapping for a live object. A later `handle_for` on the same
/// object mints a fresh handle.
#[allow(dead_code)]
pub(crate) fn retire(obj: &PyAny) {
    with_registry(|reg| {
        reg.by_id.remove(&(obj.as_ptr() as usize));
    });
}

/// Clear the thread-local registry and bump the generation. Returns the new
/// generation. Call at the same lifecycle boundary the enclosing build
/// clears its per-build resolvers (guarantee 3).
#[allow(dead_code)]
pub(crate) fn reset() -> u64 {
    with_registry(|reg| {
        let generation = reg.generation + 1;
        // `next_handle` is preserved: handles are never re-issued after a
        // reset (guarantee 3), so pre-reset handles stay detectable as stale.
        *reg = Registry {
            next_handle: reg.next_handle,
            by_id: HashMap::new(),
            generation,
        };
        generation
    })
}

/// Current registry generation (lets callers detect stale handles).
#[allow(dead_code)]
pub(crate) fn generation() -> u64 {
    with_registry(|reg| reg.generation)
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    #[test]
    fn test_handle_for_is_idempotent() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = handle_for(obj).unwrap();
            assert_eq!(handle_for(obj), Some(h));
            assert_ne!(h, NO_IDENTITY);
        });
    }

    #[test]
    fn test_distinct_objects_get_distinct_handles() {
        with_py(|py| {
            reset();
            let a = fresh_object(py);
            let b = fresh_object(py);
            assert_ne!(handle_for(a), handle_for(b));
        });
    }

    #[test]
    fn test_handle_of_does_not_mint() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            assert_eq!(handle_of(obj), None);
            let h = handle_for(obj).unwrap();
            assert_eq!(handle_of(obj), Some(h));
        });
    }

    #[test]
    fn test_retire_drops_and_re_mints() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h1 = handle_for(obj).unwrap();
            retire(obj);
            assert_eq!(handle_of(obj), None);
            assert_ne!(handle_for(obj).unwrap(), h1);
        });
    }

    #[test]
    fn test_reset_invalidates_and_bumps_generation() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h1 = handle_for(obj).unwrap();
            let g1 = generation();
            let g2 = reset();
            assert!(g2 > g1);
            assert_eq!(handle_of(obj), None);
            assert_ne!(handle_for(obj).unwrap(), h1);
        });
    }

    #[test]
    fn test_thread_local_isolation() {
        // Never `thread::spawn`+`join` while the spawning thread holds the
        // GIL: the child needs the GIL for its own `with_gil` and joining
        // inside the parent's `with_gil` self-deadlocks. Three phases:
        pyo3::prepare_freethreaded_python();
        let (owned, main_handle) = Python::with_gil(|py| {
            reset();
            let obj = fresh_object(py);
            let owned: Py<PyAny> = Py::from(obj);
            (owned, handle_for(obj).unwrap())
        });
        // Phase 2: child thread exercises its own thread-local registry.
        std::thread::spawn({
            let owned = owned.clone();
            move || {
                Python::with_gil(|py2| {
                    let foreign = owned.as_ref(py2);
                    // Fresh registry on this thread: no handle for the main
                    // thread's object, and minting is independent.
                    assert_eq!(handle_of(foreign), None);
                    handle_for(foreign).unwrap()
                })
            }
        })
        .join()
        .unwrap();
        // Phase 3: parent re-acquires the GIL and checks isolation.
        Python::with_gil(|py| {
            let obj = owned.as_ref(py);
            // Main thread still sees its own handle; the child's mint never
            // leaked into this thread's registry.
            assert_eq!(handle_of(obj), Some(main_handle));
        });
    }
}
