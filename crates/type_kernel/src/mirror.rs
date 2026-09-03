//! Phase F1 dual-write shadow storage (issue #1370).
//!
//! While Python stays canonical, the mirror keeps one wire blob per live
//! family object (Instance, CallableType, TypeVarType, UnionType), written
//! on construction and every attribute change. Python asserts at the
//! serializer seams that a fresh `Type.write` equals the blob, proving
//! Python's serialized graph is what a Rust owner would have had. No
//! consumer reads this storage in F1.
//!
//! Keys come from `identity::handle_for` (raw `id()` layer, thread-local).
//! Python pins every mirrored object strongly until the per-build reset,
//! so a recycled `id()` cannot adopt a stale entry before `reset`.
//! Byte storage is unbounded by design: blobs are small (a few KB), and
//! run length is bounded by the gate harness; memory is watched there.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::identity;
use crate::wire::{
    ReadBuffer, Type, WriteBuffer, read_type, read_type_list, write_type, write_type_list,
};

pub(crate) struct MirrorEntry {
    pub(crate) family: String,
    pub(crate) bytes: Vec<u8>,
}

struct Mirror {
    by_handle: HashMap<u64, MirrorEntry>,
    /// child handle -> handles of parents whose wire code contains it.
    parents_of: HashMap<u64, Vec<u64>>,
    /// parent handle -> children listed at its last registration.
    children_of: HashMap<u64, Vec<u64>>,
}

thread_local! {
    static MIRROR: RefCell<Mirror> = RefCell::new(Mirror::new());
}

impl Mirror {
    fn new() -> Self {
        Mirror {
            by_handle: HashMap::new(),
            parents_of: HashMap::new(),
            children_of: HashMap::new(),
        }
    }
}

fn with_mirror<T>(f: impl FnOnce(&mut Mirror) -> T) -> T {
    MIRROR.with(|cell| f(&mut cell.borrow_mut()))
}

/// Describe the first differing region of two blobs for an assert message.
fn diff_description(old: &[u8], new: &[u8]) -> String {
    let shared = old.len().min(new.len());
    let mut pos = shared;
    for i in 0..shared {
        if old[i] != new[i] {
            pos = i;
            break;
        }
    }
    let hex = |b: &[u8], at: usize| -> String {
        let start = at.saturating_sub(8);
        let end = (at + 8).min(b.len());
        b[start..end].iter().map(|x| format!("{:02x}", x)).collect()
    };
    let context_end = shared.min(pos + 32);
    let first = if pos == shared {
        "same prefix".to_string()
    } else {
        format!(
            "byte {}: old[{}] new[{}]",
            pos,
            hex(old, pos),
            hex(new, pos)
        )
    };
    format!(
        "mirror mismatch: lens {} vs {}, {} (old …{} | new …{})",
        old.len(),
        new.len(),
        first,
        hex(old, context_end),
        hex(new, context_end)
    )
}

/// Register (or overwrite) the mirror entry for a live object.
/// `child_handles` lists already-mirrored children inside the blob.
pub(crate) fn register(
    obj: &PyAny,
    family: &str,
    bytes: Vec<u8>,
    child_handles: Vec<u64>,
) -> PyResult<u64> {
    let handle = identity::handle_for(obj)
        .ok_or_else(|| PyValueError::new_err("mirror: object has no identity handle"))?;
    with_mirror(|m| {
        // Unlink the previous child list (kept in children_of) before
        // installing the new one, so re-registration cannot leave ghosts.
        let old_children: Vec<u64> = m.children_of.remove(&handle).unwrap_or_default();
        for child in old_children {
            if let Some(list) = m.parents_of.get_mut(&child) {
                list.retain(|p| *p != handle);
            }
        }
        m.by_handle.insert(
            handle,
            MirrorEntry {
                family: family.to_string(),
                bytes,
            },
        );
        if !child_handles.is_empty() {
            m.children_of.insert(handle, child_handles.clone());
        }
        for child in child_handles {
            m.parents_of.entry(child).or_default().push(handle);
        }
    });
    Ok(handle)
}

