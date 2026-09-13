//! Phase G3.0a namespace dual-write capture shadow (issue #1581).
//!
//! Per-namespace shadow storage for symbol tables: one record per
//! `(owner handle, name)` pair carrying the table generation, a monotonic
//! capture seq, the referenced node's fullname, and the symbol ref flags.
//! The "owner" is the `SymbolTable` object itself (module, class and
//! function namespaces are all `SymbolTable` instances), so a namespace
//! rebind (`owner.names = SymbolTable()`) mints a fresh owner handle and
//! a fresh generation: per-name records never merge across generations.
//!
//! Guarantees:
//! 1. **Thread-local.** Entries and pins live in a `thread_local!` cell
//!    keyed by handles minted on the same thread, so no locking is needed.
//! 2. **Strong pins.** Every stored owner table and referenced node is
//!    held by a `Py<PyAny>` until the store resets, so a recycled `id()`
//!    can never adopt a stale entry (the handle keys on `id()`).
//! 3. **Merge-on-capture.** A put for an existing pair replaces the record
//!    in place (seq/generation stay the owner's), and a node flag refresh
//!    updates every record that references the node.
//! 4. **Identity is not owned here.** `reset` clears entries and pins
//!    only; `identity::reset` stays with `rust_mirror_reset` (mirror.rs)
//!    so namespace-shadow state cannot invalidate handles other seams hold.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::identity;

/// One `(owner, name)` record. Flags are a snapshot at the last put or
/// flag refresh; `node_handle` keys the reverse index used to refresh
/// every record that references the same node.
pub(crate) struct SymEntry {
    pub(crate) generation: u64,
    pub(crate) seq: u64,
    pub(crate) node_handle: u64,
    pub(crate) kind: i64,
    pub(crate) node_fullname: Option<String>,
    pub(crate) module_public: bool,
    pub(crate) module_hidden: bool,
    pub(crate) implicit: bool,
    pub(crate) plugin_generated: bool,
    pub(crate) no_serialize: bool,
    pub(crate) cross_ref: Option<String>,
}

/// The ref flags passed on every put/refresh.
pub(crate) struct SymFlags {
    pub(crate) kind: i64,
    pub(crate) node_fullname: Option<String>,
    pub(crate) module_public: bool,
    pub(crate) module_hidden: bool,
    pub(crate) implicit: bool,
    pub(crate) plugin_generated: bool,
    pub(crate) no_serialize: bool,
    pub(crate) cross_ref: Option<String>,
}

struct SymStore {
    entries: HashMap<(u64, String), SymEntry>,
    /// Owner handle -> table generation. Minted per owner identity.
    generations: HashMap<u64, u64>,
    /// Node handle -> the (owner, name) pairs referencing it.
    by_node: HashMap<u64, Vec<(u64, String)>>,
    /// Strong pins: handles key on raw `id()`s, so each stored object
    /// stays alive until its entry is dropped or the store resets.
    pins: HashMap<u64, Py<PyAny>>,
    next_generation: u64,
    next_seq: u64,
}

impl SymStore {
    fn new() -> Self {
        SymStore {
            entries: HashMap::new(),
            generations: HashMap::new(),
            by_node: HashMap::new(),
            pins: HashMap::new(),
            next_generation: 0,
            next_seq: 0,
        }
    }

    fn generation_for(&mut self, owner: u64) -> u64 {
        *self.generations.entry(owner).or_insert_with(|| {
            self.next_generation += 1;
            self.next_generation
        })
    }
}

thread_local! {
    static STORE: RefCell<SymStore> = RefCell::new(SymStore::new());
}

fn with_store<T>(f: impl FnOnce(&mut SymStore) -> T) -> T {
    STORE.with(|cell| f(&mut cell.borrow_mut()))
}

fn handle_or_error(obj: &PyAny) -> PyResult<u64> {
    identity::handle_for(obj)
        .ok_or_else(|| PyValueError::new_err("symtable_mirror: object has no identity handle"))
}

