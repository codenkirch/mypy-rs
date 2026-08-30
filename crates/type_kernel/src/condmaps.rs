//! Native port of `mypy.checker.and_conditional_maps` and
//! `or_conditional_maps` (checker.py:9361-9427).
//!
//! TypeMap keys are live Python `Expression` objects that cannot cross the
//! wire. Python passes each map's keys as `i64` hashes (from `hash(literal_hash(e))`)
//! and values as serialized wire-format blobs. Rust matches keys by hash,
//! computes the combined values using the existing `meet_types` /
//! `make_simplified_union` kernels, and returns parallel arrays of
//! `key_hashes` + serialized result values. Python rebuilds the dict by
//! mapping hashes back to the original `Expression` keys.
//!
//! Both functions return `None` (Python defers) when any value fails to
//! decode or when a `meet_types` / `make_simplified_union` call defers.

use pyo3::prelude::*;

use crate::checkmember::{decode_type, encode_type};
use crate::setops::{make_simplified_union, meet_types, SetOpResult};
use crate::subtypes::SubtypeContext;
use crate::typeinfo::NativeTypeResolver;
use crate::wire::Type;

/// Convert a `SetOpResult` from `meet_types` back into a concrete `Type`.
/// Mirrors the inline conversion in `setops.rs` (around line 672).
fn meet_result_to_type(r: SetOpResult, s: &Type, t: &Type) -> Option<Type> {
    match r {
        SetOpResult::SameS => Some(s.clone()),
        SetOpResult::SameT => Some(t.clone()),
        SetOpResult::Bottom => Some(Type::UninhabitedType { ambiguous: true }),
        SetOpResult::Any => Some(Type::AnyType {
            // TypeOfAny.special_form (types.py:309); the Python meet
            // mirror builds AnyType(TypeOfAny.special_form).
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        }),
        SetOpResult::Object => Some(Type::Instance {
            type_ref: "builtins.object".into(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }),
        SetOpResult::Ancestor(fullname) => Some(Type::Instance {
            type_ref: fullname,
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }),
        SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        } => {
            let (Type::Instance { args: s_args, .. }, Type::Instance { args: t_args, .. }) = (s, t)
            else {
                return None;
            };
            if arg_discs.len() != s_args.len() || arg_discs.len() != t_args.len() {
                return None;
            }
            let args = reconstruct_args_from_discs(&arg_discs, s_args, t_args);
            Some(Type::Instance {
                type_ref,
                args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
        SetOpResult::Encoded(bytes) => decode_type(&bytes),
    }
}

/// Reconstruct per-arg types from discriminators. 0=left (s), 1=right (t),
/// 4=AnyType(special_form) (types.py:309). Mirrors `setops.rs`
/// (SameTypeWithArgs arm).
fn reconstruct_args_from_discs(arg_discs: &[i8], s_args: &[Type], t_args: &[Type]) -> Vec<Type> {
    arg_discs
        .iter()
        .enumerate()
        .map(|(i, d)| match d {
            0 => s_args[i].clone(),
            1 => t_args[i].clone(),
            4 => Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            },
            _ => s_args[i].clone(),
        })
        .collect()
}

/// Whether any value in the decoded list is an `UninhabitedType`.
/// Mirrors `is_unreachable_map` (checker.py:9354-9358).
fn is_unreachable_map(types: &[Type]) -> bool {
    types
        .iter()
        .any(|t| matches!(t, Type::UninhabitedType { .. }))
}

/// Whether a decoded type is `AnyType`. Mirrors `isinstance(pt, AnyType)`.
fn is_any_type(t: &Type) -> bool {
    matches!(t, Type::AnyType { .. })
}

/// Whether a decoded type is `UninhabitedType`.
fn is_uninhabited_type(t: &Type) -> bool {
    matches!(t, Type::UninhabitedType { .. })
}

/// Whether a decoded type is a `UnionType` containing at least one `AnyType`
/// item (after `get_proper_type` on each item). Mirrors
/// `isinstance(pt1, UnionType) and any(isinstance(get_proper_type(item),
/// AnyType) for item in pt1.items)`.
fn is_union_containing_any(t: &Type) -> bool {
    match t {
        Type::UnionType { items, .. } => items.iter().any(is_any_type),
        _ => false,
    }
}

/// `mypy.checker.and_conditional_maps` (checker.py:9361-9401), Rust port.
///
/// Python passes parallel arrays of key hashes and serialized values for
/// both maps. Rust matches keys by hash and computes combined values:
///   * Keys only in `m1` are added with `m1`'s value.
///   * For common keys with `use_meet=False`: give precedence to `m2` unless
///     `m1[key]` is `UninhabitedType`, or `m2[key]` is `AnyType` and `m1[key]`
///     is not a union containing `AnyType` (then use `m1[key]`).
///   * For common keys with `use_meet=True`: `meet_types(m1[key], m2[key])`.
///
/// Returns `None` when any value fails to decode or `meet_types` defers.
#[pyfunction]
#[allow(clippy::type_complexity)]
#[allow(clippy::if_same_then_else)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_and_conditional_maps(
    keys1: Vec<i64>,
    values1: Vec<Vec<u8>>,
    keys2: Vec<i64>,
    values2: Vec<Vec<u8>>,
    use_meet: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Vec<i64>, Vec<Vec<u8>>)>> {
    if keys1.len() != values1.len() || keys2.len() != values2.len() {
        return Ok(None);
    }
    let r = resolver.resolver();
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);

    // Defer use_meet path: the meet_types Rust kernel does not yet handle
    // all type combinations correctly (TypedDict meets, intersection types).
    // Python handles these correctly; only the common use_meet=False path

    // goes through Rust.
    if use_meet {
        return Ok(None);
    }

    // Decode all values. Defer on any decode failure.
    let mut decoded1: Vec<Type> = Vec::with_capacity(values1.len());
    for v in &values1 {
        decoded1.push(match decode_type(v) {
            Some(t) => t,
            None => return Ok(None),
        });
    }
    let mut decoded2: Vec<Type> = Vec::with_capacity(values2.len());
    for v in &values2 {
        decoded2.push(match decode_type(v) {
            Some(t) => t,
            None => return Ok(None),
        });
    }

    // Build hash -> index map for m2.
    let mut m2_index: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for (i, &h) in keys2.iter().enumerate() {
        m2_index.insert(h, i);
    }

    // result = m2.copy() — build initial output from m2, tracked by key hash
    // so common keys can be replaced in-place (Python: result[e1] = ...).
    let mut out_keys: Vec<i64> = Vec::new();
    let mut out_vals: Vec<Vec<u8>> = Vec::new();
    let mut key_pos: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for (i, &h) in keys2.iter().enumerate() {
        key_pos.insert(h, out_keys.len());
        out_keys.push(h);
        out_vals.push(values2[i].clone());
    }

    for (i, &h) in keys1.iter().enumerate() {
        let t1 = &decoded1[i];
        match m2_index.get(&h) {
            None => {
                // Key only in m1: add m1[key].
                let enc = match encode_type(t1) {
                    Some(b) => b,
                    None => return Ok(None),
                };
                key_pos.insert(h, out_keys.len());
                out_keys.push(h);
                out_vals.push(enc);
            }
            Some(&j) => {
                // Common key: replace existing entry.
                let t2 = &decoded2[j];
                if use_meet {
                    let mr = match meet_types(t1, t2, &ctx, r) {
                        Some(r) => r,
                        None => return Ok(None),
                    };
                    let combined = match meet_result_to_type(mr, t1, t2) {
                        Some(t) => t,
                        None => return Ok(None),
                    };
                    let enc = match encode_type(&combined) {
                        Some(b) => b,
                        None => return Ok(None),
                    };
                    let pos = key_pos[&h];
                    out_vals[pos] = enc;
                } else {
                    // Precedence logic from checker.py:9376-9391.
                    // If m1[key] is UninhabitedType: use m1[key].
                    // Else if m2[key] is AnyType and m1[key] is not a union

                    // containing AnyType: use m1[key].
                    // Else: keep m2[key] (already in result).
                    if is_uninhabited_type(t1) {
                        let enc = match encode_type(t1) {
                            Some(b) => b,
                            None => return Ok(None),
                        };
                        let pos = key_pos[&h];
                        out_vals[pos] = enc;
                    } else if is_any_type(t2) && !is_union_containing_any(t1) {
                        let enc = match encode_type(t1) {
                            Some(b) => b,
                            None => return Ok(None),
                        };
                        let pos = key_pos[&h];
                        out_vals[pos] = enc;
                    }
                    // Else: keep m2[key] (already in result).
                }
            }
        }
    }

    Ok(Some((out_keys, out_vals)))
}

