//! `_remove_redundant_union_items` (typeops.py:1089-1230), Rust port.
//!
//! Two-pass union dedup behind the `_native_typeops_active` gate. The
//! Python shim serializes the already flattened union items plus the
//! `keep_erased` flag; Rust returns the deduped item list as wire bytes
//! or `None` (defer to the pure-Python body).

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// Why a `find_subtype_index` scan produced no duplicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanOutcome {
    /// No earlier item is a proper supertype of the candidate.
    NoneFound,
    /// An earlier item is a proper supertype of the candidate.
    Found(usize),
    /// The subtype engine deferred (wire-unsupported form); the whole
    /// call must defer to Python to avoid a parity divergence.
    Deferred,
}

fn encode_type_list(items: &[Type]) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type_list(&mut buf, items).ok()?;
    Some(buf.into_bytes())
}

/// True when any `TypeAliasType` survives in the tree. Used as a cheap
/// pre-check to decide whether an item needs deep alias expansion before
/// the dedup scan (the scan itself defers on any surviving alias node).
fn tree_has_alias(t: &Type) -> bool {
    match t {
        Type::TypeAliasType { .. } => true,
        Type::Instance {
            args,
            last_known_value,
            extra_attrs,
            ..
        } => {
            args.iter().any(tree_has_alias)
                || last_known_value.as_deref().is_some_and(tree_has_alias)
                || extra_attrs
                    .as_ref()
                    .is_some_and(|e| e.attrs.values().any(tree_has_alias))
        }
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            values.iter().any(tree_has_alias)
                || tree_has_alias(upper_bound)
                || tree_has_alias(default)
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            upper_bound,
            default,
            ..
        } => {
            tree_has_alias(tuple_fallback) || tree_has_alias(upper_bound) || tree_has_alias(default)
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            prefix.arg_types.iter().any(tree_has_alias)
                || prefix.variables.iter().any(tree_has_alias)
                || tree_has_alias(upper_bound)
                || tree_has_alias(default)
        }
        Type::UnboundType { args, .. } => args.iter().any(tree_has_alias),
        Type::UnpackType { typ } => tree_has_alias(typ),
        Type::AnyType { source_any, .. } => source_any.as_deref().is_some_and(tree_has_alias),
        Type::CallableType {
            fallback,
            instance_type,
            arg_types,
            ret_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            tree_has_alias(fallback)
                || instance_type.as_deref().is_some_and(tree_has_alias)
                || arg_types.iter().any(tree_has_alias)
                || tree_has_alias(ret_type)
                || variables.iter().any(tree_has_alias)
                || type_guard.as_deref().is_some_and(tree_has_alias)
                || type_is.as_deref().is_some_and(tree_has_alias)
        }
        Type::Overloaded { items } => items.iter().any(tree_has_alias),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => tree_has_alias(partial_fallback) || items.iter().any(tree_has_alias),
        Type::TypedDictType {
            fallback, items, ..
        } => tree_has_alias(fallback) || items.iter().any(|(_, t)| tree_has_alias(t)),
        Type::LiteralType { fallback, .. } => tree_has_alias(fallback),
        Type::UnionType { items, .. } => items.iter().any(tree_has_alias),
        Type::TypeType { item, .. } => tree_has_alias(item),
        Type::Parameters(p) => {
            p.arg_types.iter().any(tree_has_alias) || p.variables.iter().any(tree_has_alias)
        }
        Type::UninhabitedType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::DeletedType { .. } => false,
    }
}

/// One pass of `_remove_redundant_union_items` (typeops.py:1399-1468):
/// scan `items`, build `new_items` dropping UninhabitedType,
/// exact-duplicates (via a seen map), and subtypes of earlier items.
/// Mirrors the pure-Python loop including the LiteralType fallback-set
pub(crate) fn remove_redundant_pass(
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    keep_erased: bool,
) -> Option<Vec<Type>> {
    Some(
        remove_redundant_pass_indices(items, ctx, resolver, keep_erased)?
            .into_iter()
            .map(|i| items[i].clone())
            .collect(),
    )
}

