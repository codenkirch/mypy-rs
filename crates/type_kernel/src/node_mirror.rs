//! Phase G1.0a expression dual-write node shadow (issue #1572).
//!
//! Per-node shadow storage for the first AST family: the `RefExpr`
//! binding scalars (`kind`, target `node` fullname, `_fullname`,
//! `is_new_def`, `is_inferred_def`) plus the last `analyzed` replacement
//! class name (record-only). Python stays authoritative; no consumer
//! reads this store in G1.0a, so every op is a pure capture.
//!
//! Guarantees:
//! 1. **Thread-local.** Entries and pins live in a `thread_local!` cell
//!    keyed by handles minted on the same thread, so no locking is needed.
//! 2. **Strong pins.** Each stored object is held by a `Py<PyAny>` until
//!    its entry is dropped or the store resets, so a recycled `id()` can
//!    never adopt a stale entry (the handle keys on `id()`).
//! 3. **Merge-on-capture.** `capture_ref` and `capture_analyzed` update
//!    one record per object: a `CallExpr` that both binds a callee name
//!    and gets an analyzed replacement keeps both shadows, and the
//!    monotonic capture counters prove each write was seen.
//! 4. **Identity is not owned here.** `reset` clears entries and pins
//!    only; `identity::reset` stays with `rust_mirror_reset` (mirror.rs)
//!    so node-shadow state cannot invalidate handles other seams hold.
//! 5. **Serving reads is opt-in and provable.** `serve_ref_scalars` (G1.1)
//!    answers a `RefExpr` scalar read from the record when the record is
//!    provably exact, and returns `None` otherwise so every caller keeps
//!    its live read. Mode 0 (default) serves nothing; mode 2 additionally
//!    compares every served read against the live slots and counts the
//!    mismatches, so the differential cannot pass vacuously.

use std::cell::RefCell;
use std::collections::HashMap;
use std::thread::LocalKey;

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::identity;

/// One object's shadow record. `kind`/`node_fullname` are `Option` so an
/// unbound node (`node is None`) stays distinguishable from a resolved
/// name; `has_analyzed` separates "never captured" from "cleared to None".
#[derive(Default)]
pub(crate) struct NodeShadow {
    pub(crate) kind: Option<i64>,
    pub(crate) node_fullname: Option<String>,
    pub(crate) fullname: String,
    pub(crate) is_new_def: bool,
    pub(crate) is_inferred_def: bool,
    pub(crate) has_analyzed: bool,
    pub(crate) analyzed_kind: Option<String>,
    pub(crate) ref_captures: u64,
    pub(crate) analyzed_captures: u64,
    // ---- G1.0b: remaining G1 expression fields (wave 71A, #1576) ----
    /// Per-field records keyed by Python slot name; map presence is the
    /// "captured at least once" marker.
    pub(crate) fields: HashMap<String, FieldValue>,
    pub(crate) field_captures: u64,
}

struct NodeStore {
    by_handle: HashMap<u64, NodeShadow>,
    /// Strong pins: handles key on raw `id()`s, so each stored object
    /// stays alive until its entry is dropped or the store resets.
    pins: HashMap<u64, Py<PyAny>>,
}

impl NodeStore {
    fn new() -> Self {
        NodeStore {
            by_handle: HashMap::new(),
            pins: HashMap::new(),
        }
    }
}

thread_local! {
    static STORE: RefCell<NodeStore> = RefCell::new(NodeStore::new());
}

fn with_store<T>(f: impl FnOnce(&mut NodeStore) -> T) -> T {
    STORE.with(|cell| f(&mut cell.borrow_mut()))
}

fn handle_or_error(obj: &PyAny) -> PyResult<u64> {
    identity::handle_for(obj)
        .ok_or_else(|| PyValueError::new_err("node_mirror: object has no identity handle"))
}

/// Capture the five binding scalars for `obj`; returns the identity handle.
pub(crate) fn capture_ref(
    obj: &PyAny,
    kind: Option<i64>,
    node_fullname: Option<String>,
    fullname: String,
    is_new_def: bool,
    is_inferred_def: bool,
) -> PyResult<u64> {
    let handle = handle_or_error(obj)?;
    with_store(|store| {
        let entry = store.by_handle.entry(handle).or_default();
        entry.kind = kind;
        entry.node_fullname = node_fullname;
        entry.fullname = fullname;
        entry.is_new_def = is_new_def;
        entry.is_inferred_def = is_inferred_def;
        entry.ref_captures += 1;
        store.pins.insert(handle, Py::from(obj));
    });
    Ok(handle)
}

/// Capture the last `analyzed` replacement class name for `obj`
/// (record-only); returns the identity handle.
pub(crate) fn capture_analyzed(obj: &PyAny, analyzed_kind: Option<String>) -> PyResult<u64> {
    let handle = handle_or_error(obj)?;
    with_store(|store| {
        let entry = store.by_handle.entry(handle).or_default();
        entry.has_analyzed = true;
        entry.analyzed_kind = analyzed_kind;
        entry.analyzed_captures += 1;
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
/// is owned by `rust_mirror_reset`, and node-shadow state must not
/// invalidate handles other seams still hold. The binding-target pins
/// drop here too: they were minted by the captures this reset erases, so
/// the per-build boundary keeps handles from resolving stale targets.
pub(crate) fn reset() -> usize {
    let (entries, pins) = with_store(|store| {
        let entries = store.by_handle.len();
        let pins: Vec<Py<PyAny>> = store.pins.drain().map(|(_, pin)| pin).collect();
        store.by_handle.clear();
        (entries, pins)
    });
    // Drop the pins only after the guard is released (see `retire`).
    drop(pins);
    reset_pins();
    entries
}

/// Number of live entries.
pub(crate) fn entry_count() -> usize {
    with_store(|store| store.by_handle.len())
}

// ---- G2 (#1787): binding-target handle pins ----

// One strong pin per captured `RefExpr` binding target, keyed by the
// shared identity handle, so a later consumer can resolve a handle back to
// the exact live object (`rust_node_mirror_object_of`).

// The Var key scheme depends on this: a key holding a handle is sound iff
// the pinned object outlives every read-back, and injectivity holds because
// a pinned object cannot die and free its address for reuse while pinned.

thread_local! {
    static TARGET_PINS: RefCell<HashMap<u64, Py<PyAny>>> = RefCell::new(HashMap::new());
}

/// Pin `obj` under its identity handle (idempotent); returns the handle.
pub(crate) fn capture_pin(obj: &PyAny) -> PyResult<u64> {
    let handle = handle_or_error(obj)?;
    TARGET_PINS.with(|cell| cell.borrow_mut().insert(handle, Py::from(obj)));
    Ok(handle)
}

/// Resolve a pinned handle back to the live object; `None` when the handle
/// was never minted, the pins were reset (the per-build boundary), or the
/// identity registry no longer maps the pinned object to this handle
/// (#1795: an identity-only reset must not leave `object_of` answering a
/// handle `handle_of` no longer knows - the two read-backs stay coherent,
/// so a consumer can assert one from the other).
///
/// The validation is a non-minting thread-local lookup, not a PyO3
/// crossing, and the fail direction is the defer: a stale handle resolves
/// to `None`, never to a wrong object (handles are never re-issued after a
/// reset, and the pin keeps the referent alive, so a mismatch can only
/// mean the identity layer moved on). The pin is cloned out of the map so
/// the store borrow is released before the identity call: its defensive
/// mixed-pairing drop can release a `Py<PyAny>`, and the deallocator must
/// not see the pin-store borrow still active.
pub(crate) fn object_of(py: Python<'_>, handle: u64) -> Option<Py<PyAny>> {
    let pin = TARGET_PINS.with(|cell| cell.borrow().get(&handle).map(|p| p.clone_ref(py)))?;
    (identity::handle_of(pin.as_ref(py)) == Some(handle)).then_some(pin)
}

/// Drop every target pin; returns how many were held. The pins drop only
/// after the borrow is released (a release can run a Python deallocator
/// that would re-enter on the active borrow).
pub(crate) fn reset_pins() -> usize {
    let (count, pins) = TARGET_PINS.with(|cell| {
        let mut pins = cell.borrow_mut();
        let count = pins.len();
        (count, pins.drain().map(|(_, pin)| pin).collect::<Vec<_>>())
    });
    drop(pins);
    count
}

/// Number of live target pins (audit + tests).
pub(crate) fn pin_count() -> usize {
    TARGET_PINS.with(|cell| cell.borrow().len())
}

// ---- pyfunction wrappers ----

/// Capture the `RefExpr` binding scalars; returns the identity handle.
#[pyfunction]
#[pyo3(signature = (obj, kind, node_fullname, fullname, is_new_def, is_inferred_def))]
pub(crate) fn rust_node_mirror_capture_ref(
    obj: &PyAny,
    kind: Option<i64>,
    node_fullname: Option<String>,
    fullname: String,
    is_new_def: bool,
    is_inferred_def: bool,
) -> PyResult<u64> {
    capture_ref(
        obj,
        kind,
        node_fullname,
        fullname,
        is_new_def,
        is_inferred_def,
    )
}

/// Capture an `analyzed` replacement class name (record-only).
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_analyzed(
    obj: &PyAny,
    analyzed_kind: Option<String>,
) -> PyResult<u64> {
    capture_analyzed(obj, analyzed_kind)
}

/// Read the RefExpr shadow: `(kind, node_fullname, fullname, is_new_def,
/// is_inferred_def)`; None when the object has no entry.
type RefShadowOut = (Option<i64>, Option<String>, String, bool, bool);

#[pyfunction]
pub(crate) fn rust_node_mirror_ref(handle: u64) -> Option<RefShadowOut> {
    with_store(|store| {
        store.by_handle.get(&handle).map(|entry| {
            (
                entry.kind,
                entry.node_fullname.clone(),
                entry.fullname.clone(),
                entry.is_new_def,
                entry.is_inferred_def,
            )
        })
    })
}

/// Read the analyzed shadow as `(has_analyzed, kind)`; None when the
/// object has no entry. The pair keeps "never captured" distinct from
/// "captured a cleared `analyzed`" (a plain Option<Option<String>> would
/// collapse both to Python None).
#[pyfunction]
pub(crate) fn rust_node_mirror_analyzed(handle: u64) -> Option<(bool, Option<String>)> {
    with_store(|store| {
        store
            .by_handle
            .get(&handle)
            .map(|entry| (entry.has_analyzed, entry.analyzed_kind.clone()))
    })
}

/// Capture counters `(ref_captures, analyzed_captures)`; None when the
/// object has no entry.
#[pyfunction]
pub(crate) fn rust_node_mirror_captures(handle: u64) -> Option<(u64, u64)> {
    with_store(|store| {
        store
            .by_handle
            .get(&handle)
            .map(|entry| (entry.ref_captures, entry.analyzed_captures))
    })
}

/// Drop the entry (and pin) for `handle`; returns whether one existed.
#[pyfunction]
pub(crate) fn rust_node_mirror_drop(handle: u64) -> bool {
    retire(handle)
}

/// Clear all node-shadow entries and pins; returns the dropped count.
#[pyfunction]
pub(crate) fn rust_node_mirror_reset() -> usize {
    reset()
}

/// Live entry count (audit + tests).
#[pyfunction]
pub(crate) fn rust_node_mirror_entry_count() -> usize {
    entry_count()
}

/// Non-minting identity handle lookup; None when never registered.
#[pyfunction]
pub(crate) fn rust_node_mirror_handle_of(obj: &PyAny) -> Option<u64> {
    identity::handle_of(obj)
}

/// Pin a captured binding target under its identity handle (#1787);
/// returns the handle. Idempotent: a target reached by many RefExprs
/// holds one pin.
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_pin(obj: &PyAny) -> PyResult<u64> {
    capture_pin(obj)
}

/// Resolve a pinned handle back to the live object; None when the handle
/// was never minted or the per-build reset dropped its pin (#1787).
#[pyfunction]
pub(crate) fn rust_node_mirror_object_of(py: Python<'_>, handle: u64) -> Option<Py<PyAny>> {
    object_of(py, handle)
}

/// Live binding-target pin count (audit + tests).
#[pyfunction]
pub(crate) fn rust_node_mirror_pin_count() -> usize {
    pin_count()
}

// ---- G1.0b: remaining G1 expression fields (wave 71A, #1576) ----
// Record-only shape capture for the fields beyond the G1.0a RefExpr
// bindings; G1.1 extends the type-valued records with wire bytes.

/// One shadowed analysis field value. The map entry's presence is the
/// "captured at least once" marker; the variant carries the last write,
/// so a cleared slot (`method_type = None`) stays distinguishable from a
/// never-captured one (the `has_analyzed` contract).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FieldValue {
    /// Type-valued field: the value's class name, None when cleared.
    Kind(Option<String>),
    /// Bool flag.
    Flag(bool),
    /// Definition link (`MemberExpr.def_var`): the target fullname.
    Name(Option<String>),
    /// Plain string slot (`NameExpr.name`, `MemberExpr.name`); G1.2
    /// (#1674) closes the name gap the aststrip lvalue read needs.
    Text(String),
    /// Type list (`ComparisonExpr.method_types`): class per item.
    Kinds(Vec<Option<String>>),
    /// G1.1: wire bytes for a type-valued field, plus the class name
    /// for parity-checking. None when the type was cleared.
    Wire {
        kind: Option<String>,
        bytes: Vec<u8>,
    },
}

fn capture_field_value(obj: &PyAny, field: String, value: FieldValue) -> PyResult<u64> {
    let handle = handle_or_error(obj)?;
    with_store(|store| {
        let entry = store.by_handle.entry(handle).or_default();
        entry.fields.insert(field, value);
        entry.field_captures += 1;
        store.pins.insert(handle, Py::from(obj));
    });
    Ok(handle)
}

// ===========================================================================
// G2.0 statement/def metadata shadow (issue #1577)
// ===========================================================================

// Record-only metadata store for the second AST family: statement metadata
// (`AssignmentStmt` type fields, `ForStmt` index/inferred fields, ...) and
// def-family metadata (`FuncDef`, `OverloadedFuncDef`, `Decorator`, ...).

// Same gate and identity base as G1.0a; Python stays authoritative, no
// consumer reads a record, every op is a pure capture, and the store is
// separate so the two scaffolds merge independently.

// The record is a field-name keyed map of tagged scalar values, because
// the shadowed fields are heterogeneous (bools, ints, strings, type
// objects, node collections).

// Object fields are marker-only, except the node serve fields, whose record
// also carries the captured handle; pinning every object would retain a live
// graph for the whole build, which this store avoids.

// Guarantees mirror G1.0a: thread-local entries and pins, strong pins so
// a recycled `id()` cannot adopt a stale entry, and one merged record per
// object where a repeated field write replaces its value in place.

// `reset` clears entries/pins but never touches `identity::reset`
// (owned by `rust_mirror_reset`).

// Astmerge identity is untouched: `replace_object_state` copies slots
// through `setattr`, so a surviving identity re-registers through the
// normal capture hook.

/// Interned field names. Field names come from the finite Python-side
/// `_META_PATCHED` table, leaked once, so records store `&'static str`
/// keys without a per-capture key allocation.
fn intern_meta_field(field: &str) -> &'static str {
    static INTERNED: std::sync::OnceLock<std::sync::Mutex<HashMap<String, &'static str>>> =
        std::sync::OnceLock::new();
    let map = INTERNED.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    if let Some(existing) = guard.get(field) {
        return existing;
    }
    let leaked: &'static str = Box::leak(field.to_string().into_boxed_str());
    guard.insert(field.to_string(), leaked);
    leaked
}

/// One tagged scalar value. `NoneVal` keeps a captured `None` distinct
/// from "this field was never written"; `List` is a record-only marker
/// list (`Class` or `Class:fullname`) for values the Python side refuses
/// to serialize.
///
/// `Obj` is the marker (`Class` or `Class:fullname`) plus, for a
/// node-valued serve field, the identity handle of the captured object.
/// `handle: None` means the capture could not mint a pin (or the field is
/// not in the node serve set), so the record stays marker-only and can
/// never be resolved to an object; the marker is kept either way, so a
/// handle-less record is still readable and still compare-able.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum MetaValue {
    NoneVal,
    Bool(bool),
    Int(i64),
    Text(String),
    Obj { marker: String, handle: Option<u64> },
    List(Vec<String>),
}

