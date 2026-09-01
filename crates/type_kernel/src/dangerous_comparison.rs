//! Native port of `mypy.checkexpr.ExpressionChecker.dangerous_comparison`
//! (checkexpr.py:5008-5137): the pure type-predicate decision tree behind
//! the `--strict-equality` "non-overlapping comparison" diagnostics.
//!
//! The Python shim computes the checker/binder-derived flags
//! (`strict_equality`, `strict_equality_for_none`, `unreachable_suppressed`,
//! `identity_check`, `prefer_literal`, and the two
//! `custom_special_method(..., "__eq__")` booleans) and serializes the four
//! type arguments (`left`, `right`, `original_container`, and the two
//! mapped-supertype fullnames for the AbstractSet/Mapping recursion). Rust
//! reruns the branch-for-branch decision tree on the wire types and returns
//! `Some(bool)`, or `None` to defer to the untouched pure-Python body.
//!
//! Deferral policy (correctness > coverage): every sub-decision that Rust
//! cannot reproduce exactly falls back to Python. That includes a
//! `TypeAliasType` whose alias snapshot is missing, cyclic, or undecodable
//! (well-formed aliases expand natively via the alias resolver, mirroring
//! `get_proper_type`), an unresolved
//! `Instance` snapshot, a missing known-fullname snapshot for the mapped
//! AbstractSet/Mapping item recursion, and an unknown `LiteralValue`
//! variant. `None` is always safe: the Python fallback body is unchanged.
//!
//! The `seen_types` recursion guard is passed from the Python shim as a
//! `bool` (true when the pair was already visited), because the wire format
//! cannot carry live `(Type, Type)` identity pairs. The AbstractSet/Mapping/
//! list-tuple item recursion here mirrors the fresh recursive
//! `dangerous_comparison` call Python makes (checkexpr.py:5145-5179): the
//! item pair's custom-`__eq__` suppression is re-evaluated (MRO walk) and the
//! per-call flags reset to the Python defaults (`identity_check=False`,
//! `prefer_literal=True`, `original_container=None`).

use pyo3::prelude::*;

use crate::checker_helpers::custom_special_method_inner;
use crate::checkexpr_functions::try_getting_literal_inner;
use crate::meet::overlap;
use crate::subtypes::map_instance_to_supertype;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, LiteralValue, ReadBuffer, Type};

// ---------------------------------------------------------------------------
// Constants (checkexpr.py:928-941)
// ---------------------------------------------------------------------------

/// `OVERLAPPING_TYPES_ALLOWLIST`: builtin AbstractSet implementations whose
/// `__eq__` compares values, so item comparison decides the overlap.
const OVERLAPPING_TYPES_ALLOWLIST: &[&str] = &[
    "builtins.set",
    "builtins.frozenset",
    "typing.KeysView",
    "typing.ItemsView",
    "_collections_abc.dict_keys",
    "_collections_abc.dict_items",
];

/// `OVERLAPPING_BYTES_ALLOWLIST`: byte-ish builtin types that compare safely
/// with each other (`97 in b'abc'` etc.).
const OVERLAPPING_BYTES_ALLOWLIST: &[&str] = &[
    "builtins.bytes",
    "builtins.bytearray",
    "builtins.memoryview",
];

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `mypy.types_utils.remove_optional` (types_utils.py:132-140): expand a
/// top-level alias, drop `NoneType` items from a union (each item is
/// expanded only to *test* for `NoneType`; the original item is kept in
/// the output, mirroring Python), `NoneType` -> bottom, else identity.
fn remove_optional(typ: &Type, aliases: &crate::aliases::TypeAliasResolver) -> Option<Type> {
    let proper = crate::checkexpr_functions::get_proper_or_expand(typ, aliases)?;
    match &proper {
        Type::UnionType { items, .. } => {
            let mut out: Vec<Type> = Vec::with_capacity(items.len());
            for item in items {
                let proper_item = crate::checkexpr_functions::get_proper_or_expand(item, aliases)?;
                if matches!(proper_item, Type::NoneType) {
                    continue;
                }
                out.push(item.clone());
            }
            Some(crate::setops::union_make_union(out))
        }
        Type::NoneType => Some(Type::UninhabitedType { ambiguous: false }),
        _ => Some(proper),
    }
}