/// The same pass, returning the *indices* into `items` that survived.
/// The rru entry maps these onto the deep-expanded scan tree, which
/// becomes the alias-free, round-trip-safe output (the wire fixup
/// decode defers on a surviving `TypeAliasType`).
///
/// Python's truthiness-widening write (`true_or_false` on a duplicate's
/// survivor, typeops.py:1449-1455) is a content no-op here: the wire
/// seam only engages when every item has unmutated truthiness flags
/// (`_has_mutated_truthiness` gate in typeops.py), so resetting a
/// survivor to its default flags never changes content.
pub(crate) fn remove_redundant_pass_indices(
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    keep_erased: bool,
) -> Option<Vec<usize>> {
    // Survivors, as indices into `items` in output order. Since wire
    // `Type` is not `Hash`, resolve exact duplicates by a linear
    // equality scan (Python's `seen` dict: identical content membership).
    let mut surv: Vec<usize> = Vec::new();
    // unduplicated_literal_fallbacks: wire bytes of the fallback
    // Instances already added without a supertype found.
    let mut unduplicated_literal_fallbacks: Option<Vec<Vec<u8>>> = None;
    for (i, ti) in items.iter().enumerate() {
        // UninhabitedType is always redundant (typeops.py:1407-1408).
        if matches!(ti, Type::UninhabitedType { .. }) {
            continue;
        }
        // ErasedType: with keep_erased=False it is a proper subtype of
        // everything and always drops; with keep_erased=True it is
        // never a subtype and is added without a scan.
        if matches!(ti, Type::ErasedType) {
            if !keep_erased {
                continue;
            }
            surv.push(i);
            continue;
        }
        // Exact-duplicate fast path (typeops.py:1412-1414): Python keys
        // `seen` on structural equality; mirror it with a linear scan.
        let mut duplicate = surv.iter().position(|&k| items[k] == *ti);
        if duplicate.is_none() {
            // A LiteralType whose fallback already failed the subtype
            // scan is a known non-duplicate; skip the scan (typeops.py:
            // 1415-1427).
            let skip_scan = matches!(ti, Type::LiteralType { .. })
                && unduplicated_literal_fallbacks
                    .as_ref()
                    .is_some_and(|fbs| encode_fallback_key(ti).is_some_and(|k| fbs.contains(&k)));
            if !skip_scan {
                match find_subtype_index(items, &surv, ti, ctx, resolver) {
                    ScanOutcome::Found(j) => duplicate = Some(j),
                    ScanOutcome::NoneFound => {}
                    ScanOutcome::Deferred => return None,
                }
            }
        }
        if duplicate.is_some() {
            continue;
        }
        surv.push(i);
        if let Type::LiteralType { .. } = ti {
            let key = encode_fallback_key(ti)?;
            if unduplicated_literal_fallbacks.is_none() {
                unduplicated_literal_fallbacks = Some(Vec::new());
            }
            unduplicated_literal_fallbacks.as_mut().unwrap().push(key);
        }
    }
    Some(surv)
}

/// Serialize a LiteralType's fallback Instance to a stable byte key.
fn encode_fallback_key(t: &Type) -> Option<Vec<u8>> {
    let Type::LiteralType { fallback, .. } = t else {
        return None;
    };
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, fallback).ok()?;
    Some(buf.into_bytes())
}

