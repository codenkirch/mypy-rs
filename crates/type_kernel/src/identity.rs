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
//! 2. **Idempotent per live object.** `handle_for` / `handle_for_stable`
//!    return the same handle for the same object until the entry is retired
//!    or the registry resets.
//! 3. **Raw layer bounded lifetime by contract.** Keys are raw `id()`s: a
//!    freed object can have its id recycled, so the enclosing seam MUST reset
//!    the registry at the same lifecycle boundary it clears the per-build
//!    resolvers (the `_clear_native_*` discipline in `mypy/build.py`).
//!    `reset_raw` bumps a generation counter so stale handles are detectable;
//!    handles minted before a reset are never re-issued, but ids may be
//!    re-registered with fresh handles after one.
//! 4. **Stable layer pins strongly (#1528).** A stable handle owns a
//!    `Py<PyAny>` pin, never a weakref: every probed `Type` class refuses
//!    weakrefs (wave-66C correction), and the pin is what makes the mapping
//!    safe without changing `Type.__slots__`. Because the pin keeps the
//!    named object alive, its `id()` cannot be recycled, and the O(1) hit
//!    path re-checks pointer identity against the pin with no sweep cost.
//!    Raw and stable share one handle per object: each layer adopts a
//!    handle the other already minted (`handle_for_stable` migrates a raw
//!    entry under the pin; `handle_for` / `handle_of` answer stable-first),
//!    so the proxy and the mirror never mint two identities for one object.
//! 5. **Stable pins survive a preserving reset (#1528).** The daemon calls
//!    `reset(preserve_stable = true)` at a recheck boundary: blobs and raw
//!    ids drop while the stable layer keeps the handle of every object the
//!    live graph still references, so astmerge-preserved objects re-register
//!    under the same handle. Pins release on `retire_stable`, at a
//!    non-preserving `reset`, and at a preserving reset for entries whose
//!    refcount shows the stable pin is their only owner (`sweep_stable`).
//!    An object retained by a dropped reference cycle keeps its pin until
//!    then: without weakrefs a cycle root is indistinguishable from a live
//!    object, the documented residual of the strong-pin protocol.

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
    /// Stable layer: raw pointer -> stable handle and handle -> strong pin.
    /// The pin keeps the named object alive, so a pointer key cannot be
    /// recycled while its entry exists.
    stable_by_id: HashMap<usize, u64>,
    stable_by_handle: HashMap<u64, Py<PyAny>>,
    /// Bumped by `reset_raw` so callers can detect stale handles.
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

/// Mint (or return the existing) raw handle for a live Python object.
///
/// One handle namespace is shared with the stable layer (#1528): an object
/// the mirror pinned stably answers with that same handle here, so the
/// proxy and the mirror never mint two identities for one object.
pub(crate) fn handle_for(obj: &PyAny) -> Option<u64> {
    if let Some(handle) = handle_of_stable(obj) {
        return Some(handle);
    }
    with_registry(|reg| {
        let key = obj.as_ptr() as usize;
        Some(*reg.by_id.entry(key).or_insert_with(|| {
            let h = reg.next_handle;
            reg.next_handle += 1;
            h
        }))
    })
}

/// Look up an existing handle without minting one: the stable layer first,
/// then the raw layer. `None` means the object has never been registered on
/// this thread (or both layers were reset).
pub(crate) fn handle_of(obj: &PyAny) -> Option<u64> {
    handle_of_stable(obj)
        .or_else(|| with_registry(|reg| reg.by_id.get(&(obj.as_ptr() as usize)).copied()))
}

/// Drop the raw mapping for a live object. A later `handle_for` on the same
/// object mints a fresh handle.
#[allow(dead_code)]
pub(crate) fn retire(obj: &PyAny) {
    with_registry(|reg| {
        reg.by_id.remove(&(obj.as_ptr() as usize));
    });
}

/// O(1) resolve of `obj`'s stable entry. A hit re-checks that the pinned
/// object is `obj` itself: a live strong pin makes a mismatch impossible
/// (the pin keeps the pointer valid), but a mixed pairing is dropped
/// defensively instead of being handed out.
fn stable_lookup_locked(reg: &mut Registry, obj: &PyAny) -> Option<u64> {
    let key = obj.as_ptr() as usize;
    let handle = *reg.stable_by_id.get(&key)?;
    if reg
        .stable_by_handle
        .get(&handle)
        .is_some_and(|pin| pin.as_ptr() == obj.as_ptr())
    {
        return Some(handle);
    }
    reg.stable_by_id.remove(&key);
    reg.stable_by_handle.remove(&handle);
    None
}