impl MetaValue {
    /// A marker-only `Obj` record: the shape every non-node object field
    /// has, and the shape a node-valued field degrades to when the capture
    /// could not mint a pin.
    #[cfg(test)]
    pub(crate) fn obj(marker: &str) -> Self {
        MetaValue::Obj {
            marker: marker.to_string(),
            handle: None,
        }
    }
}

/// One object's metadata shadow: insertion-ordered field values plus a
/// monotonic capture counter.
#[derive(Default)]
pub(crate) struct MetaEntry {
    fields: Vec<(&'static str, MetaValue)>,
    pub(crate) captures: u64,
}

struct MetaStore {
    by_handle: HashMap<u64, MetaEntry>,
    /// Strong pins: handles key on raw `id()`s, so each stored object
    /// stays alive until its entry is dropped or the store resets.
    pins: HashMap<u64, Py<PyAny>>,
}

impl MetaStore {
    fn new() -> Self {
        MetaStore {
            by_handle: HashMap::new(),
            pins: HashMap::new(),
        }
    }
}

thread_local! {
    static META_STORE: RefCell<MetaStore> = RefCell::new(MetaStore::new());
}

fn with_meta_store<T>(f: impl FnOnce(&mut MetaStore) -> T) -> T {
    META_STORE.with(|cell| f(&mut cell.borrow_mut()))
}

/// Capture `(field, value)` for `obj`; returns the identity handle.
/// A repeated field write replaces the tagged value in place.
pub(crate) fn capture_meta(obj: &PyAny, field: &str, value: MetaValue) -> PyResult<u64> {
    let handle = handle_or_error(obj)?;
    let field = intern_meta_field(field);
    with_meta_store(|store| {
        let entry = store.by_handle.entry(handle).or_default();
        if let Some(existing) = entry.fields.iter_mut().find(|(name, _)| *name == field) {
            existing.1 = value;
        } else {
            entry.fields.push((field, value));
        }
        entry.captures += 1;
        store.pins.insert(handle, Py::from(obj));
    });
    Ok(handle)
}

/// One encoded field record as the load-time seed takes it:
/// `(field, kind, text, num, items)`, the same five values
/// `rust_node_mirror_capture_meta` takes one of.
pub(crate) type MetaFieldRecord = (
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<Vec<String>>,
);

/// G2.4 (#1825): record every tracked slot of one finished cache-loaded
/// object, in a single crossing.
///
/// The fixed-format and JSON cache readers materialize a def-family node
/// through plain attribute writes. Those writes do reach the patch, but
/// which of them become records depends on the written value: a slot the
/// reader writes at its constructor default is skipped, so a cached node's
/// coverage described the reader's write order rather than a contract.
/// The seed runs once per finished node and takes the whole field list the
/// caller derived from the live object, so the record no longer depends on
/// what the reader happened to write.
///
/// Returns `(handle, preexisting, minted, replaced)`. The handle feeds the
/// same caller-side bookkeeping `capture_meta`'s return value feeds, so a
/// seeded node is a known adopted node from the moment the seed returns.
/// `preexisting` is whether the node already had an entry when the seed
/// ran, which is the incidental side of the coverage question: true means
/// a patched write adopted this node before the seed could, so part of its
/// record came from the reader. `minted` counts the field records the seed
/// is the first writer of and `replaced` the ones a write had already
/// recorded. One crossing per node, not one per field: a def-family class
/// tracks ~30 slots.
#[pyfunction]
pub(crate) fn rust_node_mirror_seed_loaded(
    obj: &PyAny,
    fields: Vec<MetaFieldRecord>,
) -> PyResult<(u64, bool, usize, usize)> {
    // No field list means there is nothing to take: refuse before minting
    // a handle or an empty entry, so a caller that derives its field list
    // from an empty tracked set leaves the store untouched.
    if fields.is_empty() {
        return Ok((0, false, 0, 0));
    }
    // Decode before taking the store borrow: a bad kind must fail without
    // a partially seeded entry.
    let mut encoded: Vec<(&'static str, MetaValue)> = Vec::with_capacity(fields.len());
    for (field, kind, text, num, items) in fields {
        let value = meta_value_for(&kind, text, num, items)?;
        encoded.push((intern_meta_field(&field), value));
    }
    let handle = handle_or_error(obj)?;
    let (preexisting, minted, replaced) = with_meta_store(|store| {
        let preexisting = store.by_handle.contains_key(&handle);
        let entry = store.by_handle.entry(handle).or_default();
        let mut minted = 0usize;
        let mut replaced = 0usize;
        for (field, value) in encoded {
            match entry.fields.iter_mut().find(|(name, _)| *name == field) {
                Some(existing) => {
                    existing.1 = value;
                    replaced += 1;
                }
                None => {
                    entry.fields.push((field, value));
                    minted += 1;
                }
            }
            entry.captures += 1;
        }
        store.pins.insert(handle, Py::from(obj));
        (preexisting, minted, replaced)
    });
    Ok((handle, preexisting, minted, replaced))
}

/// Decode one encoded field record into a store value. Shared by the
/// single-field capture and the load-time seed, so the two cannot drift.
fn meta_value_for(
    kind: &str,
    text: Option<String>,
    num: Option<i64>,
    items: Option<Vec<String>>,
) -> PyResult<MetaValue> {
    Ok(match kind {
        "none" => MetaValue::NoneVal,
        "bool" => MetaValue::Bool(num.unwrap_or(0) != 0),
        "int" => MetaValue::Int(num.unwrap_or(0)),
        "str" => MetaValue::Text(text.unwrap_or_default()),
        "obj" => MetaValue::Obj {
            marker: text.unwrap_or_default(),
            handle: num.and_then(|n| u64::try_from(n).ok()),
        },
        "list" => MetaValue::List(items.unwrap_or_default()),
        other => {
            return Err(PyValueError::new_err(format!(
                "node_mirror: unknown meta kind {other:?}"
            )))
        }
    })
}

/// Capture a type-valued field (`method_type`, `as_type`,
/// `type_guard`, `type_is`) as the current value's class name.
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_field_kind(
    obj: &PyAny,
    field: String,
    kind: Option<String>,
) -> PyResult<u64> {
    capture_field_value(obj, field, FieldValue::Kind(kind))
}

/// G1.1: Capture a type-valued field with its wire bytes. The kind is the
/// class name (None when cleared); the bytes are the serialized Type form
/// so Rust can serve reads without crossing back to Python. When `kind`
/// is None the bytes are ignored (a cleared field has no wire form).
#[pyfunction]
#[pyo3(signature = (obj, field, kind, wire))]
pub(crate) fn rust_node_mirror_capture_field_wire(
    obj: &PyAny,
    field: String,
    kind: Option<String>,
    wire: Vec<u8>,
) -> PyResult<u64> {
    let value = match &kind {
        None => FieldValue::Wire {
            kind: None,
            bytes: vec![],
        },
        Some(_) => FieldValue::Wire { kind, bytes: wire },
    };
    capture_field_value(obj, field, value)
}

/// Capture a bool field (`right_always`, `right_unreachable`,
/// `is_special_form`, `is_alias_rvalue`).
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_flag(
    obj: &PyAny,
    field: String,
    value: bool,
) -> PyResult<u64> {
    capture_field_value(obj, field, FieldValue::Flag(value))
}

/// Capture a definition link (`MemberExpr.def_var`) by fullname.
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_field_name(
    obj: &PyAny,
    field: String,
    name: Option<String>,
) -> PyResult<u64> {
    capture_field_value(obj, field, FieldValue::Name(name))
}

/// Capture a plain string slot (`NameExpr.name`, `MemberExpr.name`).
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_field_text(
    obj: &PyAny,
    field: String,
    value: String,
) -> PyResult<u64> {
    capture_field_value(obj, field, FieldValue::Text(value))
}

/// Capture a type list (`ComparisonExpr.method_types`).
#[pyfunction]
pub(crate) fn rust_node_mirror_capture_field_kinds(
    obj: &PyAny,
    field: String,
    kinds: Vec<Option<String>>,
) -> PyResult<u64> {
    capture_field_value(obj, field, FieldValue::Kinds(kinds))
}

fn field_value_object(py: Python<'_>, value: &FieldValue) -> PyObject {
    match value {
        FieldValue::Kind(kind) => ("kind", kind.clone()).into_py(py),
        FieldValue::Flag(flag) => ("flag", *flag).into_py(py),
        FieldValue::Name(name) => ("name", name.clone()).into_py(py),
        FieldValue::Text(text) => ("text", text.clone()).into_py(py),
        FieldValue::Kinds(kinds) => ("kinds", kinds.clone()).into_py(py),
        FieldValue::Wire { kind, bytes } => ("wire", kind.clone(), bytes.clone()).into_py(py),
    }
}

/// G1.1: Read the wire bytes for a type-valued field; returns
/// `(kind, bytes)` or None when the object/field has no entry or the
/// field is not wire-captured.
#[pyfunction]
pub(crate) fn rust_node_mirror_field_wire(
    py: Python<'_>,
    handle: u64,
    field: &str,
) -> Option<(Option<String>, Py<PyBytes>)> {
    with_store(|store| {
        store.by_handle.get(&handle).and_then(|entry| {
            entry.fields.get(field).and_then(|value| match value {
                FieldValue::Wire { kind, bytes } => {
                    Some((kind.clone(), PyBytes::new(py, bytes).into()))
                }
                _ => None,
            })
        })
    })
}

/// Read one field record as a tagged tuple; None when the object or the
/// field has no entry. Tags: `("kind", str | None)`, `("flag", bool)`,
/// `("name", str | None)`, `("kinds", list[str | None])`.
#[pyfunction]
pub(crate) fn rust_node_mirror_field(py: Python<'_>, handle: u64, field: &str) -> Option<PyObject> {
    with_store(|store| {
        store.by_handle.get(&handle).and_then(|entry| {
            entry
                .fields
                .get(field)
                .map(|value| field_value_object(py, value))
        })
    })
}

/// Field names captured for `handle`, sorted; None when the object has
/// no entry.
#[pyfunction]
pub(crate) fn rust_node_mirror_fields(handle: u64) -> Option<Vec<String>> {
    with_store(|store| {
        store.by_handle.get(&handle).map(|entry| {
            let mut names: Vec<String> = entry.fields.keys().cloned().collect();
            names.sort();
            names
        })
    })
}

/// Total field write captures for `handle`; None when the object has no
/// entry.
#[pyfunction]
pub(crate) fn rust_node_mirror_field_captures(handle: u64) -> Option<u64> {
    with_store(|store| {
        store
            .by_handle
            .get(&handle)
            .map(|entry| entry.field_captures)
    })
}

// ---- G1.2 (#1674): internal accessors for shadow-served reads ----
// Native seams read a shadowed slot through these instead of crossing to
// the live object; `None` keeps the caller's live read (F2 fallback).

/// `RefExpr.is_new_def` from the record; None when the object has no
/// entry (the lazy-adoption baseline: an unrecorded node is never served).
pub(crate) fn shadow_is_new_def(handle: u64) -> Option<bool> {
    with_store(|store| store.by_handle.get(&handle).map(|e| e.is_new_def))
}

/// A `Text`-valued field (`NameExpr.name`, `MemberExpr.name`); None when
/// the object/field has no record or the field is another shape.
pub(crate) fn shadow_field_text(handle: u64, field: &str) -> Option<String> {
    with_store(|store| {
        store.by_handle.get(&handle).and_then(|entry| {
            entry.fields.get(field).and_then(|value| match value {
                FieldValue::Text(text) => Some(text.clone()),
                _ => None,
            })
        })
    })
}

// ===========================================================================
// G1.1 (#1776): first serving read channel, the `RefExpr` binding scalars
// ===========================================================================

// A read is served only when at least one ref capture ever landed
// (`ref_captures > 0`): `capture_ref` snapshots the five scalars as one
// post-write record, so another field's entry holds the `Default`.

// Mode 0 serves nothing, mode 1 serves from the record, mode 2 also runs
// the differential compare below, which is what makes a corpus run
// evidence rather than a silent pass.

// ---- shared flip-state plumbing (G1.1/G2.1/G2.2) ----
// The expression, statement and var-key serving channels each keep their
// own thread-local cell, but all three hold this one state shape.

/// One serving channel's thread-local state: a mode plus the seven
/// provenance counters every channel reports.
#[derive(Default)]
struct FlipState {
    /// 0 off (default), 1 serve, 2 serve + differential compare.
    mode: u8,
    /// Provenance. `consulted`: a lookup or translation ran. `served`: the
    /// record (or handle) answered with an exact snapshot.
    /// `deferred_off`: mode 0. `deferred_unrecorded`: no exact record, so
    /// the live read stayed in charge. `compared`/`mismatched`/
    /// `compare_errors`: the mode-2 differential.
    consulted: u64,
    served: u64,
    deferred_off: u64,
    deferred_unrecorded: u64,
    compared: u64,
    mismatched: u64,
    compare_errors: u64,
}

fn flip_mode(key: &'static LocalKey<RefCell<FlipState>>) -> u8 {
    key.with(|cell| cell.borrow().mode)
}

/// Set a channel's serving mode. Only 0, 1 and 2 exist; any other value
/// raises so the raw pyfunction cannot select a mode no read path
/// implements. `label` names the channel so each keeps its own message.
fn flip_set_mode(
    key: &'static LocalKey<RefCell<FlipState>>,
    label: &str,
    mode: u8,
) -> PyResult<u8> {
    if mode > 2 {
        return Err(PyValueError::new_err(format!(
            "{label} flip mode must be 0, 1 or 2, got {mode}"
        )));
    }
    key.with(|cell| cell.borrow_mut().mode = mode);
    Ok(flip_mode(key))
}

/// `(consulted, served, deferred_off, deferred_unrecorded, compared,
/// mismatched, compare_errors)`.
fn flip_counters(key: &'static LocalKey<RefCell<FlipState>>) -> ReadCounters {
    key.with(|cell| {
        let state = cell.borrow();
        (
            state.consulted,
            state.served,
            state.deferred_off,
            state.deferred_unrecorded,
            state.compared,
            state.mismatched,
            state.compare_errors,
        )
    })
}

/// Clear a channel's counters. The mode is deliberately kept: it is set
/// once per process (or per test) and a reset must not silently stop
/// serving.
fn flip_reset(key: &'static LocalKey<RefCell<FlipState>>) {
    key.with(|cell| {
        let mode = cell.borrow().mode;
        *cell.borrow_mut() = FlipState {
            mode,
            ..FlipState::default()
        };
    });
}

// Thread-local like the store it reads: a mode set on one thread cannot
// make another thread's empty store look authoritative, and the unit tests
// stay isolated from each other.
thread_local! {
    static READ_STATE: RefCell<FlipState> = RefCell::new(FlipState::default());
}

/// One leaf update of the read state. Never called while a read-state
/// borrow is held: `RefCell` would panic on the second borrow.
fn bump(update: impl FnOnce(&mut FlipState)) {
    READ_STATE.with(|cell| update(&mut cell.borrow_mut()));
}

/// The five `RefExpr` binding scalars of one record, served verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefScalars {
    pub(crate) kind: Option<i64>,
    pub(crate) node_fullname: Option<String>,
    pub(crate) fullname: String,
    pub(crate) is_new_def: bool,
    pub(crate) is_inferred_def: bool,
}

/// `rust_node_mirror_serve_ref`'s answer: `(kind, node_fullname, fullname,
/// is_new_def, is_inferred_def)`.
pub(crate) type ServedRef = (Option<i64>, Option<String>, String, bool, bool);

/// `rust_node_mirror_read_counters`'s answer: `(consulted, served,
/// deferred_off, deferred_unrecorded, compared, mismatched,
/// compare_errors)`.
pub(crate) type ReadCounters = (u64, u64, u64, u64, u64, u64, u64);

pub(crate) fn read_mode() -> u8 {
    flip_mode(&READ_STATE)
}

/// Set the serving mode. Only 0, 1 and 2 exist; any other value raises so
/// the raw pyfunction cannot select a mode no read path implements.
pub(crate) fn set_read_mode(mode: u8) -> PyResult<u8> {
    flip_set_mode(&READ_STATE, "node read", mode)
}

/// `(consulted, served, deferred_off, deferred_unrecorded, compared,
/// mismatched, compare_errors)`.
pub(crate) fn read_counters() -> ReadCounters {
    flip_counters(&READ_STATE)
}

/// Clear the counters. The mode is deliberately kept: it is set once per
/// process (or per test) and a reset must not silently stop serving.
pub(crate) fn reset_read_counters() {
    flip_reset(&READ_STATE);
}

/// Serve the five `RefExpr` binding scalars for `obj`, or `None` when the
/// read must stay live: mode 0, no identity handle, or an entry no ref
/// capture ever refreshed.
///
/// The store borrow is released before the mode-2 compare, which reads
/// live Python attributes and could re-enter the store.
pub(crate) fn serve_ref_scalars(obj: &PyAny) -> Option<RefScalars> {
    let served = read_ref_record(obj)?;
    if read_mode() >= 2 && !compare_ref_scalars(obj, &served) {
        bump(|state| state.mismatched += 1);
    }
    bump(|state| state.served += 1);
    Some(served)
}

/// The differential entry point: serve the record and compare it against
/// the live slots in any serving mode, counting one comparison. This is
/// the read the negative control drives, so it must be usable in mode 1
/// where the production path does not compare by itself.
pub(crate) fn verify_ref_scalars(obj: &PyAny) -> Option<bool> {
    let served = read_ref_record(obj)?;
    let matched = compare_ref_scalars(obj, &served);
    if !matched {
        bump(|state| state.mismatched += 1);
    }
    bump(|state| state.served += 1);
    Some(matched)
}

/// The record's snapshot for `obj`, or `None` when the read must stay
/// live. Counts provenance only: it neither compares nor counts a serve,
/// so both read entry points above own those counters.
fn read_ref_record(obj: &PyAny) -> Option<RefScalars> {
    if read_mode() == 0 {
        bump(|state| state.deferred_off += 1);
        return None;
    }
    bump(|state| state.consulted += 1);
    let handle = match identity::handle_of(obj) {
        Some(handle) => handle,
        None => {
            bump(|state| state.deferred_unrecorded += 1);
            return None;
        }
    };
    // The store borrow is released here, before any live attribute read.
    let served = with_store(|store| {
        store.by_handle.get(&handle).and_then(|entry| {
            if entry.ref_captures == 0 {
                return None;
            }
            Some(RefScalars {
                kind: entry.kind,
                node_fullname: entry.node_fullname.clone(),
                fullname: entry.fullname.clone(),
                is_new_def: entry.is_new_def,
                is_inferred_def: entry.is_inferred_def,
            })
        })
    });
    if served.is_none() {
        bump(|state| state.deferred_unrecorded += 1);
    }
    served
}

/// `obj.name` as `Option<i64>`; `Err` when the slot is unreadable.
fn live_opt_int(obj: &PyAny, name: &str) -> Result<Option<i64>, ()> {
    let value = obj.getattr(name).map_err(|_| ())?;
    if value.is_none() {
        return Ok(None);
    }
    value.extract::<i64>().map(Some).map_err(|_| ())
}

/// `obj.name` as a `str`; `Err` when the slot is unreadable.
fn live_str(obj: &PyAny, name: &str) -> Result<String, ()> {
    obj.getattr(name)
        .map_err(|_| ())?
        .extract::<String>()
        .map_err(|_| ())
}

/// `obj.name` as a `bool`; `Err` when the slot is unreadable.
fn live_flag(obj: &PyAny, name: &str) -> Result<bool, ()> {
    obj.getattr(name)
        .map_err(|_| ())?
        .extract::<bool>()
        .map_err(|_| ())
}

/// `obj.node.fullname` with `_capture_ref`'s own semantics: `None` when the
/// target is `None`, its `fullname` is not a `str`, or reading it raises
/// (the capture records `None` for the same states, so both sides agree).
/// An unreadable `node` slot itself stays an error: the capture would have
/// failed wholesale there, so no record exists to compare against. Only an
/// `Exception` from `fullname` reads as `None`, matching `_capture_ref`'s
/// `except Exception`; a broader error stays a compare error.
fn live_node_fullname(obj: &PyAny) -> Result<Option<String>, ()> {
    let target = obj.getattr("node").map_err(|_| ())?;
    if target.is_none() {
        return Ok(None);
    }
    let py = obj.py();
    match target.getattr("fullname") {
        Ok(fullname) => Ok(fullname.extract::<String>().ok()),
        Err(err) if err.is_instance(py, py.get_type::<PyException>()) => Ok(None),
        Err(_) => Err(()),
    }
}

/// Mode-2 differential: compare the served snapshot against the live slots
/// through the conversions the capture hook uses.
///
/// Every call is counted, and an unreadable scalar counts as a compare
/// error *and* a mismatch, so a probe that cannot read cannot report
/// success. The one exception is the target `fullname`, which the capture
/// records as `None` when it raises `Exception` (`live_node_fullname`); a
/// `BaseException` stays an error like any other unreadable scalar.
pub(crate) fn compare_ref_scalars(obj: &PyAny, served: &RefScalars) -> bool {
    bump(|state| state.compared += 1);
    let checks: [Result<bool, ()>; 5] = [
        live_opt_int(obj, "kind").map(|v| v == served.kind),
        live_str(obj, "fullname").map(|v| v == served.fullname),
        live_flag(obj, "is_new_def").map(|v| v == served.is_new_def),
        live_flag(obj, "is_inferred_def").map(|v| v == served.is_inferred_def),
        live_node_fullname(obj).map(|v| v == served.node_fullname),
    ];
    let errors = checks.iter().filter(|check| check.is_err()).count() as u64;
    if errors > 0 {
        bump(|state| state.compare_errors += errors);
    }
    checks.iter().all(|check| *check == Ok(true))
}

// ---- pyfunctions: mode, counters, and the two read entry points ----

#[pyfunction]
pub(crate) fn rust_node_mirror_set_read_mode(mode: u8) -> PyResult<u8> {
    set_read_mode(mode)
}

#[pyfunction]
pub(crate) fn rust_node_mirror_read_mode() -> u8 {
    read_mode()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_read_counters() -> ReadCounters {
    read_counters()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_read_reset() {
    reset_read_counters();
}

/// The `RefExpr` record as `(kind, node_fullname, fullname, is_new_def,
/// is_inferred_def)`; `None` when the read must stay live.
#[pyfunction]
pub(crate) fn rust_node_mirror_serve_ref(obj: &PyAny) -> Option<ServedRef> {
    serve_ref_scalars(obj).map(|s| {
        (
            s.kind,
            s.node_fullname,
            s.fullname,
            s.is_new_def,
            s.is_inferred_def,
        )
    })
}

/// The differential entry point: `Some(matched)` when the store served the
/// read, `None` when it stayed live. Counts one comparison either way.
#[pyfunction]
pub(crate) fn rust_node_mirror_verify_ref(obj: &PyAny) -> Option<bool> {
    verify_ref_scalars(obj)
}

/// Drop one metadata entry and its pin. Returns whether one was present.
///
/// The pin is moved out of the map under the borrow and dropped only
/// after the `RefCell` guard is released: releasing the last reference
/// can run a Python deallocator, and a callback re-entering the store
/// would panic on the active mutable borrow.
pub(crate) fn retire_meta(handle: u64) -> bool {
    let (present, pin) = with_meta_store(|store| {
        let present = store.by_handle.remove(&handle).is_some();
        let pin = store.pins.remove(&handle);
        (present, pin)
    });
    drop(pin);
    present
}

/// Clear every metadata entry and pin; returns how many entries dropped.
/// Deliberately does NOT call `identity::reset`: the raw handle registry
/// is owned by `rust_mirror_reset`, and node-shadow state must not
/// invalidate handles other seams still hold.
pub(crate) fn reset_meta() -> usize {
    let (entries, pins) = with_meta_store(|store| {
        let entries = store.by_handle.len();
        let pins: Vec<Py<PyAny>> = store.pins.drain().map(|(_, pin)| pin).collect();
        store.by_handle.clear();
        (entries, pins)
    });
    // Drop the pins only after the guard is released (see `retire_meta`).
    drop(pins);
    entries
}

/// Number of live metadata entries.
pub(crate) fn meta_entry_count() -> usize {
    with_meta_store(|store| store.by_handle.len())
}

/// The identity handle an `Obj` record holds for `handle`'s `field`; None
/// when the entry or field is absent, the field is another shape, or the
/// capture left it marker-only (no pin to resolve).
///
/// The store borrow is released before the caller resolves anything with
/// the handle, so a failed resolution cannot re-enter an active borrow.
pub(crate) fn meta_field_node_handle(handle: u64, field: &str) -> Option<u64> {
    with_meta_store(|store| {
        store.by_handle.get(&handle).and_then(|entry| {
            entry
                .fields
                .iter()
                .find(|(name, _)| *name == field)
                .and_then(|(_, value)| match value {
                    MetaValue::Obj {
                        handle: Some(target),
                        ..
                    } => Some(*target),
                    _ => None,
                })
        })
    })
}

// ---- G2 pyfunction wrappers ----

/// Capture one tagged field value; returns the identity handle.
///
/// `num` carries the identity handle for kind `"obj"` (the node-valued
/// serve fields) and is ignored by every other kind; `None` leaves the
/// record marker-only.
#[pyfunction]
#[pyo3(signature = (obj, field, kind, text=None, num=None, items=None))]
pub(crate) fn rust_node_mirror_capture_meta(
    obj: &PyAny,
    field: &str,
    kind: &str,
    text: Option<String>,
    num: Option<i64>,
    items: Option<Vec<String>>,
) -> PyResult<u64> {
    let value = meta_value_for(kind, text, num, items)?;
    capture_meta(obj, field, value)
}

/// Read the metadata record as `{field: (kind, text, num, items)}`; None
/// when the object has no entry. Kind is one of `none`, `bool`, `int`,
/// `str`, `obj`, `list`.
///
/// An `obj` record reports its marker in `text` and `None` in `num`; the
/// captured identity handle is reported by
/// `rust_node_mirror_meta_field_handle` instead, so the record shape every
/// existing reader sees is unchanged.
#[pyfunction]
pub(crate) fn rust_node_mirror_meta(py: Python<'_>, handle: u64) -> PyResult<Option<PyObject>> {
    with_meta_store(|store| {
        store
            .by_handle
            .get(&handle)
            .map(|entry| {
                let dict = pyo3::types::PyDict::new(py);
                for (field, value) in &entry.fields {
                    let item: (&str, Option<&str>, Option<i64>, Option<Vec<String>>) = match value {
                        MetaValue::NoneVal => ("none", None, None, None),
                        MetaValue::Bool(v) => ("bool", None, Some(i64::from(*v)), None),
                        MetaValue::Int(v) => ("int", None, Some(*v), None),
                        MetaValue::Text(v) => ("str", Some(v.as_str()), None, None),
                        MetaValue::Obj { marker, .. } => ("obj", Some(marker.as_str()), None, None),
                        MetaValue::List(v) => ("list", None, None, Some(v.clone())),
                    };
                    dict.set_item(*field, item)?;
                }
                Ok(dict.into())
            })
            .transpose()
    })
}

/// The identity handle a node-valued field's `Obj` record captured; None
/// when the object or field has no entry, the field is another shape, or
/// the capture could not mint a pin.
///
/// Read-only: it mints no handle and pins nothing, so a caller can assert
/// what the record holds without changing it.
#[pyfunction]
pub(crate) fn rust_node_mirror_meta_field_handle(handle: u64, field: &str) -> Option<u64> {
    meta_field_node_handle(handle, field)
}

/// Capture counter for `handle`; None when the object has no entry.
#[pyfunction]
pub(crate) fn rust_node_mirror_meta_captures(handle: u64) -> Option<u64> {
    with_meta_store(|store| store.by_handle.get(&handle).map(|entry| entry.captures))
}

/// Drop the metadata entry (and pin) for `handle`; whether one existed.
#[pyfunction]
pub(crate) fn rust_node_mirror_meta_drop(handle: u64) -> bool {
    retire_meta(handle)
}

/// Clear all metadata entries and pins; returns the dropped count.
#[pyfunction]
pub(crate) fn rust_node_mirror_meta_reset() -> usize {
    reset_meta()
}

/// Live metadata entry count (audit + tests).
#[pyfunction]
pub(crate) fn rust_node_mirror_meta_entry_count() -> usize {
    meta_entry_count()
}

// ===========================================================================
// G2.1 (#1787 PR B): statement-family serving read flip
// ===========================================================================

// Two record shapes share this one channel: the `Bool` record of the one
// registered statement scalar native code reads live, and a node record
// (`AssertStmt`/`ReturnStmt`/`ExpressionStmt` `expr`) served from the pin.

// A flag serves only in the exact shape the capture wrote, and a node only
// when its handle still resolves through `object_of`; anything else defers
// rather than answering wrongly (#1785 class).

// Thread-local like the G1.1 read state: a mode set on one thread cannot
// make another thread's empty store look authoritative, and the unit tests
// stay isolated from each other.
thread_local! {
    static STMT_READ_STATE: RefCell<FlipState> = RefCell::new(FlipState::default());
}

/// One leaf update of the read state. Never called while a read-state
/// borrow is held: `RefCell` would panic on the second borrow.
fn bump_stmt(update: impl FnOnce(&mut FlipState)) {
    STMT_READ_STATE.with(|cell| update(&mut cell.borrow_mut()));
}

/// `rust_node_mirror_stmt_read_counters`'s answer: `(consulted, served,
/// deferred_off, deferred_unrecorded, compared, mismatched,
/// compare_errors)`.
pub(crate) type StmtReadCounters = (u64, u64, u64, u64, u64, u64, u64);

pub(crate) fn stmt_read_mode() -> u8 {
    flip_mode(&STMT_READ_STATE)
}

/// Set the serving mode. Only 0, 1 and 2 exist; any other value raises so
/// the raw pyfunction cannot select a mode no read path implements.
pub(crate) fn set_stmt_read_mode(mode: u8) -> PyResult<u8> {
    flip_set_mode(&STMT_READ_STATE, "stmt read", mode)
}

/// `(consulted, served, deferred_off, deferred_unrecorded, compared,
/// mismatched, compare_errors)`.
pub(crate) fn stmt_read_counters() -> StmtReadCounters {
    flip_counters(&STMT_READ_STATE)
}

/// Clear the counters. The mode is deliberately kept: it is set once per
/// process (or per test) and a reset must not silently stop serving.
pub(crate) fn reset_stmt_read_counters() {
    flip_reset(&STMT_READ_STATE);
}

/// Serve `obj.field` from the metadata record, or `None` when the read
/// must stay live: mode 0, no identity handle, or no `Bool` record.
///
/// The store borrow is released before the mode-2 compare, which reads
/// live Python attributes and could re-enter the store.
pub(crate) fn serve_stmt_flag(obj: &PyAny, field: &str) -> Option<bool> {
    let served = read_stmt_flag_record(obj, field)?;
    if stmt_read_mode() >= 2 && !compare_stmt_flag(obj, field, served) {
        bump_stmt(|state| state.mismatched += 1);
    }
    bump_stmt(|state| state.served += 1);
    Some(served)
}

/// The differential entry point: serve the record and compare it against
/// the live slot in any serving mode, counting one comparison. This is
/// the read the negative control drives, so it must be usable in mode 1
/// where the production path does not compare by itself.
pub(crate) fn verify_stmt_flag(obj: &PyAny, field: &str) -> Option<bool> {
    let served = read_stmt_flag_record(obj, field)?;
    let matched = compare_stmt_flag(obj, field, served);
    if !matched {
        bump_stmt(|state| state.mismatched += 1);
    }
    bump_stmt(|state| state.served += 1);
    Some(matched)
}

/// The record's `Bool` snapshot for `obj.field`, or `None` when the read
/// must stay live. Counts provenance only: it neither compares nor counts
/// a serve, so both read entry points above own those counters.
fn read_stmt_flag_record(obj: &PyAny, field: &str) -> Option<bool> {
    if stmt_read_mode() == 0 {
        bump_stmt(|state| state.deferred_off += 1);
        return None;
    }
    bump_stmt(|state| state.consulted += 1);
    let handle = match identity::handle_of(obj) {
        Some(handle) => handle,
        None => {
            bump_stmt(|state| state.deferred_unrecorded += 1);
            return None;
        }
    };
    // The store borrow is released here, before any live attribute read.
    let served = with_meta_store(|store| {
        store
            .by_handle
            .get(&handle)
            .and_then(|entry| {
                entry
                    .fields
                    .iter()
                    .find(|(name, _)| *name == field)
                    .map(|(_, value)| value)
            })
            .and_then(|value| match value {
                MetaValue::Bool(v) => Some(*v),
                _ => None,
            })
    });
    if served.is_none() {
        bump_stmt(|state| state.deferred_unrecorded += 1);
    }
    served
}

/// Mode-2 differential: compare the served `Bool` against the live slot.
///
/// Every call is counted, and an unreadable or non-`bool` live slot counts
/// as a compare error *and* a mismatch: the capture encodes `Bool` only
/// for literal `True`/`False`, so such a slot on a served record means an
/// out-of-contract post-capture mutation, which a probe must not report
/// as success.
fn compare_stmt_flag(obj: &PyAny, field: &str, served: bool) -> bool {
    bump_stmt(|state| state.compared += 1);
    match live_flag(obj, field) {
        Ok(live) => live == served,
        Err(()) => {
            bump_stmt(|state| state.compare_errors += 1);
            false
        }
    }
}

// ---- pyfunctions: mode, counters, and the two read entry points ----

#[pyfunction]
pub(crate) fn rust_node_mirror_set_stmt_read_mode(mode: u8) -> PyResult<u8> {
    set_stmt_read_mode(mode)
}

#[pyfunction]
pub(crate) fn rust_node_mirror_stmt_read_mode() -> u8 {
    stmt_read_mode()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_stmt_read_counters() -> StmtReadCounters {
    stmt_read_counters()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_stmt_read_reset() {
    reset_stmt_read_counters();
}

/// `Some(flag)` served from the record for `obj.field`; `None` when the
/// read must stay live (mode 0, unrecorded, or a shape-crossed record).
#[pyfunction]
pub(crate) fn rust_node_mirror_serve_stmt_flag(obj: &PyAny, field: &str) -> Option<bool> {
    serve_stmt_flag(obj, field)
}

/// The differential entry point: `Some(matched)` when the record served the
/// read, `None` when it stayed live. Counts one comparison either way.
#[pyfunction]
pub(crate) fn rust_node_mirror_verify_stmt_flag(obj: &PyAny, field: &str) -> Option<bool> {
    verify_stmt_flag(obj, field)
}

// ---- node-valued fields: the same channel, a servable object ----

/// Serve `obj.field` as the live object the record pinned, or `None` when
/// the read must stay live: mode 0, no identity handle, no `Obj` record
/// for the field, a record whose capture could not mint a pin, or a handle
/// the pin layer can no longer resolve (a reset, or an identity-only reset
/// that moved on). The fail direction is always the defer, never a wrong
/// object.
///
/// The store borrow is released before the mode-2 compare, which reads
/// live Python attributes and could re-enter the store.
pub(crate) fn serve_stmt_node(py: Python<'_>, obj: &PyAny, field: &str) -> Option<Py<PyAny>> {
    let served = read_stmt_node_record(py, obj, field)?;
    if stmt_read_mode() >= 2 && !compare_stmt_node(py, obj, field, &served) {
        bump_stmt(|state| state.mismatched += 1);
    }
    bump_stmt(|state| state.served += 1);
    Some(served)
}

/// The differential entry point for a node-valued field: resolve the record
/// and compare it against the live slot in any serving mode, counting one
/// comparison. Usable in mode 1, where the production path does not compare
/// by itself, so the negative control can drive it.
pub(crate) fn verify_stmt_node(py: Python<'_>, obj: &PyAny, field: &str) -> Option<bool> {
    let served = read_stmt_node_record(py, obj, field)?;
    let matched = compare_stmt_node(py, obj, field, &served);
    if !matched {
        bump_stmt(|state| state.mismatched += 1);
    }
    bump_stmt(|state| state.served += 1);
    Some(matched)
}

/// The live object `obj.field`'s record pinned, or `None` when the read
/// must stay live. Counts provenance only: it neither compares nor counts a
/// serve, so both read entry points above own those counters.
fn read_stmt_node_record(py: Python<'_>, obj: &PyAny, field: &str) -> Option<Py<PyAny>> {
    if stmt_read_mode() == 0 {
        bump_stmt(|state| state.deferred_off += 1);
        return None;
    }
    bump_stmt(|state| state.consulted += 1);
    let handle = match identity::handle_of(obj) {
        Some(handle) => handle,
        None => {
            bump_stmt(|state| state.deferred_unrecorded += 1);
            return None;
        }
    };
    // The store borrow is released here, before the pin layer runs and
    // before any live attribute read.
    let target = meta_field_node_handle(handle, field);
    let served = target.and_then(|target| object_of(py, target));
    if served.is_none() {
        bump_stmt(|state| state.deferred_unrecorded += 1);
    }
    served
}

/// Mode-2 differential: the served object against the live slot, compared
/// by identity. `object_of` answers the exact pinned object, so a
/// value-equal imitation or a different node counts as a mismatch.
///
/// Additive-only (#1780): a live slot the probe cannot read counts as a
/// compare error and *not* as a mismatch. The record can only exist where
/// the capture read that slot successfully, so an unreadable slot is a
/// probe limitation rather than a shown disagreement; a run where reads
/// fail wholesale therefore stays visible in `compare_errors` instead of
/// reporting a clean differential. `compare_stmt_flag`'s stricter rule is
/// deliberately left as it is: a `Bool` record is exact by construction,
/// so an unreadable live flag there can only mean an out-of-contract
/// post-capture mutation.
fn compare_stmt_node(py: Python<'_>, obj: &PyAny, field: &str, served: &Py<PyAny>) -> bool {
    bump_stmt(|state| state.compared += 1);
    match obj.getattr(field) {
        Ok(live) => live.as_ptr() == served.as_ref(py).as_ptr(),
        Err(_) => {
            bump_stmt(|state| state.compare_errors += 1);
            true
        }
    }
}

/// `Some(object)` served from the record for `obj.field`; `None` when the
/// read must stay live (mode 0, unrecorded, marker-only, or an unresolvable
/// handle).
#[pyfunction]
pub(crate) fn rust_node_mirror_serve_stmt_node(
    py: Python<'_>,
    obj: &PyAny,
    field: &str,
) -> Option<Py<PyAny>> {
    serve_stmt_node(py, obj, field)
}

/// The differential entry point: `Some(matched)` when the record served the
/// read, `None` when it stayed live. Counts one comparison either way.
#[pyfunction]
pub(crate) fn rust_node_mirror_verify_stmt_node(
    py: Python<'_>,
    obj: &PyAny,
    field: &str,
) -> Option<bool> {
    verify_stmt_node(py, obj, field)
}

// ===========================================================================
// G2.2 (#1787): the `Var` binder-key handle translation
// ===========================================================================

// The narrowing key embeds the live `Var` object, and the object IS the
// identity (name-based keys are wrong under shadowing). This seam emits the
// stored identity handle in the key when the gate serves, else the live object.

// Thread-local like the other read states: a mode set on one thread cannot
// make another thread's empty pin store look authoritative.
thread_local! {
    static VAR_KEY_STATE: RefCell<FlipState> = RefCell::new(FlipState::default());
}

/// One leaf update of the var-key state. Never called while a var-key-state
/// borrow is held: `RefCell` would panic on the second borrow.
fn bump_var_key(update: impl FnOnce(&mut FlipState)) {
    VAR_KEY_STATE.with(|cell| update(&mut cell.borrow_mut()));
}

/// `rust_node_mirror_var_key_counters`'s answer: `(consulted, served,
/// deferred_off, deferred_unrecorded, compared, mismatched,
/// compare_errors)`.
pub(crate) type VarKeyCounters = (u64, u64, u64, u64, u64, u64, u64);

pub(crate) fn var_key_mode() -> u8 {
    flip_mode(&VAR_KEY_STATE)
}

/// Set the serving mode. Only 0, 1 and 2 exist; any other value raises so
/// the raw pyfunction cannot select a mode no translation path implements.
pub(crate) fn set_var_key_mode(mode: u8) -> PyResult<u8> {
    flip_set_mode(&VAR_KEY_STATE, "var key", mode)
}

/// `(consulted, served, deferred_off, deferred_unrecorded, compared,
/// mismatched, compare_errors)`.
pub(crate) fn var_key_counters() -> VarKeyCounters {
    flip_counters(&VAR_KEY_STATE)
}

/// Clear the counters. The mode is deliberately kept: it is set once per
/// process (or per test) and a reset must not silently stop translating.
pub(crate) fn reset_var_key_counters() {
    flip_reset(&VAR_KEY_STATE);
}

/// Whether the handle resolves back to this exact live object.
///
/// The fail direction is the defer: a poisoned or reset pin makes
/// `object_of` answer `None`, so a translated key can never key a lookup on
/// the wrong `Var`.
fn resolves_to(py: Python<'_>, obj: &PyAny, handle: u64) -> bool {
    object_of(py, handle).is_some_and(|resolved| resolved.as_ptr() == obj.as_ptr())
}

/// The mode-2 differential: the emitted handle against the live-key
/// computation. `resolves_to` is the whole check, because `object_of`'s
/// #1795 coherence validation makes "the handle resolves to this exact
/// object" equivalent to "the identity key the live object would hash to is
/// the emitted handle": a poisoned pin reads as a mismatch, never as a wrong
/// key. Every call counts one comparison; a failure also counts a compare
/// error, and the caller owns the mismatch counter.
fn compare_var_key(py: Python<'_>, obj: &PyAny, handle: u64) -> bool {
    bump_var_key(|state| state.compared += 1);
    let ok = resolves_to(py, obj, handle);
    if !ok {
        bump_var_key(|state| state.compare_errors += 1);
    }
    ok
}

/// Emit the key element for `obj`, or `None` when the key must stay the
/// live object: mode 0, no identity handle, or a handle that does not
/// resolve back. Mode 2 additionally counts the differential per emitted
/// key, so `compared == served` whenever no translation mismatched.
///
/// Soundness: the handle must resolve to the same live object for as long as
/// any key holds it, which the capture-time pin guarantees per build, and
/// `object_of`'s #1795 coherence check turns a poisoned pin into a defer
/// rather than a key that answers for another `Var`.
pub(crate) fn translate_var_key(py: Python<'_>, obj: &PyAny) -> Option<u64> {
    if var_key_mode() == 0 {
        bump_var_key(|state| state.deferred_off += 1);
        return None;
    }
    bump_var_key(|state| state.consulted += 1);
    let handle = match identity::handle_of(obj) {
        Some(handle) => handle,
        None => {
            bump_var_key(|state| state.deferred_unrecorded += 1);
            return None;
        }
    };
    // The emitted key is sound only if the handle resolves back to this
    // exact live object; an unresolvable handle defers to the live key. Mode
    // 2 owns the per-key differential, mode 1 keeps every read live.
    let sound = if var_key_mode() >= 2 {
        let ok = compare_var_key(py, obj, handle);
        if !ok {
            bump_var_key(|state| state.mismatched += 1);
        }
        ok
    } else {
        resolves_to(py, obj, handle)
    };
    if !sound {
        bump_var_key(|state| state.deferred_unrecorded += 1);
        return None;
    }
    bump_var_key(|state| state.served += 1);
    Some(handle)
}

/// The standalone differential for one `Var`: `Some(matched)` when the
/// identity handle resolves to this exact object, `None` when mode 0 or no
/// handle/pin exists. It emits no key, so it never moves `served`; the
/// producing path (`translate_var_key`) owns that counter, and the lane's
/// suite drives this entry directly. Counts one comparison per call.
pub(crate) fn verify_var_key(py: Python<'_>, obj: &PyAny) -> Option<bool> {
    if var_key_mode() == 0 {
        bump_var_key(|state| state.deferred_off += 1);
        return None;
    }
    bump_var_key(|state| state.consulted += 1);
    let handle = match identity::handle_of(obj) {
        Some(handle) => handle,
        None => {
            bump_var_key(|state| state.deferred_unrecorded += 1);
            return None;
        }
    };
    let matched = compare_var_key(py, obj, handle);
    if !matched {
        bump_var_key(|state| state.mismatched += 1);
    }
    Some(matched)
}

// ---- pyfunctions: mode, counters, and the two key entry points ----

#[pyfunction]
pub(crate) fn rust_node_mirror_set_var_key_mode(mode: u8) -> PyResult<u8> {
    set_var_key_mode(mode)
}

#[pyfunction]
pub(crate) fn rust_node_mirror_var_key_mode() -> u8 {
    var_key_mode()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_var_key_counters() -> VarKeyCounters {
    var_key_counters()
}

#[pyfunction]
pub(crate) fn rust_node_mirror_var_key_reset() {
    reset_var_key_counters();
}

/// The key element for `obj` as `("Var", handle)`'s second element, or
/// `None` when the key must stay the live object.
#[pyfunction]
pub(crate) fn rust_node_mirror_serve_var_key(py: Python<'_>, obj: &PyAny) -> Option<u64> {
    translate_var_key(py, obj)
}

/// The differential entry point: `Some(matched)` when the handle was
/// emitted, `None` when the key stayed live. Counts one comparison either
/// way.
#[pyfunction]
pub(crate) fn rust_node_mirror_verify_var_key(py: Python<'_>, obj: &PyAny) -> Option<bool> {
    verify_var_key(py, obj)
}

#[cfg(test)]
mod g2_meta_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    fn field_value(handle: u64, field: &str) -> Option<MetaValue> {
        with_meta_store(|store| {
            store.by_handle.get(&handle).and_then(|entry| {
                entry
                    .fields
                    .iter()
                    .find(|(name, _)| *name == field)
                    .map(|(_, value)| value.clone())
            })
        })
    }

    #[test]
    fn test_capture_meta_roundtrip_all_kinds() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = capture_meta(obj, "type", MetaValue::NoneVal).unwrap();
            assert_eq!(
                capture_meta(obj, "is_alias_def", MetaValue::Bool(true)).unwrap(),
                h
            );
            assert_eq!(
                capture_meta(obj, "abstract_status", MetaValue::Int(2)).unwrap(),
                h
            );
            assert_eq!(
                capture_meta(obj, "_fullname", MetaValue::Text("mod.f".into())).unwrap(),
                h
            );
            assert_eq!(
                capture_meta(obj, "info", MetaValue::obj("TypeInfo:mod.C")).unwrap(),
                h
            );
            assert_eq!(
                capture_meta(
                    obj,
                    "items",
                    MetaValue::List(vec!["FuncDef".into(), "Decorator".into()])
                )
                .unwrap(),
                h
            );
            assert_eq!(meta_entry_count(), 1);
            assert_eq!(field_value(h, "type"), Some(MetaValue::NoneVal));
            assert_eq!(field_value(h, "is_alias_def"), Some(MetaValue::Bool(true)));
            assert_eq!(field_value(h, "abstract_status"), Some(MetaValue::Int(2)));
            assert_eq!(
                field_value(h, "_fullname"),
                Some(MetaValue::Text("mod.f".into()))
            );
            assert_eq!(
                field_value(h, "info"),
                Some(MetaValue::obj("TypeInfo:mod.C"))
            );
            assert_eq!(
                field_value(h, "items"),
                Some(MetaValue::List(vec!["FuncDef".into(), "Decorator".into()]))
            );
            assert_eq!(with_meta_store(|s| s.by_handle[&h].captures), 6);
        });
    }

    #[test]
    fn test_repeated_write_replaces_in_place() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = capture_meta(obj, "type", MetaValue::NoneVal).unwrap();
            capture_meta(obj, "is_final_def", MetaValue::Bool(false)).unwrap();
            capture_meta(obj, "type", MetaValue::obj("InstanceType")).unwrap();
            let order: Vec<&str> = with_meta_store(|s| {
                s.by_handle[&h]
                    .fields
                    .iter()
                    .map(|(name, _)| *name)
                    .collect()
            });
            assert_eq!(order, vec!["type", "is_final_def"]);
            assert_eq!(field_value(h, "type"), Some(MetaValue::obj("InstanceType")));
            assert_eq!(with_meta_store(|s| s.by_handle[&h].captures), 3);
        });
    }

    #[test]
    fn test_drop_and_reset() {
        with_py(|py| {
            reset_meta();
            let a = fresh_object(py);
            let b = fresh_object(py);
            let ha = capture_meta(a, "type", MetaValue::NoneVal).unwrap();
            capture_meta(b, "type", MetaValue::NoneVal).unwrap();
            assert_eq!(meta_entry_count(), 2);
            assert!(retire_meta(ha));
            assert!(!retire_meta(ha));
            assert_eq!(meta_entry_count(), 1);
            assert_eq!(reset_meta(), 1);
            assert_eq!(meta_entry_count(), 0);
        });
    }

    /// One encoded field record as the Python seed sends it.
    fn record(field: &str, kind: &str, text: Option<&str>, num: Option<i64>) -> MetaFieldRecord {
        (
            field.to_string(),
            kind.to_string(),
            text.map(str::to_string),
            num,
            None,
        )
    }

    fn field_order(handle: u64) -> Vec<&'static str> {
        with_meta_store(|s| {
            s.by_handle[&handle]
                .fields
                .iter()
                .map(|(name, _)| *name)
                .collect()
        })
    }

    #[test]
    fn test_seed_loaded_takes_the_whole_field_list_in_one_entry() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = handle_or_error(obj).unwrap();
            let fields = vec![
                record("_name", "str", Some("x"), None),
                record("is_final", "bool", None, Some(1)),
                record("final_value", "none", None, None),
            ];
            assert_eq!(
                rust_node_mirror_seed_loaded(obj, fields).unwrap(),
                (h, false, 3, 0),
                "a fresh node mints every record and had no entry"
            );
            assert_eq!(meta_entry_count(), 1, "one node, one entry");
            assert_eq!(field_value(h, "_name"), Some(MetaValue::Text("x".into())));
            assert_eq!(field_value(h, "is_final"), Some(MetaValue::Bool(true)));
            assert_eq!(field_value(h, "final_value"), Some(MetaValue::NoneVal));
            assert_eq!(with_meta_store(|s| s.by_handle[&h].captures), 3);
            assert!(
                with_meta_store(|s| s.pins.contains_key(&h)),
                "the seed must pin the node its records describe"
            );
        });
    }

    #[test]
    fn test_seed_loaded_tops_up_an_entry_a_write_already_adopted() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = capture_meta(obj, "_fullname", MetaValue::Text("mod.f".into())).unwrap();
            let fields = vec![
                record("_fullname", "str", Some("mod.f"), None),
                record("is_final", "bool", None, Some(0)),
            ];
            assert_eq!(
                rust_node_mirror_seed_loaded(obj, fields).unwrap(),
                (h, true, 1, 1),
                "the entry the write adopted is reported as preexisting"
            );
            assert_eq!(meta_entry_count(), 1);
            assert_eq!(
                field_order(h),
                vec!["_fullname", "is_final"],
                "a topped-up entry appends; it does not rewrite the reader's record"
            );
            assert_eq!(field_value(h, "is_final"), Some(MetaValue::Bool(false)));
        });
    }

    #[test]
    fn test_seed_loaded_with_no_field_list_leaves_the_store_untouched() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            assert_eq!(
                rust_node_mirror_seed_loaded(obj, vec![]).unwrap(),
                (0, false, 0, 0)
            );
            assert_eq!(
                meta_entry_count(),
                0,
                "an empty field list must not mint an empty entry"
            );
            assert!(
                with_meta_store(|s| s.pins.is_empty()),
                "and must not pin the node either"
            );
        });
    }

    #[test]
    fn test_seed_loaded_refuses_an_unknown_kind_without_partial_state() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let fields = vec![
                record("_name", "str", Some("x"), None),
                record("type", "bogus", None, None),
            ];
            assert!(rust_node_mirror_seed_loaded(obj, fields).is_err());
            assert_eq!(
                meta_entry_count(),
                0,
                "a refused seed must leave no half-populated record"
            );
        });
    }

    #[test]
    fn test_seed_loaded_keeps_an_obj_records_marker_without_a_handle() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = handle_or_error(obj).unwrap();
            let fields = vec![record("type", "obj", Some("InstanceType:mod.C"), None)];
            assert_eq!(
                rust_node_mirror_seed_loaded(obj, fields).unwrap(),
                (h, false, 1, 0)
            );
            assert_eq!(
                field_value(h, "type"),
                Some(MetaValue::Obj {
                    marker: "InstanceType:mod.C".into(),
                    handle: None,
                }),
                "a marker-only field stays unservable, so its read defers"
            );
        });
    }

    #[test]
    fn test_meta_reset_does_not_reset_identity() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = capture_meta(obj, "type", MetaValue::NoneVal).unwrap();
            reset_meta();
            // `rust_mirror_reset` alone owns `identity::reset`; the G2
            // store reset must leave the raw handle registry alive.
            assert_eq!(identity::handle_of(obj), Some(h));
            assert_eq!(identity::handle_for(obj), Some(h));
        });
    }

    #[test]
    fn test_pyfunctions_answer_the_record() {
        with_py(|py| {
            reset_meta();
            let obj = fresh_object(py);
            let h = capture_meta(obj, "items", MetaValue::List(vec!["FuncDef".into()])).unwrap();
            assert_eq!(rust_node_mirror_meta_captures(h), Some(1));
            assert_eq!(rust_node_mirror_meta_captures(h + 1), None);
            assert!(rust_node_mirror_meta_drop(h));
            assert_eq!(rust_node_mirror_meta_entry_count(), 0);
        });
    }
}

