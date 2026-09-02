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
//! The Rust port dispatches on the wire-format `Type` enum and speaks the
//! #1101 decided-None protocol (`(decided, value)`): a large share of the
//! self-check calls land on shapes whose Python answer IS `None` (non-
//! container instances, unparameterized containers, `Any` first args,
//! tuples with `Any` items, TypedDicts without a `Mapping` base, and any
//! non-Instance/Tuple/TypedDict type). Those are decided-None, not
//! deferrals — reporting them as defers forced the pure-Python body to
//! re-run (~224 fallbacks @ 84% native on the self-check corpus, #1297).
//!
//! Decided positive cases (`decided=true, value=Some(bytes)`):
//! - `Instance` of one of the 7 builtin containers with a non-empty arg
//!   list whose first arg is not `Any` -> that first arg.
//! - `TupleType` -> normalize `UnpackType` items (a `TypeVarTupleType`
//!   unpacks through its `upper_bound`), and if none of the normalized
//!   items is `Any`, return `make_simplified_union(normalized)`.
//! - `TypedDictType` -> walk `fallback.type.mro` for `typing.Mapping`
//!   and return `map_instance_to_supertype(fallback, base).args[0]`, the
//!   key type.
//!
//! Decided-None cases (`decided=true, value=None`): every shape above
//! whose Python answer is `None`, plus any other type (the Python body
//! falls off the end).
//!
//! Deferred (`decided=false` -> the pure-Python path re-runs): a
//! `TypeAliasType` at any position where `get_proper_type` expansion is
//! needed (the wire cannot resolve live aliases or snapshots), an
//! `UnpackType` that does not normalize to a `builtins.tuple` instance
//! (Python asserts there), a missing resolver snapshot (mro / bases /
//! derivation path), a `make_simplified_union` /
//! `map_instance_to_supertype` deferral, or a wire decode/encode
//! failure. See docs/swarm-candidates-2026-08-19.md, Candidate 3.

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
/// Returns `Some(Some(item_type))` when the element type is found,
/// `Some(None)` when Python's answer is `None` (decided-None), and
/// `None` to defer to the pure-Python body.
fn builtin_item_type_inner(
    t: &Type,
    resolver: &NativeTypeResolver,
    strict_optional: bool,
) -> Option<Option<Type>> {
    let r = resolver.resolver();
    match t {
        // Instance: one of the 7 builtin containers with a non-empty args
        // list and a first arg that is not Any -> that arg (checker.py:
        // 9497-9511). Not-a-container / empty-args / Any are decided-None
        // (Python's answer is None in all three); a TypeAliasType first
        // arg defers (get_proper_type needs the live alias).
        Type::Instance { type_ref, args, .. } => {
            if !BUILTIN_CONTAINERS.contains(&type_ref.as_str()) {
                return Some(None);
            }
            let first = match args.first() {
                Some(f) => f,
                // Unparameterized container: Python returns None.
                None => return Some(None),
            };
            let proper = get_proper_or_defer(first)?;
            if is_any(proper) {
                return Some(None);
            }
            Some(Some(first.clone()))
        }
        // TupleType: normalize unpacks, then make_simplified_union of the
        // items if none is Any (checker.py:9512-9527). An Any item is
        // decided-None; a TypeAliasType item defers (get_proper_types
        // would expand it from the live alias).
        Type::TupleType { items, .. } => {
            let mut normalized = Vec::with_capacity(items.len());
            for it in items {
                let item = match it {
                    Type::UnpackType { typ, .. } => {
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
                    return Some(None);
                }
            }
            let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&normalized, &ctx, r, true, false).map(Some)
        }
        // TypedDictType: the key type of the Mapping base found in the
        // fallback's mro (checker.py:9528-9534). No Mapping base in the
        // mro is decided-None (the Python loop falls through to None).
        Type::TypedDictType { fallback, .. } => {
            let Type::Instance { type_ref, args, .. } = fallback.as_ref() else {
                return None;
            };
            let snap = r.get(type_ref)?;
            match snap.mro.iter().find(|b| *b == "typing.Mapping") {
                Some(mapping) => map_instance_to_supertype(type_ref, args, mapping, r)
                    .and_then(|mapped| mapped.first().cloned())
                    .map(Some),
                None => Some(None),
            }
        }
        // Any other type: the Python body falls off the end -> None.
        _ => Some(None),
    }
}

