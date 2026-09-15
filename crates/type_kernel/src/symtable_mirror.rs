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
//! 5. **Namespace order.** Each record carries the insertion ordinal of
//!    its name (assigned on insert, kept on replace, released on delete),
//!    which reproduces `dict.items()` order for insert, replace, delete
//!    and re-insert. That is what lets G3.1 serve a read from here.
//! 6. **The read gate is the consistency invariant (#1670).** A read is
//!    served only when `entry_count(owner) == len(owner.names)`;
//!    `entries_if_mirrored` returns the `ShadowGap` reason otherwise, so a
//!    namespace the capture could not see (a C-level `dict` write, a
//!    never-adopted table) never answers a read.

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
    pub(crate) order: u64,
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

/// Why the store cannot stand in for a live table (G3.1 read flip).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ShadowGap {
    /// The table was never adopted (no `put` ever recorded it).
    NoHandle,
    /// Fewer records than live entries: a write bypassed the capture.
    LenShort,
    /// More records than live entries: a delete bypassed the capture.
    LenLong,
    /// A record's pinned symbol is gone (defensive; cannot happen while
    /// the pins are held).
    NoPin,
}

/// G3.1 read-flip evidence counters. Process lifetime: `reset` drops
/// entries and pins but keeps these, so a corpus run accumulates them
/// across build boundaries (tests clear them explicitly).
#[derive(Default)]
pub(crate) struct FlipCounts {
    /// Tables consulted through the mirror gate (top level and nested).
    pub(crate) tables_looked: u64,
    /// Tables the store mirrored exactly, read in insertion order.
    pub(crate) tables_mirrored: u64,
    /// Entries handed to the consumer from the store.
    pub(crate) entries_mirrored: u64,
    pub(crate) defer_no_handle: u64,
    pub(crate) defer_len_short: u64,
    pub(crate) defer_len_long: u64,
    pub(crate) defer_no_pin: u64,
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

/// One TypeInfo meta-field record (G3.0c). `bases_count` / `mro_count`
/// are the list lengths at the last write; `metaclass_fullname` is the
/// fullname of the metaclass_type Instance or None; `fullname` is the
/// TypeInfo's `_fullname`; `names_handle` is the handle of the bound
/// `SymbolTable` (changes on a namespace rebind). `seq` is monotonic.
/// `extra` holds the extended G3.0c fields (bool flags, type-var
/// count, self_type/metaclass fullnames, deprecated string) keyed by
/// field name.
pub(crate) struct MetaEntry {
    pub(crate) seq: u64,
    pub(crate) bases_count: usize,
    pub(crate) mro_count: usize,
    pub(crate) metaclass_fullname: Option<String>,
    pub(crate) fullname: Option<String>,
    pub(crate) names_handle: u64,
    pub(crate) extra: HashMap<String, String>,
}

struct SymStore {
    entries: HashMap<(u64, String), SymEntry>,
    /// Owner handle -> table generation. Minted per owner identity.
    generations: HashMap<u64, u64>,
    /// Node handle -> the (owner, name) pairs referencing it.
    by_node: HashMap<u64, Vec<(u64, String)>>,
    /// Owner handle -> the names recorded for it, so serving one
    /// namespace costs its own entries instead of a scan of the store.
    by_owner: HashMap<u64, Vec<String>>,
    /// Strong pins: handles key on raw `id()`s, so each stored object
    /// stays alive until its entry is dropped or the store resets.
    pins: HashMap<u64, Py<PyAny>>,
    /// G3.0c: TypeInfo meta-field records keyed by the TypeInfo handle.
    meta: HashMap<u64, MetaEntry>,
    /// G3.1: read-flip evidence counters.
    flip: FlipCounts,
    next_generation: u64,
    next_seq: u64,
    /// G3.1: monotonic namespace insertion ordinal (never reused while
    /// the entry lives, so ordering is a total order).
    next_order: u64,
}

impl SymStore {
    fn new() -> Self {
        SymStore {
            entries: HashMap::new(),
            generations: HashMap::new(),
            by_node: HashMap::new(),
            by_owner: HashMap::new(),
            pins: HashMap::new(),
            meta: HashMap::new(),
            flip: FlipCounts::default(),
            next_generation: 0,
            next_seq: 0,
            next_order: 0,
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
        // Namespace order follows `dict` semantics: a replace keeps the
        // original position, a delete releases it and a re-insert lands
        // at the end (fresh ordinal).
        let order = match store.entries.get(&key) {
            Some(old) => {
                let old_order = old.order;
                let old_node = old.node_handle;
                if old_node != node_handle {
                    unlink_node(store, old_node, owner_handle, name);
                }
                old_order
            }
            None => {
                store.next_order += 1;
                store.next_order
            }
        };
        let entry = SymEntry {
            generation,
            seq,
            order,
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
        let is_new = store.entries.insert(key, entry).is_none();
        if is_new {
            store
                .by_owner
                .entry(owner_handle)
                .or_default()
                .push(name.to_string());
        }
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
            unlink_owner(store, owner_handle, name);
            true
        } else {
            false
        }
    }))
}