#[cfg(test)]
mod node_mirror_tests {
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
    fn test_capture_ref_roundtrip() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(
                obj,
                Some(1),
                Some("mod.x".to_string()),
                "x".to_string(),
                true,
                false,
            )
            .unwrap();
            assert_eq!(entry_count(), 1);
            let (kind, node_fullname, fullname, new_def, inferred) = with_store(|s| {
                let e = s.by_handle.get(&h).unwrap();
                (
                    e.kind,
                    e.node_fullname.clone(),
                    e.fullname.clone(),
                    e.is_new_def,
                    e.is_inferred_def,
                )
            });
            assert_eq!(kind, Some(1));
            assert_eq!(node_fullname.as_deref(), Some("mod.x"));
            assert_eq!(fullname, "x");
            assert!(new_def);
            assert!(!inferred);
        });
    }

    #[test]
    fn test_capture_merges_into_one_record() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(obj, None, None, String::new(), false, false).unwrap();
            let h2 = capture_analyzed(obj, Some("CastExpr".to_string())).unwrap();
            assert_eq!(h, h2);
            assert_eq!(entry_count(), 1);
            // The analyzed capture must not clobber the ref capture.
            let (kind, node_fullname, fullname, new_def, inferred) = with_store(|s| {
                let e = s.by_handle.get(&h).unwrap();
                (
                    e.kind,
                    e.node_fullname.clone(),
                    e.fullname.clone(),
                    e.is_new_def,
                    e.is_inferred_def,
                )
            });
            assert_eq!(kind, None);
            assert_eq!(node_fullname, None);
            assert_eq!(fullname, "");
            assert!(!new_def);
            assert!(!inferred);
            let (seen, kind) = with_store(|s| {
                let e = s.by_handle.get(&h).unwrap();
                (e.has_analyzed, e.analyzed_kind.clone())
            });
            assert!(seen);
            assert_eq!(kind.as_deref(), Some("CastExpr"));
        });
    }

    #[test]
    fn test_counters_track_each_capture() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(obj, Some(2), None, "y".to_string(), false, true).unwrap();
            capture_ref(obj, Some(2), None, "y".to_string(), false, true).unwrap();
            capture_analyzed(obj, None).unwrap();
            let (refs, analyzed) = with_store(|s| {
                let e = s.by_handle.get(&h).unwrap();
                (e.ref_captures, e.analyzed_captures)
            });
            assert_eq!(refs, 2);
            assert_eq!(analyzed, 1);
        });
    }

    #[test]
    fn test_drop_removes_entry_and_pin() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(obj, None, None, String::new(), false, false).unwrap();
            assert!(retire(h));
            assert_eq!(entry_count(), 0);
            assert!(!retire(h));
        });
    }

    #[test]
    fn test_reset_clears_entries_and_pins() {
        with_py(|py| {
            reset();
            let a = fresh_object(py);
            let b = fresh_object(py);
            capture_ref(a, None, None, String::new(), false, false).unwrap();
            capture_ref(b, None, None, String::new(), false, false).unwrap();
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
            let h = capture_ref(obj, None, None, String::new(), false, false).unwrap();
            reset();
            // `rust_mirror_reset` alone owns `identity::reset`; the node
            // shadow reset must leave the raw handle registry alive.
            assert_eq!(identity::handle_of(obj), Some(h));
            assert_eq!(identity::handle_for(obj), Some(h));
        });
    }

    #[test]
    fn test_pyfunction_readers_answer_the_record() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(
                obj,
                Some(3),
                Some("m.n".to_string()),
                "n".to_string(),
                false,
                true,
            )
            .unwrap();
            assert_eq!(
                rust_node_mirror_ref(h),
                Some((
                    Some(3),
                    Some("m.n".to_string()),
                    "n".to_string(),
                    false,
                    true
                ))
            );
            assert_eq!(rust_node_mirror_analyzed(h), Some((false, None)));
            assert_eq!(rust_node_mirror_captures(h), Some((1, 0)));
            assert_eq!(rust_node_mirror_ref(h + 1), None);
            assert_eq!(rust_node_mirror_analyzed(h + 1), None);
            assert_eq!(rust_node_mirror_captures(h + 1), None);
            capture_analyzed(obj, None).unwrap();
            assert_eq!(rust_node_mirror_analyzed(h), Some((true, None)));
        });
    }

    #[test]
    fn test_capture_pin_roundtrip_resolves_identity() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_pin(obj).unwrap();
            assert_eq!(identity::handle_of(obj), Some(h));
            assert_eq!(pin_count(), 1);
            let back = object_of(py, h).unwrap();
            assert_eq!(back.as_ptr(), obj.as_ptr());
            // Idempotent: the same target keeps one pin under one handle.
            assert_eq!(capture_pin(obj).unwrap(), h);
            assert_eq!(pin_count(), 1);
        });
    }

    #[test]
    fn test_two_targets_never_share_a_handle() {
        with_py(|py| {
            reset();
            let a = fresh_object(py);
            let b = fresh_object(py);
            let ha = capture_pin(a).unwrap();
            let hb = capture_pin(b).unwrap();
            assert_ne!(ha, hb);
            assert_eq!(object_of(py, ha).unwrap().as_ptr(), a.as_ptr());
            assert_eq!(object_of(py, hb).unwrap().as_ptr(), b.as_ptr());
            assert_eq!(pin_count(), 2);
        });
    }

    #[test]
    fn test_reset_drops_target_pins_not_identity() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_pin(obj).unwrap();
            assert_eq!(reset(), 0);
            assert_eq!(pin_count(), 0);
            assert!(object_of(py, h).is_none());
            // Like the entry/pin resets, the identity registry survives so
            // other seams' handles stay valid across the build boundary.
            assert_eq!(identity::handle_of(obj), Some(h));
        });
    }

    #[test]
    fn test_object_of_defers_when_identity_forgets_the_handle() {
        with_py(|py| {
            reset();
            identity::reset(false, py);
            let obj = fresh_object(py);
            let h = capture_pin(obj).unwrap();
            assert!(object_of(py, h).is_some());
            // #1795: an identity-only reset must not leave `object_of`
            // answering a handle `handle_of` no longer knows. Handles are
            // never re-issued, so the stale pin resolves to `None`.
            identity::reset(false, py);
            assert_eq!(identity::handle_of(obj), None);
            assert!(object_of(py, h).is_none());
        });
    }

    #[test]
    fn test_object_of_unknown_handle_is_none() {
        with_py(|py| {
            reset();
            assert!(object_of(py, 0).is_none());
            assert!(object_of(py, u64::MAX).is_none());
        });
    }
}

