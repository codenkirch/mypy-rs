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
use crate::wire::{read_type, read_type_list, write_type, ReadBuffer, Type, WriteBuffer};

pub(crate) struct MirrorEntry {
    pub(crate) family: String,
    pub(crate) bytes: Vec<u8>,
    /// Value of the Python-side unprotected-write epoch when these bytes
    /// were last written from an authoritative sync. A funnel skip is
    /// allowed only while the current epoch equals this stamp: every
    /// write path the mirror does not capture-and-sync bumps that epoch
    /// (construction-window and serialize-window setattrs, failed
    /// capture/sync attempts), so one post-bump funnel verifies once and
    /// re-stamps.
    pub(crate) stamp: u64,
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
                stamp: 0,
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

/// Funnel skip decision (F3 slice 6, #1397): true when `handle` is
/// registered and its bytes were last written at the unprotected epoch
/// `stamp_epoch`. On true the stamp is refreshed to `stamp_epoch`
/// (idempotent; lets a post-bump verify re-arm future skips), so Python
/// needs one call per funnel assert instead of tick + check + sync.
pub(crate) fn write_skip(handle: u64, stamp_epoch: u64) -> bool {
    with_mirror(|m| match m.by_handle.get_mut(&handle) {
        Some(entry) if entry.stamp == stamp_epoch => true,
        Some(_) => false,
        None => false,
    })
}

/// Record that the stored bytes are authoritative as of `stamp_epoch`.
pub(crate) fn stamp_sync(handle: u64, stamp_epoch: u64) -> bool {
    with_mirror(|m| match m.by_handle.get_mut(&handle) {
        Some(entry) => {
            entry.stamp = stamp_epoch;
            true
        }
        None => false,
    })
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

/// Lookup the mirror parents index (cascade loop input).
#[pyfunction]
pub(crate) fn rust_mirror_parents(handle: u64) -> Vec<u64> {
    parents(handle)
}

/// Funnel skip op: see `write_skip`. Returns whether the funnel can skip
/// the fresh serialization because nothing unprotected has drifted the
/// object's subtree since its blobs last synced at `stamp_epoch`.
#[pyfunction]
pub(crate) fn rust_mirror_write_skip(handle: u64, stamp_epoch: u64) -> bool {
    write_skip(handle, stamp_epoch)
}

/// Record an authoritative sync of the stored blob at `stamp_epoch`.
#[pyfunction]
pub(crate) fn rust_mirror_stamp_sync(handle: u64, stamp_epoch: u64) -> bool {
    stamp_sync(handle, stamp_epoch)
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

/// Splice `Instance.type` into the stored blob (F3 slice 2, #1397). The wire
/// only carries the `type.fullname` string (`mypy/types.py:1941`), so the
/// op takes the new fullname and leaves args/lkv/extra_attrs untouched.
///
/// Returns the stored blob unchanged when the fullname already matches
/// (Python counts that as a noop), the new blob after `update` when it
/// differs, or `None` to defer (unregistered handle, undecodable blob,
/// or non-Instance stored family).
pub(crate) fn patch_instance_type(handle: u64, new_type_ref: &str) -> Option<Vec<u8>> {
    let old = entry_bytes(handle)?;
    let stored = {
        let mut buf = ReadBuffer::new(&old);
        read_type(&mut buf, None).ok()?
    };
    let Type::Instance {
        type_ref,
        args,
        last_known_value,
        extra_attrs,
    } = stored
    else {
        return None;
    };
    if type_ref == new_type_ref {
        return Some(old);
    }
    let patched = Type::Instance {
        type_ref: new_type_ref.to_string(),
        args,
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
#[pyo3(signature = (handle, new_type_ref))]
pub(crate) fn rust_mirror_patch_instance_type(handle: u64, new_type_ref: &str) -> Option<Vec<u8>> {
    patch_instance_type(handle, new_type_ref)
}

/// Splice `Instance.last_known_value` into the stored blob (F3 slice 2,
/// #1397). `lkv_blob` is a single serialized `Type` (`Some`) or `None`
/// (the `write_type_opt` `LITERAL_NONE` clear path, types.py:1943).
///
/// Same return protocol as `patch_instance_type`: stored blob on noop,
/// new blob on change, `None` to defer. Also defers on an undecodable
/// `lkv_blob` (a garbage value would silently corrupt the blob, so it
/// must leak to the full fresh path, which re-serializes everything).
pub(crate) fn patch_instance_lkv(handle: u64, lkv_blob: Option<&[u8]>) -> Option<Vec<u8>> {
    let old = entry_bytes(handle)?;
    let stored = {
        let mut buf = ReadBuffer::new(&old);
        read_type(&mut buf, None).ok()?
    };
    // Distinguish the legitimate `None` clear from a failed decode: a
    // garbage `lkv_blob` must defer (None), not collapse into a noop.
    let new_lkv = match lkv_blob {
        None => None,
        Some(b) => {
            let mut buf = ReadBuffer::new(b);
            match read_type(&mut buf, None) {
                Ok(t) => Some(Box::new(t)),
                Err(_) => return None,
            }
        }
    };
    let Type::Instance {
        type_ref,
        args,
        last_known_value,
        extra_attrs,
    } = stored
    else {
        return None;
    };
    if last_known_value == new_lkv {
        return Some(old);
    }
    let patched = Type::Instance {
        type_ref,
        args,
        last_known_value: new_lkv,
        extra_attrs,
    };
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, &patched).ok()?;
    let blob = wbuf.into_bytes();
    update(handle, blob.clone()).ok()?;
    Some(blob)
}

#[pyfunction]
#[pyo3(signature = (handle, lkv_blob))]
pub(crate) fn rust_mirror_patch_instance_lkv(
    handle: u64,
    lkv_blob: Option<&[u8]>,
) -> Option<Vec<u8>> {
    patch_instance_lkv(handle, lkv_blob)
}

// ---- CallableType splice ops (F3 slice 8, #1397) ----

/// The wire fields of a decoded `Type::CallableType`, grouped so the
/// per-field splice ops can read/patch/re-encode without repeating the
/// variant's 12-field pattern.
struct CallableFields {
    fallback: Box<Type>,
    instance_type: Option<Box<Type>>,
    // Wire write order: is_ellipsis_args, implicit, is_bound,
    // from_concatenate, imprecise_arg_kinds, unpack_kwargs, from_type_type.
    is_ellipsis_args: bool,
    implicit: bool,
    is_bound: bool,
    from_concatenate: bool,
    imprecise_arg_kinds: bool,
    unpack_kwargs: bool,
    from_type_type: bool,
    arg_types: Vec<Type>,
    arg_kinds: Vec<i64>,
    arg_names: Vec<Option<String>>,
    ret_type: Box<Type>,
    name: Option<String>,
    variables: Vec<Type>,
    type_guard: Option<Box<Type>>,
    type_is: Option<Box<Type>>,
}

/// Decode the stored blob for `handle` into its wire fields. `None`
/// defers (unregistered handle, undecodable blob, or a non-CallableType
/// stored family — the mirror must never retype a stored blob).
fn callable_fields(handle: u64) -> Option<CallableFields> {
    let old = entry_bytes(handle)?;
    let stored = {
        let mut buf = ReadBuffer::new(&old);
        read_type(&mut buf, None).ok()?
    };
    let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        variables,
        type_guard,
        type_is,
        special_sig: _,
    } = stored
    else {
        return None;
    };
    Some(CallableFields {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        variables,
        type_guard,
        type_is,
    })
}

/// Re-encode `cf` and store it under `handle`; returns the new blob.
/// `None` propagates encode/update failures to the full fresh path, the
/// same protocol as the Instance splice ops.
fn store_callable(handle: u64, cf: CallableFields) -> Option<Vec<u8>> {
    let patched = Type::CallableType {
        fallback: cf.fallback,
        instance_type: cf.instance_type,
        // `special_sig` is Rust-resident only; re-encode resets it to the
        // decoded default (None), matching the pre-splice blob bytes.
        special_sig: None,
        is_ellipsis_args: cf.is_ellipsis_args,
        implicit: cf.implicit,
        is_bound: cf.is_bound,
        from_concatenate: cf.from_concatenate,
        imprecise_arg_kinds: cf.imprecise_arg_kinds,
        unpack_kwargs: cf.unpack_kwargs,
        from_type_type: cf.from_type_type,
        arg_types: cf.arg_types,
        arg_kinds: cf.arg_kinds,
        arg_names: cf.arg_names,
        ret_type: cf.ret_type,
        name: cf.name,
        variables: cf.variables,
        type_guard: cf.type_guard,
        type_is: cf.type_is,
    };
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, &patched).ok()?;
    let blob = wbuf.into_bytes();
    update(handle, blob.clone()).ok()?;
    Some(blob)
}

/// Splice `CallableType.ret_type` into the stored blob. `ret_blob` is one
/// serialized `Type`. Stored blob on noop, new blob on change, `None` to
/// defer; an undecodable `ret_blob` defers too (garbage must never
/// silently corrupt the blob).
fn patch_callable_ret_type(handle: u64, ret_blob: &[u8]) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_ret = {
        let mut buf = ReadBuffer::new(ret_blob);
        match read_type(&mut buf, None) {
            Ok(t) => Box::new(t),
            Err(_) => return None,
        }
    };
    if cf.ret_type == new_ret {
        return entry_bytes(handle);
    }
    cf.ret_type = new_ret;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, ret_blob))]