/// Drop one name from the owner index (and the owner when it empties).
fn unlink_owner(store: &mut SymStore, owner_handle: u64, name: &str) {
    if let Some(names) = store.by_owner.get_mut(&owner_handle) {
        names.retain(|n| n != name);
        if names.is_empty() {
            store.by_owner.remove(&owner_handle);
        }
    }
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
        store.by_owner.clear();
        store.meta.clear();
        store.next_generation = 0;
        store.next_seq = 0;
        store.next_order = 0;
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
    Ok(entry_count_of(owner_handle))
}

/// Entry count for a handle already known to the caller (no minting).
pub(crate) fn entry_count_of(owner_handle: u64) -> usize {
    with_store(|store| {
        store
            .entries
            .keys()
            .filter(|(handle, _)| *handle == owner_handle)
            .count()
    })
}

/// Live entry count across all owners.
pub(crate) fn total_entry_count() -> usize {
    with_store(|store| store.entries.len())
}

fn bump_flip(f: impl FnOnce(&mut FlipCounts)) {
    with_store(|store| f(&mut store.flip));
}

/// G3.1 read-flip read path: the `(name, symbol)` pairs the store holds
/// for `table`, in the namespace's insertion order, or the reason the
/// store cannot stand in for the live table.
///
/// The gate is the shadow-consistency invariant the G3 brief pins:
/// `entry_count(owner) == len(owner.names)`. A table the store does not
/// mirror exactly (never adopted, a write or delete the class patch could
/// not see, a lost pin) never serves a read, so a read flip can only ever
/// return what the live table would have returned. Ordering is the
/// namespace ordinal, which reproduces `dict.items()` for insert,
/// replace, delete and re-insert.
pub(crate) fn entries_if_mirrored(
    py: Python<'_>,
    table: &PyAny,
) -> Result<Vec<(String, Py<PyAny>)>, ShadowGap> {
    bump_flip(|c| c.tables_looked += 1);
    let Some(handle) = identity::handle_of(table) else {
        bump_flip(|c| c.defer_no_handle += 1);
        return Err(ShadowGap::NoHandle);
    };
    let Ok(table_len) = table.len() else {
        bump_flip(|c| c.defer_no_handle += 1);
        return Err(ShadowGap::NoHandle);
    };
    let triples: Vec<(u64, String, u64)> = with_store(|store| {
        let mut triples: Vec<(u64, String, u64)> = store
            .by_owner
            .get(&handle)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| {
                        store
                            .entries
                            .get(&(handle, name.clone()))
                            .map(|entry| (entry.order, name.clone(), entry.node_handle))
                    })
                    .collect()
            })
            .unwrap_or_default();
        triples.sort_by_key(|(order, _, _)| *order);
        triples
    });
    if triples.len() < table_len {
        bump_flip(|c| c.defer_len_short += 1);
        return Err(ShadowGap::LenShort);
    }
    if triples.len() > table_len {
        bump_flip(|c| c.defer_len_long += 1);
        return Err(ShadowGap::LenLong);
    }
    let mut out: Vec<(String, Py<PyAny>)> = Vec::with_capacity(triples.len());
    for (_, name, node_handle) in triples {
        match with_store(|store| store.pins.get(&node_handle).map(|pin| pin.clone_ref(py))) {
            Some(symbol) => out.push((name, symbol)),
            None => {
                bump_flip(|c| c.defer_no_pin += 1);
                return Err(ShadowGap::NoPin);
            }
        }
    }
    bump_flip(|c| {
        c.tables_mirrored += 1;
        c.entries_mirrored += out.len() as u64;
    });
    Ok(out)
}

