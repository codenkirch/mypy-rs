//! `_remove_redundant_union_items` (typeops.py:1089-1230), Rust port.
//!
//! Two-pass union dedup behind the `_native_typeops_active` gate. The
//! Python shim serializes the already flattened union items plus the
//! `keep_erased` flag; Rust returns the deduped item list as wire bytes
//! or `None` (defer to the pure-Python body).

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

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

/// True when any `TypeAliasType` survives in the tree. The pass re-encodes
/// its result as wire bytes, and the Python side's wire fixup defers on a
/// decoded alias, so a surviving alias makes the whole result undecodable.
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

/// `Type.can_be_true_default()` for the wire variants, for the
/// truthiness merge in `remove_redundant_pass`. Mirrors
/// `types.py:295-3459` restricted to the forms that can appear as
/// flattened union items. Enum literals, tuples, aliases, and the
/// numeric builtins defer (their defaults need live `TypeInfo` reads).
fn wire_can_be_true_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(false),
        Type::LiteralType { value, fallback } => {
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(true);
            };
            // Enum literals need the snapshot's is_enum + fallback
            // truthiness; defer rather than guess.
            if type_ref.starts_with("builtins.") {
                Some(match value {
                    LiteralValue::Int(v) => *v != 0,
                    LiteralValue::Str(v) => !v.is_empty(),
                    LiteralValue::Bool(v) => *v,
                    LiteralValue::Bytes(v) => !v.is_empty(),
                    LiteralValue::Float(v) => *v != 0.0,
                })
            } else {
                None
            }
        }
        Type::UnionType { items, .. } => {
            for item in items {
                match wire_can_be_true_default(item) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(false)
        }
        _ => Some(true),
    }
}

/// `Type.can_be_false_default()` for the wire variants; see
/// `wire_can_be_true_default` for the deferral policy.
fn wire_can_be_false_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(true),
        Type::LiteralType { value, fallback } => {
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(true);
            };
            if type_ref.starts_with("builtins.") {
                Some(match value {
                    LiteralValue::Int(v) => *v == 0,
                    LiteralValue::Str(v) => v.is_empty(),
                    LiteralValue::Bool(v) => !*v,
                    LiteralValue::Bytes(v) => v.is_empty(),
                    LiteralValue::Float(v) => *v == 0.0,
                })
            } else {
                None
            }
        }
        Type::UnionType { items, .. } => {
            for item in items {
                match wire_can_be_false_default(item) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(false)
        }
        _ => Some(true),
    }
}

/// `true_or_false(t)` (typeops.py:1336-1358): widen a type so both
/// truthiness values are possible. Python's `copy_type` + default flag
/// reset is a no-op on the wire (defaults are not carried).
fn true_or_false(t: &Type) -> Type {
    t.clone()
}