pub(crate) fn rust_mirror_patch_callable_ret_type(handle: u64, ret_blob: &[u8]) -> Option<Vec<u8>> {
    patch_callable_ret_type(handle, ret_blob)
}

/// Splice `CallableType.arg_types` from `write_type_list` bytes.
fn patch_callable_arg_types(handle: u64, args_blob: &[u8]) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_args = {
        let mut buf = ReadBuffer::new(args_blob);
        match read_type_list(&mut buf) {
            Ok(ts) => ts,
            Err(_) => return None,
        }
    };
    if cf.arg_types == new_args {
        return entry_bytes(handle);
    }
    cf.arg_types = new_args;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, args_blob))]
pub(crate) fn rust_mirror_patch_callable_arg_types(
    handle: u64,
    args_blob: &[u8],
) -> Option<Vec<u8>> {
    patch_callable_arg_types(handle, args_blob)
}

/// Splice `CallableType.arg_kinds` as raw kind ints. Python passes
/// `[int(k.value) for k in self.arg_kinds]`.
fn patch_callable_arg_kinds(handle: u64, kinds: Vec<i64>) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    if cf.arg_kinds == kinds {
        return entry_bytes(handle);
    }
    cf.arg_kinds = kinds;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, kinds))]
pub(crate) fn rust_mirror_patch_callable_arg_kinds(
    handle: u64,
    kinds: Vec<i64>,
) -> Option<Vec<u8>> {
    patch_callable_arg_kinds(handle, kinds)
}

/// Splice `CallableType.arg_names` as `Option<&str>` entries (None is a
/// positional/unnamed argument).
fn patch_callable_arg_names(handle: u64, names: Vec<Option<String>>) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    if cf.arg_names == names {
        return entry_bytes(handle);
    }
    cf.arg_names = names;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, names))]
pub(crate) fn rust_mirror_patch_callable_arg_names(
    handle: u64,
    names: Vec<Option<String>>,
) -> Option<Vec<u8>> {
    patch_callable_arg_names(handle, names)
}

/// Splice `CallableType.name` (the function-ish name; `None` unnames).
fn patch_callable_name_field(handle: u64, name: Option<&str>) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_name = name.map(|s| s.to_string());
    if cf.name == new_name {
        return entry_bytes(handle);
    }
    cf.name = new_name;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, name))]
pub(crate) fn rust_mirror_patch_callable_name(handle: u64, name: Option<&str>) -> Option<Vec<u8>> {
    patch_callable_name_field(handle, name)
}

/// Splice `CallableType.variables` from `write_type_list` bytes (the
/// TypeVarLikeType entries; the wire reads them through the
/// `read_type_var_likes` list framing, so the bytes must contain the
/// type-var-like list — produced by Python's `write_type_list`-shaped
/// list serializer).
fn patch_callable_variables(handle: u64, vars_blob: &[u8]) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_vars = {
        let mut buf = ReadBuffer::new(vars_blob);
        match read_type_list(&mut buf) {
            Ok(ts) => ts,
            Err(_) => return None,
        }
    };
    if cf.variables == new_vars {
        return entry_bytes(handle);
    }
    cf.variables = new_vars;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, vars_blob))]
