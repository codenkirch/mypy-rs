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
//! 5. **Stable layer survives `reset`.** `handle_for_stable` keys on a
//!    `weakref.ref` pinned to the live object instead of the raw `id()`, so
//!    a handle survives daemon-style rebuilds that call `reset` (which now
//!    clears only the raw layer). Entries whose weakref died are swept on
//!    every lookup, so a recycled `id()` can never adopt a stale handle.
//!    A non-weakref-able object (e.g. a plain `int`) defers to `None`, the
//!    same shape the raw layer's seam fallbacks expect. The raw layer stays
//!    untouched for seams that only need within-build identity.

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
    /// Stable layer: raw pointer -> handle and handle -> `weakref.ref`.
    /// Entries are swept when the weakref's target dies, so the pointer
    /// key can never outlive the object it names.
    stable_by_id: HashMap<usize, u64>,
    stable_by_handle: HashMap<u64, Py<PyAny>>,
    /// Bumped by `reset` so callers can detect stale handles.
    generation: u64,
}

impl Registry {
    fn new() -> Self {
        Registry {
            next_handle: 1,
            by_id: HashMap::new(),
            stable_by_id: HashMap::new(),
            stable_by_handle: HashMap::new(),
            generation: 1,
        }
    }

    /// Minted-handle counter shared by both layers so a handle is
    /// unambiguous whichever layer a caller consulted.
    fn mint(&mut self) -> u64 {
        let h = self.next_handle;
        self.next_handle += 1;
        h
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

/// Clear the raw layer of the thread-local registry and bump the
/// generation. Returns the new generation. Call at the same lifecycle
/// boundary the enclosing build clears its per-build resolvers
/// (guarantee 3). The stable layer survives untouched so weakref-pinned
/// handles keep their identity across a daemon-style rebuild
/// (guarantee 5).
#[allow(dead_code)]
pub(crate) fn reset() -> u64 {
    with_registry(|reg| {
        let generation = reg.generation + 1;
        // `next_handle` is preserved: handles are never re-issued after a
        // reset (guarantee 3), so pre-reset handles stay detectable as stale.
        *reg = Registry {
            next_handle: reg.next_handle,
            by_id: HashMap::new(),
            stable_by_id: std::mem::take(&mut reg.stable_by_id),
            stable_by_handle: std::mem::take(&mut reg.stable_by_handle),
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

/// Drop every stable-layer entry whose weakref target is dead. Broken or
/// dead weakrefs can occur at any point (refcount-collected objects
/// vanish without a callback pre-run), so sweeping on access is what
/// keeps a recycled `id()` from adopting a stale handle.
fn sweep_stable_locked(reg: &mut Registry, py: Python<'_>) {
    let dead: Vec<u64> = reg
        .stable_by_handle
        .iter()
        .filter(|(_, wr)| {
            // `weakref.ref()` is itself the callable: `call0` returns the
            // live object, or `None` once its target is collected.
            wr.as_ref(py).call0().map_or(true, |live| live.is_none())
        })
        .map(|(h, _)| *h)
        .collect();
    if dead.is_empty() {
        return;
    }
    let dead: std::collections::HashSet<u64> = dead.into_iter().collect();
    reg.stable_by_handle.retain(|h, _| !dead.contains(h));
    reg.stable_by_id.retain(|_, h| !dead.contains(h));
}

/// Mint (or return the existing) stable handle for a live Python object.
///
/// Unlike the raw layer, the handle survives `reset` (guarantee 5) and is
/// swept against dead entries before every mint, so no recycled `id()`
/// can adopt a stale handle. Returns `None` when the object does not
/// support weakrefs (the seam should fall back to the raw layer then).
#[allow(dead_code)]
pub(crate) fn handle_for_stable(obj: &PyAny) -> Option<u64> {
    let py = obj.py();
    with_registry(|reg| {
        sweep_stable_locked(reg, py);
        let key = obj.as_ptr() as usize;
        if let Some(&h) = reg.stable_by_id.get(&key) {
            return Some(h);
        }
        // `weakref.ref` rejects non-weakref-able objects with TypeError;
        // defer to `None` instead of risking a raw-key alias here.
        let fresh: &PyAny = py
            .import("weakref")
            .ok()?
            .getattr("ref")
            .ok()?
            .call1((obj,))
            .ok()?;
        let h = reg.mint();
        reg.stable_by_id.insert(key, h);
        reg.stable_by_handle.insert(h, Py::from(fresh));
        Some(h)
    })
}

/// Look up an existing stable handle without minting one.
#[allow(dead_code)]
pub(crate) fn handle_of_stable(obj: &PyAny) -> Option<u64> {
    let py = obj.py();
    with_registry(|reg| {
        sweep_stable_locked(reg, py);
        reg.stable_by_id.get(&(obj.as_ptr() as usize)).copied()
    })
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

    /// Weakref-able stand-in: plain `object()` instances are not
    /// weakref-able, but instances of a synthesized class (whose instances
    /// get a `__weakref__` slot by default) are.
    fn fresh_weakrefable(py: Python<'_>) -> &PyAny {
        py.eval("type('Weak', (), {})()", None, None).unwrap()
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

    #[cfg(test)]
    pub(crate) fn stable_entry_count() -> usize {
        with_registry(|reg| reg.stable_by_handle.len())
    }

    #[test]
    fn test_stable_layer_survives_reset() {
        with_py(|py| {
            reset();
            let obj = fresh_weakrefable(py);
            let h = handle_for_stable(obj).unwrap();
            // Raw layer gets a different key class, but must be
            // idempotent independently of the stable entry.
            assert_eq!(handle_for_stable(obj).unwrap(), h);
            let raw = handle_for(obj).unwrap();
            let g1 = generation();
            reset();
            assert!(generation() > g1);
            // Raw layer cleared, stable layer intact (guarantee 5).
            assert_eq!(handle_of(obj), None);
            assert_eq!(handle_for_stable(obj).unwrap(), h);
            // And sharing the monotonic counter means no collision with
            // the fresh raw handle minted after the reset.
            assert_ne!(handle_for(obj).unwrap(), h);
            assert_ne!(raw, h);
        });
    }

    #[test]
    fn test_stable_layer_sweeps_dead_entries() {
        reset();
        // The mint runs in an inner GIL scope: at scope exit the pool
        // flush drops the last owning reference, so the weakref target
        // dies deterministically at the end of `with_gil`.
        let dead_handle = Python::with_gil(|py| handle_for_stable(fresh_weakrefable(py)).unwrap());
        assert_eq!(stable_entry_count(), 1);
        // Next access sweeps: the pruned entry can never be adopted
        // through a recycled id.
        with_py(|py| {
            let other = fresh_weakrefable(py);
            assert_eq!(handle_of_stable(other), None);
            assert_eq!(stable_entry_count(), 0);
            let other_handle = handle_for_stable(other).unwrap();
            assert_ne!(other_handle, dead_handle);
            assert_eq!(stable_entry_count(), 1);
        });
    }

    #[test]
    fn test_stable_handles_distinct_and_raw_separate() {
        with_py(|py| {
            reset();
            let a = fresh_weakrefable(py);
            let b = fresh_weakrefable(py);
            let ha = handle_for_stable(a).unwrap();
            let hb = handle_for_stable(b).unwrap();
            assert_ne!(ha, hb);
            // Raw and stable layers are independent namespaces.
            assert_eq!(handle_of(a), None);
        });
    }

    #[test]
    fn test_stable_layer_defers_on_non_weakref_object() {
        with_py(|py| {
            reset();
            // A plain small int does not support weakrefs.
            let n = py.eval("5", None, None).unwrap();
            assert_eq!(handle_for_stable(n), None);
            // And the raw layer still answers for it.
            assert!(handle_for(n).is_some());
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