/// One pass of `_remove_redundant_union_items` (typeops.py:1098-1167):
/// scan `items`, build `new_items` dropping UninhabitedType,
/// exact-duplicates (via a seen map), and subtypes of earlier items.
/// Mirrors the pure-Python loop including the LiteralType fallback-set
/// optimization and the last_known_value guard.
pub(crate) fn remove_redundant_pass(
    items: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    keep_erased: bool,
) -> Option<Vec<Type>> {
    let mut new_items: Vec<Type> = Vec::new();
    // seen maps a type to its index in new_items. Since wire `Type` is
    // not `Hash`, resolve exact duplicates by a linear equality scan;
    // Python's `seen` dict does the same membership test.
    let mut seen: Vec<(Type, usize)> = Vec::new();
    // unduplicated_literal_fallbacks: wire bytes of the fallback
    // Instances already added without a supertype found.
    let mut unduplicated_literal_fallbacks: Option<Vec<Vec<u8>>> = None;
    for ti in items {
        // UninhabitedType is always redundant (typeops.py:1107-1108).
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
            seen.push((ti.clone(), new_items.len()));
            new_items.push(ti.clone());
            continue;
        }
        let mut duplicate_index: Option<usize> = None;
        // Exact-duplicate fast path (typeops.py:1112-1113): Python keys
        // `seen` on structural equality; mirror it with a linear scan.
        let exact_dup = seen.iter().any(|(t, _)| t == ti);
        if exact_dup {
            duplicate_index = seen.iter().find(|(t, _)| t == ti).map(|(_, j)| *j);
        } else {
            // A LiteralType whose fallback already failed the subtype
            // scan is a known non-duplicate; skip the scan (typeops.py:
            // 1114-1126).
            let skip_scan = matches!(ti, Type::LiteralType { .. })
                && unduplicated_literal_fallbacks
                    .as_ref()
                    .is_some_and(|fbs| encode_fallback_key(ti).is_some_and(|k| fbs.contains(&k)));
            if !skip_scan {
                match find_subtype_index(&new_items, ti, ctx, resolver) {
                    ScanOutcome::Found(j) => duplicate_index = Some(j),
                    ScanOutcome::NoneFound => {}
                    ScanOutcome::Deferred => return None,
                }
            }
        }
        match duplicate_index {
            Some(j) => {
                // If the deleted subtype had more general truthiness,
                // widen the surviving item (typeops.py:1148-1154).
                let orig = &new_items[j];
                let orig_can_be_true = wire_can_be_true_default(orig);
                let ti_can_be_true = wire_can_be_true_default(ti);
                let orig_can_be_false = wire_can_be_false_default(orig);
                let ti_can_be_false = wire_can_be_false_default(ti);
                if (orig_can_be_true == Some(false) && ti_can_be_true == Some(true))
                    || (orig_can_be_false == Some(false) && ti_can_be_false == Some(true))
                {
                    new_items[j] = true_or_false(orig);
                }
            }
            None => {
                seen.push((ti.clone(), new_items.len()));
                new_items.push(ti.clone());
                if let Type::LiteralType { .. } = ti {
                    let key = encode_fallback_key(ti)?;
                    if unduplicated_literal_fallbacks.is_none() {
                        unduplicated_literal_fallbacks = Some(Vec::new());
                    }
                    unduplicated_literal_fallbacks.as_mut().unwrap().push(key);
                }
            }
        }
    }
    Some(new_items)
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

/// The subtype scan against `new_items` (typeops.py:1128-1147).
/// Returns the index of the first earlier item that is a proper
/// supertype of `ti`, skipping any previous item that has a
/// `last_known_value` differing from `ti`'s. Deferred when the
/// subtype engine cannot decide a pair.
fn find_subtype_index(
    new_items: &[Type],
    ti: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> ScanOutcome {
    for (j, tj) in new_items.iter().enumerate() {
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
                // is not a supertype of this item (typeops.py:1131-1141).
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
    // Python computes `proper_ti = get_proper_type(ti)` per item
    // (typeops.py:1148): an alias-backed item dedups as its expanded
    // target. Expand alias items up front (raw target); defer on `?`.
    let mut current = Vec::with_capacity(items.len());
    for ti in items {
        match ti {
            Type::TypeAliasType { .. } => {
                let target = crate::checkexpr_functions::expand_alias_target_raw(
                    &ti,
                    resolver.alias_resolver(),
                )?;
                current.push(target);
            }
            _ => {
                // Python only expands alias roots per item, so a nested
                // alias may survive here; the sweep below fixes that.
                current.push(ti);
            }
        }
    }
    // Post-root-expansion sweep, covering chain aliases whose raw target
    // still carries an alias node.
    for ti in &current {
        if tree_has_alias(ti) {
            return None;
        }
    }
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    for _direction in 0..2 {
        current = remove_redundant_pass(&current, &ctx, resolver.resolver(), keep_erased)?;
        if current.len() <= 1 {
            break;
        }
        current.reverse();
    }
    encode_type_list(&current)
}

#[cfg(test)]
mod tests {
    use super::*;

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