fn unlink_node(store: &mut SymStore, node_handle: u64, owner: u64, name: &str) {
    if let Some(refs) = store.by_node.get_mut(&node_handle) {
        refs.retain(|(o, n)| !(*o == owner && n == name));
        if refs.is_empty() {
            store.by_node.remove(&node_handle);
        }
    }
}

/// Record (or replace) the entry for `(owner, name)`; returns
/// `(owner_handle, node_handle, seq, generation)`.
pub(crate) fn put(
    owner: &PyAny,
    name: &str,
    symbol: &PyAny,
    flags: SymFlags,
) -> PyResult<(u64, u64, u64, u64)> {
    let owner_handle = handle_or_error(owner)?;
    let node_handle = handle_or_error(symbol)?;
    Ok(with_store(|store| {
        let generation = store.generation_for(owner_handle);
        store.next_seq += 1;
        let seq = store.next_seq;
        let key = (owner_handle, name.to_string());
        if let Some(old) = store.entries.get(&key) {
            let old_node = old.node_handle;
            if old_node != node_handle {
                unlink_node(store, old_node, owner_handle, name);
            }
        }
        let entry = SymEntry {
            generation,
            seq,
            node_handle,
            kind: flags.kind,
            node_fullname: flags.node_fullname,
            module_public: flags.module_public,
            module_hidden: flags.module_hidden,
            implicit: flags.implicit,
            plugin_generated: flags.plugin_generated,
            no_serialize: flags.no_serialize,
            cross_ref: flags.cross_ref,
        };
        store.entries.insert(key, entry);
        let refs = store.by_node.entry(node_handle).or_default();
        if !refs.iter().any(|(o, n)| *o == owner_handle && n == name) {
            refs.push((owner_handle, name.to_string()));
        }
        store.pins.insert(owner_handle, Py::from(owner));
        store.pins.insert(node_handle, Py::from(symbol));
        (owner_handle, node_handle, seq, generation)
    }))
}

/// Remove the record for `(owner, name)`; returns whether one existed.
///
/// The pins (owner and node) stay until reset: another record may still
/// reference the node, and a live pin keeps `id()` keys unambiguous.
pub(crate) fn delete(owner: &PyAny, name: &str) -> PyResult<bool> {
    let owner_handle = handle_or_error(owner)?;
    Ok(with_store(|store| {
        let key = (owner_handle, name.to_string());
        if let Some(entry) = store.entries.remove(&key) {
            unlink_node(store, entry.node_handle, owner_handle, name);
            true
        } else {
            false
        }
    }))
}

/// Refresh the flags of every record referencing `node`; returns whether
/// any record was updated (false for a node the store never adopted).
pub(crate) fn refresh_flags(node: &PyAny, flags: SymFlags) -> PyResult<bool> {
    let node_handle = match identity::handle_for(node) {
        Some(handle) => handle,
        None => return Ok(false),
    };
    Ok(with_store(|store| {
        let keys = match store.by_node.get(&node_handle) {
            Some(refs) => refs.clone(),
            None => return false,
        };
        let mut updated = false;
        for key in keys {
            if let Some(entry) = store.entries.get_mut(&key) {
                entry.kind = flags.kind;
                if flags.node_fullname.is_some() {
                    entry.node_fullname = flags.node_fullname.clone();
                }
                entry.module_public = flags.module_public;
                entry.module_hidden = flags.module_hidden;
                entry.implicit = flags.implicit;
                entry.plugin_generated = flags.plugin_generated;
                entry.no_serialize = flags.no_serialize;
                entry.cross_ref = flags.cross_ref.clone();
                updated = true;
            }
        }
        updated
    }))
}

/// Remove every entry and pin; returns how many entries were dropped.
/// Deliberately does NOT call `identity::reset`: the raw handle registry
/// is owned by `rust_mirror_reset`, and namespace-shadow state must not
/// invalidate handles other seams still hold.
pub(crate) fn reset() -> usize {
    let (entries, pins) = with_store(|store| {
        let entries = store.entries.len();
        let pins: Vec<Py<PyAny>> = store.pins.drain().map(|(_, pin)| pin).collect();
        store.entries.clear();
        store.generations.clear();
        store.by_node.clear();
        store.next_generation = 0;
        store.next_seq = 0;
        (entries, pins)
    });
    // Drop the pins only after the guard is released: releasing the last
    // reference can run a Python deallocator that re-enters the store.
    drop(pins);
    entries
}

