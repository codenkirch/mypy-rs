//! `TypeChecker.check_overlapping_overloads` screening-loop port
//! (mypy.checker).
//!
//! The Python method (checker.py:1537-1658) drives three per-pair predicates
//! over every ordered pair of overload items and emits mypy messages from
//! the results:
//!
//! * `are_argument_counts_overlapping` (checkexpr_functions.rs),
//! * `overload_can_never_match` (overload_never.rs),
//! * `is_unsafe_overlapping_overload_signatures` (overlap_unsafe.rs).
//!
//! This module ports only the pairwise *screening* part of the loop
//! (checker.py:1559-1603): the decision algebra runs in one Rust call on
//! wire callables, decoding each signature once and reusing the already
//! native predicate engines. The impl-vs-items tail (checker.py:1605-1658)
//! and the message emission (`self.msg.*`) stay in Python.
//!
//! Strangler-fig contract: the Python shim engages only when every item's
//! `var.type` is already a plain `CallableType` (checked without extraction,
//! so no `not_callable` side effects can diverge) and the resolver is
//! installed. Rust defers (`None`) whenever any predicate cannot decide a
//! pair; the Python shim then runs the original pure-Python loop unchanged.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::checkexpr_functions::are_argument_counts_overlapping_inner;
use crate::overlap_unsafe::is_unsafe_overlapping_overload_signatures_inner;
use crate::overload_never::overload_can_never_match_inner;
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type};

/// Decision kinds; values must match `NATIVE_OVERLOAD_KIND_*` in
/// mypy/checker.py.
const KIND_UNSAFE_OVERLAP: usize = 0;
const KIND_NEVER_MATCH: usize = 1;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn decode_type_list(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type_list(&mut buf).ok()
}

/// The pairwise screening loop, mirroring checker.py:1559-1603.
///
/// `sigs` holds one decoded wire callable per overload item, `class_vars`
/// the active class's `defn.type_vars` (empty outside a class), and
/// `strict_optional` the state at entry (the main `overload_can_never_match`
/// check reads it, while the unsafe-overlap check and the flip-note reversed
/// checks always run under `strict_optional=True`, matching the Python
/// `state.strict_optional_set(True)` wrapper). Each returned record is
/// `(i, j, kind, flip_note)` with `kind` one of `KIND_*`; `i` and `j` mirror
/// the Python loop variables: `i` indexes the earlier item and `j` the later
/// one relative to the `defn.items[i + 1 :]` slice.
///
/// Any pair any predicate cannot decide defers the whole call (`None`): the
/// Python shim falls back to the full pure-Python loop, which handles those
/// shapes through its own per-call gates.
fn screening_decisions(
    sigs: &[Type],
    class_vars: &[Type],
    is_descriptor_get: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<(usize, usize, usize, bool)>> {
    let mut decisions = Vec::new();
    for i in 0..sigs.len() {
        if !matches!(sigs[i], Type::CallableType { .. }) {
            continue;
        }
        for k in (i + 1)..sigs.len() {
            if !matches!(sigs[k], Type::CallableType { .. }) {
                continue;
            }
            let j = k - i - 1;
            // checker.py:1571-1572.
            if !are_argument_counts_overlapping_inner(&sigs[i], &sigs[k])? {
                continue;
            }
            // checker.py:1574-1575 (outside the strict-optional wrapper).
            if overload_can_never_match_inner(&sigs[i], &sigs[k], strict_optional, resolver)? {
                decisions.push((i, j, KIND_NEVER_MATCH, false));
            } else if !is_descriptor_get {
                // checker.py:1576-1589: the unsafe check (and the flip-note
                // reversed checks) run under strict_optional=True.
                let unsafe_fwd = is_unsafe_overlapping_overload_signatures_inner(
                    &sigs[i], &sigs[k], class_vars, true, // partial_only
                    true, // strict_optional
                    resolver,
                )?;
                if unsafe_fwd {
                    let flip_note = j == 0
                        && !is_unsafe_overlapping_overload_signatures_inner(
                            &sigs[k], &sigs[i], class_vars, true, // partial_only
                            true, // strict_optional
                            resolver,
                        )?
                        && !overload_can_never_match_inner(
                            &sigs[k], &sigs[i], true, // strict_optional
                            resolver,
                        )?;
                    decisions.push((i, j, KIND_UNSAFE_OVERLAP, flip_note));
                }
            }
        }
    }
    Some(decisions)
}

/// `#[pyfunction]` entry for `TypeChecker.check_overlapping_overloads`
/// screening (mypy/checker.py:1559-1603).
///
/// `signatures` holds one wire blob per overload item (each a serialized
/// `CallableType`, produced by `checker._serialize_type_for_checker`),
/// `class_type_vars` a wire type-list of the active class's type variables,
/// and `is_descriptor_get` / `strict_optional` the flags the Python method
/// computes from `defn` and the running state. Returns `Some(list)` of
/// `(i, j, kind, flip_note)` decision records, or `None` to defer the whole
/// loop to Python.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_check_overlapping_overloads(
    py: Python<'_>,
    signatures: Vec<&[u8]>,
    class_type_vars: &[u8],
    is_descriptor_get: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<Vec<Py<PyAny>>>> {
    let mut sigs = Vec::with_capacity(signatures.len());
    for blob in &signatures {
        match decode_type(blob) {
            Some(t) => sigs.push(t),
            None => return Ok(None),
        }
    }
    let class_vars = match decode_type_list(class_type_vars) {
        Some(v) => v,
        None => return Ok(None),
    };
    let Some(decisions) = screening_decisions(
        &sigs,
        &class_vars,
        is_descriptor_get,
        strict_optional,
        resolver,
    ) else {
        return Ok(None);
    };
    let mut records: Vec<Py<PyAny>> = Vec::with_capacity(decisions.len());
    for (i, j, kind, flip_note) in decisions {
        let record: Py<PyAny> =
            PyTuple::new(py, [i as i64, j as i64, kind as i64, flip_note as i64]).into_py(py);
        records.push(record);
    }
    Ok(Some(records))
}