/// G1.0b field-record tests (wave 71A, #1576), separate from the G1.0a
/// module above so parallel edits stay disjoint.
#[cfg(test)]
mod node_field_shadow_tests {
    use super::*;

    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    fn obj_field<'py>(py: Python<'py>, handle: u64, field: &str) -> Option<PyObject> {
        rust_node_mirror_field(py, handle, field)
    }

    #[test]
    fn test_capture_field_kind_roundtrip() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_field_value(
                obj,
                "method_type".to_string(),
                FieldValue::Kind(Some("CallableType".to_string())),
            )
            .unwrap();
            assert_eq!(entry_count(), 1);
            let out = obj_field(py, h, "method_type").unwrap();
            let (tag, kind) = out.extract::<(String, Option<String>)>(py).unwrap();
            assert_eq!(tag, "kind");
            assert_eq!(kind.as_deref(), Some("CallableType"));
        });
    }

    #[test]
    fn test_capture_field_kind_none_stays_present() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h =
                capture_field_value(obj, "as_type".to_string(), FieldValue::Kind(None)).unwrap();
            let (tag, kind) = obj_field(py, h, "as_type")
                .unwrap()
                .extract::<(String, Option<String>)>(py)
                .unwrap();
            assert_eq!(tag, "kind");
            assert_eq!(kind, None);
            assert_eq!(
                rust_node_mirror_fields(h),
                Some(vec!["as_type".to_string()])
            );
        });
    }

    #[test]
    fn test_capture_field_flag_and_name() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_field_value(obj, "right_always".to_string(), FieldValue::Flag(true))
                .unwrap();
            let (tag, flag) = obj_field(py, h, "right_always")
                .unwrap()
                .extract::<(String, bool)>(py)
                .unwrap();
            assert_eq!((tag.as_str(), flag), ("flag", true));
            capture_field_value(
                obj,
                "def_var".to_string(),
                FieldValue::Name(Some("mod.v".to_string())),
            )
            .unwrap();
            let (tag, name) = obj_field(py, h, "def_var")
                .unwrap()
                .extract::<(String, Option<String>)>(py)
                .unwrap();
            assert_eq!((tag.as_str(), name.as_deref()), ("name", Some("mod.v")));
        });
    }

    #[test]
    fn test_capture_field_kinds_list() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_field_value(
                obj,
                "method_types".to_string(),
                FieldValue::Kinds(vec![Some("Instance".to_string()), None]),
            )
            .unwrap();
            let (tag, kinds) = obj_field(py, h, "method_types")
                .unwrap()
                .extract::<(String, Vec<Option<String>>)>(py)
                .unwrap();
            assert_eq!(tag, "kinds");
            assert_eq!(kinds, vec![Some("Instance".to_string()), None]);
        });
    }

    #[test]
    fn test_fields_merge_with_ref_record() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_ref(obj, None, None, "x".to_string(), false, false).unwrap();
            capture_field_value(obj, "type_guard".to_string(), FieldValue::Kind(None)).unwrap();
            capture_field_value(obj, "right_always".to_string(), FieldValue::Flag(true)).unwrap();
            assert_eq!(entry_count(), 1);
            // The field captures must not clobber the ref record.
            assert_eq!(
                rust_node_mirror_ref(h),
                Some((None, None, "x".to_string(), false, false))
            );
            assert_eq!(
                rust_node_mirror_fields(h),
                Some(vec!["right_always".to_string(), "type_guard".to_string()])
            );
        });
    }

    #[test]
    fn test_field_captures_count_and_missing_handle() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = capture_field_value(obj, "method_type".to_string(), FieldValue::Kind(None))
                .unwrap();
            capture_field_value(obj, "method_type".to_string(), FieldValue::Kind(None)).unwrap();
            assert_eq!(rust_node_mirror_field_captures(h), Some(2));
            assert_eq!(rust_node_mirror_fields(h + 1), None);
            assert_eq!(rust_node_mirror_field_captures(h + 1), None);
            assert!(obj_field(py, h + 1, "method_type").is_none());
        });
    }

    #[test]
    fn test_reset_clears_fields() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            capture_field_value(obj, "method_type".to_string(), FieldValue::Kind(None)).unwrap();
            assert_eq!(reset(), 1);
            assert_eq!(entry_count(), 0);
            assert_eq!(rust_node_mirror_field_captures(1), None);
        });
    }

    #[test]
    fn test_pyfunction_capture_wrappers() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = rust_node_mirror_capture_field_kind(
                obj,
                "method_type".to_string(),
                Some("AnyType".to_string()),
            )
            .unwrap();
            assert_eq!(
                rust_node_mirror_capture_flag(obj, "right_always".to_string(), true).unwrap(),
                h
            );
            assert_eq!(
                rust_node_mirror_capture_field_name(
                    obj,
                    "def_var".to_string(),
                    Some("m.v".to_string())
                )
                .unwrap(),
                h
            );
            assert_eq!(
                rust_node_mirror_capture_field_kinds(obj, "method_types".to_string(), vec![None])
                    .unwrap(),
                h
            );
            assert_eq!(rust_node_mirror_field_captures(h), Some(4));
            assert_eq!(
                rust_node_mirror_fields(h),
                Some(vec![
                    "def_var".to_string(),
                    "method_type".to_string(),
                    "method_types".to_string(),
                    "right_always".to_string()
                ])
            );
        });
    }

    #[test]
    fn test_capture_field_text_roundtrip() {
        with_py(|py| {
            reset();
            let obj = fresh_object(py);
            let h = rust_node_mirror_capture_field_text(obj, "name".to_string(), "x".to_string())
                .unwrap();
            let (tag, text) = obj_field(py, h, "name")
                .unwrap()
                .extract::<(String, String)>(py)
                .unwrap();
            assert_eq!((tag.as_str(), text.as_str()), ("text", "x"));
            assert_eq!(shadow_field_text(h, "name").as_deref(), Some("x"));
            // A field written as another shape is not served as text.
            rust_node_mirror_capture_flag(obj, "is_special_form".to_string(), true).unwrap();
            assert_eq!(shadow_field_text(h, "is_special_form"), None);
            // `is_new_def` is the G1.0a record field (never a map entry):
            // its default is False until a binding capture writes it.
            assert_eq!(shadow_is_new_def(h), Some(false));
            rust_node_mirror_capture_ref(obj, Some(1), None, "m.x".to_string(), true, false)
                .unwrap();
            assert_eq!(shadow_is_new_def(h), Some(true));
            // An unknown handle serves nothing (the caller keeps live).
            assert_eq!(shadow_field_text(h + 1, "name"), None);
            assert_eq!(shadow_is_new_def(h + 1), None);
        });
    }
}

