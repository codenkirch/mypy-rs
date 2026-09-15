//! F reopening experiment (#1671): Rust-owned storage for one `Type`
//! family, the "replacement view" arm ADR-0004 Decision 1 declined.
//!
//! The current native default routes wire-seam reads through Python: the
//! funnel (`mypy.types._serialize_type_for_visitor`) walks the live
//! `Instance` with `getattr`, encodes it, and hands the bytes across FFI.
//! A *view* inverts that: the `Instance` field set lives in Rust, Python
//! attribute writes push it in, and the encode is emitted from the store
//! with no Python walk at all.
//!
//! Scope, deliberately one family and deliberately bounded:
//!
//! - Storage is per live object, keyed by the shared `identity::handle_for`
//!   handle (#1528), never by `id()`. The store pins the object, so a
//!   handle cannot outlive its referent.
//! - `args` are stored as child handles, not as encoded bytes. An
//!   `Instance` is served only when every argument is itself a registered,
//!   fresh, tvar-clean view, so the recursion never encodes a Python leaf.
//!   Any other argument kind (a `CallableType`, a `TupleType`, a typevar)
//!   makes the serve a miss and the caller falls back to its own encode.
//! - `Instance.type` has no writer hook in Python (a plain slot), so the
//!   stored fullname is verified against the live `TypeInfo.fullname` on
//!   every serve. A mismatch is a miss, never stale bytes.
//! - `last_known_value` and `extra_attrs` are stored as presence flags only;
//!   an `Instance` carrying either is not served.
//!
//! Soundness rule: every path that cannot prove the stored entry current
//! returns `None`. The caller's existing encode then runs unchanged, so a
//! miss costs a store lookup and nothing else.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};

use crate::identity;
use crate::wire::{
    write_int_bare, write_str, write_str_bare, write_tag, WriteBuffer, END_TAG, INSTANCE,
    INSTANCE_BOOL, INSTANCE_FUNCTION, INSTANCE_GENERIC, INSTANCE_INT, INSTANCE_OBJECT,
    INSTANCE_SIMPLE, INSTANCE_STR, LIST_GEN, LITERAL_NONE,
};

/// Recursion budget for a serve. mypy type graphs are DAGs in practice, but
/// "in practice" is not a soundness argument, so the walk is bounded rather
/// than trusting the graph to be acyclic.
const ENCODE_DEPTH_BUDGET: u32 = 64;

/// One stored `Instance` field set.
struct InstanceView {
    /// `Instance.type.fullname`, verified against the live `TypeInfo` on
    /// every serve (the field has no Python writer hook).
    fullname: String,
    /// The live argument objects, pinned. `arg_handles[i] == 0` means the
    /// argument has no view of its own; a serve then misses.
    args: Vec<Py<PyAny>>,
    arg_handles: Vec<u64>,
    /// `Instance.type_ref is None`: the instance has been fixed up. A
    /// pre-fixup instance is never served (mirrors the F3 cache rule).
    fixed_up: bool,
    /// No direct argument is `TypeVarType`/`ParamSpecType`/`TypeVarTupleType`.
    /// Nested taint is caught by the recursion, which requires every child
    /// entry to be tvar-clean as well.
    args_tvar_clean: bool,
    stamp: u64,
}

/// The field set one registration carries, grouped so the two adjacent
/// booleans and the two parallel lists cannot be transposed at a call site.
pub(crate) struct InstanceFields {
    pub(crate) fullname: String,
    pub(crate) args: Vec<Py<PyAny>>,
    pub(crate) arg_handles: Vec<u64>,
    pub(crate) fixed_up: bool,
    pub(crate) args_tvar_clean: bool,
    pub(crate) stamp: u64,
}