/// Update a mirror entry's bytes; returns error on unknown handle.
pub(crate) fn update(handle: u64, bytes: Vec<u8>) -> PyResult<()> {
    let stamp = with_mirror(|m| {
        m.by_handle
            .get_mut(&handle)
            .map(|entry| entry.bytes = bytes)
            .is_some()
    });
    if stamp {
        Ok(())
    } else {
        Err(PyValueError::new_err(format!(
            "mirror: unknown handle {}",
            handle
        )))
    }
}

/// Assert `bytes` equals the stored blob for `handle`.
/// Unknown handles report "unregistered" so Python can adopt the object.
pub(crate) fn expect(handle: u64, bytes: &[u8]) -> Result<(), String> {
    let stored = with_mirror(|m| m.by_handle.get(&handle).map(|e| e.bytes.clone()));
    match stored {
        None => Err("mirror: unregistered handle".to_string()),
        Some(old) if old == bytes => Ok(()),
        Some(old) => Err(diff_description(&old, bytes)),
    }
}

/// Stored blob for `handle`, or None when unregistered.
pub(crate) fn entry_bytes(handle: u64) -> Option<Vec<u8>> {
    with_mirror(|m| m.by_handle.get(&handle).map(|e| e.bytes.clone()))
}

/// Family tag stored for `handle`, or None when unregistered.
pub(crate) fn entry_family(handle: u64) -> Option<String> {
    with_mirror(|m| m.by_handle.get(&handle).map(|e| e.family.clone()))
}

/// Parent handles whose wire code contains the given child.
pub(crate) fn parents(handle: u64) -> Vec<u64> {
    with_mirror(|m| m.parents_of.get(&handle).cloned().unwrap_or_default())
}

/// Clear all mirror state and reset the identity registry.
pub(crate) fn reset() -> u64 {
    with_mirror(|m| {
        *m = Mirror::new();
    });
    identity::reset()
}

/// Number of live mirror entries.
pub(crate) fn entry_count() -> usize {
    with_mirror(|m| m.by_handle.len())
}

// ---- pyfunction wrappers ----

/// Register a mirrored object; returns its (minted) handle.
#[pyfunction]
#[pyo3(signature = (obj, family, bytes, child_handles))]
pub(crate) fn rust_mirror_register(
    obj: &PyAny,
    family: &str,
    bytes: &[u8],
    child_handles: Vec<u64>,
) -> PyResult<u64> {
    register(obj, family, bytes.to_vec(), child_handles)
}

/// Write a new blob for a registered handle.
#[pyfunction]
pub(crate) fn rust_mirror_update(handle: u64, bytes: &[u8]) -> PyResult<()> {
    update(handle, bytes.to_vec())
}

/// Assert equality between the stored blob and the fresh live bytes.
/// Raises ValueError with a diff; Python converts that into its funnel.
#[pyfunction]
pub(crate) fn rust_mirror_expect(handle: u64, bytes: &[u8]) -> PyResult<()> {
    match expect(handle, bytes) {
        Ok(()) => Ok(()),
        Err(msg) => Err(PyValueError::new_err(msg)),
    }
}

/// Stored bytes, or None when unregistered (adoption decision input).
#[pyfunction]
pub(crate) fn rust_mirror_bytes(handle: u64) -> Option<Vec<u8>> {
    entry_bytes(handle)
}

/// Family tag, or None when unregistered.
#[pyfunction]
pub(crate) fn rust_mirror_family(handle: u64) -> Option<String> {
    entry_family(handle)
}

/// Parents whose wire code contains this child (cascade loop input).
#[pyfunction]
pub(crate) fn rust_mirror_parents(handle: u64) -> Vec<u64> {
    parents(handle)
}