/// Live entry count for one owner.
pub(crate) fn entry_count(owner: &PyAny) -> PyResult<usize> {
    let owner_handle = handle_or_error(owner)?;
    Ok(with_store(|store| {
        store
            .entries
            .keys()
            .filter(|(handle, _)| *handle == owner_handle)
            .count()
    }))
}

/// Live entry count across all owners.
pub(crate) fn total_entry_count() -> usize {
    with_store(|store| store.entries.len())
}

// ---- pyfunction wrappers ----

/// Record one committed symbol-table put; returns
/// `(owner_handle, node_handle, seq, generation)`.
#[pyfunction]
#[pyo3(signature = (
    owner,
    name,
    symbol,
    kind,
    node_fullname,
    module_public,
    module_hidden,
    implicit,
    plugin_generated,
    no_serialize,
    cross_ref,
))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_symtable_mirror_put(
    owner: &PyAny,
    name: &str,
    symbol: &PyAny,
    kind: i64,
    node_fullname: Option<String>,
    module_public: bool,
    module_hidden: bool,
    implicit: bool,
    plugin_generated: bool,
    no_serialize: bool,
    cross_ref: Option<String>,
) -> PyResult<(u64, u64, u64, u64)> {
    let flags = SymFlags {
        kind,
        node_fullname,
        module_public,
        module_hidden,
        implicit,
        plugin_generated,
        no_serialize,
        cross_ref,
    };
    put(owner, name, symbol, flags)
}

/// Remove one record; returns whether one existed.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_delete(owner: &PyAny, name: &str) -> PyResult<bool> {
    delete(owner, name)
}

/// Refresh ref flags on every record referencing `node`; false when the
/// node was never adopted by a put.
#[pyfunction]
#[pyo3(signature = (
    node,
    kind,
    node_fullname,
    module_public,
    module_hidden,
    implicit,
    plugin_generated,
    no_serialize,
    cross_ref,
))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_symtable_mirror_refresh_flags(
    node: &PyAny,
    kind: i64,
    node_fullname: Option<String>,
    module_public: bool,
    module_hidden: bool,
    implicit: bool,
    plugin_generated: bool,
    no_serialize: bool,
    cross_ref: Option<String>,
) -> PyResult<bool> {
    let flags = SymFlags {
        kind,
        node_fullname,
        module_public,
        module_hidden,
        implicit,
        plugin_generated,
        no_serialize,
        cross_ref,
    };
    refresh_flags(node, flags)
}

/// Read one record as a dict; None when `(owner, name)` has no entry.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_lookup<'py>(
    py: Python<'py>,
    owner: &PyAny,
    name: &str,
) -> PyResult<Option<&'py PyDict>> {
    let owner_handle = match identity::handle_of(owner) {
        Some(handle) => handle,
        None => return Ok(None),
    };
    let record = with_store(|store| {
        store
            .entries
            .get(&(owner_handle, name.to_string()))
            .map(|e| {
                (
                    e.generation,
                    e.seq,
                    e.node_handle,
                    e.kind,
                    e.node_fullname.clone(),
                    e.module_public,
                    e.module_hidden,
                    e.implicit,
                    e.plugin_generated,
                    e.no_serialize,
                    e.cross_ref.clone(),
                )
            })
    });
    let Some((generation, seq, node_handle, kind, fullname, mp, mh, implicit, pg, ns, cr)) = record
    else {
        return Ok(None);
    };
    let dict = PyDict::new(py);
    dict.set_item("generation", generation)?;
    dict.set_item("seq", seq)?;
    dict.set_item("node_handle", node_handle)?;
    dict.set_item("kind", kind)?;
    dict.set_item("node_fullname", fullname)?;
    dict.set_item("module_public", mp)?;
    dict.set_item("module_hidden", mh)?;
    dict.set_item("implicit", implicit)?;
    dict.set_item("plugin_generated", pg)?;
    dict.set_item("no_serialize", ns)?;
    dict.set_item("cross_ref", cr)?;
    Ok(Some(dict))
}

