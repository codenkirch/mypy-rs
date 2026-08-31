//! parity for `mypy.checker.partition_equality_ambiguous_types`
//! (checker.py:10674) and `is_equality_ambiguous_for_narrowing`
//! (checker.py:10703).
//!
//! Some values compare equal through a value domain broader than their
//! nominal type: an IntEnum member vs int, a StrEnum member vs str. When
//! narrowing a union against an equality target, the per-item split happens
//! here: items whose value info is ambiguous against the target's value
//! info are kept in both branches; the rest are ordinary-narrowable.
//!
//! The per-type domain collection reuses
//! `equality_info::equality_value_info_inner` (the #679 port) so both
//! functions share one source of truth. Alias nodes expand via the type
//! alias snapshot (`expanded_alias_target`); the call defers (`None`) when
//! an alias snapshot is missing or any `Instance` snapshot is absent.
//!
//! Return protocol of `rust_partition_equality_ambiguous_types`:
//! `Some((narrowable, ambiguous))` where each side is `None` (empty side)
//! or a `write_type_list(items)` blob. The Python seam decodes the list and
//! wraps it with `UnionType.make_union`, the same call the pure-Python body
//! makes: `len == 1` collapses to the bare item, `len > 1` builds a fresh
//! `UnionType`.

use pyo3::prelude::*;

use crate::equality_info::{equality_value_info_inner, EqualityValueInfo};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// `OPEN_VALUE_EQUALITY_DOMAIN_NAMES` (checker.py:10648): the values of the
/// open value-equality domains.
const OPEN_VALUE_EQUALITY_DOMAIN_NAMES: &[&str] = &["builtins.str", "builtins.numeric"];

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `write_type_list(items)` blob, or `None` if an item cannot be written.
fn encode_type_list(items: &[Type]) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type_list(&mut buf, items).ok()?;
    Some(buf.into_bytes())
}

/// `get_proper_type`: a `TypeAliasType` has no proper form on the wire, so
/// defer to the Python path.
fn get_proper_type(t: &Type) -> Option<Type> {
    if let Type::TypeAliasType { .. } = t {
        return None;
    }
    Some(t.clone())
}

/// `is_equality_ambiguous_for_narrowing` (checker.py:10703-10741) domain
/// loop applied to already-computed value infos.
fn is_equality_ambiguous_for_infos(left: &EqualityValueInfo, right: &EqualityValueInfo) -> bool {
    if left.is_top || right.is_top {
        // Only open-domain enum values make a top-like type ambiguous;
        // closed domains can narrow to their complete known set.
        let other = if left.is_top { right } else { left };
        return other.domains.iter().any(|(domain, info)| {
            OPEN_VALUE_EQUALITY_DOMAIN_NAMES.contains(&domain.as_str())
                && !info.enum_type_names.is_empty()
        });
    }

    for domain in left.domains.keys() {
        let Some(right_domain) = right.domains.get(domain) else {
            continue;
        };
        let left_domain = &left.domains[domain];
        // Equality between two values from the same enum can still narrow
        // by literal member.
        if !left_domain.enum_type_names.is_empty()
            && left_domain.enum_type_names == right_domain.enum_type_names
            && left_domain.type_names == left_domain.enum_type_names
            && right_domain.type_names == right_domain.enum_type_names
        {
            continue;
        }
        // Different domain-member types may compare equal, but nominal
        // narrowing would otherwise treat them as disjoint.
        if left_domain.type_names != right_domain.type_names {
            return true;
        }
        // Same domain-member types are only ambiguous if an enum value may
        // compare equal to its underlying value type.
        if !left_domain.enum_type_names.is_empty() || !right_domain.enum_type_names.is_empty() {
            return true;
        }
    }
    false
}

