//! Issue #749: `find_type_overlaps` (mypy/messages.py:3055-3079) to Rust.
//!
//! Mirrors the pure-Python body: collect all named types (Instances,
//! TypeAliasTypes, TypeVarLikes) reachable from each input type, group them
//! by short name, inject `typing.<shortname>` for names in
//! `TYPES_FOR_UNIMPORTED_HINTS`, and return the union of fullnames of every
//! group holding more than one distinct entry.
//!
//! Wire-computable: the inputs arrive as serialized `Type` blobs, and the
//! fullname / short-name / namespace facts all live on the wire
//! (`type_ref`, `name`, `namespace`). No live node reads are needed, so the
//! seam takes only the serialized blobs.
//!
//! The traversal mirrors `mypy/typetraverser.py` (`TypeTraverserVisitor`)
//! exactly, including its asymmetries:
//! - `CallableType.variables` walk `upper_bound` always, plus `values`
//!   only for `TypeVarType` (defaults are not traversed there).
//! - Standalone `TypeVarType` / `ParamSpecType` / `TypeVarTupleType`
//!   traverse only their `default`.
//! - `CallableType.fallback`, `instance_type`, `type_guard`, `type_is` are
//!   traversed; `Parameters` traverses only `arg_types`.
//!
//! Defer (None) on `TypeAliasType`: the Python body expands non-recursive
//! aliases via the live `alias.target` (`messages.py:3028-3029`) and asserts
//! `alias is not None`, but the wire carries only the unresolved `type_ref`
//! and no `is_recursive` flag. Rust declines so Python runs unmodified.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type};

/// `TYPES_FOR_UNIMPORTED_HINTS` (mypy/messages.py:139-152): short names that
/// always print with their `typing.` prefix when they collide.
const TYPES_FOR_UNIMPORTED_HINTS: &[&str] = &[
    "typing.Any",
    "typing.Callable",
    "typing.Dict",
    "typing.Iterable",
    "typing.Iterator",
    "typing.List",
    "typing.Optional",
    "typing.Set",
    "typing.Tuple",
    "typing.TypeVar",
    "typing.Union",
    "typing.cast",
];

/// Native `find_type_overlaps(*types) -> list[str] | None`.
///
/// Input: `type_bytes_list`, one serialized `mypy.types.Type` blob per
/// argument. Output: the overlapping fullnames (as a list; the Python seam
/// wraps it in a set). Returns `None` when a child type cannot be traversed
/// from the wire, so the Python caller runs the pure body.
#[pyfunction]
pub(crate) fn rust_find_type_overlaps(
    type_bytes_list: Vec<Vec<u8>>,
) -> PyResult<Option<Vec<String>>> {
    let mut d: HashMap<String, HashSet<String>> = HashMap::new();
    for bytes in &type_bytes_list {
        let Ok(t) = read_type(&mut ReadBuffer::new(bytes), None) else {
            return Ok(None);
        };
        if collect_named_types(&t, &mut d).is_err() {
            return Ok(None);
        }
    }

    // `if f"typing.{shortname}" in TYPES_FOR_UNIMPORTED_HINTS` — add the
    // typing fullname to the short-name group so its collision is detected.
    let typing_additions: Vec<(String, String)> = d
        .keys()
        .filter_map(|shortname| {
            let full = format!("typing.{shortname}");
            if TYPES_FOR_UNIMPORTED_HINTS.contains(&full.as_str()) {
                Some((shortname.clone(), full))
            } else {
                None
            }
        })
        .collect();
    for (shortname, fullname) in typing_additions {
        d.get_mut(&shortname).unwrap().insert(fullname);
    }

    let mut overlaps: Vec<String> = Vec::new();
    for fullnames in d.values() {
        if fullnames.len() > 1 {
            overlaps.extend(fullnames.iter().cloned());
        }
    }
    // Deterministic order for the parity differential; the seam compares
    // sets, so order is not observable to callers.
    overlaps.sort();
    Ok(Some(overlaps))
}

/// `scoped_type_var_name` (mypy/messages.py:3047-3052): `name@namespace`
/// suffix unless the namespace is empty.
fn scoped_type_var_name(name: &str, namespace: &str) -> String {
    if namespace.is_empty() {
        name.to_string()
    } else {
        let suffix = namespace.rsplit('.').next().unwrap_or(namespace);
        format!("{name}@{suffix}")
    }
}