/// Entry count for one owner.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_entry_count(owner: &PyAny) -> PyResult<usize> {
    entry_count(owner)
}

/// Entry count across every owner (audit).
#[pyfunction]
pub(crate) fn rust_symtable_mirror_total_entry_count() -> usize {
    total_entry_count()
}

/// Names recorded for one owner (audit + convergence checks).
#[pyfunction]
pub(crate) fn rust_symtable_mirror_names(owner: &PyAny) -> PyResult<Vec<String>> {
    let owner_handle = handle_or_error(owner)?;
    let mut names: Vec<String> = with_store(|store| {
        store
            .entries
            .keys()
            .filter(|(handle, _)| *handle == owner_handle)
            .map(|(_, name)| name.clone())
            .collect()
    });
    names.sort();
    Ok(names)
}

/// Table generation for one owner; None when the owner is unknown.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_generation(owner: &PyAny) -> Option<u64> {
    let handle = identity::handle_of(owner)?;
    with_store(|store| store.generations.get(&handle).copied())
}

/// Clear all namespace-shadow entries and pins; returns the dropped count.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_reset() -> usize {
    reset()
}

/// Non-minting identity handle lookup; None when never registered.
#[pyfunction]
pub(crate) fn rust_symtable_mirror_handle_of(obj: &PyAny) -> Option<u64> {
    identity::handle_of(obj)
}

#[cfg(test)]
mod symtable_mirror_tests {
    use super::*;

    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    fn flags(kind: i64) -> SymFlags {
        SymFlags {
            kind,
            node_fullname: Some("mod.x".to_string()),
            module_public: true,
            module_hidden: false,
            implicit: false,
            plugin_generated: false,
            no_serialize: false,
            cross_ref: None,
        }
    }