pub(crate) fn rust_mirror_patch_callable_variables(
    handle: u64,
    vars_blob: &[u8],
) -> Option<Vec<u8>> {
    patch_callable_variables(handle, vars_blob)
}

/// Splice `CallableType.type_guard` / `type_is` (single optional Type):
/// `None` is the write_type_opt LITERAL_NONE clear, `Some(blob)` is one
/// serialized Type. Both branches route through `which` to keep the two
/// call sites on one pyfunction each.
const OPT_FIELD_TYPE_GUARD: u32 = 0;
const OPT_FIELD_TYPE_IS: u32 = 1;
fn patch_callable_opt_field(handle: u64, which: u32, blob: Option<&[u8]>) -> Option<Vec<u8>> {
    let slot_is_guard = match which {
        OPT_FIELD_TYPE_GUARD => true,
        OPT_FIELD_TYPE_IS => false,
        _ => return None,
    };
    let mut cf = callable_fields(handle)?;
    let new_val = match blob {
        None => None,
        Some(b) => {
            let mut buf = ReadBuffer::new(b);
            match read_type(&mut buf, None) {
                Ok(t) => Some(Box::new(t)),
                Err(_) => return None,
            }
        }
    };
    let current_matches = if slot_is_guard {
        cf.type_guard.as_deref() == new_val.as_deref()
    } else {
        cf.type_is.as_deref() == new_val.as_deref()
    };
    if current_matches {
        return entry_bytes(handle);
    }
    if slot_is_guard {
        cf.type_guard = new_val;
    } else {
        cf.type_is = new_val;
    }
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, which, blob))]
pub(crate) fn rust_mirror_patch_callable_opt_field(
    handle: u64,
    which: u32,
    blob: Option<&[u8]>,
) -> Option<Vec<u8>> {
    patch_callable_opt_field(handle, which, blob)
}

/// Splice `CallableType.fallback` (a full serialized Instance type).
fn patch_callable_fallback(handle: u64, fb_blob: &[u8]) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_fb = {
        let mut buf = ReadBuffer::new(fb_blob);
        match read_type(&mut buf, None) {
            Ok(t) => Box::new(t),
            Err(_) => return None,
        }
    };
    if cf.fallback == new_fb {
        return entry_bytes(handle);
    }
    cf.fallback = new_fb;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, fb_blob))]
pub(crate) fn rust_mirror_patch_callable_fallback(handle: u64, fb_blob: &[u8]) -> Option<Vec<u8>> {
    patch_callable_fallback(handle, fb_blob)
}

/// Splice `CallableType.instance_type` (the write_type_opt LITERAL_NONE
/// clear vs one serialized Type, same protocol as `patch_instance_lkv`).
fn patch_callable_instance_type(handle: u64, blob: Option<&[u8]>) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    let new_val = match blob {
        None => None,
        Some(b) => {
            let mut buf = ReadBuffer::new(b);
            match read_type(&mut buf, None) {
                Ok(t) => Some(Box::new(t)),
                Err(_) => return None,
            }
        }
    };
    if cf.instance_type == new_val {
        return entry_bytes(handle);
    }
    cf.instance_type = new_val;
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, blob))]
pub(crate) fn rust_mirror_patch_callable_instance_type(
    handle: u64,
    blob: Option<&[u8]>,
) -> Option<Vec<u8>> {
    patch_callable_instance_type(handle, blob)
}

/// Splice the seven wire bools. `flags` must be seven long, wire write
/// order: is_ellipsis_args, implicit, is_bound, from_concatenate,
/// imprecise_arg_kinds, unpack_kwargs, from_type_type.
fn patch_callable_flags(handle: u64, flags: Vec<bool>) -> Option<Vec<u8>> {
    let mut cf = callable_fields(handle)?;
    if flags.len() != 7 {
        return None;
    }
    let cur = [
        cf.is_ellipsis_args,
        cf.implicit,
        cf.is_bound,
        cf.from_concatenate,
        cf.imprecise_arg_kinds,
        cf.unpack_kwargs,
        cf.from_type_type,
    ];
    if cur == flags.as_slice() {
        return entry_bytes(handle);
    }
    cf.is_ellipsis_args = flags[0];
    cf.implicit = flags[1];
    cf.is_bound = flags[2];
    cf.from_concatenate = flags[3];
    cf.imprecise_arg_kinds = flags[4];
    cf.unpack_kwargs = flags[5];
    cf.from_type_type = flags[6];
    store_callable(handle, cf)
}

#[pyfunction]
#[pyo3(signature = (handle, flags))]
pub(crate) fn rust_mirror_patch_callable_flags(handle: u64, flags: Vec<bool>) -> Option<Vec<u8>> {
    patch_callable_flags(handle, flags)
}

// ---- reverse-index collection walk (Slice 7) ----
// Port of the fused `_walk_indices` traversal from mypy/types_mirror.py: one
// descent over the live graph collecting tvids, TypeAlias nodes, family Types.

// Python keeps the apply steps (`_apply_tvid_carriers` / `_apply_alias_carriers`
// / `_apply_hidden_embeds`), which own the real bookkeeping; only the slot-scan
// tree walk moves here. The dispatch order mirrors Python exactly:

// scalar -> TypeVarId -> Type -> "TypeAlias" by type name -> list/tuple ->
// dict -> ExtraAttrs, with a per-class memoized slot-name list (the analogue
// of `_type_names`). None defers: unreadable shape, undecodable slots,

// unknown module, or walk depth past the recursion cap (Python's recursion
// limit then governs, raising or succeeding exactly as before).

use std::collections::HashSet;
use std::rc::Rc;

use pyo3::exceptions::PyAttributeError;
use pyo3::types::{PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple, PyType};
use pyo3::PyDowncastError;

/// Slots that never hold a Type child (positions; _-prefixed slot names
/// are skipped by prefix in `collect_slot_names`).
const WALK_SKIP_SLOTS: [&str; 4] = ["line", "column", "end_line", "end_column"];

/// Depth cap for the recursive walk. Each walked edge costs the pure-Python
/// fallback roughly one `_walk_value` frame (two through a Type-node hop),
/// so a default 1000-frame limit raises `RecursionError` by ~500 edges. The
/// cap sits below that horizon: past the cap we defer and the fallback
/// raises or succeeds exactly as the pre-port Python did.
const WALK_DEPTH_CAP: usize = 400;

