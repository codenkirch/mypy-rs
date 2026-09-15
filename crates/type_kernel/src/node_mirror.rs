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

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
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
/// invalidate handles other seams still hold.
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

// Object-valued fields are stored as class/fullname markers only, so no
// live type or node graph is retained beyond the strong pin that keeps
// the record key valid.

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
/// from "this field was never written"; `Obj`/`List` are record-only
/// markers (`Class` or `Class:fullname`) for values the Python side
/// refuses to serialize.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum MetaValue {
    NoneVal,
    Bool(bool),
    Int(i64),
    Text(String),
    Obj(String),
    List(Vec<String>),
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

// ---- G2 pyfunction wrappers ----

/// Capture one tagged field value; returns the identity handle.
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
    let value = match kind {
        "none" => MetaValue::NoneVal,
        "bool" => MetaValue::Bool(num.unwrap_or(0) != 0),
        "int" => MetaValue::Int(num.unwrap_or(0)),
        "str" => MetaValue::Text(text.unwrap_or_default()),
        "obj" => MetaValue::Obj(text.unwrap_or_default()),
        "list" => MetaValue::List(items.unwrap_or_default()),
        other => {
            return Err(PyValueError::new_err(format!(
                "node_mirror: unknown meta kind {other:?}"
            )))
        }
    };
    capture_meta(obj, field, value)
}

/// Read the metadata record as `{field: (kind, text, num, items)}`; None
/// when the object has no entry. Kind is one of `none`, `bool`, `int`,
/// `str`, `obj`, `list`.
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
                        MetaValue::Obj(v) => ("obj", Some(v.as_str()), None, None),
                        MetaValue::List(v) => ("list", None, None, Some(v.clone())),
                    };
                    dict.set_item(*field, item)?;
                }
                Ok(dict.into())
            })
            .transpose()
    })
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
                capture_meta(obj, "info", MetaValue::Obj("TypeInfo:mod.C".into())).unwrap(),
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
                Some(MetaValue::Obj("TypeInfo:mod.C".into()))
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
            capture_meta(obj, "type", MetaValue::Obj("InstanceType".into())).unwrap();
            let order: Vec<&str> = with_meta_store(|s| {
                s.by_handle[&h]
                    .fields
                    .iter()
                    .map(|(name, _)| *name)
                    .collect()
            });
            assert_eq!(order, vec!["type", "is_final_def"]);
            assert_eq!(
                field_value(h, "type"),
                Some(MetaValue::Obj("InstanceType".into()))
            );
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