/// Drop all mirror state; returns the new identity generation.
#[pyfunction]
pub(crate) fn rust_mirror_reset() -> u64 {
    reset()
}

/// Live entry count (audit + tests).
#[pyfunction]
pub(crate) fn rust_mirror_entry_count() -> usize {
    entry_count()
}

/// Non-minting handle lookup: None when the object was never registered.
#[pyfunction]
pub(crate) fn rust_mirror_handle_of(obj: &PyAny) -> Option<u64> {
    identity::handle_of(obj)
}

/// Splice `Instance.args` into the stored blob (F3 slice 1, #1397): decode
/// the stored blob into the F0 `Type` enum, swap in the `write_type_list`
/// arg list from `args_blob`, re-encode, and store the result.
///
/// Returns the new full blob for the parent cascade; `None` defers
/// (unregistered handle, undecodable blob, or non-Instance stored family).
pub(crate) fn patch_instance_args(handle: u64, args_blob: &[u8]) -> Option<Vec<u8>> {
    let old = entry_bytes(handle)?;
    let stored = {
        let mut buf = ReadBuffer::new(&old);
        read_type(&mut buf, None).ok()?
    };
    let new_args = {
        let mut buf = ReadBuffer::new(args_blob);
        read_type_list(&mut buf).ok()?
    };
    let Type::Instance {
        type_ref,
        last_known_value,
        extra_attrs,
        ..
    } = stored
    else {
        return None;
    };
    let patched = Type::Instance {
        type_ref,
        args: new_args,
        last_known_value,
        extra_attrs,
    };
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, &patched).ok()?;
    let blob = wbuf.into_bytes();
    update(handle, blob.clone()).ok()?;
    Some(blob)
}

#[pyfunction]
#[pyo3(signature = (handle, args_blob))]
pub(crate) fn rust_mirror_patch_instance_args(handle: u64, args_blob: &[u8]) -> Option<Vec<u8>> {
    patch_instance_args(handle, args_blob)
}