/// `has_bytes_component` (checkexpr.py:8451-8465): is this a byte type or a
/// union containing one? Reuses the native `checkexpr_functions` worker.
fn has_bytes_component(typ: &Type, aliases: &crate::aliases::TypeAliasResolver) -> Option<bool> {
    crate::checkexpr_functions::has_bytes_component_inner(typ, aliases)
}

// ---------------------------------------------------------------------------
// dangerous_comparison (checkexpr.py:5008-5137)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.dangerous_comparison`, Rust subset.
///
/// Mirrors the Python body branch-for-branch. Deferral (`None`) fires on:
///   * a wire `TypeAliasType` whose alias snapshot is missing, cyclic, or
///     undecodable (well-formed aliases expand natively, mirroring
///     `get_proper_type`),
///   * a missing `TypeInfo` snapshot for an `Instance`,
///   * an AbstractSet/Mapping recursion whose mapped supertype snapshot is
///     not among the passed known fullnames (the shim passes the resolved
///     `abstract_set`/`abstract_map` fullnames, and the recursion's item
///     pairs are passed back out for Python to run),
///   * an unknown `LiteralValue` variant (a non-to-string-able literal),
///   * any `overlap(...)` decision Rust cannot make.
///
/// `seen` is the Python `seen_types` guard already checked on this pair;
/// the Python shim holds the live guard and recurses into the item pairs
/// itself when the Rust return box says so.
#[allow(clippy::too_many_arguments)]
fn dangerous_comparison_inner(
    left: &Type,
    right: &Type,
    original_container: Option<&Type>,
    python_seen: bool,
    prefer_literal: bool,
    identity_check: bool,
    strict_equality_for_none: bool,
    unreachable_suppressed: bool,
    has_custom_eq_left: bool,
    has_custom_eq_right: bool,
    strict_optional: bool,
    abstract_set_ref: Option<&str>,
    abstract_map_ref: Option<&str>,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    if python_seen {
        return Some(false);
    }

    let left = crate::checkexpr_functions::get_proper_or_expand(left, aliases)?;
    let right = crate::checkexpr_functions::get_proper_or_expand(right, aliases)?;

    // Custom __eq__ on either side suppresses the error (equal
    // non-overlapping types are allowed). The shim computed these via the
    // seam, or passed Python's result on deferral.
    if (has_custom_eq_left || has_custom_eq_right) && !identity_check {
        return Some(false);
    }

    let (left, right) = if prefer_literal {
        (
            try_getting_literal_inner(&left)?,
            try_getting_literal_inner(&right)?,
        )
    } else {
        (left, right)
    };

    if unreachable_suppressed {
        return Some(false);
    }
    if strict_equality_for_none {
        if matches!(left, Type::NoneType) && matches!(right, Type::NoneType) {
            return Some(false);
        }
    } else if matches!(left, Type::NoneType) || matches!(right, Type::NoneType) {
        return Some(false);
    }

    let (left, right) =
        if matches!(left, Type::UnionType { .. }) && matches!(right, Type::UnionType { .. }) {
            let l = remove_optional(&left, aliases)?;
            let r = remove_optional(&right, aliases)?;
            // checkexpr.py:5737: `left, right = get_proper_types(...)` right
            // after remove_optional; a collapsed single-item union can be a
            // bare alias item, which Python expands here.
            (
                crate::checkexpr_functions::get_proper_or_expand(&l, aliases)?,
                crate::checkexpr_functions::get_proper_or_expand(&r, aliases)?,
            )
        } else {
            (left, right)
        };

    // b'abc' in b'cde' (byte containers) always returns True; only flag
    // when the check can NEVER be True.
    if let Some(original_container) = original_container {
        if has_bytes_component(original_container, aliases)? && has_bytes_component(&left, aliases)?
        {
            return Some(false);
        }
    }

    if let (
        Type::Instance {
            type_ref: left_ref, ..
        },
        Type::Instance {
            type_ref: right_ref,
            ..
        },
    ) = (&left, &right)
    {
        if OVERLAPPING_TYPES_ALLOWLIST.contains(&left_ref.as_str())
            && OVERLAPPING_TYPES_ALLOWLIST.contains(&right_ref.as_str())
        {
            let abstract_set_ref = abstract_set_ref?;
            // Recurse on the two item types of the mapped AbstractSet
            // supertype, mirroring Python's fresh recursive call: the item
            // pair's custom-__eq__ is re-evaluated and the per-call flags

            // reset. The mapped supertype may be a non-generated pair, but
            // `map_instance_to_supertype` resolves to the wire snapshot.
            let left_args =
                map_instance_to_supertype(left_ref, &args_of(&left)?, abstract_set_ref, resolver)?;
            let right_args = map_instance_to_supertype(
                right_ref,
                &args_of(&right)?,
                abstract_set_ref,
                resolver,
            )?;
            let left_item = left_args.first()?;
            let right_item = right_args.first()?;
            let (item_custom_eq_left, item_custom_eq_right) =
                recursion_custom_eq(left_item, right_item, resolver)?;
            return dangerous_comparison_inner(
                left_item,
                right_item,
                None,
                false,
                true,
                false,
                strict_equality_for_none,
                unreachable_suppressed,
                item_custom_eq_left,
                item_custom_eq_right,
                strict_optional,
                Some(abstract_set_ref),
                abstract_map_ref,
                resolver,
                aliases,
            );
        } else if resolver.get(left_ref)?.has_base("typing.Mapping")
            && resolver.get(right_ref)?.has_base("typing.Mapping")
        {
            let abstract_map_ref = abstract_map_ref?;
            let left_args =
                map_instance_to_supertype(left_ref, &args_of(&left)?, abstract_map_ref, resolver)?;
            let right_args = map_instance_to_supertype(
                right_ref,
                &args_of(&right)?,
                abstract_map_ref,
                resolver,
            )?;
            let left_key = left_args.first()?;
            let right_key = right_args.first()?;
            let (key_custom_eq_left, key_custom_eq_right) =
                recursion_custom_eq(left_key, right_key, resolver)?;
            let key_dangerous = dangerous_comparison_inner(
                left_key,
                right_key,
                None,
                false,
                true,
                false,
                strict_equality_for_none,
                unreachable_suppressed,
                key_custom_eq_left,
                key_custom_eq_right,
                strict_optional,
                abstract_set_ref,
                Some(abstract_map_ref),
                resolver,
                aliases,
            )?;
            if key_dangerous {
                return Some(true);
            }
            let left_value = left_args.get(1)?;
            let right_value = right_args.get(1)?;
            let (value_custom_eq_left, value_custom_eq_right) =
                recursion_custom_eq(left_value, right_value, resolver)?;
            return dangerous_comparison_inner(
                left_value,
                right_value,
                None,
                false,
                true,
                false,
                strict_equality_for_none,
                unreachable_suppressed,
                value_custom_eq_left,
                value_custom_eq_right,
                strict_optional,
                abstract_set_ref,
                Some(abstract_map_ref),
                resolver,
                aliases,
            );
        } else if (left_ref == "builtins.list" || left_ref == "builtins.tuple")
            && right_ref == left_ref
        {
            let left_item = args_of(&left)?.first().cloned()?;
            let right_item = args_of(&right)?.first().cloned()?;
            let (item_custom_eq_left, item_custom_eq_right) =
                recursion_custom_eq(&left_item, &right_item, resolver)?;
            return dangerous_comparison_inner(
                &left_item,
                &right_item,
                None,
                false,
                true,
                false,
                strict_equality_for_none,
                unreachable_suppressed,
                item_custom_eq_left,
                item_custom_eq_right,
                strict_optional,
                abstract_set_ref,
                abstract_map_ref,
                resolver,
                aliases,
            );
        } else if OVERLAPPING_BYTES_ALLOWLIST.contains(&left_ref.as_str())
            && OVERLAPPING_BYTES_ALLOWLIST.contains(&right_ref.as_str())
        {
            return Some(false);
        }
    }

    if let (
        Type::LiteralType {
            value: LiteralValue::Bool(l),
            ..
        },
        Type::LiteralType {
            value: LiteralValue::Bool(r),
            ..
        },
    ) = (&left, &right)
    {
        // Comparing different booleans is not dangerous.
        if l != r {
            return Some(false);
        }
    }

    // bytes/bytearray comparisons are supported.
    if let (Type::LiteralType { fallback, .. }, Type::Instance { type_ref, .. }) = (&left, &right) {
        if let Type::Instance {
            type_ref: fb_ref, ..
        } = &**fallback
        {
            if fb_ref == "builtins.bytes" && resolver.get(type_ref)?.has_base("builtins.bytearray")
            {
                return Some(false);
            }
        }
    }
    if let (Type::Instance { type_ref, .. }, Type::LiteralType { fallback, .. }) = (&left, &right) {
        if let Type::Instance {
            type_ref: fb_ref, ..
        } = &**fallback
        {
            if fb_ref == "builtins.bytes" && resolver.get(type_ref)?.has_base("builtins.bytearray")
            {
                return Some(false);
            }
        }
    }

    // Final: never of any pair of the two types.
    let result = overlap(&left, &right, strict_optional, false, false, resolver, 0);
    result.map(|overlaps| !overlaps)
}