/// Read-flip counters snapshot (evidence).
pub(crate) fn flip_counts() -> FlipCounts {
    with_store(|store| FlipCounts {
        tables_looked: store.flip.tables_looked,
        tables_mirrored: store.flip.tables_mirrored,
        entries_mirrored: store.flip.entries_mirrored,
        defer_no_handle: store.flip.defer_no_handle,
        defer_len_short: store.flip.defer_len_short,
        defer_len_long: store.flip.defer_len_long,
        defer_no_pin: store.flip.defer_no_pin,
    })
}

/// Drop the read-flip counters (tests and per-run evidence boundaries).
pub(crate) fn flip_counts_reset() {
    with_store(|store| store.flip = FlipCounts::default());
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

/// G3.1 read-flip evidence counters: tables consulted / mirrored from
/// the store, entries served, and the per-reason defer counts of the
/// mirror gate. Process lifetime (reset keeps them).
#[pyfunction]
pub(crate) fn rust_symtable_mirror_flip_counts<'py>(py: Python<'py>) -> PyResult<&'py PyDict> {
    let counts = flip_counts();
    let dict = PyDict::new(py);
    dict.set_item("tables_looked", counts.tables_looked)?;
    dict.set_item("tables_mirrored", counts.tables_mirrored)?;
    dict.set_item("entries_mirrored", counts.entries_mirrored)?;
    dict.set_item("defer_no_handle", counts.defer_no_handle)?;
    dict.set_item("defer_len_short", counts.defer_len_short)?;
    dict.set_item("defer_len_long", counts.defer_len_long)?;
    dict.set_item("defer_no_pin", counts.defer_no_pin)?;
    Ok(dict)
}

/// Drop the read-flip counters; returns the tables-mirrored count that
/// was cleared (evidence receipt).
#[pyfunction]
pub(crate) fn rust_symtable_mirror_flip_counts_reset() -> u64 {
    let mirrored = flip_counts().tables_mirrored;
    flip_counts_reset();
    mirrored
}

// ---- G3.0c: TypeInfo meta-field capture ----

/// Record (or replace) the meta fields for one TypeInfo. `info` is the
/// live TypeInfo; `names_table` is the bound `SymbolTable` (may be a new
/// object after a namespace rebind). `metaclass_fullname` is the fullname
/// of the metaclass Instance or None. Returns the seq.
pub(crate) fn meta_put(
    info: &PyAny,
    bases_count: usize,
    mro_count: usize,
    metaclass_fullname: Option<String>,
    fullname: Option<String>,
    names_table: &PyAny,
) -> PyResult<u64> {
    let info_handle = handle_or_error(info)?;
    let names_handle = handle_or_error(names_table)?;
    Ok(with_store(|store| {
        store.next_seq += 1;
        let seq = store.next_seq;
        let extra = store
            .meta
            .get(&info_handle)
            .map(|e| e.extra.clone())
            .unwrap_or_default();
        store.meta.insert(
            info_handle,
            MetaEntry {
                seq,
                bases_count,
                mro_count,
                metaclass_fullname,
                fullname,
                names_handle,
                extra,
            },
        );
        store.pins.insert(info_handle, Py::from(info));
        store.pins.insert(names_handle, Py::from(names_table));
        seq
    }))
}

/// Record one extended G3.0c field on an existing MetaEntry (or create
/// a minimal entry if none exists yet). The value is a string encoding
/// (bool as "true"/"false", int as its string form, fullname string).
pub(crate) fn meta_put_field(info: &PyAny, field: &str, value: &str) -> PyResult<()> {
    let info_handle = handle_or_error(info)?;
    with_store(|store| {
        store.next_seq += 1;
        let seq = store.next_seq;
        let entry = store.meta.entry(info_handle).or_insert(MetaEntry {
            seq,
            bases_count: 0,
            mro_count: 0,
            metaclass_fullname: None,
            fullname: None,
            names_handle: 0,
            extra: HashMap::new(),
        });
        entry.seq = seq;
        entry.extra.insert(field.to_string(), value.to_string());
    });
    Ok(())
}