struct ViewStore {
    by_handle: HashMap<u64, InstanceView>,
    /// Strong pins, keyed like the entries: the handle keys are `id()`-derived
    /// at the identity layer, so a stored object must be held alive.
    pins: HashMap<u64, Py<PyAny>>,
    encodes: u64,
    defers: u64,
    read_routes: u64,
    /// Bumped by every mutation of `by_handle`. A serve captures it with the
    /// header it wrote, so a re-entrant put or touch mid-encode is a miss
    /// rather than a hybrid of two registrations.
    mutations: u64,
}

impl ViewStore {
    fn new() -> Self {
        ViewStore {
            by_handle: HashMap::new(),
            pins: HashMap::new(),
            encodes: 0,
            defers: 0,
            read_routes: 0,
            mutations: 0,
        }
    }
}

thread_local! {
    static STORE: RefCell<ViewStore> = RefCell::new(ViewStore::new());
}

fn with_store<T>(f: impl FnOnce(&mut ViewStore) -> T) -> T {
    STORE.with(|cell| f(&mut cell.borrow_mut()))
}

/// The singleton tags the Python writer emits for an argument-less
/// `Instance`, or `None` for the `INSTANCE_SIMPLE` branch (which needs the
/// bare fullname).
fn singleton_tag(fullname: &str) -> Option<u8> {
    match fullname {
        "builtins.str" => Some(INSTANCE_STR),
        "builtins.function" => Some(INSTANCE_FUNCTION),
        "builtins.int" => Some(INSTANCE_INT),
        "builtins.bool" => Some(INSTANCE_BOOL),
        "builtins.object" => Some(INSTANCE_OBJECT),
        _ => None,
    }
}

/// The live object pinned for `handle` (a registered child), or `None`.
fn pinned(py: Python<'_>, handle: u64) -> Option<Py<PyAny>> {
    with_store(|store| store.pins.get(&handle).map(|pin| pin.clone_ref(py)))
}

/// The live `TypeInfo.fullname` behind a registered object's `type` slot.
fn live_fullname(py: Python<'_>, obj: &Py<PyAny>) -> Option<String> {
    obj.as_ref(py)
        .getattr("type")
        .ok()?
        .getattr("fullname")
        .ok()?
        .extract::<String>()
        .ok()
}

/// Emit one registered `Instance` from the store, or `None` on any doubt.
fn encode_instance(
    py: Python<'_>,
    handle: u64,
    stamp: u64,
    live: &str,
    budget: u32,
) -> Option<Vec<u8>> {
    if budget == 0 {
        return None;
    }
    let mut buf = WriteBuffer::new();
    // The header is built while the store is borrowed: nothing is copied out
    // except the child list length, so a serve costs no per-entry allocation
    // and a miss costs no allocation at all.
    let (child_count, header_mutations) = with_store(|store| -> Option<(usize, u64)> {
        let entry = store.by_handle.get(&handle)?;
        if entry.stamp != stamp || !entry.fixed_up || !entry.args_tvar_clean {
            return None;
        }
        // The `type` slot has no Python writer hook, so the live fullname is
        // the only trustworthy source. A mismatch means the instance was
        // retyped in place; refuse rather than emit bytes for the old type.
        if entry.fullname != live {
            return None;
        }
        write_tag(&mut buf, INSTANCE);
        if entry.arg_handles.is_empty() {
            match singleton_tag(&entry.fullname) {
                Some(tag) => write_tag(&mut buf, tag),
                None => {
                    write_tag(&mut buf, INSTANCE_SIMPLE);
                    write_str_bare(&mut buf, &entry.fullname).ok()?;
                }
            }
            return Some((0, store.mutations));
        }
        write_tag(&mut buf, INSTANCE_GENERIC);
        write_str(&mut buf, &entry.fullname).ok()?;
        write_tag(&mut buf, LIST_GEN);
        write_int_bare(&mut buf, entry.arg_handles.len() as i64).ok()?;
        Some((entry.arg_handles.len(), store.mutations))
    })?;
    if child_count == 0 {
        return Some(buf.into_bytes());
    }
    for index in 0..child_count {
        // The header was written from one registration; any mutation since (a
        // re-entrant put or touch) means this list is not the one the header
        // describes, so miss rather than emit a hybrid encode.
        let child = with_store(|store| {
            if store.mutations != header_mutations {
                return None;
            }
            store
                .by_handle
                .get(&handle)?
                .arg_handles
                .get(index)
                .copied()
        })?;
        if child == 0 {
            return None;
        }
        let child_obj = pinned(py, child)?;
        let child_live = live_fullname(py, &child_obj)?;
        buf.extend(&encode_instance(py, child, stamp, &child_live, budget - 1)?);
    }
    // `args_tvar_clean` is required at registration and both scalar side
    // fields refuse registration, so the shorthand tags never apply here and
    // the two optional slots are always the `LITERAL_NONE` clears.
    write_tag(&mut buf, LITERAL_NONE);
    write_tag(&mut buf, LITERAL_NONE);
    write_tag(&mut buf, END_TAG);
    Some(buf.into_bytes())
}