/// Mint (or return the existing) stable handle for a live Python object.
///
/// Unlike the raw layer, the handle survives a preserving `reset` (guarantee
/// 5) and is backed by a strong `Py<PyAny>` pin instead of a weakref: the
/// pinned object cannot be collected, so its pointer key stays valid and a
/// recycled `id()` can never adopt a stale handle. An existing raw handle is
/// adopted instead of minting a second identity for the same object
/// (one shared namespace, #1528). Returns `None` only when the caller cannot
/// own a pin (the mirror falls back to the raw layer then).
pub(crate) fn handle_for_stable(obj: &PyAny) -> Option<u64> {
    with_registry(|reg| {
        if let Some(handle) = stable_lookup_locked(reg, obj) {
            return Some(handle);
        }
        let key = obj.as_ptr() as usize;
        let handle = match reg.by_id.remove(&key) {
            Some(raw) => raw,
            None => reg.mint(),
        };
        reg.stable_by_id.insert(key, handle);
        reg.stable_by_handle.insert(handle, Py::from(obj));
        Some(handle)
    })
}

/// Look up an existing stable handle without minting one.
pub(crate) fn handle_of_stable(obj: &PyAny) -> Option<u64> {
    with_registry(|reg| stable_lookup_locked(reg, obj))
}

/// Whether `handle` still owns a stable pin (audit + tests).
pub(crate) fn stable_alive(handle: u64) -> bool {
    with_registry(|reg| reg.stable_by_handle.contains_key(&handle))
}

/// Retire one stable handle: release its pin and drop both mappings.
/// Returns whether an entry existed. The pin is dropped after the registry
/// borrow is released, so its destructor cannot re-enter the registry while
/// it is borrowed.
pub(crate) fn retire_stable(handle: u64) -> bool {
    let pin = with_registry(|reg| {
        let pin = reg.stable_by_handle.remove(&handle);
        if let Some(pin) = &pin {
            reg.stable_by_id.remove(&(pin.as_ptr() as usize));
        }
        pin
    });
    pin.is_some()
}

/// Drop the whole stable layer, releasing every pin (non-preserving reset).
pub(crate) fn reset_stable() {
    let pins: Vec<Py<PyAny>> = with_registry(|reg| {
        reg.stable_by_id.clear();
        reg.stable_by_handle.drain().map(|(_, pin)| pin).collect()
    });
    drop(pins);
}

/// Release stable entries whose only remaining reference is the stable pin.
///
/// Called at a preserving reset (daemon recheck boundary) after the Python
/// mirror dropped its own pins: an entry at refcount 1 is owned by nothing
/// else, so the build no longer references the object and the pin can go.
/// An object inside a retained reference cycle stays above 1 and keeps its
/// pin; that is the documented cost of a weakref-free pin (see guarantee 5).
pub(crate) fn sweep_stable(py: Python<'_>) {
    let unreferenced: Vec<u64> = with_registry(|reg| {
        reg.stable_by_handle
            .iter()
            .filter(|(_, pin)| pin.as_ref(py).get_refcnt() == 1)
            .map(|(handle, _)| *handle)
            .collect()
    });
    for handle in unreferenced {
        retire_stable(handle);
    }
}

/// Clear the raw layer and bump the generation. Returns the new generation.
/// Call at the same lifecycle boundary the enclosing build clears its
/// per-build resolvers (guarantee 3). The stable layer is untouched so the
/// caller can preserve or sweep it (`reset`).
pub(crate) fn reset_raw() -> u64 {
    with_registry(|reg| {
        let generation = reg.generation + 1;
        // `next_handle` is preserved: handles are never re-issued after a
        // reset (guarantee 3), so pre-reset handles stay detectable as stale.
        reg.by_id.clear();
        reg.generation = generation;
        generation
    })
}