/// Extract the `args` field of a wire `Instance`; `None` on any other shape.
fn args_of(typ: &Type) -> Option<Vec<Type>> {
    match typ {
        Type::Instance { args, .. } => Some(args.clone()),
        _ => None,
    }
}

/// Python's `custom_special_method(t, "__eq__")` for the Instance case
/// (typeops.py:1834-1877, `TypeInfo.get` MRO walk). A method counts as
/// custom iff its defining class is not `builtins.*` / `typing.*`. The
/// snapshot's per-type `member_definers` has no MRO walk, so walk the
/// snapshot MRO here to match Python exactly. `None` when a snapshot is
/// unreadable, which safely defers the recursion to the Python fallback.
fn instance_has_custom_eq_mro(type_ref: &str, resolver: &TypeResolver) -> Option<bool> {
    let snap = resolver.get(type_ref)?;
    for ancestor in &snap.mro {
        let ancestor_snap = resolver.get(ancestor)?;
        let Some((_kind, definer)) = ancestor_snap.member_definers.get("__eq__") else {
            continue;
        };
        // typeops.py:1860 — builtins/typing methods are not custom.
        if definer.starts_with("builtins.") || definer.starts_with("typing.") {
            return Some(false);
        }
        return Some(true);
    }
    Some(false)
}

