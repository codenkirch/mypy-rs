//! `detach_callable` port (mypy.checker.detach_callable).
//!
//! Ensures a callable's `variables` include the class type variables it uses,
//! so the signature is independent of the containing class context.
//!
//! Mirrors `mypy/checker.py:detach_callable` as a single Rust function:
//!
//! ```text
//! rust_detach_callable(typ_bytes, class_type_vars_bytes) -> Option<Vec<Vec<u8>>>
//! ```
//!
//! `typ_bytes` is the wire-format blob of the `CallableType`,
//! `class_type_vars_bytes` the wire-format blob of a `list[TypeVarLikeType]`
//! (`LIST_GEN + bare size + N` tagged types, the same layout
//! `mypy.types.write_type_list` emits). Rust reads `typ.variables` from the
//! callable record and returns the concatenated variables list
//! (`typ.variables + class_type_vars`) as a list of per-variable wire blobs.
//! A non-`None` result is a "handled" signal: the Python shim decodes each
//! blob back to a live `TypeVarLikeType` and rebuilds the callable via
//! `copy_modified(variables=...)` on the live object, so non-wire fields
//! (definition, line, column, special_sig) survive. `None` means "Rust
//! doesn't handle this", and Python falls back to the pure-Python path
//! (strangler-fig per-call gate).
//!
//! The fast path (`class_type_vars` empty, return `typ` unchanged) never
//! reaches Rust.

use pyo3::prelude::*;

use crate::wire::{self, ReadBuffer, Type};

/// Concatenate the callable's variables with the class type variables and
/// encode each resulting variable as a standalone wire-type blob. Returns
/// `None` on any decode/encode ambiguity so the caller defers to Python.
fn detach_callable_inner(typ_bytes: &[u8], class_type_vars_bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let typ = decode_type(typ_bytes)?;
    let class_type_vars = decode_type_list(class_type_vars_bytes)?;
    let callable_variables = match &typ {
        Type::CallableType { variables, .. } => variables.clone(),
        _ => return None,
    };
    let mut combined = Vec::with_capacity(callable_variables.len() + class_type_vars.len());
    combined.extend(callable_variables);
    combined.extend(class_type_vars);
    let mut out = Vec::with_capacity(combined.len());
    for v in &combined {
        out.push(encode_type(v)?);
    }
    Some(out)
}

/// Decode one full tagged `Type` from a blob.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// Decode a `LIST_GEN + bare size + N types` wire list.
fn decode_type_list(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type_list(&mut buf).ok()
}

/// Encode a single `Type` as a standalone wire-type blob.
fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = wire::WriteBuffer::new();
    wire::write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// `#[pyfunction]` entry for `detach_callable` (checker.py:9943-9964).
/// Returns the concatenated variables list as per-type wire blobs, or
/// `None` (defer to Python).
#[pyfunction]
pub(crate) fn rust_detach_callable(
    typ_bytes: &[u8],
    class_type_vars_bytes: &[u8],
) -> Option<Vec<Vec<u8>>> {
    detach_callable_inner(typ_bytes, class_type_vars_bytes)
}