/// Register (or re-register) the field set of `obj`. Returns the handle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn put(obj: &PyAny, fields: InstanceFields) -> PyResult<u64> {
    let InstanceFields {
        fullname,
        args,
        arg_handles,
        fixed_up,
        args_tvar_clean,
        stamp,
    } = fields;
    if args.len() != arg_handles.len() {
        return Err(PyValueError::new_err(
            "typeview: args and arg_handles must have equal length",
        ));
    }
    // FFI boundary into caller-supplied parallel lists: a handle naming a
    // different object than its argument would silently encode the wrong
    // child and still count as a hit.
    let py = obj.py();
    for (arg, &child) in args.iter().zip(arg_handles.iter()) {
        if child != 0 && identity::handle_of(arg.as_ref(py)) != Some(child) {
            return Err(PyValueError::new_err(
                "typeview: arg handle does not name the matching argument",
            ));
        }
    }
    let handle = identity::handle_for_registration(obj)
        .ok_or_else(|| PyValueError::new_err("typeview: object has no identity handle"))?;
    // A re-registration displaces the previous entry and pin; both are
    // dropped after the borrow is released (see `reset`).
    let (replaced, replaced_pin) = with_store(|store| {
        let replaced = store.by_handle.insert(
            handle,
            InstanceView {
                fullname,
                args,
                arg_handles,
                fixed_up,
                args_tvar_clean,
                stamp,
            },
        );
        let replaced_pin = store.pins.insert(handle, Py::from(obj));
        store.mutations += 1;
        (replaced, replaced_pin)
    });
    drop(replaced);
    drop(replaced_pin);
    Ok(handle)
}

/// Serve the wire bytes for `handle` when the stored field set is current.
pub(crate) fn encode(py: Python<'_>, handle: u64, stamp: u64, live: &str) -> Option<Vec<u8>> {
    let bytes = encode_instance(py, handle, stamp, live, ENCODE_DEPTH_BUDGET);
    with_store(|store| {
        if bytes.is_some() {
            store.encodes += 1;
        } else {
            store.defers += 1;
        }
    });
    bytes
}

/// The stored `args` as a live tuple (the read-route arm).
pub(crate) fn args_tuple(py: Python<'_>, handle: u64, stamp: u64) -> Option<Py<PyTuple>> {
    let items: Vec<Py<PyAny>> = with_store(|store| {
        let entry = store.by_handle.get(&handle)?;
        if entry.stamp != stamp {
            return None;
        }
        Some(entry.args.iter().map(|o| o.clone_ref(py)).collect())
    })?;
    with_store(|store| store.read_routes += 1);
    Some(PyTuple::new(py, items).into())
}