/// Register this module's Python-facing seam surface (#1677).
pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    // Phase G1.0a (#1572): expression dual-write node shadow. Capture-only
    // in G1 (no consumer reads an entry), keyed by the shared identity
    // handles like the type mirror.
    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_ref, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_analyzed, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_ref, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_analyzed, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_captures, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_drop, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_entry_count, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_handle_of, m)?)?;

    // Phase G2 (#1787): binding-target handle pins. The mint rides the
    // RefExpr capture path; `object_of` is the read-back a later serving
    // or Var-key consumer resolves handles through. Record-only.
    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_pin, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_object_of, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_pin_count, m)?)?;

    // Phase G1.0b (#1576): per-field records for the remaining G1
    // expression analysis fields. Capture-only, same identity handles.
    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_field_kind, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_flag, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_field_name, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_field_kinds, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_field_text, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_field, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_fields, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_field_captures, m)?)?;

    // Phase G1.1 (#1576 follow-up): wire bytes for type-valued fields so
    // Rust can serve reads without crossing back to Python.
    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_field_wire, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_field_wire, m)?)?;

    // Phase G1.1 (serving read channel): mode, provenance counters, and
    // the two `RefExpr` read entry points the deps walker and the
    // differential suite use.
    m.add_function(wrap_pyfunction!(rust_node_mirror_set_read_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_read_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_read_counters, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_read_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_serve_ref, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_verify_ref, m)?)?;

    // Phase G2.0 (#1577): statement/def metadata shadow store. Record-only
    // like G1.0a: no consumer reads an entry, same gate and identity base.
    m.add_function(wrap_pyfunction!(rust_node_mirror_capture_meta, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta_captures, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta_drop, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta_entry_count, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_meta_field_handle, m)?)?;

    // Phase G2.4 (#1825): the load-time seed of a cache-loaded def-family
    // node. One crossing per finished node instead of one per tracked slot.
    m.add_function(wrap_pyfunction!(rust_node_mirror_seed_loaded, m)?)?;

    // Phase G2.1 (#1787 PR B): statement-family serving read flip for
    // `Block.is_unreachable`. Mode 0 by default; mode 2 adds the
    // differential compare that makes a corpus run evidence.
    m.add_function(wrap_pyfunction!(rust_node_mirror_set_stmt_read_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_stmt_read_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_stmt_read_counters, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_stmt_read_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_serve_stmt_flag, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_verify_stmt_flag, m)?)?;

    // Phase G2.3 (#1787): the same statement channel over the node-valued
    // serve set (`AssertStmt`/`ReturnStmt`/`ExpressionStmt` `expr`), served
    // as the live object the capture pinned.
    m.add_function(wrap_pyfunction!(rust_node_mirror_serve_stmt_node, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_verify_stmt_node, m)?)?;

    // Phase G2.2 (#1787): the `Var` binder-key handle translation. Mode 0
    // by default; mode 2 adds the per-key differential that makes a corpus
    // run evidence and reports `deferred == 0` when the pin precondition holds.
    m.add_function(wrap_pyfunction!(rust_node_mirror_set_var_key_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_var_key_mode, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_var_key_counters, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_var_key_reset, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_serve_var_key, m)?)?;

    m.add_function(wrap_pyfunction!(rust_node_mirror_verify_var_key, m)?)?;
    Ok(())
}