    #[test]
    fn test_put_lookup_roundtrip() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let node = fresh_object(py);
            let (_, node_handle, seq, generation) = put(owner, "x", node, flags(1)).unwrap();
            assert_eq!(entry_count(owner).unwrap(), 1);
            assert_eq!(total_entry_count(), 1);
            assert_eq!(generation, 1);
            assert_eq!(seq, 1);
            let (handle, n_handle, _, _) = put(owner, "y", node, flags(2)).unwrap();
            assert_eq!(n_handle, node_handle);
            assert_eq!(handle, identity::handle_for(owner).unwrap());
            assert_eq!(generation_for(owner), 1);
        });
    }

    #[test]
    fn test_replace_relinks_by_node() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let first = fresh_object(py);
            let second = fresh_object(py);
            put(owner, "x", first, flags(1)).unwrap();
            assert!(refresh_flags(first, flags(9)).unwrap());
            put(owner, "x", second, flags(2)).unwrap();
            // The replaced node no longer updates, the new one does.
            assert!(!refresh_flags(first, flags(9)).unwrap());
            assert!(refresh_flags(second, flags(3)).unwrap());
            let record = lookup_raw(owner, "x");
            assert_eq!(record.unwrap().0, 3);
            assert_eq!(entry_count(owner).unwrap(), 1);
        });
    }

    #[test]
    fn test_delete_unlinks_and_missing_returns_false() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let node = fresh_object(py);
            put(owner, "x", node, flags(1)).unwrap();
            assert!(delete(owner, "x").unwrap());
            assert!(!delete(owner, "x").unwrap());
            assert_eq!(entry_count(owner).unwrap(), 0);
            // A deleted node no longer has a record to refresh.
            assert!(!refresh_flags(node, flags(2)).unwrap());
        });
    }

    #[test]
    fn test_one_node_two_names_both_refresh() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let node = fresh_object(py);
            put(owner, "a", node, flags(1)).unwrap();
            put(owner, "b", node, flags(1)).unwrap();
            assert!(refresh_flags(node, flags(7)).unwrap());
            assert_eq!(lookup_raw(owner, "a").unwrap().0, 7);
            assert_eq!(lookup_raw(owner, "b").unwrap().0, 7);
            delete(owner, "a").unwrap();
            assert!(refresh_flags(node, flags(8)).unwrap());
            assert_eq!(lookup_raw(owner, "b").unwrap().0, 8);
        });
    }

    #[test]
    fn test_generations_are_per_owner() {
        with_py(|py| {
            reset();
            let owner_a = fresh_object(py);
            let owner_b = fresh_object(py);
            let node = fresh_object(py);
            let (_, _, _, gen_a) = put(owner_a, "x", node, flags(1)).unwrap();
            let (_, _, _, gen_b) = put(owner_b, "x", node, flags(1)).unwrap();
            assert_ne!(gen_a, gen_b);
            assert_eq!(entry_count(owner_a).unwrap(), 1);
            assert_eq!(entry_count(owner_b).unwrap(), 1);
        });
    }

    #[test]
    fn test_reset_clears_store_but_not_identity() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let node = fresh_object(py);
            let (handle, node_handle, _, _) = put(owner, "x", node, flags(1)).unwrap();
            assert_eq!(reset(), 1);
            assert_eq!(total_entry_count(), 0);
            assert_eq!(entry_count(owner).unwrap(), 0);
            // `rust_mirror_reset` alone owns `identity::reset`.
            assert_eq!(identity::handle_of(owner), Some(handle));
            assert_eq!(identity::handle_of(node), Some(node_handle));
        });
    }

    #[test]
    fn test_pyfunctions_answer_the_record() {
        with_py(|py| {
            reset();
            let owner = fresh_object(py);
            let node = fresh_object(py);
            let (handle, node_handle, _, generation) = rust_symtable_mirror_put(
                owner,
                "x",
                node,
                1,
                Some("mod.x".to_string()),
                true,
                false,
                false,
                false,
                false,
                None,
            )
            .unwrap();
            assert_eq!(rust_symtable_mirror_entry_count(owner).unwrap(), 1);
            assert_eq!(rust_symtable_mirror_total_entry_count(), 1);
            assert_eq!(rust_symtable_mirror_generation(owner), Some(generation));
            assert_eq!(
                rust_symtable_mirror_names(owner).unwrap(),
                vec!["x".to_string()]
            );
            let dict = rust_symtable_mirror_lookup(py, owner, "x")
                .unwrap()
                .unwrap();
            assert_eq!(
                dict.get_item("node_handle")
                    .unwrap()
                    .unwrap()
                    .extract::<u64>()
                    .unwrap(),
                node_handle
            );
            assert_eq!(
                dict.get_item("kind")
                    .unwrap()
                    .unwrap()
                    .extract::<i64>()
                    .unwrap(),
                1
            );
            assert!(rust_symtable_mirror_refresh_flags(
                node,
                2,
                None,
                false,
                true,
                true,
                true,
                true,
                Some("m".to_string())
            )
            .unwrap());
            assert_eq!(rust_symtable_mirror_handle_of(owner), Some(handle));
            assert!(rust_symtable_mirror_delete(owner, "x").unwrap());
            assert!(rust_symtable_mirror_lookup(py, owner, "x")
                .unwrap()
                .is_none());
        });
    }

    fn lookup_raw(owner: &PyAny, name: &str) -> Option<(i64, Option<String>)> {
        let owner_handle = identity::handle_for(owner)?;
        with_store(|store| {
            store
                .entries
                .get(&(owner_handle, name.to_string()))
                .map(|e| {
                    let fullname = e.node_fullname.clone();
                    (e.kind, fullname)
                })
        })
    }

    fn generation_for(owner: &PyAny) -> u64 {
        let handle = identity::handle_for(owner).unwrap();
        with_store(|store| *store.generations.get(&handle).unwrap())
    }
}