/// Invalidate one entry after a Python field write. The pin is released too:
/// the next registration re-mints it, so a stale pin cannot keep a retyped
/// instance alive under an old fullname.
pub(crate) fn touch(handle: u64) -> bool {
    let (entry, pin) = with_store(|store| {
        let entry = store.by_handle.remove(&handle);
        let pin = store.pins.remove(&handle);
        if entry.is_some() {
            store.mutations += 1;
        }
        (entry, pin)
    });
    // Both are dropped after the borrow is released: the entry owns the
    // pinned argument objects, so releasing the last reference here can run a
    // Python deallocator that re-enters the store.
    let present = entry.is_some();
    drop(entry);
    drop(pin);
    present
}

/// Clear every entry and pin, and zero the serve counters. Identity is not
/// owned here: `rust_mirror_reset` alone resets the handle registry, so view
/// state cannot invalidate handles other seams hold.
pub(crate) fn reset() -> usize {
    let (count, entries, pins) = with_store(|store| {
        let count = store.by_handle.len();
        // Drained, not cleared: each entry owns its pinned argument objects,
        // and dropping one inside the borrow can run a Python deallocator
        // that re-enters the store (same rule as the pins below).
        let entries: Vec<InstanceView> = store.by_handle.drain().map(|(_, e)| e).collect();
        let pins: Vec<Py<PyAny>> = store.pins.drain().map(|(_, pin)| pin).collect();
        store.encodes = 0;
        store.defers = 0;
        store.read_routes = 0;
        store.mutations += 1;
        (count, entries, pins)
    });
    drop(entries);
    drop(pins);
    count
}

pub(crate) fn entry_count() -> usize {
    with_store(|store| store.by_handle.len())
}

/// `(encodes, defers, read_routes)` since the last `reset`.
pub(crate) fn stats() -> (u64, u64, u64) {
    with_store(|store| (store.encodes, store.defers, store.read_routes))
}

// ---- pyfunction wrappers ----

/// Register the `Instance` field set for `obj`; returns the identity handle.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_view_put(
    obj: &PyAny,
    fullname: String,
    args: Vec<Py<PyAny>>,
    arg_handles: Vec<u64>,
    fixed_up: bool,
    args_tvar_clean: bool,
    stamp: u64,
) -> PyResult<u64> {
    put(
        obj,
        InstanceFields {
            fullname,
            args,
            arg_handles,
            fixed_up,
            args_tvar_clean,
            stamp,
        },
    )
}

/// Wire bytes for `handle`, or `None` when the stored field set cannot be
/// proven current.
#[pyfunction]
pub(crate) fn rust_view_encode(
    py: Python<'_>,
    handle: u64,
    stamp: u64,
    live_fullname: &str,
) -> Option<Py<PyBytes>> {
    encode(py, handle, stamp, live_fullname).map(|bytes| PyBytes::new(py, &bytes).into())
}

/// The stored `args` as a live tuple, or `None` when the entry is not current.
#[pyfunction]
pub(crate) fn rust_view_args(py: Python<'_>, handle: u64, stamp: u64) -> Option<Py<PyTuple>> {
    args_tuple(py, handle, stamp)
}

/// Drop the entry for `handle` (a Python field write invalidated it).
#[pyfunction]
pub(crate) fn rust_view_touch(handle: u64) -> bool {
    touch(handle)
}

/// Clear every entry and pin; returns the dropped entry count.
#[pyfunction]
pub(crate) fn rust_view_reset() -> usize {
    reset()
}

/// Live entry count (audit + tests).
#[pyfunction]
pub(crate) fn rust_view_count() -> usize {
    entry_count()
}

/// `(encodes, defers, read_routes)` since the last reset.
#[pyfunction]
pub(crate) fn rust_view_stats() -> (u64, u64, u64) {
    stats()
}

#[cfg(test)]
mod typeview_tests {
    use super::*;

    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    /// A `types.SimpleNamespace` carrying the given attributes.
    fn namespace(py: Python<'_>, attrs: &[(&str, PyObject)]) -> Py<PyAny> {
        let ns = pyo3::types::PyDict::new(py);
        for (name, value) in attrs {
            ns.set_item(name, value).unwrap();
        }
        pyo3::types::PyModule::import(py, "types")
            .unwrap()
            .getattr("SimpleNamespace")
            .unwrap()
            .call((), Some(ns))
            .unwrap()
            .into()
    }

