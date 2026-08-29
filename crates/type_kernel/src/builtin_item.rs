//! Parity for `mypy.checker.builtin_item_type` (checker.py:9481-9535).
//!
//! Gets the element type of a builtin container when the membership
//! (`in`) behavior of its `__contains__` is known, so optional types can
//! be narrowed:
//!
//! ```text
//! x: Optional[int]
//! if x in (1, 2, 3):
//!     x + 42  # OK
//! ```
//!
//! The Rust port dispatches on the wire-format `Type` enum and claims
//! only the positive cases (where there IS an element type); every other
//! case defers (`None`) so the pure-Python path produces the answer (a
//! value or `None`):
//!
//! - `Instance` of one of the 7 builtin containers with a non-empty arg
//!   list whose first arg is not `Any` -> that first arg.
//! - `TupleType` -> normalize `UnpackType` items (a `TypeVarTupleType`
//!   unpacks through its `upper_bound`), and if none of the normalized
//!   items is `Any`, return `make_simplified_union(normalized)`.
//! - `TypedDictType` -> walk `fallback.type.mro` for `typing.Mapping`
//!   and return `map_instance_to_supertype(fallback, base).args[0]`, the
//!   key type.
//!
//! Deferred (`None` -> the pure-Python path runs): any type that is not a
//! builtin container (the answer is `None`), a `TypeAliasType` at any
//! position where `get_proper_type` expansion would be needed (the wire
//! cannot resolve live aliases or snapshots), an `UnpackType` that does
//! not normalize to a `builtins.tuple` instance (Python asserts there),
//! a missing resolver snapshot (mro / bases / derivation path), or a
//! `make_simplified_union` / `map_instance_to_supertype` deferral. Each
//! deferral reproduces the Python result via the fallback, so parity is
//! preserved. See docs/swarm-candidates-2026-08-19.md, Candidate 3.

use pyo3::prelude::*;

use crate::setops::make_simplified_union;
use crate::subtypes::{map_instance_to_supertype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// The 7 builtin container fullnames whose `__contains__` semantics are
/// known (checker.py:9498-9506).
const BUILTIN_CONTAINERS: &[&str] = &[
    "builtins.list",
    "builtins.tuple",
    "builtins.dict",
    "builtins.set",
    "builtins.frozenset",
    "_collections_abc.dict_keys",
    "typing.KeysView",
];

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Wire `get_proper_type`: `TypeAliasType` cannot be expanded (live alias
/// resolution is Python-only), so defer; every other variant is proper.
fn get_proper_or_defer(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        t => Some(t),
    }
}

/// Wire `isinstance(it, AnyType)` (checker.py:9526).
fn is_any(typ: &Type) -> bool {
    matches!(typ, Type::AnyType { .. })
}

/// Port of `checker.builtin_item_type` on the wire `Type` enum.
///
/// Returns `Some(item_type)` when the element type is found, and `None`
/// to defer to Python (the answer is `None`, or the wire path cannot
/// decide).
fn builtin_item_type_inner(
    t: &Type,
    resolver: &NativeTypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    let r = resolver.resolver();
    match t {
        // Instance: one of the 7 builtin containers with a non-empty args
        // list and a first arg that is not Any -> that arg (checker.py:
        // 9497-9511). Not-a-container / empty-args / Any all defer.
        Type::Instance { type_ref, args, .. } => {
            if !BUILTIN_CONTAINERS.contains(&type_ref.as_str()) {
                return None;
            }
            let first = args.first()?;
            let proper = get_proper_or_defer(first)?;
            if is_any(proper) {
                return None;
            }
            Some(first.clone())
        }
        // TupleType: normalize unpacks, then make_simplified_union of the
        // items if none is Any (checker.py:9512-9527).
        Type::TupleType { items, .. } => {
            let mut normalized = Vec::with_capacity(items.len());
            for it in items {
                let item = match it {
                    Type::UnpackType { typ } => {
                        let unpacked = get_proper_or_defer(typ)?;
                        // TypeVarTuple unpacks through its upper_bound.
                        let unpacked = match unpacked {
                            Type::TypeVarTupleType { upper_bound, .. } => {
                                get_proper_or_defer(upper_bound)?
                            }
                            other => other,
                        };
                        // Python asserts a builtins.tuple instance here;
                        // anything else defers (the shim also catches a
                        // Python AssertionError along the same path).
                        let Type::Instance { type_ref, args, .. } = unpacked else {
                            return None;
                        };
                        if type_ref != "builtins.tuple" {
                            return None;
                        }
                        args.first()?.clone()
                    }
                    other => other.clone(),
                };
                normalized.push(item);
            }
            for it in &normalized {
                let proper = get_proper_or_defer(it)?;
                if is_any(proper) {
                    return None;
                }
            }
            let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&normalized, &ctx, r, true, false)
        }
        // TypedDictType: the key type of the Mapping base found in the
        // fallback's mro (checker.py:9528-9534).
        Type::TypedDictType { fallback, .. } => {
            let Type::Instance { type_ref, args, .. } = fallback.as_ref() else {
                return None;
            };
            let snap = r.get(type_ref)?;
            let mapping = snap.mro.iter().find(|b| *b == "typing.Mapping")?;
            let mapped = map_instance_to_supertype(type_ref, args, mapping, r)?;
            mapped.first().cloned()
        }
        _ => None,
    }
}

/// Native `rust_builtin_item_type(t_bytes, strict_optional, resolver)` —
/// parity seam for `mypy.checker.builtin_item_type`.
///
/// Returns encoded single-`Type` bytes for the element type, or `None` to
/// defer to Python.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_builtin_item_type(
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let result = builtin_item_type_inner(&t, resolver, strict_optional)?;
    encode_type(&result)
}