enum WalkErr {
    /// Any failure the pure-Python body will reproduce identically. The
    /// payload is never inspected (both defers become `None`).
    Defer,
    /// Depth cap reached: defer so Python's recursion limit governs.
    Depth,
}

impl From<PyErr> for WalkErr {
    fn from(_: PyErr) -> Self {
        WalkErr::Defer
    }
}

impl From<PyDowncastError<'_>> for WalkErr {
    fn from(_: PyDowncastError<'_>) -> Self {
        WalkErr::Defer
    }
}

struct WalkCtx {
    /// `mypy.types.Type` base class (`isinstance(value, Type)` check).
    type_cls: Py<PyAny>,
    /// `mypy.types.TypeVarId` class (checked before Type, like Python).
    tvid_cls: Py<PyAny>,
    /// The four family class objects (`type(value) in FAMILY_NAME`).
    family: Vec<Py<PyAny>>,
    /// Per-class usable slot names (`_SLOT_NAMES` minus skipped names).
    slot_cache: HashMap<usize, Rc<Vec<String>>>,
}

/// Read one slot; `Ok(None)` is an unset slot descriptor (the AttributeError
/// pass), any other error propagates as a defer.
/// Slot reads use the full builtin attribute lookup, while Python's
/// `_walk_slots` uses `object.__getattribute__` (no `__getattr__`
/// fallback). Parity holds because the mypy family/alias/ExtraAttrs
/// classes define no attribute hooks; revisit if one ever does.
fn read_slot(obj: &PyAny, name: &str, py: Python) -> Result<Option<PyObject>, WalkErr> {
    match obj.getattr(name) {
        Ok(v) => Ok(Some(v.to_object(py))),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(_) => Err(WalkErr::Defer),
    }
}

/// Collect a class's MRO __slots__ names (the `_type_names` fold),
/// deduplicated in order, filtered to the names the walk reads.
fn collect_slot_names(ty: &PyType, py: Python) -> Result<Vec<String>, WalkErr> {
    let mro = ty.getattr("__mro__").map_err(WalkErr::from)?;
    let mut collected: Vec<String> = Vec::new();
    for klass in mro.iter().map_err(WalkErr::from)? {
        let klass = klass.map_err(WalkErr::from)?;
        let slots = match klass.getattr("__slots__") {
            Ok(s) => s,
            Err(e) if e.is_instance_of::<PyAttributeError>(py) => continue,
            Err(_) => return Err(WalkErr::Defer),
        };
        if !slots.is_true().map_err(WalkErr::from)? {
            continue;
        }
        if slots.is_instance_of::<PyString>() {
            // A string __slots__ contributes its characters, like the
            // Python list.extend over the string would.
            let s: &PyString = slots.downcast().map_err(WalkErr::from)?;
            for ch in s.to_str().map_err(WalkErr::from)?.chars() {
                collected.push(ch.to_string());
            }
        } else {
            for item in slots.iter().map_err(WalkErr::from)? {
                let item = item.map_err(WalkErr::from)?;
                let s: &PyString = item.downcast().map_err(WalkErr::from)?;
                collected.push(s.to_str().map_err(WalkErr::from)?.to_string());
            }
        }
    }
    // dict.fromkeys dedup (first occurrence wins), then name filters.
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for name in collected {
        if seen.insert(name.clone()) {
            if name.starts_with('_') || WALK_SKIP_SLOTS.contains(&name.as_str()) {
                continue;
            }
            out.push(name);
        }
    }
    Ok(out)
}

fn slot_names_for(ctx: &mut WalkCtx, py: Python, t: &PyAny) -> Result<Rc<Vec<String>>, WalkErr> {
    let ty = t.get_type();
    let key = ty.as_ptr() as usize;
    if let Some(v) = ctx.slot_cache.get(&key) {
        return Ok(v.clone());
    }
    let names = Rc::new(collect_slot_names(ty, py)?);
    ctx.slot_cache.insert(key, names.clone());
    Ok(names)
}

/// The three walk output lists, grouped so walk_slots/walk_value stay
/// under the clippy argument limit.
struct WalkOut {
    tvids: Vec<Py<PyAny>>,
    aliases: Vec<Py<PyAny>>,
    embeds: Vec<Py<PyAny>>,
}

fn walk_slots(
    ctx: &mut WalkCtx,
    py: Python,
    t: &PyAny,
    seen: &mut HashSet<usize>,
    depth: usize,
    out: &mut WalkOut,
) -> Result<(), WalkErr> {
    let names = slot_names_for(ctx, py, t)?;
    for name in names.iter() {
        let depth = depth + 1;
        if depth > WALK_DEPTH_CAP {
            return Err(WalkErr::Depth);
        }
        if let Some(value) = read_slot(t, name, py)? {
            walk_value(ctx, py, value.as_ref(py), seen, depth, out)?;
        }
    }
    Ok(())
}

fn is_scalar(value: &PyAny) -> bool {
    value.is_none()
        || value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyBytes>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
}

/// `type(value).__name__` — pyo3's `PyType::name()` returns `__qualname__`,
/// which diverges for classes defined in a local scope (duck-typed stubs
/// like the ExtraAttrs test double).
fn py_type_name(value: &PyAny) -> Result<String, WalkErr> {
    let name = value
        .get_type()
        .getattr("__name__")
        .map_err(WalkErr::from)?;
    let s = name.downcast::<PyString>().map_err(WalkErr::from)?;
    Ok(s.to_str().map_err(WalkErr::from)?.to_string())
}