/// Native `rust_builtin_item_type(t_bytes, strict_optional, resolver)` —
/// parity seam for `mypy.checker.builtin_item_type`.
///
/// #1101 decided-None protocol: returns `(decided, bytes_or_None)`.
/// `(true, Some(bytes))` is the element type, `(true, None)` is a decided
/// no-result (Python answers None), `(false, _)` defers to the
/// pure-Python body.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_builtin_item_type(
    t_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<(bool, Option<Vec<u8>>)> {
    let t = decode_type(t_bytes)?;
    let result = builtin_item_type_inner(&t, resolver, strict_optional)?;
    Some(match result {
        Some(typ) => {
            let bytes = encode_type(&typ)?;
            (true, Some(bytes))
        }
        None => (true, None),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_snapshot(fullname: &str) -> crate::typeinfo::TypeInfoSnapshot {
        use std::collections::HashSet;
        crate::typeinfo::TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_string(),
            mro: vec![fullname.to_string()],
            has_base: HashSet::from([fullname.to_string()]),
            ..Default::default()
        }
    }

    fn make_resolver() -> NativeTypeResolver {
        let mut r = crate::typeinfo::TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        NativeTypeResolver::new(r, crate::aliases::TypeAliasResolver::new())
    }

    #[test]
    fn test_container_instance_decides_item() {
        let r = make_resolver();
        let t = make_instance("builtins.list", vec![make_instance("builtins.int", vec![])]);
        let Type::Instance { args, .. } = &t else {
            unreachable!()
        };
        assert_eq!(
            builtin_item_type_inner(&t, &r, true),
            Some(Some(args[0].clone()))
        );
    }

    #[test]
    fn test_non_container_decided_none() {
        let r = make_resolver();
        let t = make_instance("mymod.Custom", vec![]);
        assert_eq!(builtin_item_type_inner(&t, &r, true), Some(None));
    }

    #[test]
    fn test_unparameterized_container_decided_none() {
        let r = make_resolver();
        let t = make_instance("builtins.dict", vec![]);
        assert_eq!(builtin_item_type_inner(&t, &r, true), Some(None));
    }

    #[test]
    fn test_any_first_arg_decided_none() {
        let r = make_resolver();
        let t = make_instance("builtins.list", vec![any()]);
        assert_eq!(builtin_item_type_inner(&t, &r, true), Some(None));
    }

    #[test]
    fn test_alias_first_arg_defers() {
        let r = make_resolver();
        let alias = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mymod.Alias".to_string(),
            is_recursive: false,
        };
        let t = make_instance("builtins.list", vec![alias]);
        assert_eq!(builtin_item_type_inner(&t, &r, true), None);
    }

    #[test]
    fn test_tuple_with_any_item_decided_none() {
        let r = make_resolver();
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("builtins.int", vec![]), any()],
            implicit: false,
        };
        assert_eq!(builtin_item_type_inner(&t, &r, true), Some(None));
    }

    #[test]
    fn test_other_type_decided_none() {
        let r = make_resolver();
        assert_eq!(builtin_item_type_inner(&any(), &r, true), Some(None));
        assert_eq!(
            builtin_item_type_inner(&Type::NoneType, &r, true),
            Some(None)
        );
    }

    #[test]
    fn test_typeddict_without_mapping_decided_none() {
        let mut r = crate::typeinfo::TypeResolver::new();
        r.insert("mymod.TD".to_string(), make_snapshot("mymod.TD"));
        let r = NativeTypeResolver::new(r, crate::aliases::TypeAliasResolver::new());
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("mymod.TD", vec![])),
            items: Vec::new(),
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(builtin_item_type_inner(&t, &r, true), Some(None));
    }
}