#[cfg(test)]
mod mirror_tests {
    use super::*;

    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    #[test]
    fn test_register_returns_same_handle_for_same_object() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h1 = register(obj, "instance", b"abc".to_vec(), vec![]).unwrap();
            let h2 = register(obj, "instance", b"abz".to_vec(), vec![]).unwrap();
            assert_eq!(h1, h2);
            assert_eq!(entry_bytes(h1), Some(b"abz".to_vec()));
        });
    }

    #[test]
    fn test_distinct_objects_distinct_handles() {
        with_py(|py| {
            reset();
            let a = fresh(py);
            let b = fresh(py);
            let ha = register(a, "instance", b"x".to_vec(), vec![]).unwrap();
            let hb = register(b, "callable", b"y".to_vec(), vec![]).unwrap();
            assert_ne!(ha, hb);
            assert_eq!(entry_family(hb).as_deref(), Some("callable"));
        });
    }

    #[test]
    fn test_update_changes_bytes_and_errors_on_unknown() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "union", b"old".to_vec(), vec![]).unwrap();
            update(h, b"new".to_vec()).unwrap();
            assert_eq!(entry_bytes(h), Some(b"new".to_vec()));
            assert!(update(h + 999, b"z".to_vec()).is_err());
        });
    }

    #[test]
    fn test_expect_passes_and_reports_diff() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "tvar", b"aaaa1111bbbb".to_vec(), vec![]).unwrap();
            assert!(expect(h, b"aaaa1111bbbb").is_ok());
            let err = expect(h, b"aaaa1111ccbb").unwrap_err();
            assert!(err.contains("byte 8"), "bad diff: {}", err);
            assert!(err.contains("mirror mismatch"));
        });
    }

    #[test]
    fn test_expect_unregistered_is_unregistered_not_diff() {
        assert_eq!(
            expect(4242, b"x"),
            Err("mirror: unregistered handle".to_string())
        );
    }

    #[test]
    fn test_parents_index_roundtrip() {
        with_py(|py| {
            reset();
            let child = fresh(py);
            let parent = fresh(py);
            let hc = register(child, "instance", b"c".to_vec(), vec![]).unwrap();
            let hp = register(parent, "union", b"u".to_vec(), vec![hc]).unwrap();
            assert_eq!(parents(hc), vec![hp]);
            let other = fresh(py);
            let ho = register(other, "union", b"u2".to_vec(), vec![]).unwrap();
            assert_eq!(parents(ho), vec![] as Vec<u64>);
            // Re-registering the same parent replaces its child list.
            let hp2 = register(parent, "union", b"u3".to_vec(), vec![]).unwrap();
            assert_eq!(hp2, hp);
            assert_eq!(parents(hc), vec![] as Vec<u64>);
        });
    }

    #[test]
    fn test_reset_clears_entries_and_identity() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "instance", b"q".to_vec(), vec![]).unwrap();
            assert!(entry_count() >= 1);
            reset();
            assert_eq!(entry_count(), 0);
            assert_eq!(entry_bytes(h), None);
            // Identity registry reset: the same object re-mints fresh.
            assert_ne!(identity::handle_for(obj).unwrap(), h);
        });
    }

    #[test]
    fn test_handle_of_does_not_mint() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            assert_eq!(identity::handle_of(obj), None);
            let h = identity::handle_for(obj).unwrap();
            assert_eq!(identity::handle_of(obj), Some(h));
        });
    }

    #[test]
    fn test_diff_description_length_change() {
        let d = diff_description(b"prefix", b"prefixlong");
        assert!(d.contains("lens 6 vs 10"), "bad: {}", d);
    }

    #[test]
    fn test_patch_instance_args_roundtrip() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let inst = |arg: Type| Type::Instance {
                type_ref: "builtins.list".to_string(),
                args: vec![arg],
                last_known_value: None,
                extra_attrs: None,
            };
            let mut w = WriteBuffer::new();
            write_type(&mut w, &inst(Type::NoneType)).unwrap();
            let h = register(obj, "instance", w.into_bytes(), vec![]).unwrap();

            let mut ab = WriteBuffer::new();
            write_type_list(&mut ab, &[Type::AnyType { type_of_any: 2, source_any: None, missing_import_name: None }]).unwrap();
            let new = patch_instance_args(h, &ab.into_bytes()).unwrap();

            let mut rbuf = ReadBuffer::new(&new);
            if let Type::Instance { args, .. } = read_type(&mut rbuf, None).unwrap() {
                assert!(matches!(args[0], Type::AnyType { .. }));
                assert_eq!(args.len(), 1);
            } else {
                panic!("not an Instance after patch");
            }
        });
    }

    #[test]
    fn test_patch_instance_args_defers_on_non_instance() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let tvar = Type::TypeVarType {
                name: "T".to_string(),
                fullname: "T".to_string(),
                raw_id: 0,
                namespace: "".to_string(),
                values: vec![],
                upper_bound: Box::new(Type::NoneType),
                default: Box::new(Type::NoneType),
                variance: 0,
                meta_level: 0,
            };
            let mut w = WriteBuffer::new();
            write_type(&mut w, &tvar).unwrap();
            let h = register(obj, "tvar", w.into_bytes(), vec![]).unwrap();
            let mut ab = WriteBuffer::new();
            write_type_list(&mut ab, &[]).unwrap();
            assert_eq!(patch_instance_args(h, &ab.into_bytes()), None);
        });
    }

    #[test]
    fn test_patch_instance_args_defers_on_bad_blob() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "instance", b"garbage".to_vec(), vec![]).unwrap();
            assert_eq!(patch_instance_args(h, &[9]), None);
            // Unregistered handle also defers.
            assert_eq!(patch_instance_args(h + 1, &[9]), None);
        });
    }
}