/// Dispatch order mirrors `_walk_value` exactly. The cap gate here plus the
/// `depth + 1` passed into every direct recursion bounds all descent edges,
/// so a pure-container cycle defers at the cap instead of overflowing the
/// native stack (Python's fallback then hits its own RecursionError, as
/// before the port).
fn walk_value(
    ctx: &mut WalkCtx,
    py: Python,
    value: &PyAny,
    seen: &mut HashSet<usize>,
    depth: usize,
    out: &mut WalkOut,
) -> Result<(), WalkErr> {
    if depth > WALK_DEPTH_CAP {
        return Err(WalkErr::Depth);
    }
    if is_scalar(value) {
        return Ok(());
    }
    let vptr = value.as_ptr() as usize;
    if value
        .is_instance(ctx.tvid_cls.as_ref(py))
        .map_err(WalkErr::from)?
    {
        // TypeVarId items are collected per occurrence, like Python.
        out.tvids.push(value.to_object(py));
        return Ok(());
    }
    if value
        .is_instance(ctx.type_cls.as_ref(py))
        .map_err(WalkErr::from)?
    {
        if !seen.contains(&vptr) {
            seen.insert(vptr);
            let ty_obj = value.get_type();
            if ctx
                .family
                .iter()
                .any(|f| f.as_ref(py).as_ptr() == ty_obj.as_ptr())
            {
                out.embeds.push(value.to_object(py));
            }
            walk_slots(ctx, py, value, seen, depth, out)?;
        }
        return Ok(());
    }
    let type_name = py_type_name(value)?;
    if type_name == "TypeAlias" {
        if !seen.contains(&vptr) {
            seen.insert(vptr);
            out.aliases.push(value.to_object(py));
            if let Some(target) = read_slot(value, "target", py)? {
                walk_value(ctx, py, target.as_ref(py), seen, depth + 1, out)?;
            }
        }
        return Ok(());
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        seen.insert(vptr);
        for item in value.iter().map_err(WalkErr::from)? {
            let item = item.map_err(WalkErr::from)?;
            walk_value(ctx, py, item, seen, depth + 1, out)?;
        }
        return Ok(());
    }
    if value.is_instance_of::<PyDict>() {
        seen.insert(vptr);
        let values = value.call_method0("values").map_err(WalkErr::from)?;
        for item in values.iter().map_err(WalkErr::from)? {
            let item = item.map_err(WalkErr::from)?;
            walk_value(ctx, py, item, seen, depth + 1, out)?;
        }
        return Ok(());
    }
    if type_name == "ExtraAttrs" {
        // Python reads `value.attrs` with NO AttributeError suppression
        // here (unlike the slot scan and the alias target); an unreadable
        // read defers, so the fallback re-raises identically.
        match value.getattr("attrs") {
            Ok(attrs) => walk_value(ctx, py, attrs, seen, depth + 1, out)?,
            Err(_) => return Err(WalkErr::Defer),
        }
    }
    Ok(())
}

/// Family classes in registration order (FAMILY_NAME in types_mirror.py).
const FAMILY_ORDER: [&str; 4] = ["Instance", "CallableType", "TypeVarType", "UnionType"];

/// The three index lists; alias keeps the pyfunction signature out of the
/// clippy type-complexity limit.
type WalkLists = (Vec<Py<PyAny>>, Vec<Py<PyAny>>, Vec<Py<PyAny>>);

#[pyfunction]
pub(crate) fn rust_mirror_walk_indices(py: Python, root: &PyAny) -> Option<WalkLists> {
    let types_mod = py.import("mypy.types").ok()?;
    let cls = |name: &str| types_mod.getattr(name).ok().map(|o| o.to_object(py));
    let mut ctx = WalkCtx {
        type_cls: cls("Type")?,
        tvid_cls: cls("TypeVarId")?,
        family: FAMILY_ORDER
            .iter()
            .map(|name| cls(name))
            .collect::<Option<Vec<_>>>()?,
        slot_cache: HashMap::new(),
    };
    let mut seen = HashSet::new();
    seen.insert(root.as_ptr() as usize);
    let mut out = WalkOut {
        tvids: Vec::new(),
        aliases: Vec::new(),
        embeds: Vec::new(),
    };
    walk_slots(&mut ctx, py, root, &mut seen, 0, &mut out).ok()?;
    let WalkOut {
        tvids,
        aliases,
        embeds,
    } = out;
    Some((tvids, aliases, embeds))
}

#[cfg(test)]
mod mirror_tests {
    use super::*;
    use crate::wire::{write_type_list, LiteralValue};
    use pyo3::types::PyModule;
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
            write_type_list(
                &mut ab,
                &[Type::AnyType {
                    type_of_any: 2,
                    source_any: None,
                    missing_import_name: None,
                }],
            )
            .unwrap();
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