/// The full reset protocol behind `rust_mirror_reset(preserve_stable)`:
/// the raw layer always clears; the stable layer is swept when preserving
/// (live-object handles survive) and dropped otherwise (no cross-build leak).
pub(crate) fn reset(preserve_stable: bool, py: Python<'_>) -> u64 {
    let generation = reset_raw();
    if preserve_stable {
        sweep_stable(py);
    } else {
        reset_stable();
    }
    generation
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

    #[cfg(test)]
    pub(crate) fn stable_entry_count() -> usize {
        with_registry(|reg| reg.stable_by_handle.len())
    }

    #[test]
    fn test_handle_for_is_idempotent() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let h = handle_for(obj).unwrap();
            assert_eq!(handle_for(obj), Some(h));
            assert_ne!(h, NO_IDENTITY);
        });
    }

    #[test]
    fn test_distinct_objects_get_distinct_handles() {
        with_py(|py| {
            reset(false, py);
            let a = fresh_object(py);
            let b = fresh_object(py);
            assert_ne!(handle_for(a), handle_for(b));
        });
    }

    #[test]
    fn test_handle_of_does_not_mint() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            assert_eq!(handle_of(obj), None);
            let h = handle_for(obj).unwrap();
            assert_eq!(handle_of(obj), Some(h));
        });
    }

    #[test]
    fn test_retire_drops_and_re_mints() {
        with_py(|py| {
            reset(false, py);
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
            reset(false, py);
            let obj = fresh_object(py);
            let h1 = handle_for(obj).unwrap();
            let g1 = generation();
            let g2 = reset(true, py);
            assert!(g2 > g1);
            assert_eq!(handle_of(obj), None);
            assert_ne!(handle_for(obj).unwrap(), h1);
        });
    }

    #[test]
    fn test_stable_handle_is_shared_across_layers() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let h = handle_for_stable(obj).unwrap();
            assert_eq!(handle_for_stable(obj), Some(h));
            assert_eq!(handle_of_stable(obj), Some(h));
            assert_ne!(h, NO_IDENTITY);
            // One identity namespace: the raw lookups answer the same
            // handle instead of minting a second one for the object.
            assert_eq!(handle_of(obj), Some(h));
            assert_eq!(handle_for(obj), Some(h));
        });
    }

    #[test]
    fn test_raw_handle_is_adopted_by_stable_layer() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let raw = handle_for(obj).unwrap();
            // The stable mint adopts the raw handle instead of splitting
            // the namespace.
            assert_eq!(handle_for_stable(obj), Some(raw));
            assert_eq!(handle_of(obj), Some(raw));
            assert!(stable_alive(raw));
        });
    }

    #[test]
    fn test_stable_layer_survives_preserving_reset() {
        with_py(|py| {
            reset(false, py);
            let raw_only = fresh_object(py);
            let raw_handle = handle_for(raw_only).unwrap();
            let obj = fresh_object(py);
            let h = handle_for_stable(obj).unwrap();
            assert_eq!(handle_for(obj), Some(h));
            let g1 = generation();
            // `obj` is still referenced by this frame, so the sweep keeps
            // its pin at the preserving reset.
            let g2 = reset(true, py);
            assert!(g2 > g1);
            // The raw map cleared (the raw-only object forgets its handle),
            // but the shared identity answers through the surviving pin.
            assert_eq!(handle_of(raw_only), None);
            assert_eq!(handle_of(obj), Some(h));
            assert_eq!(handle_of_stable(obj), Some(h));
            assert!(stable_alive(h));
            assert_eq!(handle_for(obj), Some(h));
            assert_ne!(raw_handle, h);
        });
    }

    #[test]
    fn test_full_reset_drops_stable_layer() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let h = handle_for_stable(obj).unwrap();
            reset(false, py);
            assert!(!stable_alive(h));
            assert_eq!(handle_of_stable(obj), None);
            // Re-minting after a full reset never re-issues the old handle.
            assert_ne!(handle_for_stable(obj).unwrap(), h);
        });
    }

    #[test]
    fn test_retire_stable_releases_and_re_mints() {
        with_py(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let h = handle_for_stable(obj).unwrap();
            assert!(retire_stable(h));
            assert!(!stable_alive(h));
            assert_eq!(handle_of_stable(obj), None);
            // Retiring twice is a no-op.
            assert!(!retire_stable(h));
            assert_ne!(handle_for_stable(obj).unwrap(), h);
        });
    }

    #[test]
    fn test_stable_lookup_rejects_mismatched_identity() {
        with_py(|py| {
            reset(false, py);
            let a = fresh_object(py);
            let b = fresh_object(py);
            let ha = handle_for_stable(a).unwrap();
            let hb = handle_for_stable(b).unwrap();
            // Model a mixed pairing: a's pointer key now points at b's
            // handle, whose pin names b, not a.
            with_registry(|reg| {
                reg.stable_by_id.insert(a.as_ptr() as usize, hb);
            });
            assert_eq!(handle_of_stable(a), None);
            // The mismatched pair was dropped, so both objects re-mint
            // under fresh handles (the forged one is never re-issued).
            let ha2 = handle_for_stable(a).unwrap();
            let hb2 = handle_for_stable(b).unwrap();
            assert_ne!(ha2, ha);
            assert_ne!(ha2, hb);
            assert_ne!(hb2, hb);
            assert_eq!(handle_of_stable(a), Some(ha2));
            assert_eq!(handle_of_stable(b), Some(hb2));
        });
    }

    #[test]
    fn test_sweep_releases_unreferenced_entries() {
        // The mint runs in its own GIL scope: when the scope exits, the
        // eval-pool reference is dropped too, so the sweep sees refcount 1
        // (the stable pin itself) and releases the entry.
        Python::with_gil(|py| reset(false, py));
        let dead_handle = Python::with_gil(|py| {
            let obj = fresh_object(py);
            handle_for_stable(obj).unwrap()
        });
        assert_eq!(stable_entry_count(), 1);
        with_py(|py| {
            sweep_stable(py);
            assert_eq!(stable_entry_count(), 0);
            assert!(!stable_alive(dead_handle));
        });
    }

    #[test]
    fn test_sweep_keeps_referenced_entries() {
        // Each `Python::with_gil` scope owns one GIL pool; the eval-pool
        // reference to `obj` is released when the mint scope exits, so
        // `held` is the only owner the next scope can observe.
        let (h, held) = Python::with_gil(|py| {
            reset(false, py);
            let obj = fresh_object(py);
            let held: Py<PyAny> = Py::from(obj);
            (handle_for_stable(obj).unwrap(), held)
        });
        Python::with_gil(|py| {
            sweep_stable(py);
            assert_eq!(stable_entry_count(), 1);
            assert!(stable_alive(h));
            assert_eq!(handle_of_stable(held.as_ref(py)), Some(h));
        });
        // Dropping the extra owner makes the next sweep collect it.
        drop(held);
        Python::with_gil(|py| sweep_stable(py));
        assert_eq!(stable_entry_count(), 0);
    }

    #[test]
    fn test_preserving_reset_sweeps_dropped_and_keeps_live() {
        let (kept_handle, dropped_handle, kept) = Python::with_gil(|py| {
            reset(false, py);
            let kept = fresh_object(py);
            let kept_handle = handle_for_stable(kept).unwrap();
            let dropped = fresh_object(py);
            let dropped_handle = handle_for_stable(dropped).unwrap();
            (kept_handle, dropped_handle, Py::from(kept) as Py<PyAny>)
        });
        Python::with_gil(|py| {
            assert_eq!(stable_entry_count(), 2);
            // `kept` is still referenced outside the pool; the other
            // object is pin-owned only, so the preserving reset releases
            // just its entry.
            reset(true, py);
            assert!(stable_alive(kept_handle));
            assert_eq!(handle_of_stable(kept.as_ref(py)), Some(kept_handle));
            assert!(!stable_alive(dropped_handle));
            assert_eq!(stable_entry_count(), 1);
        });
    }

    #[test]
    fn test_thread_local_isolation() {
        // Never `thread::spawn`+`join` while the spawning thread holds the
        // GIL: the child needs the GIL for its own `with_gil` and joining
        // inside the parent's `with_gil` self-deadlocks. Three phases:
        pyo3::prepare_freethreaded_python();
        let (owned, main_handle) = Python::with_gil(|py| {
            reset(false, py);
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
                    assert_eq!(handle_of_stable(foreign), None);
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
