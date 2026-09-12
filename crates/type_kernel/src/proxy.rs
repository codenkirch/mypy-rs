//! ADR-0004 proxy store scaffold (P1, issue #1553).
//!
//! Blob-backed read-shadow storage keyed by `identity::handle_for`
//! handles. P1 ships the store, its pyfunctions, and the Python gate
//! (`mypy/type_proxy.py`) only: no funnel reads an entry yet, so the
//! produced behavior is unchanged. P2 wires the lazy Instance read
//! shadow through the `type_proxy` shim.
//!
//! Guarantees:
//! 1. **Thread-local.** Entries and pins live in a `thread_local!` cell
//!    keyed by handles minted on the same thread, so no locking is needed.
//! 2. **Strong pins.** Each stored object is held by a `Py<PyAny>` until
//!    its entry is dropped or the store resets, so a recycled `id()` can
//!    never adopt a stale entry (the raw handle keys on `id()`).
//! 3. **Epoch-stamped reads.** A read serves bytes only while the
//!    Python-side epoch the entry was stored at still matches; an epoch
//!    mismatch is a miss, and the caller re-serializes and re-puts.
//! 4. **Identity is not owned here.** `reset` clears entries and pins
//!    only; `identity::reset` stays with `rust_mirror_reset` (mirror.rs)
//!    so proxy state cannot invalidate handles other seams still hold.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::identity;

/// One blob-backed proxy entry: the object's wire bytes plus the
/// Python-side epoch stamp they were stored at.
pub(crate) struct ProxyEntry {
    pub(crate) bytes: Vec<u8>,
    pub(crate) stamp: u64,
}

struct ProxyStore {
    by_handle: HashMap<u64, ProxyEntry>,
    /// Strong pins: handles key on raw `id()`s, so each stored object
    /// stays alive until its entry is dropped or the store resets.
    pins: HashMap<u64, Py<PyAny>>,
}

impl ProxyStore {
    fn new() -> Self {
        ProxyStore {
            by_handle: HashMap::new(),
            pins: HashMap::new(),
        }
    }
}

thread_local! {
    static STORE: RefCell<ProxyStore> = RefCell::new(ProxyStore::new());
}

fn with_store<T>(f: impl FnOnce(&mut ProxyStore) -> T) -> T {
    STORE.with(|cell| f(&mut cell.borrow_mut()))
}

/// Stored bytes for `handle` when the entry carries `stamp`, else None.
pub(crate) fn read(handle: u64, stamp: u64) -> Option<Vec<u8>> {
    with_store(|store| match store.by_handle.get(&handle) {
        Some(entry) if entry.stamp == stamp => Some(entry.bytes.clone()),
        _ => None,
    })
}

/// Store `bytes` for `obj` at `stamp`; returns the identity handle.
/// Re-putting the same object overwrites its blob and stamp.
pub(crate) fn put(obj: &PyAny, bytes: Vec<u8>, stamp: u64) -> PyResult<u64> {
    let handle = identity::handle_for(obj)
        .ok_or_else(|| PyValueError::new_err("proxy: object has no identity handle"))?;
    with_store(|store| {
        store.by_handle.insert(handle, ProxyEntry { bytes, stamp });
        store.pins.insert(handle, Py::from(obj));
    });
    Ok(handle)
}

/// Drop one entry and its pin. Returns whether an entry was present.
///
/// The pin is moved out of the map under the borrow and dropped only
/// after the `RefCell` guard is released: releasing the last reference
/// can run a Python deallocator, and a callback re-entering the store
/// would panic on the active mutable borrow.
pub(crate) fn retire(handle: u64) -> bool {
    let (present, pin) = with_store(|store| {
        let present = store.by_handle.remove(&handle).is_some();
        let pin = store.pins.remove(&handle);
        (present, pin)
    });
    drop(pin);
    present
}

/// Clear every entry and pin; returns how many entries were dropped.
/// Deliberately does NOT call `identity::reset`: the raw handle registry
/// is owned by `rust_mirror_reset`, and proxy state must not invalidate
/// handles other seams still hold.
pub(crate) fn reset() -> usize {
    let (entries, pins) = with_store(|store| {
        let entries = store.by_handle.len();
        let pins: Vec<Py<PyAny>> = store.pins.drain().map(|(_, pin)| pin).collect();
        store.by_handle.clear();
        (entries, pins)
    });
    // Drop the pins only after the guard is released (see `retire`).
    drop(pins);
    entries
}

