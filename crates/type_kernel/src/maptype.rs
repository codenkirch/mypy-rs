//! Stage 6 seam: nominal supertype mapping for `mypy.maptype`.
//!
//! Ports `map_instance_to_supertype` from `mypy/maptype.py` — the hot-path
//! that maps an `Instance` up one derivation path to a `superclass` frame.
//! Returns `None` for unsupported edges (missing TypeInfo, variadic tuples,
//! multi-level path expansion not covered by subtypes), so the Python shim
//! falls through to the pure-Python path.
//!
//! Shares the existing `subtypes::map_instance_to_supertype` as its derivation
//! path primitive.  Also exposes `class_derivation_paths` (pure BFS) and
//! `map_instance_to_direct_supertypes` (direct-base walk) for parity.
//!
//! IMPORTANT: the `tuple_fallback` special case (`builtins.tuple` in
//! `map_instance_to_direct_supertypes`, maptype.py:78-97) is NOT ported.
//! Rust returns `None` for that path so Python computes it.

use pyo3::prelude::*;

use crate::subtypes::map_instance_to_supertype as subtypes_map;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Decode a wire-format blob to a `Type`, returning `None` on any read
/// failure or unknown tag.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Encode a `Type` to wire-format bytes, returning `None` if the type
/// cannot be serialized (e.g. TypeAliasType, Callable with definition).
fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// Extract the `type_ref` from a wire-format Instance blob.
/// Returns `None` on any read failure or non-Instance blob.
fn instance_type_ref(bytes: &[u8]) -> Option<String> {
    decode_type(bytes).and_then(|t| match t {
        Type::Instance { type_ref, .. } => Some(type_ref),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// 1. rust_class_derivation_paths — walk all derivation paths
// ---------------------------------------------------------------------------

/// Find all non-empty paths of direct base classes from `typ` to
/// `supertype`.
///
/// `typ_ref` and `supertype_ref` are fullnames.  The resolver provides
/// the `TypeInfoSnapshot` with `bases` (wire-format Instance blobs).
///
/// Returns `None` if any snapshot is missing.  Returns an empty list
/// when no path exists.
///
/// Port of `mypy/maptype.py:46-67` (class_derivation_paths).
fn find_all_paths(
    typ_ref: &str,
    supertype_ref: &str,
    resolver: &TypeResolver,
    visited: &mut Vec<String>,
) -> Option<Vec<Vec<String>>> {
    let typ_snap = resolver.get(typ_ref)?;
    let mut result: Vec<Vec<String>> = Vec::new();

    for base_blob in &typ_snap.bases {
        let base_ref = instance_type_ref(base_blob)?;
        if base_ref == supertype_ref {
            // Direct base matches — one-element path.
            result.push(vec![base_ref]);
        } else {
            // Recurse: try longer paths through this base.
            if visited.contains(&base_ref) {
                // Cycle guard: skip to avoid infinite loops.
                continue;
            }
            visited.push(base_ref.clone());
            if let Some(sub_paths) = find_all_paths(&base_ref, supertype_ref, resolver, visited) {
                for path in sub_paths {
                    let mut p = vec![base_ref.clone()];
                    p.extend(path);
                    result.push(p);
                }
            }
            visited.pop();
        }
    }

    Some(result)
}

/// Public PyO3 entry for `class_derivation_paths`.
///
/// Returns a list of lists of type_ref strings, or `None` if any
/// TypeInfo snapshot is missing from the resolver.
#[pyfunction]
pub(crate) fn rust_class_derivation_paths(
    resolver: &NativeTypeResolver,
    typ_ref: String,
    supertype_ref: String,
) -> PyResult<Option<Vec<Vec<String>>>> {
    let mut visited = Vec::new();
    let result = find_all_paths(&typ_ref, &supertype_ref, resolver.resolver(), &mut visited);
    Ok(result)
}

// ---------------------------------------------------------------------------
// 2. rust_map_instance_to_supertype — single supertype mapping
// ---------------------------------------------------------------------------

/// Public PyO3 entry for `map_instance_to_supertype`.
///
/// Returns a single Instance bytes blob on success, `None` on failure.
/// Mirrors `mypy/maptype.py:8-23` with the same two fast paths:
///   1. `instance.type == superclass` → return input bytes unchanged.
///   2. `superclass.type_vars.is_empty()` → return `Instance(supertype, [])`.
#[pyfunction]
pub(crate) fn rust_map_instance_to_supertype(
    resolver: &NativeTypeResolver,
    instance_ref: String,
    instance_args: Vec<u8>,
    supertype_ref: String,
) -> PyResult<Option<Vec<u8>>> {
    // Fast path: instance.type == superclass (maptype.py:15-17).
    if instance_ref == supertype_ref {
        return Ok(Some(instance_args));
    }

    // Fast path: superclass has no type variables (maptype.py:19-21).
    if let Some(sup_snap) = resolver.resolver().get(&supertype_ref) {
        if sup_snap.type_vars.is_empty() {
            let inst = Type::Instance {
                type_ref: supertype_ref,
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            };
            return Ok(encode_type(&inst));
        }
    }

    // Walk the derivation path using the existing subtypes primitive.
    // Decode the Instance to get its args.
    let args = decode_type(&instance_args)
        .and_then(|t| match t {
            Type::Instance { args, .. } => Some(args),
            _ => None,
        })
        .ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "rust_map_instance_to_supertype: instance_args must be a serialized Instance",
            )
        })?;

    if let Some(mapped_args) =
        subtypes_map(&instance_ref, &args, &supertype_ref, resolver.resolver())
    {
        let inst = Type::Instance {
            type_ref: supertype_ref,
            args: mapped_args,
            last_known_value: None,
            extra_attrs: None,
        };
        Ok(encode_type(&inst))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// 3. rust_map_instance_to_direct_supertypes — direct supertype mapping
// ---------------------------------------------------------------------------

/// Walk `typ.bases` looking for a base whose type == `supertype_ref`.
/// For each matching base, expand the base's args by the instance frame.
///
/// Returns `None` for the `builtins.tuple` special case (`tuple_fallback`
/// is not ported) or when no matching base is found (Python handles the
/// Any fallback).
#[pyfunction]
pub(crate) fn rust_map_instance_to_direct_supertypes(
    resolver: &NativeTypeResolver,
    instance_ref: String,
    instance_args: Vec<u8>,
    supertype_ref: String,
) -> PyResult<Option<Vec<Vec<u8>>>> {
    // The `builtins.tuple` special case (maptype.py:78-97): defer to
    // Python because `tuple_fallback` is not ported.
    if supertype_ref == "builtins.tuple" {
        return Ok(None);
    }

    let args = decode_type(&instance_args)
        .and_then(|t| match t {
            Type::Instance { args, .. } => Some(args),
            _ => None,
        })
        .ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "rust_map_instance_to_direct_supertypes: instance_args must be a serialized Instance",
            )
        })?;

    let typ_snap = resolver.resolver().get(&instance_ref);

    let mut result: Vec<Vec<u8>> = Vec::new();

    if let Some(snap) = typ_snap {
        for base_blob in &snap.bases {
            let base_ref = instance_type_ref(base_blob);
            if base_ref == Some(supertype_ref.clone()) {
                // Direct base match: expand via subtypes primitive.
                if let Some(mapped_args) =
                    subtypes_map(&instance_ref, &args, &supertype_ref, resolver.resolver())
                {
                    let inst = Type::Instance {
                        type_ref: supertype_ref.clone(),
                        args: mapped_args,
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    if let Some(encoded) = encode_type(&inst) {
                        result.push(encoded);
                    }
                }
            }
        }
    }

    if result.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}
