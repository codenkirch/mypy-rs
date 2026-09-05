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
use crate::wire::{
    read_type, read_type_list, write_type, write_type_list, ReadBuffer, Type, WriteBuffer,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Decode a wire-format blob to a `Type`, returning `None` on any read
/// failure or unknown tag.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Encode a `Type` to wire-format bytes, returning `None` if a variant's
/// fields cannot be serialized (e.g. a Callable carrying its definition
/// node; TypeAliasType itself carries a wire tag and encodes fine).
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

/// Decode a LIST_GEN-wrapped wire list of types, returning the items.
fn decode_type_list(bytes: &[u8]) -> Option<Vec<Type>> {
    let mut buf = ReadBuffer::new(bytes);
    read_type_list(&mut buf).ok()
}

/// Encode a LIST_GEN-wrapped wire list of types.
fn encode_type_list(items: &[Type]) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_type_list(&mut buf, items).ok()?;
    Some(buf.into_bytes())
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
// 2b. rust_map_instance_to_supertypes — whole per-member loop
// ---------------------------------------------------------------------------

/// Whole per-member loop of `mypy/maptype.py:map_instance_to_supertypes`
/// (maptype.py:179-196).
///
/// `items_wire` is a LIST_GEN wire list of member types (the frontier
/// `types` list of `map_instance_to_supertypes`). For each member, applies
/// the supertype mapping via the type map
/// (`subtypes::map_instance_to_supertype`, mirroring
/// `map_instance_to_supertype(instance, superclass)`) and collects the
/// mapped result wire-encoded.
///
/// Each member carries its own frame (its `type_ref` and args): in a
/// multi-level derivation path the frontier after the first step holds
/// intermediate base instances whose typevars are bound to their *own*
/// frame, so the member's own `type_ref`/`args` are the correct `left`
/// side of the mapping (the original `instance` is only the first member).
///
/// Returns `Ok(Some((encoded_results, flags)))` where `encoded_results`
/// is a LIST_GEN wire list of wire-encoded `Instance` blobs for the
/// members Rust mapped, and `flags` is a parallel `Vec<bool>` over the
/// INPUT members (true = mapped, false = deferred so the shim re-runs
/// that member individually in Python). Also returns `Some` (empty
/// results, all flags false) when the whole list must defer, but `None`
/// when the input cannot be decoded at all (the shim falls through to the
/// pure-Python loop).
///
/// Each member is mapped independently so a single unsupported member
/// (TypeAlias, definition-carrying Callable, ParamSpec-carrying instance,
/// variadic) defers only itself; supported members still engage.
/// TypeVarTuple/TypeVarTuple-carrying frames defer through
/// `subtypes_map`'s own guards.
#[pyfunction]
pub(crate) fn rust_map_instance_to_supertypes(
    resolver: &NativeTypeResolver,
    items_wire: Vec<u8>,
    supertype_ref: String,
) -> PyResult<Option<(Vec<u8>, Vec<bool>)>> {
    let items = match decode_type_list(&items_wire) {
        Some(items) => items,
        None => return Ok(None),
    };
    let mut results: Vec<Type> = Vec::with_capacity(items.len());
    let mut flags: Vec<bool> = Vec::with_capacity(items.len());
    for item in &items {
        let Type::Instance {
            type_ref: member_ref,
            args,
            ..
        } = item
        else {
            // Non-Instance member (e.g. a CallableType / TypeAliasType /
            // ParamSpec member): Python re-runs it individually.
            flags.push(false);
            continue;
        };
        if let Some(mapped_args) =
            subtypes_map(member_ref, args, &supertype_ref, resolver.resolver())
        {
            let inst = Type::Instance {
                type_ref: supertype_ref.clone(),
                args: mapped_args,
                last_known_value: None,
                extra_attrs: None,
            };
            results.push(inst);
            flags.push(true);
        } else {
            // Mapping failed: Python re-runs this member.
            flags.push(false);
        }
    }
    if results.is_empty() {
        // Nothing mapped; the shim re-runs the whole list in Python.
        return Ok(Some((Vec::new(), flags)));
    }
    let encoded = match encode_type_list(&results) {
        Some(encoded) => encoded,
        None => return Ok(None),
    };
    Ok(Some((encoded, flags)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn snapshot(fullname: &str, bases: Vec<Type>) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_string(),
            type_vars: vec!["T".to_string()],
            ..Default::default()
        };
        s.bases = bases.iter().map(|b| encode_type(b).unwrap()).collect();
        s
    }

    fn inst(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tvar(namespace: &str, raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: namespace.to_string(),
            raw_id,
            namespace: namespace.to_string(),
            values: vec![],
            upper_bound: Box::new(inst("builtins.object", vec![])),
            default: Box::new(inst("builtins.object", vec![])),
            variance: 0,
            meta_level: 0,
        }
    }

    fn map_loop(
        items: Vec<Type>,
        supertype_ref: &str,
        resolver: &TypeResolver,
    ) -> Option<(Vec<Type>, Vec<bool>)> {
        let items_wire = encode_type_list(&items).unwrap();
        let mut buf = ReadBuffer::new(&items_wire);
        let decoded = read_type_list(&mut buf).ok()?;
        let mut results = Vec::new();
        let mut flags = Vec::new();
        for item in &decoded {
            let Type::Instance { type_ref, args, .. } = item else {
                flags.push(false);
                continue;
            };
            if let Some(mapped) = subtypes_map(type_ref, args, supertype_ref, resolver) {
                results.push(inst(supertype_ref, mapped));
                flags.push(true);
            } else {
                flags.push(false);
            }
        }
        Some((results, flags))
    }

    #[test]
    fn per_member_loop_maps_direct_base() {
        // class B(A[T]); B[T`1]. Members map B->A via the class tvar.
        let b = snapshot("m.B", vec![inst("m.A", vec![tvar("m.B", 1)])]);
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let items = vec![inst("m.B", vec![inst("m.X", vec![])])];
        let (results, flags) = map_loop(items, "m.A", &resolver).unwrap();
        assert_eq!(flags, vec![true]);
        assert_eq!(results, vec![inst("m.A", vec![inst("m.X", vec![])])]);
    }

    #[test]
    fn per_member_loop_maps_each_member_in_own_frame() {
        // Two members, each in its own frame (multi-level path frontier).
        let b = snapshot("m.B", vec![inst("m.A", vec![tvar("m.B", 1)])]);
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let items = vec![
            inst("m.B", vec![inst("m.X", vec![])]),
            inst("m.B", vec![inst("m.Y", vec![])]),
        ];
        let (results, flags) = map_loop(items, "m.A", &resolver).unwrap();
        assert_eq!(flags, vec![true, true]);
        assert_eq!(
            results,
            vec![
                inst("m.A", vec![inst("m.X", vec![])]),
                inst("m.A", vec![inst("m.Y", vec![])]),
            ]
        );
    }

    #[test]
    fn per_member_loop_defers_non_instance_member_only() {
        // A Callable member (e.g. a definition-carrying callable) defers
        // itself; the Instance member still maps.
        let b = snapshot("m.B", vec![inst("m.A", vec![tvar("m.B", 1)])]);
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let items_wire = encode_type_list(&[
            inst("m.B", vec![inst("m.X", vec![])]),
            Type::CallableType {
                fallback: Box::new(inst("builtins.function", vec![])),
                instance_type: None,
                is_ellipsis_args: false,
                implicit: false,
                is_bound: false,
                from_concatenate: false,
                imprecise_arg_kinds: false,
                unpack_kwargs: false,
                from_type_type: false,
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                ret_type: Box::new(inst("m.X", vec![])),
                name: None,
                variables: vec![],
                type_guard: None,
                type_is: None,
                special_sig: None,
            },
        ])
        .unwrap();
        let mut buf = ReadBuffer::new(&items_wire);
        let decoded = read_type_list(&mut buf).unwrap();
        let (results, flags) = map_loop(decoded, "m.A", &resolver).unwrap();
        assert_eq!(flags, vec![true, false]);
        assert_eq!(results, vec![inst("m.A", vec![inst("m.X", vec![])])]);
    }

    #[test]
    fn per_member_loop_all_defer_returns_empty() {
        // No member maps (missing base): results empty, all flags false
        // (the shim re-runs the whole list in Python).
        let mut resolver = TypeResolver::new();
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let items = vec![inst("m.B", vec![inst("m.X", vec![])])];
        let (results, flags) = map_loop(items, "m.A", &resolver).unwrap();
        assert!(results.is_empty());
        assert_eq!(flags, vec![false]);
    }

    fn alias(type_ref: &str, args: Vec<Type>) -> Type {
        Type::TypeAliasType {
            type_ref: type_ref.to_string(),
            args,
            is_recursive: false,
        }
    }

    // Former audit-bucket shape: derivation-path base args carry a
    // TypeAliasType node; the walker keeps it and substitutes the frame
    // arg into the alias args, matching the unified maptype.rs copy.
    #[test]
    fn map_instance_to_supertype_maps_alias_carrying_base_arg() {
        let b = snapshot(
            "m.B",
            vec![inst("m.A", vec![alias("m.AliasKey", vec![tvar("m.B", 1)])])],
        );
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let left_args = vec![inst("m.X", vec![])];
        let mapped = subtypes_map("m.B", &left_args, "m.A", &resolver).unwrap();
        assert_eq!(mapped, vec![alias("m.AliasKey", vec![inst("m.X", vec![])])]);
    }

    // (b) A frame the walker cannot fully decide (an unmatched tvar) defers:
    // Python substitutes by tvar object identity, so a wire-frame key gap has
    // no safe answer (gs4 fresh-var fixture in NativeMapFreshVarRepairSuite).
    #[test]
    fn map_instance_to_supertype_defers_unmatched_frame_tvar() {
        let b = snapshot(
            "m.B",
            vec![inst("m.A", vec![tvar("m.B", 1), tvar("m.B", 2)])],
        );
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let left_args = vec![inst("m.T", vec![])];
        assert!(
            subtypes_map("m.B", &left_args, "m.A", &resolver).is_none(),
            "the walker must defer on an unmatched frame tvar, not invent a result"
        );
    }

    // (c) An Unpack node inside alias args is a splice shape the walker
    // does not model; it defers to Python.
    #[test]
    fn map_instance_to_supertype_defers_unpack_alias_arg() {
        let b = snapshot(
            "m.B",
            vec![inst(
                "m.A",
                vec![alias(
                    "m.AliasKey",
                    vec![Type::UnpackType {
                        typ: Box::new(tvar("m.B", 1)),
                        from_star_syntax: false,
                    }],
                )],
            )],
        );
        let mut resolver = TypeResolver::new();
        resolver.insert("m.B".to_string(), b);
        resolver.insert("m.A".to_string(), snapshot("m.A", vec![]));
        let left_args = vec![inst("m.X", vec![])];
        assert!(subtypes_map("m.B", &left_args, "m.A", &resolver).is_none());
    }
}