#[cfg(test)]
mod g1_serving_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    /// A stand-in for a `RefExpr`: the serving read inspects the record,
    /// never the class, and the mode-2 compare reads exactly these slots.
    fn fresh_ref(py: Python<'_>) -> &PyAny {
        py.eval(
            "type('R', (), {'kind': None, 'fullname': '', 'is_new_def': False, \
             'is_inferred_def': False, 'node': None})()",
            None,
            None,
        )
        .unwrap()
    }

    fn start(mode: u8) {
        reset();
        reset_read_counters();
        set_read_mode(mode).unwrap();
    }

    fn served_counters() -> (u64, u64, u64, u64, u64, u64, u64) {
        read_counters()
    }

    #[test]
    fn test_mode_zero_serves_nothing_and_counts_the_deferral() {
        with_py(|py| {
            start(0);
            let obj = fresh_ref(py);
            capture_ref(
                obj,
                Some(2),
                Some("mod.v".into()),
                "mod.x".into(),
                true,
                false,
            )
            .unwrap();
            assert_eq!(serve_ref_scalars(obj), None);
            let (consulted, served, deferred_off, unrecorded, compared, mismatched, errors) =
                served_counters();
            assert_eq!(
                (
                    consulted,
                    served,
                    deferred_off,
                    unrecorded,
                    compared,
                    mismatched,
                    errors
                ),
                (0, 0, 1, 0, 0, 0, 0)
            );
        });
    }

    #[test]
    fn test_a_recorded_ref_is_served_exactly() {
        with_py(|py| {
            start(1);
            let obj = fresh_ref(py);
            obj.setattr("kind", 2i64).unwrap();
            obj.setattr("fullname", "mod.x").unwrap();
            obj.setattr("is_new_def", true).unwrap();
            capture_ref(
                obj,
                Some(2),
                Some("mod.v".into()),
                "mod.x".into(),
                true,
                false,
            )
            .unwrap();
            let served = serve_ref_scalars(obj).expect("a recorded ref must serve");
            assert_eq!(served.kind, Some(2));
            assert_eq!(served.node_fullname.as_deref(), Some("mod.v"));
            assert_eq!(served.fullname, "mod.x");
            assert!(served.is_new_def);
            assert!(!served.is_inferred_def);
            let (consulted, served_n, deferred_off, unrecorded, compared, mismatched, errors) =
                served_counters();
            assert_eq!(
                (
                    consulted,
                    served_n,
                    deferred_off,
                    unrecorded,
                    compared,
                    mismatched,
                    errors
                ),
                (1, 1, 0, 0, 0, 0, 0)
            );
        });
    }

    #[test]
    fn test_an_unrecorded_ref_defers() {
        with_py(|py| {
            start(1);
            let obj = fresh_ref(py);
            assert_eq!(serve_ref_scalars(obj), None);
            let (consulted, served, _, unrecorded, _, _, _) = served_counters();
            assert_eq!((consulted, served, unrecorded), (1, 0, 1));
        });
    }

    #[test]
    fn test_an_entry_without_a_ref_capture_is_not_served() {
        with_py(|py| {
            start(1);
            let obj = fresh_ref(py);
            obj.setattr("fullname", "mod.x").unwrap();
            // Another field's record mints the entry; the five scalars stay
            // `Default`, which must never be served as if recorded.
            capture_field_value(obj, "name".into(), FieldValue::Text("x".into())).unwrap();
            assert_eq!(serve_ref_scalars(obj), None);
            let (_, served, _, unrecorded, _, _, _) = served_counters();
            assert_eq!((served, unrecorded), (0, 1));
        });
    }

    #[test]
    fn test_a_later_ref_capture_refreshes_the_record() {
        with_py(|py| {
            start(1);
            let obj = fresh_ref(py);
            capture_ref(obj, None, None, "".into(), false, false).unwrap();
            assert!(!serve_ref_scalars(obj).unwrap().is_new_def);
            obj.setattr("is_new_def", true).unwrap();
            capture_ref(obj, Some(2), None, "mod.x".into(), true, false).unwrap();
            let served = serve_ref_scalars(obj).unwrap();
            assert!(
                served.is_new_def,
                "the second capture must refresh the record"
            );
            assert_eq!(served.fullname, "mod.x");
        });
    }

    #[test]
    fn test_serving_does_not_mutate_the_record() {
        with_py(|py| {
            start(1);
            let obj = fresh_ref(py);
            capture_ref(obj, None, None, "".into(), false, false).unwrap();
            for _ in 0..5 {
                assert!(serve_ref_scalars(obj).is_some());
            }
            let (captures, _) = rust_node_mirror_captures(rust_node_mirror_handle_of(obj).unwrap())
                .expect("entry present");
            assert_eq!(captures, 1, "reads must not write the record");
            assert_eq!(entry_count(), 1, "reads must not mint entries");
        });
    }

    #[test]
    fn test_mode_two_compares_and_passes_in_sync() {
        with_py(|py| {
            start(2);
            let obj = fresh_ref(py);
            obj.setattr("kind", 1i64).unwrap();
            obj.setattr("fullname", "mod.x").unwrap();
            capture_ref(obj, Some(1), None, "mod.x".into(), false, false).unwrap();
            assert_eq!(serve_ref_scalars(obj).is_some(), true);
            let (_, served, _, _, compared, mismatched, errors) = served_counters();
            assert_eq!((served, compared, mismatched, errors), (1, 1, 0, 0));
        });
    }

    #[test]
    fn test_mode_two_compare_detects_a_desync() {
        with_py(|py| {
            start(2);
            let obj = fresh_ref(py);
            obj.setattr("fullname", "mod.x").unwrap();
            capture_ref(obj, None, None, "mod.x".into(), false, false).unwrap();
            // A live write no ref capture saw: the record is now stale, and
            // the compare must say so instead of passing.
            obj.setattr("is_new_def", true).unwrap();
            assert_eq!(verify_ref_scalars(obj), Some(false));
            let (_, _, _, _, compared, mismatched, errors) = served_counters();
            assert_eq!((compared, mismatched, errors), (1, 1, 0));
            // And the serving path answers from the record, still stale.
            assert!(!serve_ref_scalars(obj).unwrap().is_new_def);
        });
    }

    #[test]
    fn test_an_unreadable_slot_counts_as_a_mismatch_not_a_skip() {
        with_py(|py| {
            start(1);
            // The class has no `kind` slot at all, so the compare cannot
            // read the live side: that must read as a failure, not a skip.
            let obj = py
                .eval(
                    "type('R_no_kind', (), {'fullname': '', 'is_new_def': False, \
                     'is_inferred_def': False, 'node': None})()",
                    None,
                    None,
                )
                .unwrap();
            capture_ref(obj, Some(1), None, "".into(), false, false).unwrap();
            assert_eq!(verify_ref_scalars(obj), Some(false));
            let (_, _, _, _, compared, mismatched, errors) = served_counters();
            assert_eq!((compared, mismatched, errors), (1, 1, 1));
        });
    }

    #[test]
    fn test_an_unreadable_target_fullname_matches_the_captured_none() {
        with_py(|py| {
            start(2);
            // `node.fullname` raises: the capture records `None` for that
            // state, so the compare must read the error as `None` and match
            // instead of inventing a desync for two agreeing sides (#1780).
            let obj = py
                .eval(
                    "type('R_raise', (), {'kind': None, 'fullname': '', 'is_new_def': False, \
                     'is_inferred_def': False, 'node': type('T', (), \
                     {'fullname': property(lambda self: 1/0)})()})()",
                    None,
                    None,
                )
                .unwrap();
            capture_ref(obj, None, None, "".into(), false, false).unwrap();
            assert_eq!(verify_ref_scalars(obj), Some(true));
            let (_, served, _, _, compared, mismatched, errors) = served_counters();
            assert_eq!((served, compared, mismatched, errors), (1, 1, 0, 0));
        });
    }

    #[test]
    fn test_a_base_exception_fullname_stays_a_compare_error() {
        with_py(|py| {
            start(2);
            // `_capture_ref` swallows only `Exception` subclasses, so a
            // broader error (KeyboardInterrupt) must not read as the `None`
            // the capture records: it stays an unreadable scalar (#1780).
            let obj = py
                .eval(
                    "type('R_interrupt', (), {'kind': None, 'fullname': '', 'is_new_def': False, \
                     'is_inferred_def': False, 'node': type('T', (), {'fullname': \
                     property(lambda self: (_ for _ in ()).throw(KeyboardInterrupt))})()})()",
                    None,
                    None,
                )
                .unwrap();
            capture_ref(obj, None, None, "".into(), false, false).unwrap();
            assert_eq!(verify_ref_scalars(obj), Some(false));
            let (_, served, _, _, compared, mismatched, errors) = served_counters();
            assert_eq!((served, compared, mismatched, errors), (1, 1, 1, 1));
        });
    }

    #[test]
    fn test_set_read_mode_refuses_a_mode_outside_the_range() {
        with_py(|_py| {
            assert!(set_read_mode(3).is_err(), "mode 3 does not exist");
            assert!(
                set_read_mode(255).is_err(),
                "u8 headroom must not widen the range"
            );
            assert_eq!(set_read_mode(0).unwrap(), 0);
            assert_eq!(set_read_mode(2).unwrap(), 2);
        });
    }

    #[test]
    fn test_verify_requires_the_serving_mode() {
        with_py(|py| {
            start(0);
            let obj = fresh_ref(py);
            capture_ref(obj, None, None, "".into(), false, false).unwrap();
            assert_eq!(verify_ref_scalars(obj), None);
            let (_, _, deferred_off, _, compared, _, _) = served_counters();
            assert_eq!((deferred_off, compared), (1, 0));
        });
    }
}