    /// An `Instance`-shaped stub: the only live field this store reads is
    /// `type.fullname`, which a serve verifies on every call.
    fn instance(py: Python<'_>, fullname: &str) -> Py<PyAny> {
        let type_info = namespace(py, &[("fullname", fullname.into_py(py))]);
        namespace(py, &[("type", type_info)])
    }

    /// Register a field set. Named, so the adjacent booleans and the two
    /// parallel lists cannot be transposed at a call site.
    fn register_fields(
        obj: &PyAny,
        fullname: &str,
        args: Vec<Py<PyAny>>,
        arg_handles: Vec<u64>,
        fixed_up: bool,
        args_tvar_clean: bool,
        stamp: u64,
    ) -> PyResult<u64> {
        put(
            obj,
            InstanceFields {
                fullname: fullname.into(),
                args,
                arg_handles,
                fixed_up,
                args_tvar_clean,
                stamp,
            },
        )
    }

    /// `write_int_bare`'s 1-byte tier for an in-range value.
    fn short_int(value: i64) -> u8 {
        ((value + 10) << 1) as u8
    }

    #[test]
    fn test_put_and_encode_leaf() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            assert_eq!(entry_count(), 1);
            let bytes = encode(py, h, 1, "builtins.int").unwrap();
            assert_eq!(bytes, vec![INSTANCE, INSTANCE_INT]);
            assert_eq!(stats().0, 1);
        });
    }

    #[test]
    fn test_encode_simple_fullname() {
        with_py(|py| {
            reset();
            let obj = instance(py, "foo.Bar");
            let h =
                register_fields(obj.as_ref(py), "foo.Bar", vec![], vec![], true, true, 1).unwrap();
            let bytes = encode(py, h, 1, "foo.Bar").unwrap();
            assert_eq!(bytes[0], INSTANCE);
            assert_eq!(bytes[1], INSTANCE_SIMPLE);
            assert_eq!(bytes[2], short_int(7));
            assert_eq!(&bytes[3..], b"foo.Bar");
        });
    }

    #[test]
    fn test_nested_registered_child_encodes_recursively() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let child_h = register_fields(
                child.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            let parent = instance(py, "builtins.list");
            let parent_h = register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child.clone_ref(py)],
                vec![child_h],
                true,
                true,
                1,
            )
            .unwrap();
            let bytes = encode(py, parent_h, 1, "builtins.list").unwrap();
            let mut expected = vec![INSTANCE, INSTANCE_GENERIC, crate::wire::LITERAL_STR];
            expected.push(short_int(13));
            expected.extend_from_slice(b"builtins.list");
            expected.extend_from_slice(&[LIST_GEN, short_int(1), INSTANCE, INSTANCE_INT]);
            expected.extend_from_slice(&[LITERAL_NONE, LITERAL_NONE, END_TAG]);
            assert_eq!(bytes, expected);
        });
    }

    #[test]
    fn test_stale_stamp_defers() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            assert!(encode(py, h, 2, "builtins.int").is_none());
            assert_eq!(stats().1, 1);
        });
    }

    #[test]
    fn test_retyped_instance_defers() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            // The live fullname moved: the stored entry is refused.
            assert!(encode(py, h, 1, "builtins.str").is_none());
        });
    }

    #[test]
    fn test_pre_fixup_instance_defers() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                false,
                true,
                1,
            )
            .unwrap();
            assert!(encode(py, h, 1, "builtins.int").is_none());
        });
    }

    #[test]
    fn test_tvar_tainted_args_defer() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.list");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.list",
                vec![],
                vec![],
                true,
                false,
                1,
            )
            .unwrap();
            assert!(encode(py, h, 1, "builtins.list").is_none());
        });
    }

    #[test]
    fn test_unregistered_child_defers() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let parent = instance(py, "builtins.list");
            let h = register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            assert!(encode(py, h, 1, "builtins.list").is_none());
        });
    }

    #[test]
    fn test_mismatched_child_fullname_defers() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let child_h = register_fields(
                child.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            let parent = instance(py, "builtins.list");
            let h = register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child.clone_ref(py)],
                vec![child_h],
                true,
                true,
                1,
            )
            .unwrap();
            // The child was retyped in place after registration.
            let retyped = namespace(py, &[("fullname", "builtins.str".into_py(py))]);
            child.as_ref(py).setattr("type", retyped).unwrap();
            assert!(encode(py, h, 1, "builtins.list").is_none());
        });
    }

    #[test]
    fn test_touch_drops_entry() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            assert!(touch(h));
            assert_eq!(entry_count(), 0);
            assert!(!touch(h));
        });
    }

    #[test]
    fn test_args_route_returns_live_tuple() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let parent = instance(py, "builtins.list");
            let h = register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child.clone_ref(py)],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            let tup = args_tuple(py, h, 1).unwrap();
            assert_eq!(tup.as_ref(py).len(), 1);
            assert!(tup.as_ref(py).get_item(0).unwrap().is(child.as_ref(py)));
            assert_eq!(stats().2, 1);
        });
    }

    #[test]
    fn test_reset_clears_entries() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            let h = register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![],
                true,
                true,
                1,
            )
            .unwrap();
            encode(py, h, 1, "builtins.int").unwrap();
            assert_eq!(entry_count(), 1);
            assert_eq!(stats().0, 1);
            assert_eq!(reset(), 1);
            assert_eq!(entry_count(), 0);
            assert_eq!(stats(), (0, 0, 0));
        });
    }

    #[test]
    fn test_reset_releases_the_entry_argument_pins() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let child_ref: Py<PyAny> = child.clone_ref(py);
            let parent = instance(py, "builtins.list");
            register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            let during = child_ref.as_ref(py).get_refcnt();
            reset();
            // The entry was drained and dropped after the borrow, so its copy
            // of the object is gone: the refcount falls by exactly one.
            assert_eq!(child_ref.as_ref(py).get_refcnt(), during - 1);
        });
    }

    #[test]
    fn test_touch_releases_the_entry_argument_pins() {
        with_py(|py| {
            reset();
            let child = instance(py, "builtins.int");
            let child_ref: Py<PyAny> = child.clone_ref(py);
            let parent = instance(py, "builtins.list");
            let handle = register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![child],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            let during = child_ref.as_ref(py).get_refcnt();
            assert!(touch(handle));
            assert_eq!(child_ref.as_ref(py).get_refcnt(), during - 1);
        });
    }

    #[test]
    fn test_re_registration_releases_the_displaced_entry() {
        with_py(|py| {
            reset();
            let first = instance(py, "builtins.int");
            let first_ref: Py<PyAny> = first.clone_ref(py);
            let parent = instance(py, "builtins.list");
            register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![first],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            let during = first_ref.as_ref(py).get_refcnt();
            let second = instance(py, "builtins.int");
            register_fields(
                parent.as_ref(py),
                "builtins.list",
                vec![second],
                vec![0],
                true,
                true,
                1,
            )
            .unwrap();
            // The re-registration displaced and dropped the first entry's copy.
            assert_eq!(first_ref.as_ref(py).get_refcnt(), during - 1);
        });
    }

    #[test]
    fn test_length_mismatch_rejected() {
        with_py(|py| {
            reset();
            let obj = instance(py, "builtins.int");
            assert!(register_fields(
                obj.as_ref(py),
                "builtins.int",
                vec![],
                vec![1],
                true,
                true,
                1
            )
            .is_err());
        });
    }
}