/// The subtype scan against the surviving items (typeops.py:1429-1448).
/// Returns the position in `surv` of the first survivor that is a proper
/// supertype of `ti`, skipping any survivor with a `last_known_value`
/// differing from `ti`'s. Deferred when the subtype engine cannot decide
/// a pair.
fn find_subtype_index(
    items: &[Type],
    surv: &[usize],
    ti: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> ScanOutcome {
    for (j, &k) in surv.iter().enumerate() {
        let tj = &items[k];
        // An ErasedType item is never a supertype (subtypes.py
        // visit_erased_type returns False unless keep_erased_types).
        if matches!(tj, Type::ErasedType) {
            continue;
        }
        if let Type::Instance {
            last_known_value: Some(tj_lkv),
            ..
        } = tj
        {
            if let Type::Instance {
                last_known_value: Some(ti_lkv),
                ..
            } = ti
            {
                // A previous instance carrying a differing literal value
                // is not a supertype of this item (typeops.py:1432-1442).
                if ti_lkv != tj_lkv {
                    continue;
                }
            }
        }
        match is_subtype(ti, tj, ctx, resolver) {
            Some(true) => return ScanOutcome::Found(j),
            Some(false) => {}
            None => return ScanOutcome::Deferred,
        }
    }
    ScanOutcome::NoneFound
}

/// `#[pyfunction]` entry for `_remove_redundant_union_items`. Takes
/// serialized items (LIST_GEN-tagged list of types) + `keep_erased`
/// + `strict_optional` + the native resolver. Returns the deduped item
/// list as wire bytes or `None` (defer).
///
/// Uses the exact `is_proper_subtype(ignore_promotions=True,
/// keep_erased_types=...)` context the Python body passes.
#[pyfunction]
pub(crate) fn rust_remove_redundant_union_items(
    items_bytes: &[u8],
    keep_erased: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let mut buf = ReadBuffer::new(items_bytes);
    let items = match wire::read_type_list(&mut buf) {
        Ok(items) => items,
        Err(_) => return None,
    };
    // Alias-backed items dedup as their expanded target (typeops.py:1405):
    // root-expand, scan deep-expanded copies, output deep-expanded survivors,
    // the #774 contract extended to nested aliases (`str(aliastyp)` == target).
    let mut current = Vec::with_capacity(items.len());
    for ti in items {
        let root = match ti {
            Type::TypeAliasType { .. } => {
                crate::checkexpr_functions::expand_alias_target_raw(&ti, resolver.alias_resolver())?
            }
            _ => ti,
        };
        if tree_has_alias(&root) {
            let deep =
                crate::subtypes::expand_aliases(&root, resolver.alias_resolver(), strict_optional)?;
            current.push(deep);
        } else {
            current.push(root);
        }
    }
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    for _direction in 0..2 {
        let surv = remove_redundant_pass_indices(&current, &ctx, resolver.resolver(), keep_erased)?;
        current = surv.iter().map(|&i| current[i].clone()).collect();
        if current.len() <= 1 {
            break;
        }
        current.reverse();
    }
    // An alias the snapshot could not fully expand (missing, variadic,
    // substitution wall) may survive both passes: prefer the shim's
    // fallback over bytes the Python side cannot decode.
    if current.iter().any(tree_has_alias) {
        return None;
    }
    encode_type_list(&current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::LiteralValue;

    fn snap(fullname: &str, name: &str) -> crate::typeinfo::TypeInfoSnapshot {
        let mut s = crate::typeinfo::TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s
    }

    fn test_resolver(snaps: Vec<crate::typeinfo::TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn literal(value: i64, fallback: Type) -> Type {
        Type::LiteralType {
            fallback: Box::new(fallback),
            value: LiteralValue::Int(value),
        }
    }

    fn dedup(items: &[Type], keep_erased: bool) -> Option<Vec<Type>> {
        let mut current = items.to_vec();
        let ctx = SubtypeContext::new(false, false, false, true, true, true);
        let r = test_resolver(vec![
            snap("builtins.object", "object"),
            snap("builtins.int", "int"),
        ]);
        for _direction in 0..2 {
            current = remove_redundant_pass(&current, &ctx, &r, keep_erased)?;
            if current.len() <= 1 {
                break;
            }
            current.reverse();
        }
        Some(current)
    }

    #[test]
    fn exact_duplicate_dropped() {
        let a = instance("builtins.int", vec![]);
        let out = dedup(&[a.clone(), a.clone()], false).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn uninhabited_always_dropped() {
        let a = instance("builtins.int", vec![]);
        let n = Type::UninhabitedType { ambiguous: false };
        let out = dedup(&[a.clone(), n.clone(), a], false).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn subtype_removed_int_vs_object() {
        let mut i = snap("builtins.int", "int");
        i.has_base.insert("builtins.object".to_string());
        i.mro.push("builtins.object".to_string());
        let r = test_resolver(vec![snap("builtins.object", "object"), i]);
        let ctx = SubtypeContext::new(false, false, false, true, true, true);
        let items = vec![
            instance("builtins.int", vec![]),
            instance("builtins.object", vec![]),
        ];
        // Pass 1 keeps both (object is not a subtype of int); pass 2
        // (reversed: object, int) drops int as a subtype of object.
        let mut current = items;
        current = remove_redundant_pass(&current, &ctx, &r, false).unwrap();
        assert_eq!(current.len(), 2);
        current.reverse();
        current = remove_redundant_pass(&current, &ctx, &r, false).unwrap();
        assert_eq!(current.len(), 1);
        assert!(matches!(
            current[0],
            Type::Instance { ref type_ref, .. } if type_ref == "builtins.object"
        ));
    }

    #[test]
    fn literal_dup_same_fallback() {
        let fb = instance("builtins.int", vec![]);
        let l1 = literal(1, fb.clone());
        let l2 = literal(2, fb);
        let out = dedup(&[l1, l2], false).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn erased_dropped_when_not_kept() {
        let a = instance("builtins.int", vec![]);
        let e = Type::ErasedType;
        let out = dedup(&[a.clone(), e.clone()], false).unwrap();
        assert_eq!(out.len(), 1);
        let kept = dedup(&[a, e], true).unwrap();
        assert_eq!(kept.len(), 2);
    }
}