/// Collect all named types reachable from `t` into `d` (short name ->
/// fullnames), mirroring `CollectAllNamedTypesQuery`
/// (messages.py:3019-3044) over the `TypeTraverserVisitor` component walk
/// (typetraverser.py:40-166). Defers (Err) on `TypeAliasType`.
fn collect_named_types(t: &Type, d: &mut HashMap<String, HashSet<String>>) -> Result<(), ()> {
    match t {
        Type::Instance { type_ref, args, .. } => {
            let short_name = type_ref.rsplit('.').next().unwrap_or(type_ref);
            d.entry(short_name.to_string())
                .or_default()
                .insert(type_ref.clone());
            for a in args {
                collect_named_types(a, d)?;
            }
        }
        // Python expands non-recursive aliases via the live `alias.target`;
        // the wire has no alias node or `is_recursive` — defer.
        Type::TypeAliasType { .. } => return Err(()),
        Type::TypeVarType {
            name,
            namespace,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            collect_named_types(default, d)?;
        }
        Type::ParamSpecType {
            name,
            namespace,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            collect_named_types(default, d)?;
        }
        Type::TypeVarTupleType {
            name,
            namespace,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            collect_named_types(default, d)?;
        }
        Type::CallableType {
            arg_types,
            ret_type,
            fallback,
            variables,
            type_guard,
            type_is,
            instance_type,
            ..
        } => {
            for tv in variables {
                collect_var_like(tv, d)?;
            }
            for a in arg_types {
                collect_named_types(a, d)?;
            }
            collect_named_types(ret_type, d)?;
            collect_named_types(fallback, d)?;
            if let Some(tg) = type_guard {
                collect_named_types(tg, d)?;
            }
            if let Some(ti) = type_is {
                collect_named_types(ti, d)?;
            }
            if let Some(it) = instance_type {
                collect_named_types(it, d)?;
            }
        }
        Type::Overloaded { items } => {
            for i in items {
                collect_named_types(i, d)?;
            }
        }
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            for i in items {
                collect_named_types(i, d)?;
            }
            collect_named_types(partial_fallback, d)?;
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            for (_, t) in items {
                collect_named_types(t, d)?;
            }
            collect_named_types(fallback, d)?;
        }
        Type::UnionType { items, .. } => {
            for i in items {
                collect_named_types(i, d)?;
            }
        }
        Type::TypeType { item, .. } => {
            collect_named_types(item, d)?;
        }
        Type::UnpackType { typ, .. } => {
            collect_named_types(typ, d)?;
        }
        Type::LiteralType { fallback, .. } => {
            collect_named_types(fallback, d)?;
        }
        Type::Parameters(p) => {
            for a in &p.arg_types {
                collect_named_types(a, d)?;
            }
        }
        Type::UnboundType { args, .. } => {
            for a in args {
                collect_named_types(a, d)?;
            }
        }
        // Atomic leaves: the visitor's `visit_*` pushes nothing and has no
        // children to walk.
        Type::AnyType { .. }
        | Type::UninhabitedType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::DeletedType { .. } => {}
    }
    Ok(())
}

/// `visit_callable_type`'s variables loop (typetraverser.py:85-90):
/// `upper_bound` always, plus `values` for `TypeVarType` only.
fn collect_var_like(tv: &Type, d: &mut HashMap<String, HashSet<String>>) -> Result<(), ()> {
    match tv {
        Type::TypeVarType {
            upper_bound,
            values,
            ..
        } => {
            collect_named_types(upper_bound, d)?;
            for v in values {
                collect_named_types(v, d)?;
            }
        }
        Type::ParamSpecType { upper_bound, .. } | Type::TypeVarTupleType { upper_bound, .. } => {
            collect_named_types(upper_bound, d)?;
        }
        // Callable variables are always TypeVarLikes on the wire.
        _ => return Err(()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: false,
            can_be_false: false,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn type_var(name: &str, namespace: &str) -> Type {
        Type::TypeVarType {
            name: name.into(),
            fullname: name.into(),
            raw_id: 0,
            namespace: namespace.into(),
            values: vec![],
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn overlaps(types: &[Type]) -> HashSet<String> {
        let mut d: HashMap<String, HashSet<String>> = HashMap::new();
        for t in types {
            collect_named_types(t, &mut d).unwrap();
        }
        let mut out = HashSet::new();
        for fullnames in d.values() {
            if fullnames.len() > 1 {
                out.extend(fullnames.iter().cloned());
            }
        }
        out
    }

    #[test]
    fn test_no_overlap() {
        // Two disjoint fullnames → empty set.
        assert_eq!(
            overlaps(&[instance("a.A", vec![]), instance("b.B", vec![])]),
            HashSet::new()
        );
    }

    #[test]
    fn test_override_overlap() {
        // Same short name A from two modules → both fullnames.
        let got = overlaps(&[instance("a.A", vec![]), instance("b.A", vec![])]);
        let want: HashSet<String> = ["a.A".into(), "b.A".into()].into_iter().collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_shared_name_three_types() {
        // a.A overrides b.A; c.A keeps a distinct group.
        let got = overlaps(&[
            instance("a.A", vec![]),
            instance("b.A", vec![]),
            instance("c.C", vec![]),
        ]);
        let want: HashSet<String> = ["a.A".into(), "b.A".into()].into_iter().collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_typevar_scoped_name() {
        // Two T's with different namespaces collide under the short name.
        let got = overlaps(&[type_var("T", "mod"), type_var("T", "other")]);
        let want: HashSet<String> = ["T@mod".into(), "T@other".into()].into_iter().collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_typevar_namespace_empty() {
        // No namespace → scoped name is plain "T", single entry, no overlap.
        assert_eq!(overlaps(&[type_var("T", "")]), HashSet::new());
    }

    #[test]
    fn test_typing_injection() {
        // `typing.List` collides with a user `List` from another module, and
        // the typing name is injected for the unimported-hints group.
        let got = overlaps(&[union(vec![
            instance("typing.List", vec![]),
            instance("m.List", vec![]),
        ])]);
        assert!(got.contains("typing.List"));
        assert!(got.contains("m.List"));
    }
}