    /// Wire-legal `LiteralType` fallback: an Instance (NoneType is rejected
    /// by `read_literal_type`).
    fn int_fallback() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// An empty-args Instance whose wire form is the INSTANCE_GENERIC full
    /// encoding with an lkv slice (lkv forces the generic path even here).
    fn lkv_instance_blob(value: Option<LiteralValue>) -> Vec<u8> {
        let inst = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![],
            last_known_value: value.map(|v| {
                Box::new(Type::LiteralType {
                    fallback: Box::new(int_fallback()),
                    value: v,
                })
            }),
            extra_attrs: None,
        };
        let mut w = WriteBuffer::new();
        write_type(&mut w, &inst).unwrap();
        w.into_bytes()
    }

    #[test]
    fn test_patch_instance_type_roundtrip() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "instance", lkv_instance_blob(None), vec![]).unwrap();
            let new = patch_instance_type(h, "builtins.dict").unwrap();
            let mut rbuf = ReadBuffer::new(&new);
            if let Type::Instance { type_ref, .. } = read_type(&mut rbuf, None).unwrap() {
                assert_eq!(type_ref, "builtins.dict");
            } else {
                panic!("not an Instance after patch");
            }
        });
    }

    #[test]
    fn test_patch_instance_type_is_noop_on_same_fullname() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let blob = lkv_instance_blob(None);
            let h = register(obj, "instance", blob.clone(), vec![]).unwrap();
            assert_eq!(patch_instance_type(h, "builtins.list"), Some(blob.clone()));
            // Change: new blob, and the stored bytes now say builtins.dict.
            let new = patch_instance_type(h, "builtins.dict").unwrap();
            let mut rbuf = ReadBuffer::new(&new);
            if let Type::Instance { type_ref, .. } = read_type(&mut rbuf, None).unwrap() {
                assert_eq!(type_ref, "builtins.dict");
            } else {
                panic!("not an Instance after change");
            }
            assert_eq!(entry_bytes(h), Some(new));
        });
    }

    #[test]
    fn test_patch_instance_lkv_roundtrip_and_clear() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let blob = lkv_instance_blob(Some(LiteralValue::Int(3)));
            let h = register(obj, "instance", blob, vec![]).unwrap();
            // Noop: same literal value.
            let same = {
                let lit = Type::LiteralType {
                    fallback: Box::new(int_fallback()),
                    value: LiteralValue::Int(3),
                };
                let mut w = WriteBuffer::new();
                write_type(&mut w, &lit).unwrap();
                w.into_bytes()
            };
            // Noop: same literal value, so the op returns the pre-call
            // stored bytes and leaves the handle untouched.
            let before = entry_bytes(h).unwrap();
            let ret = patch_instance_lkv(h, Some(&same)).unwrap();
            assert_eq!(ret, before);
            assert_eq!(entry_bytes(h), Some(before));
            // Clear: the wire form flips back to an INSTANCE (empty args, no
            // lkv) whose write output is the INSTANCE singleton for str...
            // builtins.list is not a singleton name, so it stays generic.
            let cleared = patch_instance_lkv(h, None).unwrap();
            let mut rbuf = ReadBuffer::new(&cleared);
            if let Type::Instance {
                last_known_value, ..
            } = read_type(&mut rbuf, None).unwrap()
            {
                assert!(last_known_value.is_none());
            } else {
                panic!("not an Instance after clear");
            }
            // Set again on a fresh handle for the set path.
            let obj2 = fresh(py);
            let h2 = register(obj2, "instance", lkv_instance_blob(None), vec![]).unwrap();
            let set = patch_instance_lkv(h2, Some(&same)).unwrap();
            let mut rbuf2 = ReadBuffer::new(&set);
            if let Type::Instance {
                last_known_value, ..
            } = read_type(&mut rbuf2, None).unwrap()
            {
                assert!(
                    matches!(last_known_value, Some(b) if matches!(&*b, Type::LiteralType { value: LiteralValue::Int(3), .. }))
                );
            } else {
                panic!("not an Instance after set");
            }
        });
    }

    #[test]
    fn test_patch_instance_type_and_lkv_defer_on_bad_shapes() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            // Bad stored blob: both defers.
            let h = register(obj, "instance", b"garbage".to_vec(), vec![]).unwrap();
            assert_eq!(patch_instance_type(h, "builtins.dict"), None);
            assert_eq!(patch_instance_lkv(h, None), None);
            assert_eq!(patch_instance_lkv(h, Some(&[9])), None);
            // Unregistered handle: both defers.
            let lit = Type::LiteralType {
                fallback: Box::new(int_fallback()),
                value: LiteralValue::Int(1),
            };
            let mut w = WriteBuffer::new();
            write_type(&mut w, &lit).unwrap();
            let blob = w.into_bytes();
            assert_eq!(patch_instance_type(h + 7, "builtins.dict"), None);
            assert_eq!(patch_instance_lkv(h + 7, Some(&blob)), None);
            // A GOOD stored blob with no lkv plus a garbage lkv payload
            // must defer, not collapse into the None clear (silent noop
            // success for a real dropped write).
            let obj3 = fresh(py);
            let h3 = register(obj3, "instance", lkv_instance_blob(None), vec![]).unwrap();
            assert_eq!(patch_instance_lkv(h3, Some(&[9])), None);
            let obj4 = fresh(py);
            let h4 = register(obj4, "instance", lkv_instance_blob(None), vec![]).unwrap();
            assert_eq!(patch_instance_lkv(h4, Some(b"garbage")), None);
        });
    }

    #[test]
    fn test_write_skip_and_stamp_semantics() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "instance", b"z".to_vec(), vec![]).unwrap();
            // A fresh entry is stamped 0: skip only at epoch 0.
            assert!(write_skip(h, 0));
            assert!(!write_skip(h, 7));
            // Stamp sync moves the epoch cursor forward.
            assert!(stamp_sync(h, 7));
            assert!(write_skip(h, 7));
            assert!(!write_skip(h, 8));
            // Unknown handles never skip and never stamp.
            assert!(!write_skip(h + 1, 0));
            assert!(!stamp_sync(h + 1, 0));
            // Reset clears stamps with the storage.
            assert!(stamp_sync(h, 3));
            reset();
            assert!(!write_skip(h, 3));
        });
    }

    #[test]
    fn test_register_reentry_resets_stamp() {
        with_py(|py| {
            reset();
            let obj = fresh(py);
            let h = register(obj, "instance", b"z".to_vec(), vec![]).unwrap();
            assert!(stamp_sync(h, 9));
            // Re-registration installs fresh unknown-provenance bytes.
            let h2 = register(obj, "instance", b"z2".to_vec(), vec![]).unwrap();
            assert_eq!(h2, h);
            assert!(!write_skip(h, 9));
            assert!(write_skip(h, 0));
        });
    }

    /// A self-referencing container held in a slots-bearing root must defer
    /// at the depth cap, not overflow the native stack (Python's fallback
    /// then hits its own RecursionError as before the port).
    #[test]
    fn test_walk_indices_defers_on_container_cycle() {
        with_py(|py| {
            let root = slots_root(py);
            let cycle = PyList::empty(py);
            cycle.append(cycle).unwrap(); // self cycle
            root.setattr("args", cycle).unwrap();
            assert!(rust_mirror_walk_indices(py, root).is_none());
        });
    }

    /// Acyclic but deeper than the cap defers the same way.
    #[test]
    fn test_walk_indices_defers_on_deep_container_nesting() {
        with_py(|py| {
            let root = slots_root(py);
            let mut cur: Py<PyAny> = PyList::empty(py).into();
            for _ in 0..(WALK_DEPTH_CAP + 10) {
                let l = PyList::empty(py);
                l.append(cur.as_ref(py)).unwrap();
                cur = l.into();
            }
            root.setattr("args", cur).unwrap();
            assert!(rust_mirror_walk_indices(py, root).is_none());
        });
    }

    /// Duck-typed "ExtraAttrs" without a readable `attrs` defers: Python
    /// reads `value.attrs` unguarded and would raise AttributeError.
    #[test]
    fn test_walk_indices_defers_on_unreadable_extra_attrs() {
        with_py(|py| {
            let root = slots_root(py);
            let fake = py.eval("type('ExtraAttrs', (), {})", None, None).unwrap();
            root.setattr("args", fake.call0().unwrap()).unwrap();
            assert!(rust_mirror_walk_indices(py, root).is_none());
        });
    }

    /// Duck-typed walk root with a single `args` slot, so the entry's
    /// slot scan reaches the container descent regardless of whether
    /// `mypy.types` is importable.
    fn slots_root(py: Python<'_>) -> &PyAny {
        let cls = PyModule::from_code(py, "class _WalkRoot:\n    __slots__ = ('args',)\n", "", "")
            .unwrap()
            .getattr("_WalkRoot")
            .unwrap();
        cls.call0().unwrap()
    }

    // ---- CallableType splice ops (F3 slice 8, #1397) ----

    /// Wire-legal non-callable carrier for deferral checks: a TVar.
    fn tvt(name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{name}"),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::NoneType),
            default: Box::new(Type::NoneType),
            variance: 0,
            meta_level: 0,
        }
    }

    /// A callable blob `f(T) -> T` on the `builtins.function` fallback.
    fn callable_blob() -> Vec<u8> {
        let t = Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            special_sig: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![tvt("T")],
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(Type::NoneType),
            name: Some("f".to_string()),
            variables: vec![tvt("T")],
            type_guard: None,
            type_is: None,
        };
        let mut w = WriteBuffer::new();
        write_type(&mut w, &t).unwrap();
        w.into_bytes()
    }

    fn registered_callable(py: Python<'_>) -> u64 {
        register(fresh(py), "callable", callable_blob(), vec![]).unwrap()
    }

    fn decode(h: u64) -> Type {
        let blob = entry_bytes(h).unwrap();
        let mut rbuf = ReadBuffer::new(&blob);
        read_type(&mut rbuf, None).unwrap()
    }

    fn anyt() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn single_blob(t: &Type) -> Vec<u8> {
        let mut w = WriteBuffer::new();
        write_type(&mut w, t).unwrap();
        w.into_bytes()
    }

    fn type_list_blob(items: &[Type]) -> Vec<u8> {
        let mut b = WriteBuffer::new();
        write_type_list(&mut b, items).unwrap();
        b.into_bytes()
    }

    #[test]
    fn test_patch_callable_ret_type_roundtrip() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            // Garbage payload defers without touching storage.
            let before = entry_bytes(h).unwrap();
            assert_eq!(patch_callable_ret_type(h, &[9]), None);
            assert_eq!(
                patch_callable_ret_type(h, &single_blob(&Type::NoneType)).unwrap(),
                before
            );
            // Changed ret: stored blob now carries the Any.
            let new = patch_callable_ret_type(h, &single_blob(&anyt())).unwrap();
            assert!(new != before);
            match decode(h) {
                Type::CallableType { ret_type, .. } => {
                    assert!(matches!(*ret_type, Type::AnyType { .. }))
                }
                _ => panic!("not a CallableType after patch"),
            }
            assert_eq!(
                patch_callable_ret_type(h, &single_blob(&anyt())).unwrap(),
                new
            );
        });
    }

    #[test]
    fn test_patch_callable_arg_types_and_variables() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            let new = patch_callable_arg_types(h, &type_list_blob(&[anyt(), tvt("Q")])).unwrap();
            match decode(h) {
                Type::CallableType { arg_types, .. } => {
                    assert_eq!(arg_types.len(), 2);
                    assert!(matches!(arg_types[1], Type::TypeVarType { .. }));
                }
                _ => panic!("not a CallableType after arg_types patch"),
            }
            // Noop: identical list round-trips the same bytes.
            assert_eq!(
                patch_callable_arg_types(h, &type_list_blob(&[anyt(), tvt("Q")])).unwrap(),
                new
            );
            // variables: same list framing; swap T for Q.
            let vars = patch_callable_variables(h, &type_list_blob(&[tvt("Q")])).unwrap();
            match decode(h) {
                Type::CallableType { variables, .. } => {
                    assert_eq!(variables.len(), 1);
                    if let Type::TypeVarType { name, .. } = &variables[0] {
                        assert_eq!(name, "Q");
                    } else {
                        panic!("variable is not a tvar");
                    }
                }
                _ => panic!("not a CallableType after variables patch"),
            }
            assert_eq!(entry_bytes(h), Some(vars));
        });
    }

    #[test]
    fn test_patch_callable_arg_kinds_names() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            let kinds = patch_callable_arg_kinds(h, vec![0, 2]).unwrap();
            match decode(h) {
                Type::CallableType { arg_kinds, .. } => assert_eq!(arg_kinds, vec![0, 2]),
                _ => panic!("not a CallableType after kinds patch"),
            }
            // Weird kind values pass through: the wire stores bare ints.
            assert_eq!(patch_callable_arg_kinds(h, vec![0, 2]).unwrap(), kinds);
            let names = patch_callable_arg_names(h, vec![Some("x".to_string()), None]).unwrap();
            match decode(h) {
                Type::CallableType { arg_names, .. } => {
                    assert_eq!(arg_names, vec![Some("x".to_string()), None])
                }
                _ => panic!("not a CallableType after names patch"),
            }
            // Noop names: same list returns the pre-call stored bytes.
            assert_eq!(
                patch_callable_arg_names(h, vec![Some("x".to_string()), None]).unwrap(),
                names
            );
            assert_eq!(entry_bytes(h), Some(names));
        });
    }

    #[test]
    fn test_patch_callable_name_and_flags() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            match decode(h) {
                Type::CallableType { name, .. } => assert_eq!(name.as_deref(), Some("f")),
                _ => panic!("setup blob wrong"),
            }
            // Noop on the same name.
            assert_eq!(
                patch_callable_name_field(h, Some("f")).unwrap(),
                entry_bytes(h).unwrap()
            );
            patch_callable_name_field(h, Some("g")).unwrap();
            // Second write of the same name is a noop: stored bytes unchanged.
            assert_eq!(
                patch_callable_name_field(h, Some("g")).unwrap(),
                entry_bytes(h).unwrap()
            );
            // Flags: full wire-order reorder, and a wrong-length list defers.
            assert_eq!(patch_callable_flags(h, vec![true]), None);
            let flags_blob =
                patch_callable_flags(h, vec![false, true, true, false, false, false, false])
                    .unwrap();
            match decode(h) {
                Type::CallableType {
                    implicit, is_bound, ..
                } => {
                    assert!(implicit);
                    assert!(is_bound);
                }
                _ => panic!("not a CallableType after flags patch"),
            }
            // Noop flags pass-through.
            assert_eq!(
                patch_callable_flags(h, vec![false, true, true, false, false, false, false])
                    .unwrap(),
                flags_blob
            );
        });
    }

    #[test]
    fn test_patch_callable_opt_fallback_instance_type() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            // type_guard: set to an Any via which=0, then clear with None.
            let guard = patch_callable_opt_field(h, 0, Some(&single_blob(&anyt()))).unwrap();
            match decode(h) {
                Type::CallableType { type_guard, .. } => {
                    assert!(
                        matches!(type_guard, Some(ref t) if matches!(&**t, Type::AnyType { .. }))
                    )
                }
                _ => panic!("not a CallableType after type_guard set"),
            }
            assert_eq!(entry_bytes(h), Some(guard));
            let cleared = patch_callable_opt_field(h, 0, None).unwrap();
            match decode(h) {
                Type::CallableType { type_guard, .. } => assert!(type_guard.is_none()),
                _ => panic!("not a CallableType after type_guard clear"),
            }
            assert_eq!(entry_bytes(h), Some(cleared));
            // type_is via which=1.
            patch_callable_opt_field(h, 1, Some(&single_blob(&Type::NoneType))).unwrap();
            match decode(h) {
                Type::CallableType { type_is, .. } => {
                    assert!(matches!(type_is, Some(ref t) if matches!(&**t, Type::NoneType)))
                }
                _ => panic!("not a CallableType after type_is set"),
            }
            // Unknown which defers without touching storage.
            let before = entry_bytes(h).unwrap();
            assert_eq!(
                patch_callable_opt_field(h, 2, Some(&single_blob(&anyt()))),
                None
            );
            assert_eq!(entry_bytes(h), Some(before));
            // fallback: swap to an int Instance.
            let fb = patch_callable_fallback(h, &single_blob(&int_fallback())).unwrap();
            match decode(h) {
                Type::CallableType { fallback, .. } => {
                    assert!(matches!(*fallback, Type::Instance { .. }))
                }
                _ => panic!("not a CallableType after fallback patch"),
            }
            assert_eq!(entry_bytes(h), Some(fb));
            // instance_type: absent -> None noop, then set.
            assert_eq!(
                patch_callable_instance_type(h, None).unwrap(),
                entry_bytes(h).unwrap()
            );
            patch_callable_instance_type(h, Some(&single_blob(&anyt()))).unwrap();
            match decode(h) {
                Type::CallableType { instance_type, .. } => {
                    assert!(
                        matches!(instance_type, Some(ref t) if matches!(&**t, Type::AnyType { .. }))
                    )
                }
                _ => panic!("not a CallableType after instance_type set"),
            }
        });
    }

    #[test]
    fn test_patch_callable_defers_on_bad_shapes() {
        with_py(|py| {
            reset();
            // Garbage stored blob: every op defers.
            let h = register(fresh(py), "callable", b"garbage".to_vec(), vec![]).unwrap();
            assert_eq!(patch_callable_ret_type(h, &single_blob(&anyt())), None);
            assert_eq!(patch_callable_arg_types(h, &type_list_blob(&[])), None);
            assert_eq!(patch_callable_arg_kinds(h, vec![0]), None);
            assert_eq!(patch_callable_arg_names(h, vec![None]), None);
            assert_eq!(patch_callable_name_field(h, Some("g")), None);
            assert_eq!(patch_callable_variables(h, &type_list_blob(&[])), None);
            assert_eq!(patch_callable_opt_field(h, 0, None), None);
            assert_eq!(patch_callable_fallback(h, &single_blob(&anyt())), None);
            assert_eq!(patch_callable_instance_type(h, None), None);
            assert_eq!(
                patch_callable_flags(h, vec![false, false, false, false, false, false, false]),
                None
            );
            // Non-callable stored family: defers instead of retyping.
            let mut w = WriteBuffer::new();
            write_type(&mut w, &tvt("T")).unwrap();
            let h2 = register(fresh(py), "tvar", w.into_bytes(), vec![]).unwrap();
            assert_eq!(patch_callable_ret_type(h2, &single_blob(&anyt())), None);
            assert_eq!(patch_callable_arg_kinds(h2, vec![0]), None);
            assert_eq!(
                patch_callable_flags(h2, vec![false, false, false, false, false, false, false]),
                None
            );
            // Unregistered handles defer.
            assert_eq!(patch_callable_ret_type(h + 1, &single_blob(&anyt())), None);
            // Registered callable, garbage list payloads defer too.
            let h3 = registered_callable(py);
            assert_eq!(patch_callable_arg_types(h3, b"garbage"), None);
            assert_eq!(patch_callable_variables(h3, b"garbage"), None);
            assert_eq!(patch_callable_fallback(h3, b"garbage"), None);
            assert_eq!(patch_callable_ret_type(h3, b"garbage"), None);
            assert_eq!(patch_callable_opt_field(h3, 0, Some(b"garbage")), None);
            assert_eq!(patch_callable_instance_type(h3, Some(b"garbage")), None);
        });
    }

    #[test]
    fn test_patch_callable_preserves_unpatched_fields() {
        with_py(|py| {
            reset();
            let h = registered_callable(py);
            // Patching one field must keep every other wire field intact,
            // including a wire-invisible round-trip (special_sig decodes to
            // the None default because the wire never carries it).
            patch_callable_arg_kinds(h, vec![0, 3]).unwrap();
            patch_callable_name_field(h, Some("g")).unwrap();
            match decode(h) {
                Type::CallableType {
                    name,
                    arg_kinds,
                    arg_types,
                    ret_type,
                    variables,
                    ..
                } => {
                    assert_eq!(name.as_deref(), Some("g"));
                    assert_eq!(arg_kinds, vec![0, 3]);
                    // arg_types was NOT patched: still the single tvar.
                    assert_eq!(arg_types.len(), 1);
                    if let Type::TypeVarType { name: n, .. } = &arg_types[0] {
                        assert_eq!(n, "T");
                    } else {
                        panic!("arg_types drifted");
                    }
                    assert!(matches!(*ret_type, Type::NoneType));
                    assert_eq!(variables.len(), 1);
                }
                _ => panic!("not a CallableType"),
            }
        });
    }
}