#[cfg(test)]
mod g2_serving_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    /// A stand-in for a `Block`: the serving read inspects the record,
    /// never the class, and the mode-2 compare reads exactly this slot.
    fn fresh_block(py: Python<'_>) -> &PyAny {
        py.eval("type('B', (), {'is_unreachable': False})()", None, None)
            .unwrap()
    }

    fn start_stmt(mode: u8) {
        reset_meta();
        reset_stmt_read_counters();
        set_stmt_read_mode(mode).unwrap();
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    fn stmt_counters() -> (u64, u64, u64, u64, u64, u64, u64) {
        stmt_read_counters()
    }

    #[test]
    fn test_mode_zero_serves_nothing_and_counts_the_deferral() {
        with_py(|py| {
            start_stmt(0);
            let obj = fresh_block(py);
            capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            assert_eq!(serve_stmt_flag(obj, "is_unreachable"), None);
            let (consulted, served, deferred_off, unrecorded, compared, mismatched, errors) =
                stmt_counters();
            assert_eq!(
                (
                    consulted,
                    served,
                    deferred_off,
                    unrecorded,
                    compared,
                    mismatched,
                    errors
                ),
                (0, 0, 1, 0, 0, 0, 0)
            );
        });
    }

    #[test]
    fn test_a_recorded_flag_is_served_exactly() {
        with_py(|py| {
            start_stmt(1);
            let obj = fresh_block(py);
            capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            assert_eq!(
                serve_stmt_flag(obj, "is_unreachable"),
                Some(true),
                "a recorded flag must serve"
            );
            let (consulted, served, deferred_off, unrecorded, compared, mismatched, errors) =
                stmt_counters();
            assert_eq!(
                (
                    consulted,
                    served,
                    deferred_off,
                    unrecorded,
                    compared,
                    mismatched,
                    errors
                ),
                (1, 1, 0, 0, 0, 0, 0)
            );
        });
    }

    #[test]
    fn test_an_unrecorded_flag_defers() {
        with_py(|py| {
            start_stmt(1);
            let obj = fresh_block(py);
            assert_eq!(serve_stmt_flag(obj, "is_unreachable"), None);
            let (consulted, served, _, unrecorded, _, _, _) = stmt_counters();
            assert_eq!((consulted, served, unrecorded), (1, 0, 1));
        });
    }

    #[test]
    fn test_a_shape_crossed_record_is_not_served() {
        with_py(|py| {
            start_stmt(1);
            let obj = fresh_block(py);
            // A raw pyfunction write lands an `Int` on the bool field: the
            // record exists, but its shape is not servable, so the live
            // read must stay in charge (#1785's drift class).
            rust_node_mirror_capture_meta(obj, "is_unreachable", "int", None, Some(1), None)
                .unwrap();
            assert_eq!(serve_stmt_flag(obj, "is_unreachable"), None);
            let (_, served, _, unrecorded, _, _, _) = stmt_counters();
            assert_eq!((served, unrecorded), (0, 1));
        });
    }

    #[test]
    fn test_mode_two_compares_and_passes_in_sync() {
        with_py(|py| {
            start_stmt(2);
            let obj = fresh_block(py);
            obj.setattr("is_unreachable", true).unwrap();
            capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            assert_eq!(serve_stmt_flag(obj, "is_unreachable"), Some(true));
            let (_, served, _, _, compared, mismatched, errors) = stmt_counters();
            assert_eq!((served, compared, mismatched, errors), (1, 1, 0, 0));
        });
    }

    #[test]
    fn test_mode_two_compare_detects_a_desync() {
        with_py(|py| {
            start_stmt(2);
            let obj = fresh_block(py);
            capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            // A live write no capture saw: the record is now stale, and
            // the compare must say so instead of passing.
            obj.setattr("is_unreachable", false).unwrap();
            assert_eq!(verify_stmt_flag(obj, "is_unreachable"), Some(false));
            let (_, _, _, _, compared, mismatched, errors) = stmt_counters();
            assert_eq!((compared, mismatched, errors), (1, 1, 0));
            // And the serving path answers from the record, still stale.
            assert_eq!(serve_stmt_flag(obj, "is_unreachable"), Some(true));
        });
    }

    #[test]
    fn test_an_unreadable_slot_counts_as_a_mismatch_not_a_skip() {
        with_py(|py| {
            start_stmt(1);
            // The object has no `is_unreachable` slot at all, so the
            // compare cannot read the live side: that must read as a
            // failure, not a skip.
            let obj = fresh_object(py);
            capture_meta(obj, "is_unreachable", MetaValue::Bool(false)).unwrap();
            assert_eq!(verify_stmt_flag(obj, "is_unreachable"), Some(false));
            let (_, _, _, _, compared, mismatched, errors) = stmt_counters();
            assert_eq!((compared, mismatched, errors), (1, 1, 1));
        });
    }

    #[test]
    fn test_serving_does_not_mutate_the_record() {
        with_py(|py| {
            start_stmt(1);
            let obj = fresh_block(py);
            let h = capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            for _ in 0..5 {
                assert!(serve_stmt_flag(obj, "is_unreachable").is_some());
            }
            assert_eq!(
                rust_node_mirror_meta_captures(h),
                Some(1),
                "reads must not write the record"
            );
            assert_eq!(meta_entry_count(), 1, "reads must not mint entries");
        });
    }

    #[test]
    fn test_set_stmt_read_mode_refuses_a_mode_outside_the_range() {
        with_py(|_py| {
            assert!(set_stmt_read_mode(3).is_err(), "mode 3 does not exist");
            assert!(
                set_stmt_read_mode(255).is_err(),
                "u8 headroom must not widen the range"
            );
            assert_eq!(set_stmt_read_mode(0).unwrap(), 0);
            assert_eq!(set_stmt_read_mode(2).unwrap(), 2);
        });
    }

    #[test]
    fn test_verify_requires_the_serving_mode() {
        with_py(|py| {
            start_stmt(0);
            let obj = fresh_block(py);
            capture_meta(obj, "is_unreachable", MetaValue::Bool(true)).unwrap();
            assert_eq!(verify_stmt_flag(obj, "is_unreachable"), None);
            let (_, _, deferred_off, _, compared, _, _) = stmt_counters();
            assert_eq!((deferred_off, compared), (1, 0));
        });
    }
}