/// Read the meta record for one TypeInfo; None when never recorded.
pub(crate) fn meta_lookup<'a>(py: Python<'a>, info: &'a PyAny) -> PyResult<Option<&'a PyDict>> {
    let info_handle = match identity::handle_of(info) {
        Some(handle) => handle,
        None => return Ok(None),
    };
    let record = with_store(|store| {
        store.meta.get(&info_handle).map(|e| {
            (
                e.seq,
                e.bases_count,
                e.mro_count,
                e.metaclass_fullname.clone(),
                e.fullname.clone(),
                e.names_handle,
                e.extra.clone(),
            )
        })
    });
    let Some((seq, bases_count, mro_count, mc_fullname, fullname, names_handle, extra)) = record
    else {
        return Ok(None);
    };
    let dict = PyDict::new(py);
    dict.set_item("seq", seq)?;
    dict.set_item("bases_count", bases_count)?;
    dict.set_item("mro_count", mro_count)?;
    dict.set_item("metaclass_fullname", mc_fullname)?;
    dict.set_item("fullname", fullname)?;
    dict.set_item("names_handle", names_handle)?;
    let extra_dict = PyDict::new(py);
    for (k, v) in &extra {
        extra_dict.set_item(k, v)?;
    }
    dict.set_item("extra", extra_dict)?;
    Ok(Some(dict))
}

/// Remove the meta record for one TypeInfo; returns whether one existed.
pub(crate) fn meta_delete(info: &PyAny) -> PyResult<bool> {
    let info_handle = handle_or_error(info)?;
    Ok(with_store(|store| {
        store.meta.remove(&info_handle).is_some()
    }))
}

/// Count of TypeInfo meta records (audit).
pub(crate) fn meta_entry_count() -> usize {
    with_store(|store| store.meta.len())
}

#[pyfunction]
#[pyo3(signature = (info, bases_count, mro_count, metaclass_fullname, fullname, names_table))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_symtable_mirror_meta_put(
    info: &PyAny,
    bases_count: usize,
    mro_count: usize,
    metaclass_fullname: Option<String>,
    fullname: Option<String>,
    names_table: &PyAny,
) -> PyResult<u64> {
    meta_put(
        info,
        bases_count,
        mro_count,
        metaclass_fullname,
        fullname,
        names_table,
    )
}

#[pyfunction]
pub(crate) fn rust_symtable_mirror_meta_put_field(
    info: &PyAny,
    field: &str,
    value: &str,
) -> PyResult<()> {
    meta_put_field(info, field, value)
}

#[pyfunction]
pub(crate) fn rust_symtable_mirror_meta_lookup<'py>(
    py: Python<'py>,
    info: &'py PyAny,
) -> PyResult<Option<&'py PyDict>> {
    meta_lookup(py, info)
}

#[pyfunction]
pub(crate) fn rust_symtable_mirror_meta_delete(info: &PyAny) -> PyResult<bool> {
    meta_delete(info)
}