/// Recompute Python's per-recursion custom-`__eq__` suppression for an item
/// pair. Python's recursion is a fresh `dangerous_comparison` call
/// (checkexpr.py:5145-5179) with `identity_check=False`, so each level
/// re-evaluates `custom_special_method(..., "__eq__")` on the items. The
/// Instance case uses the MRO walk above; other shapes delegate to the
/// shared seam (which already mirrors Python, deferring on `None`).
fn recursion_custom_eq(left: &Type, right: &Type, resolver: &TypeResolver) -> Option<(bool, bool)> {
    let left_custom = match left {
        Type::Instance { type_ref, .. } => instance_has_custom_eq_mro(type_ref, resolver)?,
        _ => custom_special_method_inner(left, "__eq__", false, resolver)?,
    };
    let right_custom = match right {
        Type::Instance { type_ref, .. } => instance_has_custom_eq_mro(type_ref, resolver)?,
        _ => custom_special_method_inner(right, "__eq__", false, resolver)?,
    };
    Some((left_custom, right_custom))
}

// ---------------------------------------------------------------------------
// pyfunction entry
// ---------------------------------------------------------------------------

/// Native `rust_dangerous_comparison`, parity seam for
/// `mypy.checkexpr.ExpressionChecker.dangerous_comparison`
/// (checkexpr.py:5008).
///
/// The shim passes the four serialized types, the checker/binder scalars,
/// the two `custom_special_method("__eq__")` results, and the
/// AbstractSet/Mapping supertype fullnames (resolved by
/// `self.chk.lookup_typeinfo` on the Python side). Returns `Some(bool)`, or
/// `None` to defer to the pure-Python body.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    left_bytes,
    right_bytes,
    original_container_bytes,
    python_seen,
    prefer_literal,
    identity_check,
    strict_equality_for_none,
    unreachable_suppressed,
    has_custom_eq_left,
    has_custom_eq_right,
    strict_optional,
    abstract_set_ref,
    abstract_map_ref,
    resolver
))]
pub(crate) fn rust_dangerous_comparison(
    left_bytes: &[u8],
    right_bytes: &[u8],
    original_container_bytes: Option<&[u8]>,
    python_seen: bool,
    prefer_literal: bool,
    identity_check: bool,
    strict_equality_for_none: bool,
    unreachable_suppressed: bool,
    has_custom_eq_left: bool,
    has_custom_eq_right: bool,
    strict_optional: bool,
    abstract_set_ref: Option<String>,
    abstract_map_ref: Option<String>,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let left = match decode_type(left_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let right = match decode_type(right_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let original_container = match original_container_bytes {
        Some(bytes) => match decode_type(bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    Ok(dangerous_comparison_inner(
        &left,
        &right,
        original_container.as_ref(),
        python_seen,
        prefer_literal,
        identity_check,
        strict_equality_for_none,
        unreachable_suppressed,
        has_custom_eq_left,
        has_custom_eq_right,
        strict_optional,
        abstract_set_ref.as_deref(),
        abstract_map_ref.as_deref(),
        resolver.resolver(),
        resolver.alias_resolver(),
    ))
}

// ---------------------------------------------------------------------------
// Unit tests for the alias-aware `remove_optional` port
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aliases::{TypeAliasResolver, TypeAliasSnapshot};
    use crate::wire::WriteBuffer;

    fn inst(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_owned(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn alias_ref(type_ref: &str) -> Type {
        Type::TypeAliasType {
            args: Vec::new(),
            type_ref: type_ref.to_owned(),
        }
    }

    fn union(items: Vec<Type>) -> Type {
        let can_be_true = true;
        let can_be_false = true;
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true,
            can_be_false,
        }
    }

    fn target_snapshot(fullname: &str, target: &Type) -> TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, target).expect("target must encode");
        TypeAliasSnapshot {
            fullname: fullname.to_owned(),
            target: buf.into_bytes(),
            ..Default::default()
        }
    }

    fn resolver_with(entries: &[(&str, Type)]) -> TypeAliasResolver {
        let mut aliases = TypeAliasResolver::new();
        for (name, target) in entries {
            aliases.insert((*name).to_owned(), target_snapshot(name, target));
        }
        aliases
    }

    #[test]
    fn remove_optional_keeps_non_none_alias_item() {
        // `mod.A = builtins.A` has no NoneType in its target, so a union
        // item holding the alias is KEPT AS THE ALIAS NODE (expanded only
        // to *test* None-ness, mirroring types_utils.remove_optional).
        let aliases = resolver_with(&[("mod.A", inst("builtins.A", Vec::new()))]);
        let alias = alias_ref("mod.A");
        let input = union(vec![Type::NoneType, alias.clone()]);
        let out = remove_optional(&input, &aliases).expect("must decide");
        assert_eq!(out, alias, "alias item must survive, not be replaced");
    }

    #[test]
    fn remove_optional_expands_top_level_alias_with_none_target() {
        // `mod.O = Union[builtins.A, None]` as a top-level operand: the
        // expansion reaches the union, whose NoneType item is dropped and
        // a single-item union collapses to the bare Instance.
        let target = union(vec![inst("builtins.A", Vec::new()), Type::NoneType]);
        let aliases = resolver_with(&[("mod.O", target)]);
        let out = remove_optional(&alias_ref("mod.O"), &aliases).expect("must decide");
        assert_eq!(out, inst("builtins.A", Vec::new()));
    }

    #[test]
    fn remove_optional_bare_none_is_uninhabited() {
        let aliases = TypeAliasResolver::new();
        let out = remove_optional(&Type::NoneType, &aliases).expect("must decide");
        assert_eq!(out, Type::UninhabitedType { ambiguous: false });
    }

    #[test]
    fn remove_optional_missing_alias_snapshot_defers() {
        // No snapshot for `mod.M`: the expansion must defer, not guess.
        let aliases = TypeAliasResolver::new();
        let input = alias_ref("mod.M");
        assert!(remove_optional(&input, &aliases).is_none());
    }

    #[test]
    fn remove_optional_alias_item_with_union_target_is_kept() {
        // `mod.B = Union[builtins.A, None]` as a UNION ITEM: the item's
        // proper type is a union, not NoneType, so the item stays (Python
        // tests only the immediate item, single expansion).
        let target = union(vec![inst("builtins.A", Vec::new()), Type::NoneType]);
        let aliases = resolver_with(&[("mod.B", target)]);
        let alias = alias_ref("mod.B");
        let input = union(vec![Type::NoneType, alias.clone()]);
        let out = remove_optional(&input, &aliases).expect("must decide");
        // Single non-None item collapses via union_make_union (len 1 ->
        // the item itself), so the bare alias node is the result.
        assert_eq!(out, alias);
    }
}