#[cfg(test)]
mod stmt_node_serving_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    /// A stand-in for a served statement node: one `expr` slot, which is the
    /// only thing the read and the compare touch.
    fn fresh_stmt(py: Python<'_>) -> &PyAny {
        py.eval("type('S', (), {})()", None, None).unwrap()
    }

    /// Fresh pins, store and counters, then the serving mode.
    fn start_node(mode: u8) {
        reset_meta();
        reset_pins();
        reset_stmt_read_counters();
        set_stmt_read_mode(mode).unwrap();
    }

    /// Capture `obj.expr` the way the Python capture does for a node-valued
    /// serve field: marker plus the identity handle of the pinned value.
    fn capture_node(obj: &PyAny, value: &PyAny) -> u64 {
        let handle = capture_pin(value).unwrap();
        capture_meta(
            obj,
            "expr",
            MetaValue::Obj {
                marker: "NameExpr".into(),
                handle: Some(handle),
            },
        )
        .unwrap();
        handle
    }

    fn stmt_counters() -> (u64, u64, u64, u64, u64, u64, u64) {
        stmt_read_counters()
    }

    #[test]
    fn test_mode_zero_does_not_serve_a_node_and_counts_the_deferral() {
        with_py(|py| {
            start_node(0);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            assert!(serve_stmt_node(py, obj, "expr").is_none());
            assert_eq!(stmt_counters(), (0, 0, 1, 0, 0, 0, 0));
        });
    }

    #[test]
    fn test_a_pinned_record_serves_the_identical_object() {
        with_py(|py| {
            start_node(1);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            let served = serve_stmt_node(py, obj, "expr").expect("a pinned record must serve");
            assert_eq!(
                served.as_ref(py).as_ptr(),
                value.as_ptr(),
                "the served object must be the pinned one, not a copy"
            );
            let (consulted, served_n, deferred_off, unrecorded, compared, mismatched, errors) =
                stmt_counters();
            assert_eq!(
                (
                    consulted,
                    served_n,
                    deferred_off,
                    unrecorded,
                    compared,
                    mismatched,
                    errors
                ),
                (1, 1, 0, 0, 0, 0, 0)
            );
        });
    }

    #[test]
    fn test_a_marker_only_record_does_not_serve_a_node() {
        with_py(|py| {
            start_node(1);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            // The record exists with the right marker but no handle: the
            // record cannot name an object, so the live read must stay in
            // charge rather than the marker being trusted.
            capture_meta(obj, "expr", MetaValue::obj("NameExpr")).unwrap();
            assert!(serve_stmt_node(py, obj, "expr").is_none());
            let (_, served_n, _, unrecorded, _, _, _) = stmt_counters();
            assert_eq!((served_n, unrecorded), (0, 1));
        });
    }

    #[test]
    fn test_another_fields_handle_is_not_served_for_this_field() {
        with_py(|py| {
            start_node(1);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            let handle = capture_pin(value).unwrap();
            capture_meta(
                obj,
                "other",
                MetaValue::Obj {
                    marker: "NameExpr".into(),
                    handle: Some(handle),
                },
            )
            .unwrap();
            assert!(
                serve_stmt_node(py, obj, "expr").is_none(),
                "a handle recorded under another field must not answer this one"
            );
        });
    }

    #[test]
    fn test_a_stale_handle_defers_rather_than_answering_wrongly() {
        with_py(|py| {
            start_node(1);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            let handle = capture_node(obj, value);
            assert!(serve_stmt_node(py, obj, "expr").is_some());
            // The per-build boundary drops the pins: the handle is stale,
            // and the fail direction must be the defer, never a wrong object.
            reset_pins();
            assert!(serve_stmt_node(py, obj, "expr").is_none());
            // The record itself is untouched - it still names the handle,
            // which is exactly why the read had to defer instead of answer.
            assert_eq!(
                meta_field_node_handle(identity::handle_of(obj).unwrap(), "expr"),
                Some(handle)
            );
        });
    }

    #[test]
    fn test_the_handle_accessor_reports_what_the_record_holds() {
        with_py(|py| {
            start_node(0);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            let handle = capture_node(obj, value);
            let owner = identity::handle_of(obj).unwrap();
            assert_eq!(meta_field_node_handle(owner, "expr"), Some(handle));
            assert_eq!(meta_field_node_handle(owner, "missing"), None);
        });
    }

    #[test]
    fn test_mode_two_compare_detects_a_desynced_node() {
        with_py(|py| {
            start_node(2);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            assert_eq!(
                serve_stmt_node(py, obj, "expr")
                    .unwrap()
                    .as_ref(py)
                    .as_ptr(),
                value.as_ptr()
            );
            // A live write the capture hook never saw, so the record still
            // names the old object: the served read and the live slot must
            // disagree here.
            let other = fresh_stmt(py);
            obj.setattr("expr", other).unwrap();
            assert_eq!(
                serve_stmt_node(py, obj, "expr")
                    .unwrap()
                    .as_ref(py)
                    .as_ptr(),
                value.as_ptr()
            );
            let (_, served_n, _, _, compared, mismatched, errors) = stmt_counters();
            assert_eq!((served_n, compared, mismatched, errors), (2, 2, 1, 0));
        });
    }

    #[test]
    fn test_an_unreadable_live_slot_is_a_compare_error_not_a_mismatch() {
        with_py(|py| {
            start_node(2);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            // Additive-only (#1780): the record can only exist where the
            // capture read the slot, so an unreadable live slot is a probe
            // limitation, counted as such instead of as a disagreement.
            obj.delattr("expr").unwrap();
            assert!(serve_stmt_node(py, obj, "expr").is_some());
            let (_, _, _, _, compared, mismatched, errors) = stmt_counters();
            assert_eq!((compared, mismatched, errors), (1, 0, 1));
        });
    }

    #[test]
    fn test_verify_serves_a_node_in_mode_one() {
        with_py(|py| {
            start_node(1);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            assert_eq!(verify_stmt_node(py, obj, "expr"), Some(true));
            let other = fresh_stmt(py);
            obj.setattr("expr", other).unwrap();
            assert_eq!(verify_stmt_node(py, obj, "expr"), Some(false));
            let (_, served_n, _, _, compared, mismatched, _) = stmt_counters();
            assert_eq!((served_n, compared, mismatched), (2, 2, 1));
        });
    }

    #[test]
    fn test_verify_requires_the_serving_mode() {
        with_py(|py| {
            start_node(0);
            let obj = fresh_stmt(py);
            let value = fresh_stmt(py);
            obj.setattr("expr", value).unwrap();
            capture_node(obj, value);
            assert_eq!(verify_stmt_node(py, obj, "expr"), None);
            let (_, _, deferred_off, _, compared, _, _) = stmt_counters();
            assert_eq!((deferred_off, compared), (1, 0));
        });
    }
}

#[cfg(test)]
mod var_key_tests {
    use super::*;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn fresh_object(py: Python<'_>) -> &PyAny {
        py.eval("object()", None, None).unwrap()
    }

    /// Fresh pins and counters, then the serving mode: the var-key state is
    /// thread-local, so every case starts clean.
    fn start_var_key(mode: u8) {
        reset_pins();
        reset_var_key_counters();
        set_var_key_mode(mode).unwrap();
    }

    fn counters() -> VarKeyCounters {
        var_key_counters()
    }

    #[test]
    fn test_mode_zero_keeps_the_live_key_and_counts_the_deferral() {
        with_py(|py| {
            start_var_key(0);
            let obj = fresh_object(py);
            capture_pin(obj).unwrap();
            assert_eq!(translate_var_key(py, obj), None);
            assert_eq!(counters(), (0, 0, 1, 0, 0, 0, 0));
        });
    }

    #[test]
    fn test_a_pinned_object_serves_its_handle() {
        with_py(|py| {
            start_var_key(1);
            let obj = fresh_object(py);
            let handle = capture_pin(obj).unwrap();
            assert_eq!(translate_var_key(py, obj), Some(handle));
            assert_eq!(counters(), (1, 1, 0, 0, 0, 0, 0));
        });
    }

    #[test]
    fn test_an_unpinned_object_defers_to_the_live_key() {
        with_py(|py| {
            start_var_key(1);
            let obj = fresh_object(py);
            assert_eq!(translate_var_key(py, obj), None);
            assert_eq!(counters(), (1, 0, 0, 1, 0, 0, 0));
        });
    }

    #[test]
    fn test_an_identity_handle_without_a_pin_defers() {
        with_py(|py| {
            start_var_key(1);
            let obj = fresh_object(py);
            // Another seam gave the object an identity handle but the capture
            // never pinned it, so a keyed lookup could not resolve it back.
            assert!(identity::handle_for(obj).is_some());
            assert_eq!(translate_var_key(py, obj), None);
            assert_eq!(counters(), (1, 0, 0, 1, 0, 0, 0));
        });
    }

    #[test]
    fn test_mode_two_compares_and_passes_in_sync() {
        with_py(|py| {
            start_var_key(2);
            let obj = fresh_object(py);
            capture_pin(obj).unwrap();
            assert!(translate_var_key(py, obj).is_some());
            assert_eq!(counters(), (1, 1, 0, 0, 1, 0, 0));
        });
    }

    #[test]
    fn test_mode_two_compare_detects_a_shared_handle() {
        with_py(|py| {
            start_var_key(2);
            let a = fresh_object(py);
            let b = fresh_object(py);
            let ha = capture_pin(a).unwrap();
            let hb = capture_pin(b).unwrap();
            assert_ne!(ha, hb, "no two live objects share a handle");
            // A poisoned pin makes the emitted handle resolve elsewhere: the
            // differential must report it rather than key a lookup on `b`.
            TARGET_PINS.with(|cell| {
                cell.borrow_mut().insert(ha, Py::from(b));
            });
            assert_eq!(verify_var_key(py, a), Some(false));
            let (_, _, _, _, compared, mismatched, errors) = counters();
            assert_eq!((compared, mismatched, errors), (1, 1, 1));
            // The producing path must defer the poisoned key too, never emit
            // a handle that resolves to another object.
            assert_eq!(translate_var_key(py, a), None);
        });
    }

    #[test]
    fn test_two_objects_never_share_a_handle() {
        with_py(|py| {
            start_var_key(1);
            let a = fresh_object(py);
            let b = fresh_object(py);
            let ha = capture_pin(a).unwrap();
            let hb = capture_pin(b).unwrap();
            assert_ne!(ha, hb);
            assert_eq!(translate_var_key(py, a), Some(ha));
            assert_eq!(translate_var_key(py, b), Some(hb));
            assert_eq!(var_key_counters().1, 2, "both keys served");
        });
    }

    #[test]
    fn test_translating_neither_mints_nor_pins() {
        with_py(|py| {
            start_var_key(1);
            let obj = fresh_object(py);
            assert_eq!(pin_count(), 0);
            capture_pin(obj).unwrap();
            let pinned = pin_count();
            for _ in 0..5 {
                assert!(translate_var_key(py, obj).is_some());
            }
            assert_eq!(pin_count(), pinned, "a read must not mint a pin");
        });
    }

    #[test]
    fn test_verify_requires_the_serving_mode() {
        with_py(|py| {
            start_var_key(0);
            let obj = fresh_object(py);
            capture_pin(obj).unwrap();
            assert_eq!(verify_var_key(py, obj), None);
            let (_, _, deferred_off, _, compared, _, _) = counters();
            assert_eq!((deferred_off, compared), (1, 0));
        });
    }

    #[test]
    fn test_set_var_key_mode_refuses_a_mode_outside_the_range() {
        with_py(|_py| {
            assert!(set_var_key_mode(3).is_err(), "mode 3 does not exist");
            assert!(
                set_var_key_mode(255).is_err(),
                "u8 headroom must not widen the range"
            );
            assert_eq!(set_var_key_mode(0).unwrap(), 0);
            assert_eq!(set_var_key_mode(2).unwrap(), 2);
        });
    }
}