/// `mypy.checker.or_conditional_maps` (checker.py:9404-9427), Rust port.
///
/// Python passes parallel arrays of key hashes and serialized values for
/// both maps. Rust:
///   * Returns `m2` if `m1` is an unreachable map, `m1` if `m2` is
///     unreachable.
///   * For common keys: if `coalesce_any` and `m1[key]` is `AnyType`, use
///     `m1[key]`; else `make_simplified_union([m1[key], m2[key]])`.
///   * Only common keys appear in the result.
///
/// Returns `None` when any value fails to decode or `make_simplified_union`
/// defers.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_or_conditional_maps(
    keys1: Vec<i64>,
    values1: Vec<Vec<u8>>,
    keys2: Vec<i64>,
    values2: Vec<Vec<u8>>,
    coalesce_any: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Vec<i64>, Vec<Vec<u8>>)>> {
    if keys1.len() != values1.len() || keys2.len() != values2.len() {
        return Ok(None);
    }
    let r = resolver.resolver();

    // Decode all values.
    let mut decoded1: Vec<Type> = Vec::with_capacity(values1.len());
    for v in &values1 {
        decoded1.push(match decode_type(v) {
            Some(t) => t,
            None => return Ok(None),
        });
    }
    let mut decoded2: Vec<Type> = Vec::with_capacity(values2.len());
    for v in &values2 {
        decoded2.push(match decode_type(v) {
            Some(t) => t,
            None => return Ok(None),
        });
    }

    // is_unreachable_map: any value is UninhabitedType.
    if is_unreachable_map(&decoded1) {
        return Ok(Some((keys2, values2)));
    }
    if is_unreachable_map(&decoded2) {
        return Ok(Some((keys1, values1)));
    }

    // Build hash -> index map for m2.
    let mut m2_index: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for (i, &h) in keys2.iter().enumerate() {
        m2_index.insert(h, i);
    }

    let mut out_keys: Vec<i64> = Vec::new();
    let mut out_vals: Vec<Vec<u8>> = Vec::new();

    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);

    for (i, &h) in keys1.iter().enumerate() {
        if let Some(&j) = m2_index.get(&h) {
            let t1 = &decoded1[i];
            let t2 = &decoded2[j];
            let combined: Type = if coalesce_any && is_any_type(t1) {
                t1.clone()
            } else {
                let items = [t1.clone(), t2.clone()];
                match make_simplified_union(&items, &ctx, r, true, false) {
                    Some(t) => t,
                    None => return Ok(None),
                }
            };
            let enc = match encode_type(&combined) {
                Some(b) => b,
                None => return Ok(None),
            };
            out_keys.push(h);
            out_vals.push(enc);
        }
    }

    Ok(Some((out_keys, out_vals)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn inst() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn meet_result_any_is_special_form() {
        // meet.py mirror: Any-typed meet result is
        // AnyType(TypeOfAny.special_form), not unannotated (types.py:309).
        let r = meet_result_to_type(SetOpResult::Any, &inst(), &inst()).unwrap();
        match r {
            Type::AnyType {
                type_of_any,
                source_any,
                missing_import_name,
            } => {
                assert_eq!(type_of_any, 6);
                assert!(source_any.is_none());
                assert!(missing_import_name.is_none());
            }
            other => panic!("expected AnyType, got {other:?}"),
        }
    }

    #[test]
    fn reconstruct_disc4_is_special_form() {
        let out = reconstruct_args_from_discs(&[4], &[any()], &[any()]);
        match &out[0] {
            Type::AnyType { type_of_any, .. } => assert_eq!(*type_of_any, 6),
            other => panic!("expected AnyType, got {other:?}"),
        }
    }
}