/// `partition_equality_ambiguous_types` (checker.py:10674-10700) driver.
/// Returns `None` to defer whenever a deferral fires.
#[allow(clippy::type_complexity)]
fn partition_inner(
    current: &Type,
    target: &Type,
    is_identity: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<(Option<Vec<u8>>, Option<Vec<u8>>)> {
    if is_identity {
        return Some((Some(encode_type_list(std::slice::from_ref(current))?), None));
    }

    let typ = get_proper_type(current)?;
    let items: Vec<Type> = match &typ {
        Type::UnionType { items, .. } if strict_optional => items.clone(),
        Type::UnionType { items, .. } => items
            // `relevant_items()`: without strict optional, NoneType items
            // are removed; an alias item resolving to NoneType defers later
            // via `equality_value_info_inner`.
            .iter()
            .filter(|item| !matches!(item, Type::NoneType))
            .cloned()
            .collect(),
        _ => vec![typ],
    };

    let target_info = equality_value_info_inner(target, resolver, aliases)?;
    let mut narrowable: Vec<Type> = Vec::new();
    let mut ambiguous: Vec<Type> = Vec::new();
    for item in &items {
        let item_info = equality_value_info_inner(item, resolver, aliases)?;
        if is_equality_ambiguous_for_infos(&item_info, &target_info) {
            ambiguous.push(item.clone());
        } else {
            narrowable.push(item.clone());
        }
    }

    let narrowable_blob = if narrowable.is_empty() {
        None
    } else {
        Some(encode_type_list(&narrowable)?)
    };
    let ambiguous_blob = if ambiguous.is_empty() {
        None
    } else {
        Some(encode_type_list(&ambiguous)?)
    };
    Some((narrowable_blob, ambiguous_blob))
}

/// Native `rust_is_equality_ambiguous_for_narrowing`, parity seam for
/// `mypy.checker.is_equality_ambiguous_for_narrowing` (checker.py:10703).
///
/// Computes both `EqualityValueInfo` records and runs the shared-domain
/// comparison loop. Returns `None` to defer (unserializable type, alias, or
/// missing snapshot) so the pure-Python body runs.
#[pyfunction]
pub(crate) fn rust_is_equality_ambiguous_for_narrowing(
    left_bytes: &[u8],
    right_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let Some(left) = decode_type(left_bytes) else {
        return Ok(None);
    };
    let Some(right) = decode_type(right_bytes) else {
        return Ok(None);
    };
    let Some(left_info) =
        equality_value_info_inner(&left, resolver.resolver(), resolver.alias_resolver())
    else {
        return Ok(None);
    };
    let Some(right_info) =
        equality_value_info_inner(&right, resolver.resolver(), resolver.alias_resolver())
    else {
        return Ok(None);
    };
    Ok(Some(is_equality_ambiguous_for_infos(
        &left_info,
        &right_info,
    )))
}

/// Native `rust_partition_equality_ambiguous_types`, parity seam for
/// `mypy.checker.partition_equality_ambiguous_types` (checker.py:10674).
///
/// Splits `current` per union item via
/// `is_equality_ambiguous_for_narrowing(item, target)`. Returns
/// `Some((narrowable, ambiguous))` where each side is `None` (empty) or a
/// `write_type_list(items)` blob for the Python seam to decode and fold
/// through `UnionType.make_union`. `None` overall defers to Python.
#[pyfunction]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_partition_equality_ambiguous_types(
    current_bytes: &[u8],
    target_bytes: &[u8],
    is_identity: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Option<Vec<u8>>, Option<Vec<u8>>)>> {
    let Some(current) = decode_type(current_bytes) else {
        return Ok(None);
    };
    let Some(target) = decode_type(target_bytes) else {
        return Ok(None);
    };
    Ok(partition_inner(
        &current,
        &target,
        is_identity,
        strict_optional,
        resolver.resolver(),
        resolver.alias_resolver(),
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::equality_info::EqualityDomainInfo;

    fn di(type_names: &[&str], enum_type_names: &[&str]) -> EqualityDomainInfo {
        EqualityDomainInfo {
            type_names: type_names
                .iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>(),
            enum_type_names: enum_type_names
                .iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>(),
        }
    }

    fn info(domains: &[(&str, EqualityDomainInfo)], is_top: bool) -> EqualityValueInfo {
        EqualityValueInfo {
            domains: domains
                .iter()
                .map(|(d, i)| (d.to_string(), i.clone()))
                .collect::<HashMap<_, _>>(),
            is_top,
        }
    }

    #[test]
    fn top_side_ambiguity_depends_on_open_domain() {
        let top = info(&[], true);
        // numeric is an OPEN domain: a top-like type is ambiguous with an
        // open-domain enum (IntEnum member vs object).
        let open = info(
            &[("builtins.numeric", di(&["mod.Color"], &["mod.Color"]))],
            false,
        );
        assert!(is_equality_ambiguous_for_infos(&top, &open));
        // bytes is a CLOSED domain: it narrows to the complete known set.
        let closed = info(
            &[("builtins.bytes", di(&["mod.BytesE"], &["mod.BytesE"]))],
            false,
        );
        assert!(!is_equality_ambiguous_for_infos(&top, &closed));
    }

    #[test]
    fn same_enum_both_sides_is_not_ambiguous() {
        let d = di(&["mod.Color"], &["mod.Color"]);
        let left = info(&[("builtins.numeric", d.clone())], false);
        let right = info(&[("builtins.numeric", d)], false);
        assert!(!is_equality_ambiguous_for_infos(&left, &right));
    }

    #[test]
    fn different_member_names_are_ambiguous() {
        let left = info(
            &[("builtins.numeric", di(&["mod.Color"], &["mod.Color"]))],
            false,
        );
        let right = info(
            &[("builtins.numeric", di(&["mod.Other"], &["mod.Other"]))],
            false,
        );
        assert!(is_equality_ambiguous_for_infos(&left, &right));
    }

    #[test]
    fn enum_vs_its_underlying_type_is_ambiguous() {
        // mod.Color (enum) vs builtins.int in the shared numeric domain.
        let left = info(
            &[("builtins.numeric", di(&["mod.Color"], &["mod.Color"]))],
            false,
        );
        let right = info(&[("builtins.numeric", di(&["builtins.int"], &[]))], false);
        assert!(is_equality_ambiguous_for_infos(&left, &right));
    }

    #[test]
    fn same_plain_types_are_not_ambiguous() {
        let d = di(&["builtins.int"], &[]);
        let left = info(&[("builtins.numeric", d.clone())], false);
        let right = info(&[("builtins.numeric", d)], false);
        assert!(!is_equality_ambiguous_for_infos(&left, &right));
    }

    #[test]
    fn no_shared_domains_is_not_ambiguous() {
        let left = info(&[("builtins.numeric", di(&["builtins.int"], &[]))], false);
        let right = info(&[("builtins.bytes", di(&["builtins.bytes"], &[]))], false);
        assert!(!is_equality_ambiguous_for_infos(&left, &right));
    }

    #[test]
    fn str_domain_mismatch_is_ambiguous() {
        // builtins.str vs a StrEnum member: open str domain, different
        // member names.
        let left = info(&[("builtins.str", di(&["builtins.str"], &[]))], false);
        let right = info(
            &[("builtins.str", di(&["mod.Fruit"], &["mod.Fruit"]))],
            false,
        );
        assert!(is_equality_ambiguous_for_infos(&left, &right));
    }
}