#[pyfunction]
pub(crate) fn rust_symtable_mirror_meta_entry_count() -> usize {
    meta_entry_count()
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
                    let node_fullname = e.node_fullname.clone();
                    (e.kind, node_fullname)
                })
        })
    }

    fn generation_for(owner: &PyAny) -> u64 {
        let handle = identity::handle_for(owner).unwrap();
        with_store(|store| *store.generations.get(&handle).unwrap())
    }

    fn live_table<'py>(py: Python<'py>, names: &[&str]) -> &'py PyAny {
        let table = py.eval("{}", None, None).unwrap();
        for name in names {
            table
                .set_item(*name, py.eval("object()", None, None).unwrap())
                .unwrap();
        }
        table
    }

    fn read_names(py: Python<'_>, table: &PyAny) -> Vec<String> {
        entries_if_mirrored(py, table)
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn test_order_follows_dict_insert_replace_delete() {
        with_py(|py| {
            reset();
            let table = live_table(py, &["a", "b", "c"]);
            let node = fresh_object(py);
            for name in ["a", "b", "c"] {
                put(table, name, node, flags(1)).unwrap();
            }
            assert_eq!(read_names(py, table), vec!["a", "b", "c"]);
            // A replace keeps the position, exactly like `dict`.
            put(table, "b", fresh_object(py), flags(2)).unwrap();
            assert_eq!(read_names(py, table), vec!["a", "b", "c"]);
            // A delete releases the ordinal; a re-insert lands last, and
            // the live dict does the same.
            delete(table, "a").unwrap();
            table.del_item("a").unwrap();
            assert_eq!(read_names(py, table), vec!["b", "c"]);
            put(table, "a", node, flags(1)).unwrap();
            table
                .set_item("a", py.eval("object()", None, None).unwrap())
                .unwrap();
            assert_eq!(read_names(py, table), vec!["b", "c", "a"]);
            assert_eq!(
                read_names(py, table),
                table
                    .call_method0("keys")
                    .unwrap()
                    .iter()
                    .unwrap()
                    .map(|k| k.unwrap().extract::<String>().unwrap())
                    .collect::<Vec<String>>()
            );
        });
    }

    #[test]
    fn test_gate_defers_on_unknown_owner_and_length_drift() {
        with_py(|py| {
            reset();
            flip_counts_reset();
            let table = live_table(py, &["a"]);
            // Never adopted -> no handle.
            assert!(matches!(
                entries_if_mirrored(py, table),
                Err(ShadowGap::NoHandle)
            ));
            put(table, "a", fresh_object(py), flags(1)).unwrap();
            assert_eq!(read_names(py, table), vec!["a"]);
            // A live entry the store never saw (a C-level write).
            table.set_item("b", fresh_object(py)).unwrap();
            assert!(matches!(
                entries_if_mirrored(py, table),
                Err(ShadowGap::LenShort)
            ));
            // A store record the live table no longer has.
            table.del_item("b").unwrap();
            put(table, "b", fresh_object(py), flags(1)).unwrap();
            assert!(matches!(
                entries_if_mirrored(py, table),
                Err(ShadowGap::LenLong)
            ));
            let counts = flip_counts();
            assert_eq!(counts.tables_looked, 4);
            assert_eq!(counts.tables_mirrored, 1);
            assert_eq!(counts.entries_mirrored, 1);
            assert_eq!(counts.defer_no_handle, 1);
            assert_eq!(counts.defer_len_short, 1);
            assert_eq!(counts.defer_len_long, 1);
        });
    }

    #[test]
    fn test_owner_index_tracks_entries_exactly() {
        with_py(|py| {
            reset();
            let table = live_table(py, &["a", "b"]);
            let node = fresh_object(py);
            let handle = identity::handle_for(table).unwrap();
            for name in ["a", "b"] {
                put(table, name, node, flags(1)).unwrap();
            }
            // A replace must not double-add the name to the index.
            put(table, "a", fresh_object(py), flags(2)).unwrap();
            let indexed = with_store(|store| store.by_owner.get(&handle).map(Vec::len));
            assert_eq!(indexed, Some(2));
            assert_eq!(entry_count_of(handle), 2);
            delete(table, "b").unwrap();
            let indexed = with_store(|store| store.by_owner.get(&handle).map(Vec::len));
            assert_eq!(indexed, Some(1));
            assert_eq!(entry_count_of(handle), 1);
            delete(table, "a").unwrap();
            // An emptied owner drops out of the index entirely.
            assert_eq!(
                with_store(|store| store.by_owner.get(&handle).map(Vec::len)),
                None
            );
            assert_eq!(entry_count_of(handle), 0);
        });
    }

    #[test]
    fn test_flip_counts_survive_entries_reset() {
        with_py(|py| {
            reset();
            flip_counts_reset();
            let table = live_table(py, &["a"]);
            put(table, "a", fresh_object(py), flags(1)).unwrap();
            assert_eq!(read_names(py, table), vec!["a"]);
            assert_eq!(rust_symtable_mirror_flip_counts_reset(), 1);
            assert_eq!(flip_counts().tables_looked, 0);
            // `reset` drops entries and pins but keeps the evidence.
            put(table, "a", fresh_object(py), flags(1)).unwrap();
            assert_eq!(read_names(py, table), vec!["a"]);
            assert_eq!(reset(), 1);
            assert_eq!(flip_counts().tables_mirrored, 1);
            assert!(matches!(
                entries_if_mirrored(py, table),
                Err(ShadowGap::LenShort)
            ));
        });
    }
}