/// Number of live entries.
pub(crate) fn entry_count() -> usize {
    with_store(|store| store.by_handle.len())
}

// ---- pyfunction wrappers ----

/// Read the blob for `handle` when it was stored at `stamp`. Returns a
/// real `bytes` object (not a `Vec<u8>`-derived list) so the hot read
/// path never materializes an int list.
#[pyfunction]
pub(crate) fn rust_proxy_read(py: Python<'_>, handle: u64, stamp: u64) -> Option<Py<PyBytes>> {
    read(handle, stamp).map(|bytes| PyBytes::new(py, &bytes).into())
}

/// Store `bytes` for `obj` at `stamp`; returns the identity handle.
#[pyfunction]
pub(crate) fn rust_proxy_put(obj: &PyAny, bytes: &[u8], stamp: u64) -> PyResult<u64> {
    put(obj, bytes.to_vec(), stamp)
}

/// Drop the entry (and pin) for `handle`; returns whether one existed.
#[pyfunction]
pub(crate) fn rust_proxy_drop(handle: u64) -> bool {
    retire(handle)
}

/// Clear all proxy entries and pins; returns the dropped entry count.
#[pyfunction]
pub(crate) fn rust_proxy_reset() -> usize {
    reset()
}

/// Live entry count (audit + tests).
#[pyfunction]
pub(crate) fn rust_proxy_entry_count() -> usize {
    entry_count()
}

/// Non-minting identity handle lookup; None when never registered.
#[pyfunction]
pub(crate) fn rust_proxy_handle_of(obj: &PyAny) -> Option<u64> {
    identity::handle_of(obj)
}

#[cfg(test)]
mod proxy_tests {
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
    fn test_put_read_roundtrip_and_count() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 7).unwrap();
            assert_eq!(entry_count(), 1);
            assert_eq!(read(h, 7).as_deref(), Some(&b"blob"[..]));
            // Re-putting the same object overwrites blob and stamp.
            put(obj, b"blob2".to_vec(), 9).unwrap();
            assert_eq!(entry_count(), 1);
            assert_eq!(read(h, 7), None);
            assert_eq!(read(h, 9).as_deref(), Some(&b"blob2"[..]));
        });
    }

    #[test]
    fn test_epoch_mismatch_returns_none() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 3).unwrap();
            assert_eq!(read(h, 4), None);
            assert_eq!(read(h, 2), None);
            assert_eq!(read(h, 3).as_deref(), Some(&b"blob"[..]));
        });
    }

    #[test]
    fn test_read_unknown_handle_returns_none() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 1).unwrap();
            assert_eq!(read(h + 1, 1), None);
        });
    }

    #[test]
    fn test_drop_removes_entry_and_pin() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 1).unwrap();
            assert!(retire(h));
            assert_eq!(read(h, 1), None);
            assert_eq!(entry_count(), 0);
            assert!(!retire(h));
        });
    }

    #[test]
    fn test_drop_keeps_identity_handle() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 1).unwrap();
            assert!(retire(h));
            // Identity is untouched by a proxy drop; only the entry is gone.
            assert_eq!(identity::handle_of(obj), Some(h));
        });
    }

    #[test]
    fn test_reset_clears_entries_and_pins() {
        with_py(|py| {
            reset();
            let a = fresh_object(py);
            let b = fresh_object(py);
            put(a, b"a".to_vec(), 1).unwrap();
            put(b, b"b".to_vec(), 1).unwrap();
            assert_eq!(entry_count(), 2);
            assert_eq!(reset(), 2);
            assert_eq!(entry_count(), 0);
            assert_eq!(reset(), 0);
        });
    }

    #[test]
    fn test_reset_does_not_reset_identity() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 1).unwrap();
            reset();
            // `rust_mirror_reset` alone owns `identity::reset`; the proxy
            // reset must leave the raw handle registry alive.
            assert_eq!(identity::handle_of(obj), Some(h));
            assert_eq!(identity::handle_for(obj), Some(h));
        });
    }

    #[test]
    fn test_handle_of_matches_minted_handle() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = put(obj, b"blob".to_vec(), 1).unwrap();
            assert_eq!(identity::handle_of(obj), Some(h));
            assert_eq!(identity::handle_for(obj), Some(h));
        });
    }
}
