//! Stage 3d `is_overlapping_types` port (`mypy.meet`, issue #266).
//!
//! Mirrors `meet.py:450-774` on the wire `Type` enum. Returns `Some(bool)`
//! when Rust fully decides, `None` so the Python shim falls through when a
//! case needs a live `TypeInfo` (`TypeAliasType` expansion,
//! `map_instance_to_supertype` of an unseen class, `find_member`,
//! `are_parameters_compatible`, `is_callable_compatible`) or a recursive
//! alias. This is the strangler-fig per-call gate; parity is asserted both
//! directions in `mypy/test/testsubtypes.py::NativeOverlapSuite`.

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::setops::{make_simplified_union, meet_types, union_make_union};
use crate::subtypes::{is_subtype, map_instance_to_supertype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::{find_unpack_in_list_inner, has_type_vars_inner};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

/// `mypy.meet.is_overlapping_types` recursion guard. Python guards
/// recursion with `seen_types`; Rust cannot carry that across the wire, so
/// we cap the call depth instead (self-recursive aliases are deferred to
/// Python by the shim before we are reached).
const MAX_DEPTH: i64 = 200;

/// `mypy.types.TypeVarLikeType` — the three type-var variants.
fn is_type_var_like(t: &Type) -> bool {
    matches!(
        t,
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
    )
}

/// `mypy.meet.is_object` (meet.py:425-427).
fn is_object(t: &Type) -> bool {
    matches!(t, Type::Instance { type_ref, .. } if type_ref == "builtins.object")
}

/// `mypy.typeops.is_named_instance` (typeops.py:636-640).
fn is_named_instance(t: &Type, name: &str) -> bool {
    matches!(t, Type::Instance { type_ref, .. } if type_ref == name)
}

/// `mypy.typeops.is_tuple`: TupleType or `Instance[builtins.tuple]`.
fn is_tuple(t: &Type) -> bool {
    matches!(t, Type::TupleType { .. })
        || matches!(t, Type::Instance { type_ref, .. } if type_ref == "builtins.tuple")
}

/// `mypy.meet.is_none_object_overlap` (meet.py:429-434).
fn is_none_object_overlap(t1: &Type, t2: &Type) -> bool {
    matches!(t1, Type::NoneType)
        && matches!(t2, Type::Instance { type_ref, .. } if type_ref == "builtins.object")
}

/// `mypy.types.get_proper_type` for the few cases we must look through.
/// `TypeAliasType` cannot be expanded without the alias resolver; return
/// `None` so the caller falls back to Python.
fn get_proper(t: &Type) -> Option<&Type> {
    match t {
        Type::TypeAliasType { .. } => None,
        other => Some(other),
    }
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `mypy.types.LiteralType` value equality (meet.py:691,421).
fn literal_value_eq(a: &LiteralValue, b: &LiteralValue) -> bool {
    a == b
}

/// `mypy.meet.is_enum_overlapping_union` (meet.py:403-413).
fn is_enum_overlapping_union(x: &Type, y: &Type, res: &TypeResolver) -> Option<bool> {
    let Type::Instance {
        type_ref: x_ref, ..
    } = x
    else {
        return Some(false);
    };
    let Type::UnionType { items, .. } = y else {
        return Some(false);
    };
    let x_snap = res.get(x_ref)?;
    if !x_snap.is_enum {
        return Some(false);
    }
    for z in items {
        let p = match get_proper(z)? {
            Type::LiteralType { fallback, .. } => fallback.as_ref(),
            _ => continue,
        };
        let Type::Instance {
            type_ref: f_ref, ..
        } = p
        else {
            continue;
        };
        if f_ref == x_ref {
            return Some(true);
        }
    }
    Some(false)
}

/// `mypy.meet.is_literal_in_union` (meet.py:416-422).
fn is_literal_in_union(x: &Type, y: &Type) -> Option<bool> {
    let Type::LiteralType { value, .. } = x else {
        return Some(false);
    };
    let Type::UnionType { items, .. } = y else {
        return Some(false);
    };
    for z in items {
        let p = get_proper(z)?;
        if let Type::LiteralType { value: p_value, .. } = p {
            if literal_value_eq(value, p_value) {
                return Some(true);
            }
        }
    }
    Some(false)
}

/// `mypy.typeops.get_possible_variants` (meet.py:353-400). Resolves
/// `TypeAliasType` operands via the alias resolver (mirroring meet.py's
/// `typ = get_proper_type(typ)` at the top).
fn get_possible_variants(
    t: &Type,
    res: &TypeResolver,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
) -> Option<Vec<Type>> {
    let t = match aliases {
        Some(a) => crate::checkexpr_functions::get_proper_or_expand(t, a)?,
        None => t.clone(),
    };
    match &t {
        Type::TypeVarType {
            values,
            upper_bound,
            ..
        } => {
            if values.is_empty() {
                Some(vec![upper_bound.as_ref().clone()])
            } else {
                Some(values.clone())
            }
        }
        Type::ParamSpecType { upper_bound, .. } => {
            let ub = match get_proper(upper_bound.as_ref())? {
                Type::Instance { type_ref, .. } => {
                    // meet.py:389 — 'object' from the final mro item.
                    let mro = res.get(type_ref)?.mro.clone();
                    vec![Type::Instance {
                        type_ref: mro
                            .into_iter()
                            .last()
                            .unwrap_or_else(|| "builtins.object".to_string()),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    }]
                }
                _ => vec![Type::AnyType {
                    type_of_any: 8, // TypeOfAny.implementation_artifact
                    source_any: None,
                    missing_import_name: None,
                }],
            };
            Some(ub)
        }
        Type::TypeVarTupleType { upper_bound, .. } => Some(vec![upper_bound.as_ref().clone()]),
        Type::UnionType { items, .. } => Some(items.clone()),
        Type::Overloaded { items } => Some(items.clone()),
        other => Some(vec![other.clone()]),
    }
}

/// `mypy.types.UnionType.relevant_items` but on Rust.
/// Under strict optional this whole branch is never reached (the caller
/// only calls it when `strict_optional` is false), so no in-Rust guard.
fn relevant_items(items: &[Type]) -> Option<Vec<Type>> {
    let mut out = Vec::new();
    for item in items {
        match get_proper(item)? {
            Type::NoneType => {}
            _ => out.push(item.clone()),
        }
    }
    Some(out)
}

/// `mypy.types.UnionType.make_union` (types.py:3502-3510). Empty becomes
/// `UninhabitedType`, singleton becomes the item itself.
fn make_union(items: Vec<Type>) -> Type {
    union_make_union(items)
}

/// The `_is_overlapping_types` recursive worker (meet.py:547-774).
/// External callers use this alias-less public entry. The meet.rs seam
/// entry points call `overlap_impl` with `Some(alias_resolver)` so
/// `TypeAliasType` operands resolve natively (see `overlap_impl`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn overlap(
    left: &Type,
    right: &Type,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    res: &TypeResolver,
    depth: i64,
) -> Option<bool> {
    overlap_impl(
        left,
        right,
        strict_optional,
        ignore_promotions,
        overlap_for_overloads,
        res,
        None,
        depth,
    )
}

/// Same as [`overlap`] but resolves `TypeAliasType` operands through the
/// alias resolver, mirroring the `get_proper_types` call at the top of each
/// `is_overlapping_types` recursion level (meet.py:556). `aliases = None`
/// leaves alias operands to defer (unchanged external behavior); the
/// meet.rs seam entries pass `Some`, closing the alias defer sites.
#[allow(clippy::too_many_arguments)]
fn overlap_impl(
    left: &Type,
    right: &Type,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    res: &TypeResolver,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
    depth: i64,
) -> Option<bool> {
    if depth > MAX_DEPTH {
        return None;
    }

    // 0. resolve TypeAliasType operands (meet.py:556). A missing resolver
    // snapshot / cycle defers (None), falling back to Python exactly as
    // `get_proper_types` would when expansion is impossible.
    let (left, right) = match aliases {
        Some(a) => {
            let lo = crate::checkexpr_functions::get_proper_or_expand(left, a)?;
            let ro = crate::checkexpr_functions::get_proper_or_expand(right, a)?;
            (lo, ro)
        }
        None => (left.clone(), right.clone()),
    };
    let left = &left;
    let right = &right;

    // 1. illegal types: Unbound/Erased/Deleted overlap in Python (True, meet.py:568).
    // The overlap shim does not filter Erased operands (meet.py:520-533 guards only
    // TypeGuardedType/PartialType), so a wire tag-122 leaf arrives; `||` = `and`.
    if matches!(
        left,
        Type::UnboundType { .. } | Type::DeletedType { .. } | Type::ErasedType
    ) || matches!(
        right,
        Type::UnboundType { .. } | Type::DeletedType { .. } | Type::ErasedType
    ) {
        return Some(true);
    }

    // 2. no-strict-optional: drop None from Union items (meet.py:491-498).
    let left = if !strict_optional {
        match left {
            Type::UnionType { items, .. } => make_union(relevant_items(items)?),
            other => other.clone(),
        }
    } else {
        left.clone()
    };
    let right = if !strict_optional {
        match right {
            Type::UnionType { items, .. } => make_union(relevant_items(items)?),
            other => other.clone(),
        }
    } else {
        right.clone()
    };

    // 3. 'Any' may or may not overlap (meet.py:500-502).
    if matches!(left, Type::AnyType { .. }) || matches!(right, Type::AnyType { .. }) {
        return Some(!overlap_for_overloads || is_object(&left) || is_object(&right));
    }

    // 4. enums expanded to Literal-Unions, and literals inside Unions.
    if is_enum_overlapping_union(&left, &right, res)?
        || is_enum_overlapping_union(&right, &left, res)?
        || is_literal_in_union(&left, &right)?
        || is_literal_in_union(&right, &left)?
    {
        return Some(true);
    }

    // 5. overload-strict: None overlaps only object (which erases to None).
    if overlap_for_overloads
        && (is_none_object_overlap(&left, &right) || is_none_object_overlap(&right, &left))
    {
        return Some(false);
    }

    // 6. complete overlap via subtyping (are_related_types).
    let ctx = SubtypeContext::new(
        false,
        false,
        false,
        ignore_promotions,
        overlap_for_overloads,
        strict_optional,
    );
    let a = is_subtype(&left, &right, &ctx, res)?;
    let b = is_subtype(&right, &left, &ctx, res)?;
    if a || b {
        return Some(true);
    }

    // 7. get_possible_variants (meet.py:526-570).
    let lv = get_possible_variants(&left, res, aliases)?;
    let rv = get_possible_variants(&right, res, aliases)?;
    if lv.len() > 1 || rv.len() > 1 || is_type_var_like(&left) || is_type_var_like(&right) {
        for l in &lv {
            for r in &rv {
                if overlap_impl(
                    l,
                    r,
                    strict_optional,
                    ignore_promotions,
                    overlap_for_overloads,
                    res,
                    aliases,
                    depth + 1,
                )? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }

    // 8. strict-optional: None only overlaps with None (meet.py:579-581).
    if strict_optional && (matches!(left, Type::NoneType) != matches!(right, Type::NoneType)) {
        return Some(false);
    }

    // 9. TypedDicts (meet.py:586-597).
    if let (Type::TypedDictType { .. }, Type::TypedDictType { .. }) = (&left, &right) {
        return are_typed_dicts_overlapping(&left, &right, &|a, b| {
            overlap_impl(
                a,
                b,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )
        });
    }
    let tdp = typed_dict_mapping_pair(&left, &right, res)?;
    let tdp2 = typed_dict_mapping_pair(&right, &left, res)?;
    if tdp || tdp2 {
        let overlapping_inner = &|a: &Type, b: &Type| -> Option<bool> {
            overlap_impl(
                a,
                b,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )
        };
        return typed_dict_mapping_overlap(&left, &right, overlapping_inner, res);
    }
    let left = match &left {
        Type::TypedDictType { fallback, .. } => fallback.as_ref().clone(),
        other => other.clone(),
    };
    let right = match &right {
        Type::TypedDictType { fallback, .. } => fallback.as_ref().clone(),
        other => other.clone(),
    };

    // 10. Tuples (meet.py:600-610).
    if is_tuple(&left) && is_tuple(&right) {
        return are_tuples_overlapping(&left, &right, &|a, b| {
            overlap_impl(
                a,
                b,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )
        });
    }
    let left = if matches!(left, Type::TupleType { .. }) {
        tuple_fallback(&left, res)?
    } else {
        left
    };
    let right = if matches!(right, Type::TupleType { .. }) {
        tuple_fallback(&right, res)?
    } else {
        right
    };

    // 11. TypeType pairs (meet.py:612-637).
    match (&left, &right) {
        (Type::TypeType { item: li, .. }, Type::TypeType { item: ri, .. }) => {
            return overlap_impl(
                li,
                ri,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            );
        }
        (Type::TypeType { .. }, _) | (_, Type::TypeType { .. }) => {
            // Either direction returns True -> overlapping (Python OR).
            // A None (deferred sub-result) defers the whole answer.
            match _type_object_overlap(
                &left,
                &right,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth,
            ) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => return None,
            }
            match _type_object_overlap(
                &right,
                &left,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth,
            ) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => return None,
            }
            return Some(false);
        }
        _ => {}
    }

    // 12. Parameters / CallableType / Literal (meet.py:639-703).
    if let (Type::Parameters(_), Type::Parameters(_)) = (&left, &right) {
        // are_parameters_compatible via the shared callable_compat engine,
        // `is_compat = overlap` (meet.py:708-716, allow_partial_overlap);
        // defers (None) on generic parameters / meet_types merge.
        let (lf, rf) = (
            crate::callable_compat::arg_list_from_type(&left)?,
            crate::callable_compat::arg_list_from_type(&right)?,
        );
        if !lf.variables_empty || !rf.variables_empty {
            return None;
        }
        let is_compat: &dyn Fn(&Type, &Type) -> Option<bool> = &|l, r| {
            overlap_impl(
                l,
                r,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )
        };
        return crate::callable_compat::are_parameters_compatible(
            lf.arg_types,
            lf.arg_kinds,
            lf.arg_names,
            lf.from_concatenate,
            rf.arg_types,
            rf.arg_kinds,
            rf.arg_names,
            rf.imprecise_arg_kinds,
            rf.is_ellipsis_args,
            is_compat,
            false,                  // is_proper_subtype
            !overlap_for_overloads, // ignore_pos_arg_names
            true,                   // allow_partial_overlap
            false,                  // strict_concatenate_check
        );
    }
    if matches!(left, Type::Parameters(_)) || matches!(right, Type::Parameters(_)) {
        return Some(false);
    }
    if let (Type::CallableType { .. }, Type::CallableType { .. }) = (&left, &right) {
        // is_callable_compatible both directions (meet.py:730-745), through
        // the shared engine with is_compat = overlap. Python's normalizations
        // (with_unpacked_kwargs, generic-left unify) defer (subtypes.py:2479).
        if crate::callable_compat::any_unpack_anywhere(&left)
            || crate::callable_compat::any_unpack_anywhere(&right)
        {
            return None;
        }
        let has_variables =
            |t: &Type| matches!(t, Type::CallableType { variables, .. } if !variables.is_empty());
        if has_variables(&left) || has_variables(&right) {
            return None;
        }
        let is_compat = |l: &Type, r: &Type| {
            overlap_impl(
                l,
                r,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )
        };
        match crate::callable_compat::is_callable_compatible(
            &left,
            &right,
            &is_compat,
            false, // is_proper_subtype
            !overlap_for_overloads,
            false, // strict_concatenate
            false, // ignore_return
            false, // check_args_covariantly
            true,  // allow_partial_overlap
            res,
        ) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => return None,
        }
        return crate::callable_compat::is_callable_compatible(
            &right,
            &left,
            &is_compat,
            false, // is_proper_subtype
            !overlap_for_overloads,
            false, // strict_concatenate
            false, // ignore_return
            true,  // check_args_covariantly
            true,  // allow_partial_overlap
            res,
        );
    }
    if matches!(left, Type::CallableType { .. }) && matches!(right, Type::Instance { .. })
        || matches!(right, Type::CallableType { .. }) && matches!(left, Type::Instance { .. })
    {
        // find_member("__call__") not ported -> defer.
        return None;
    }
    let left = if let Type::CallableType { fallback, .. } = &left {
        fallback.as_ref().clone()
    } else {
        left
    };
    let right = if let Type::CallableType { fallback, .. } = &right {
        fallback.as_ref().clone()
    } else {
        right
    };
    // Literal fallback upgrade (meet.py:691-703). Both Literals with equal
    // values fall back; unequal values are disjoint; a lone Literal falls
    // back to its own instance.
    let (left, right) = match (&left, &right) {
        (
            Type::LiteralType {
                value,
                fallback: lf,
            },
            Type::LiteralType {
                value: rv,
                fallback: rf,
            },
        ) => {
            if !literal_value_eq(value, rv) {
                return Some(false);
            }
            (lf.as_ref().clone(), rf.as_ref().clone())
        }
        (Type::LiteralType { fallback: lf, .. }, _) => (lf.as_ref().clone(), right),
        (_, Type::LiteralType { fallback: rf, .. }) => (left, rf.as_ref().clone()),
        _ => (left, right),
    };

    // 13. Instance-Instance (meet.py:705-765).
    if let (Type::Instance { .. }, Type::Instance { .. }) = (&left, &right) {
        return instances_overlap(
            &left,
            &right,
            strict_optional,
            ignore_promotions,
            overlap_for_overloads,
            res,
            aliases,
            depth + 1,
        );
    }

    // Fallback — Python's `assert type(left) != type(right); return False`.
    Some(false)
}

/// `mypy.meet.typed_dict_mapping_pair` (meet.py:1379-1396).
fn typed_dict_mapping_pair(left: &Type, right: &Type, res: &TypeResolver) -> Option<bool> {
    let (l, r) = (get_proper(left)?, get_proper(right)?);
    if matches!(l, Type::TypedDictType { .. }) {
        let Type::Instance { type_ref, .. } = r else {
            return Some(false);
        };
        Some(res.get(type_ref)?.has_base("typing.Mapping"))
    } else if matches!(r, Type::TypedDictType { .. }) {
        let Type::Instance { type_ref, .. } = l else {
            return Some(false);
        };
        Some(res.get(type_ref)?.has_base("typing.Mapping"))
    } else {
        Some(false)
    }
}

/// `mypy.meet.typed_dict_mapping_overlap` (meet.py:1503-1573).
///
/// Check if a TypedDict type is overlapping with a Mapping type.
/// Implements the logic from meet.py:1503-1573:
/// - Required keys must each overlap with the mapping's value type
/// - For TypedDicts with no required keys, at least one key must overlap with the value
///   type
fn typed_dict_mapping_overlap(
    left: &Type,
    right: &Type,
    overlapping: &dyn Fn(&Type, &Type) -> Option<bool>,
    res: &TypeResolver,
) -> Option<bool> {
    let (typed, other) = match (left, right) {
        (Type::TypedDictType { .. }, Type::Instance { .. }) => (left, right),
        (Type::Instance { .. }, Type::TypedDictType { .. }) => (right, left),
        _ => return Some(false),
    };

    let Type::TypedDictType {
        items: td_items,
        required_keys: td_required,
        readonly_keys: td_readonly,
        fallback: td_fallback,
        ..
    } = typed
    else {
        return Some(false);
    };
    let Type::Instance {
        type_ref: other_ref,
        args: other_args,
        ..
    } = other
    else {
        return Some(false);
    };

    let other_snap = res.get(other_ref)?;

    // mutable_mapping check: a TypedDict with readonly keys doesn't overlap MutableMapping
    if !td_readonly.is_empty()
        && other_snap
            .mro
            .iter()
            .any(|base| base.as_str() == "typing.MutableMapping")
    {
        return Some(false);
    }

    // Find Mapping in MRO and map other onto it
    let mapping_ref = other_snap
        .mro
        .iter()
        .find(|base| base.as_str() == "typing.Mapping")
        .cloned()?;
    let mapped_args = map_instance_to_supertype(other_ref, other_args, &mapping_ref, res)?;

    // key_type = mapped_args[0], value_type = mapped_args[1]
    let key_type = get_proper(mapped_args.first()?)?;
    let value_type = get_proper(mapped_args.get(1)?)?;

    // Get str_type from TypedDict fallback (Mapping[str, object])
    let Type::Instance {
        args: fallback_args,
        ..
    } = td_fallback.as_ref()
    else {
        return Some(false);
    };
    let str_type = fallback_args.first()?.clone();

    // Special case: no required keys + both key/value are Uninhabited
    // -> overlapping iff TypedDict has no required keys (empty TypedDict overlaps empty
    // dict)
    if matches!(key_type, Type::UninhabitedType { .. })
        && matches!(value_type, Type::UninhabitedType { .. })
    {
        return Some(td_required.is_empty());
    }

    // Check key_type overlaps str_type
    if !overlapping(key_type, &str_type)? {
        return Some(false);
    }

    // Build a lookup map for items since td_items is Vec<(String, Type)>
    let items_map: HashMap<&String, &Type> = td_items.iter().map(|(k, v)| (k, v)).collect();

    if td_required.is_empty() {
        // No required keys: at least one non-required key must overlap with value_type
        let has_overlap = td_items
            .iter()
            .filter(|(k, _)| !td_required.contains(k))
            .any(|(_, item)| overlapping(item, value_type) == Some(true));
        Some(has_overlap)
    } else {
        // All required keys must overlap with value_type
        for k in td_required.iter() {
            if let Some(item) = items_map.get(k) {
                if overlapping(item, value_type) != Some(true) {
                    return Some(false);
                }
            } else {
                return Some(false);
            }
        }
        Some(true)
    }
}

/// `mypy.meet.are_typed_dicts_overlapping` (meet.py:786-807).
fn are_typed_dicts_overlapping(
    left: &Type,
    right: &Type,
    is_overlapping: &dyn Fn(&Type, &Type) -> Option<bool>,
) -> Option<bool> {
    let (
        Type::TypedDictType {
            items: l_items,
            required_keys: l_required,
            ..
        },
        Type::TypedDictType {
            items: r_items,
            required_keys: r_required,
            ..
        },
    ) = (left, right)
    else {
        return Some(false);
    };
    let l_map: HashMap<_, _> = l_items.iter().cloned().collect();
    let r_map: HashMap<_, _> = r_items.iter().cloned().collect();
    for key in l_required {
        let Some(r_val) = r_map.get(key) else {
            return Some(false);
        };
        if !is_overlapping(&l_map[key], r_val)? {
            return Some(false);
        }
    }
    for key in r_required {
        let Some(l_val) = l_map.get(key) else {
            return Some(false);
        };
        if !is_overlapping(l_val, &r_map[key])? {
            return Some(false);
        }
    }
    Some(true)
}

/// `mypy.meet.adjust_tuple` (meet.py:867-872).
fn adjust_tuple(left: &Type, r: &Type) -> Option<Type> {
    if let Type::Instance {
        type_ref,
        args,
        last_known_value: _,
        extra_attrs: _,
    } = left
    {
        if type_ref == "builtins.tuple" {
            let n = match r {
                Type::TupleType { items, .. } => items.len(),
                _ => 1,
            };
            let item = args.first().cloned().unwrap_or(Type::AnyType {
                type_of_any: 8,
                source_any: None,
                missing_import_name: None,
            });
            let items = vec![item; n];
            return Some(Type::TupleType {
                partial_fallback: Box::new(left.clone()),
                items,
                implicit: false,
            });
        }
    }
    None
}

/// `mypy.meet.are_tuples_overlapping` (meet.py:810-843).
fn are_tuples_overlapping(
    left: &Type,
    right: &Type,
    is_overlapping: &dyn Fn(&Type, &Type) -> Option<bool>,
) -> Option<bool> {
    let left_step = adjust_tuple(left, right).unwrap_or_else(|| left.clone());
    let right_step = adjust_tuple(right, left).unwrap_or_else(|| right.clone());
    let left = left_step;
    let right = right_step;
    let Type::TupleType {
        partial_fallback: l_fb,
        items: l_items,
        ..
    } = &left
    else {
        return Some(false);
    };
    let Type::TupleType {
        partial_fallback: r_fb,
        items: r_items,
        ..
    } = &right
    else {
        return Some(false);
    };

    let l_unpack = find_unpack_in_list_inner(l_items);
    let r_unpack = find_unpack_in_list_inner(r_items);
    let mut l_items = l_items.clone();
    let mut r_items = r_items.clone();
    if l_unpack != -1 {
        let expanded = expand_tuple_if_possible(&left, r_items.len())?;
        let Type::TupleType { items: e_items, .. } = expanded else {
            return Some(false);
        };
        l_items = e_items;
    }
    if r_unpack != -1 {
        let expanded = expand_tuple_if_possible(&right, l_items.len())?;
        let Type::TupleType { items: e_items, .. } = expanded else {
            return Some(false);
        };
        r_items = e_items;
    }

    if l_items.len() != r_items.len() {
        return Some(false);
    }
    for (l, r) in l_items.iter().zip(r_items.iter()) {
        if !is_overlapping(l, r)? {
            return Some(false);
        }
    }
    if is_named_instance(r_fb.as_ref(), "builtins.tuple")
        || is_named_instance(l_fb.as_ref(), "builtins.tuple")
    {
        return Some(true);
    }
    is_overlapping(l_fb.as_ref(), r_fb.as_ref())
}

/// `mypy.meet.expand_tuple_if_possible` (meet.py:846-864).
fn expand_tuple_if_possible(tup: &Type, target: usize) -> Option<Type> {
    let Type::TupleType {
        partial_fallback,
        items,
        implicit,
    } = tup
    else {
        return Some(tup.clone());
    };
    if items.len() > target + 1 {
        return Some(tup.clone());
    }
    let extra = target + 1 - items.len();
    let mut new_items: Vec<Type> = Vec::new();
    for it in items {
        if !matches!(it, Type::UnpackType { .. }) {
            new_items.push(it.clone());
            continue;
        }
        let Type::UnpackType { typ } = it else {
            unreachable!()
        };
        let unpacked = get_proper(typ)?;
        let instance = match unpacked {
            Type::TypeVarTupleType { tuple_fallback, .. } => tuple_fallback.as_ref().clone(),
            other => other.clone(),
        };
        if !is_named_instance(&instance, "builtins.tuple") {
            return None; // assert Instance + builtins.tuple in Python
        }
        let Type::Instance { args, .. } = &instance else {
            return None;
        };
        let item = args.first().cloned().unwrap_or(Type::AnyType {
            type_of_any: 8,
            source_any: None,
            missing_import_name: None,
        });
        new_items.extend(std::iter::repeat_n(item, extra));
    }
    Some(Type::TupleType {
        partial_fallback: partial_fallback.clone(),
        items: new_items,
        implicit: *implicit,
    })
}

/// `mypy.typeops.tuple_fallback` (typeops.py:189-214). Returns a
/// `builtins.tuple` Instance whose single arg is the simplified union of the
/// items. Any UnpackType whose target is not `Instance[builtins.tuple]`
/// returns `None` (Python raises NotImplementedError).
fn tuple_fallback(t: &Type, res: &TypeResolver) -> Option<Type> {
    let Type::TupleType {
        partial_fallback, ..
    } = t
    else {
        return Some(t.clone());
    };
    let Type::Instance {
        type_ref: info_ref,
        extra_attrs,
        ..
    } = partial_fallback.as_ref()
    else {
        return Some(partial_fallback.as_ref().clone());
    };
    if info_ref != "builtins.tuple" {
        return Some(partial_fallback.as_ref().clone());
    }
    let mut items = Vec::new();
    let Type::TupleType { items: t_items, .. } = t else {
        return Some(t.clone());
    };
    for item in t_items {
        if !matches!(item, Type::UnpackType { .. }) {
            items.push(item.clone());
            continue;
        }
        let Type::UnpackType { typ } = item else {
            unreachable!()
        };
        let unpacked = match get_proper(typ)? {
            Type::TypeVarTupleType { upper_bound, .. } => get_proper(upper_bound.as_ref())?.clone(),
            other => other.clone(),
        };
        let Type::Instance { args, type_ref, .. } = &unpacked else {
            return None;
        };
        if type_ref != "builtins.tuple" {
            return None; // raise NotImplementedError in Python
        }
        items.push(args.first().cloned().unwrap_or(Type::AnyType {
            type_of_any: 8,
            source_any: None,
            missing_import_name: None,
        }));
    }
    let info = res.get(info_ref)?.fullname.clone();
    Some(Type::Instance {
        type_ref: info,
        args: vec![union_make_union(items)],
        last_known_value: None,
        extra_attrs: extra_attrs.clone(),
    })
}

/// `mypy.meet._type_object_overlap` (meet.py:617-635).
#[allow(clippy::too_many_arguments)]
fn _type_object_overlap(
    left: &Type,
    right: &Type,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    res: &TypeResolver,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
    depth: i64,
) -> Option<bool> {
    let Type::TypeType { item, .. } = get_proper(left)? else {
        return Some(false);
    };
    match get_proper(right)? {
        Type::CallableType { ret_type, .. } => overlap_impl(
            item,
            ret_type,
            strict_optional,
            ignore_promotions,
            overlap_for_overloads,
            res,
            aliases,
            depth + 1,
        ),
        Type::Instance {
            type_ref: r_ref, ..
        } => match get_proper(item)? {
            Type::Instance {
                type_ref: item_ref, ..
            } => {
                let snap = res.get(item_ref)?;
                // Tri-state metaclass fullname: Some("") = Python None
                // (no metaclass); fall through to the has_base check,
                // matching meet.py:626-630.
                if let Some(meta) = &snap.metaclass_fullname {
                    if meta.is_empty() {
                        let right_snap = res.get(r_ref)?;
                        return Some(right_snap.has_base("builtins.type"));
                    }
                    let meta_inst = Type::Instance {
                        type_ref: meta.clone(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    return overlap_impl(
                        &meta_inst,
                        right,
                        strict_optional,
                        ignore_promotions,
                        overlap_for_overloads,
                        res,
                        aliases,
                        depth + 1,
                    );
                }
                let right_snap = res.get(r_ref)?;
                Some(right_snap.has_base("builtins.type"))
            }
            Type::AnyType { .. } => {
                let right_snap = res.get(r_ref)?;
                Some(right_snap.has_base("builtins.type"))
            }
            _ => Some(false),
        },
        _ => Some(false),
    }
}

/// `mypy.meet.is_overlapping_types` final Instance-Instance branch
/// (meet.py:705-765).
#[allow(clippy::too_many_arguments)]
fn instances_overlap(
    left: &Type,
    right: &Type,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    res: &TypeResolver,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
    depth: i64,
) -> Option<bool> {
    let left = left.clone();
    let right = right.clone();
    let Type::Instance {
        type_ref: l_ref,
        args: l_args,
        ..
    } = &left
    else {
        return Some(false);
    };
    let Type::Instance {
        type_ref: r_ref,
        args: r_args,
        ..
    } = &right
    else {
        return Some(false);
    };

    let l_ref = l_ref.clone();
    let r_ref = r_ref.clone();
    let l_args = l_args.clone();
    let r_args = r_args.clone();

    let ctx = SubtypeContext::new(
        false,
        false,
        false,
        ignore_promotions,
        overlap_for_overloads,
        strict_optional,
    );
    if is_subtype(&left, &right, &ctx, res)? || is_subtype(&right, &left, &ctx, res)? {
        return Some(true);
    }

    if r_ref == "builtins.int" && MYPYC_NATIVE_INT_NAMES.contains(&l_ref.as_str()) {
        return Some(true);
    }

    let l_snap = res.get(&l_ref);
    let r_snap = res.get(&r_ref);

    // Two unrelated types cannot be partially overlapping: they're disjoint.
    // When `has_base` hits, map the derived side's args onto the base's type
    // ref so the paired args are comparable (meet.py:721-724).
    let (left2, right2) = if l_snap.is_some_and(|s| s.has_base(&r_ref)) {
        let mapped = map_instance_to_supertype(&l_ref, &l_args, &r_ref, res)?;
        (
            Type::Instance {
                type_ref: r_ref.clone(), // mapped args target the base type
                args: mapped,
                last_known_value: None,
                extra_attrs: None,
            },
            right,
        )
    } else if r_snap.is_some_and(|s| s.has_base(&l_ref)) {
        let mapped = map_instance_to_supertype(&r_ref, &r_args, &l_ref, res)?;
        (
            left,
            Type::Instance {
                type_ref: l_ref.clone(), // mapped args target the base type
                args: mapped,
                last_known_value: None,
                extra_attrs: None,
            },
        )
    } else {
        return Some(false);
    };

    let Type::Instance { args: l2_args, .. } = &left2 else {
        return None;
    };
    let Type::Instance {
        type_ref: r2_ref,
        args: r2_args,
        ..
    } = &right2
    else {
        return None;
    };

    // TypeVarTuple-delegation path (meet.py:730-747). Needs
    // `TypeInfo.defn.type_vars[prefix].tuple_fallback` — a live TypeInfo,
    // not the snapshot — so defer to Python rather than reconstructing it.
    if r2_snap_has_tvt(r2_ref, res) {
        return None;
    }

    // Pairwise overlap on the (possibly base-mapped) args. Note: symmetric,
    // variance-agnostic, so partial overlaps on any single pair win
    // (meet.py:749-766).
    let len_l = l2_args.len();
    let len_r = r2_args.len();
    if len_l == len_r {
        for (la, ra) in l2_args.iter().zip(r2_args.iter()) {
            if !overlap_impl(
                la,
                ra,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                aliases,
                depth + 1,
            )? {
                return Some(false);
            }
        }
        return Some(true);
    }
    Some(false)
}

/// Whether `right.type.has_type_var_tuple_type` — the tvt-delegation path.
fn r2_snap_has_tvt(r2_ref: &str, res: &TypeResolver) -> bool {
    res.get(r2_ref).is_some_and(|s| s.has_type_var_tuple_type)
}

// `mypy.types.MYPYC_NATIVE_INT_NAMES` (fixed-width native ints, types.py:190-195).
const MYPYC_NATIVE_INT_NAMES: [&str; 4] = [
    "mypy_extensions.i64",
    "mypy_extensions.i32",
    "mypy_extensions.i16",
    "mypy_extensions.u8",
];

/// PyO3 entry mirroring `mypy.meet.is_overlapping_types`. The Python shim
/// guards TypeGuardedType, recursive pairs, TypeAliasType expansion, and
/// the `seen_types` recursion before calling; Rust decides non-alias cases
/// and resolves `TypeAliasType` operands through the alias resolver. Returns
/// `None` (defer to Python) for any deferred case.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_is_overlapping_types(
    left_bytes: &[u8],
    right_bytes: &[u8],
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    overlap_impl(
        &left,
        &right,
        strict_optional,
        ignore_promotions,
        overlap_for_overloads,
        resolver.resolver(),
        Some(resolver.alias_resolver()),
        0,
    )
}

/// `mypy.types.UnionType.relevant_items` including NoneType under strict
/// optional (narrow_declared_type keeps `None` items when strict_optional
/// is on; meet.rs's `relevant_items` always strips them).
///
/// Under strict_optional Python returns items unchanged (types.py:3784),
/// so alias items need no expansion there. In the non-strict filter (3787)
/// a top-level alias item expands via the resolver; none keeps the defer.
fn relevant_items_with_none(
    items: &[Type],
    strict_optional: bool,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
) -> Option<Vec<Type>> {
    if strict_optional {
        return Some(items.to_vec());
    }
    let mut out = Vec::new();
    for item in items {
        let proper_owned;
        let proper = match aliases {
            Some(a) => {
                proper_owned = crate::checkexpr_functions::get_proper_or_expand(item, a)?;
                &proper_owned
            }
            None => match item {
                Type::TypeAliasType { .. } => return None,
                t => t,
            },
        };
        if !matches!(proper, Type::NoneType) {
            out.push(item.clone());
        }
    }
    Some(out)
}

/// `mypy.meet.narrow_declared_type` (meet.py:216-348), Rust subset.
///
/// The Python shim resolves the `declared == narrowed` top-level equality
/// (object identity for most types, not structural) before calling us, on
/// the proper forms. This worker resolves `TypeAliasType` operands through
/// the alias resolver (mirroring `get_proper_type` at the top of the Python
/// body) and mirrors the meet.py body after that equality check. Returns
/// `None` (defer to Python) for any case that needs a live `TypeInfo`
/// outside our snapshot (TypeForm-normalization special cases) or that we
/// simply did not port; the Python shim re-runs the pure-Python visitor
/// unchanged.
///
/// Cross-branch recursion mirrors Python's mutual recursion through
/// `narrow_declared_type` -> `is_overlapping_types`/`is_subtype`/`meet_types`
/// (Rust overlap/meet/is_subtype), and the per-item recursion for unions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn narrow_rec(
    declared: &Type,
    narrowed: &Type,
    strict_optional: bool,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
    res: &TypeResolver,
) -> Option<Type> {
    let (declared_c, narrowed_c) = match aliases {
        Some(a) => (
            crate::checkexpr_functions::get_proper_or_expand(declared, a)?,
            crate::checkexpr_functions::get_proper_or_expand(narrowed, a)?,
        ),
        None => (declared.clone(), narrowed.clone()),
    };
    let declared_p = get_proper(&declared_c)?;
    let narrowed_p = get_proper(&narrowed_c)?;

    // meet.py:224: declared == narrowed (identity-equality branch handled
    // in the Python shim; any in-branch equality here resolves via the
    // proceed-paths below, which match the Python fallthrough).

    // meet.py:225-244: declared UnionType -> cross-product of the
    // overlapping/subtype items, simplified.
    if let Type::UnionType { items, .. } = declared_p {
        let declared_items = relevant_items_with_none(items, strict_optional, aliases)?;
        // The cross-product gates (overlap/is_subtype) are alias-transparent
        // in Python; expand alias items so the kernel decisions engage.
        // Recursion re-expands at the head, so expanded inputs stay parity.
        let declared_items = match aliases {
            Some(a) => declared_items
                .iter()
                .map(|t| crate::checkexpr_functions::get_proper_or_expand(t, a))
                .collect::<Option<Vec<_>>>()?,
            None => declared_items,
        };
        let narrowed_items = match narrowed_p {
            Type::UnionType { items, .. } => {
                relevant_items_with_none(items, strict_optional, aliases)?
            }
            _ => vec![narrowed_p.clone()],
        };
        let narrowed_items = match aliases {
            Some(a) => narrowed_items
                .iter()
                .map(|t| crate::checkexpr_functions::get_proper_or_expand(t, a))
                .collect::<Option<Vec<_>>>()?,
            None => narrowed_items,
        };
        let mut results = Vec::new();
        for d in &declared_items {
            for n in &narrowed_items {
                if overlap_impl(d, n, strict_optional, true, false, res, aliases, 0)?
                    || is_subtype(
                        n,
                        d,
                        &SubtypeContext::new(false, false, false, false, false, strict_optional),
                        res,
                    )?
                {
                    results.push(narrow_rec(d, n, strict_optional, aliases, res)?);
                }
            }
        }
        return make_simplified_union(
            &results,
            &SubtypeContext::new(false, false, false, false, false, strict_optional),
            res,
            true,
            false,
        );
    }

    // meet.py:246-256: enum/union overlap shortcut.
    if is_enum_overlapping_union(declared_p, narrowed_p, res)? {
        let Type::UnionType { items, .. } = narrowed_p else {
            return None;
        };
        let relevant = relevant_items_with_none(items, strict_optional, aliases)?;
        let mut results = Vec::new();
        for x in &relevant {
            results.push(narrow_rec(declared, x, strict_optional, aliases, res)?);
        }
        return make_simplified_union(
            &results,
            &SubtypeContext::new(false, false, false, false, false, strict_optional),
            res,
            true,
            false,
        );
    }

    // meet.py:257-263: declared TypeVarType, no type vars in original
    // narrowed, narrowed subtype of declared's upper bound.
    if let Type::TypeVarType { upper_bound, .. } = declared_p {
        if !has_type_vars_inner(narrowed)
            && is_subtype(
                narrowed,
                upper_bound,
                &SubtypeContext::new(false, false, false, false, false, strict_optional),
                res,
            )?
        {
            let new_ub = narrow_rec(upper_bound, narrowed, strict_optional, aliases, res)?;
            let mut t = declared_p.clone();
            if let Type::TypeVarType { upper_bound, .. } = &mut t {
                **upper_bound = new_ub;
            }
            return Some(t);
        }
    }

    // meet.py:264-270: narrowed TypeVarType, mirror image.
    if let Type::TypeVarType { upper_bound, .. } = narrowed_p {
        if !has_type_vars_inner(declared)
            && is_subtype(
                declared,
                upper_bound,
                &SubtypeContext::new(false, false, false, false, false, strict_optional),
                res,
            )?
        {
            let new_ub = narrow_rec(declared, upper_bound, strict_optional, aliases, res)?;
            let mut t = narrowed_p.clone();
            if let Type::TypeVarType { upper_bound, .. } = &mut t {
                **upper_bound = new_ub;
            }
            return Some(t);
        }
    }

    // meet.py:271-276: disjoint -> UninhabitedType (strict) / NoneType.
    if !overlap_impl(
        declared_p,
        narrowed_p,
        strict_optional,
        false,
        false,
        res,
        aliases,
        0,
    )? {
        return if strict_optional {
            Some(Type::UninhabitedType { ambiguous: false })
        } else {
            Some(Type::NoneType)
        };
    }

    // meet.py:277-280: narrowed UnionType -> per-item.
    if let Type::UnionType { items, .. } = narrowed_p {
        let relevant = relevant_items_with_none(items, strict_optional, aliases)?;
        let mut results = Vec::new();
        for x in &relevant {
            results.push(narrow_rec(declared, x, strict_optional, aliases, res)?);
        }
        return make_simplified_union(
            &results,
            &SubtypeContext::new(false, false, false, false, false, strict_optional),
            res,
            true,
            false,
        );
    }

    // meet.py:281-282: narrowed AnyType -> original_narrowed.
    if matches!(narrowed_p, Type::AnyType { .. }) {
        return Some(narrowed.clone());
    }

    // meet.py:283-284: narrowed TypeVarType, upper_bound subtype of
    // declared -> narrowed.
    if let Type::TypeVarType { upper_bound, .. } = narrowed_p {
        if is_subtype(
            upper_bound,
            declared_p,
            &SubtypeContext::new(false, false, false, false, false, strict_optional),
            res,
        )? {
            return Some(narrowed.clone());
        }
    }

    // meet.py:287-295: TypeType both sides -> Python's
    // TypeType.make_normalized (union-splitting); 296-306 declared +
    // narrowed metaclass -> TypeForm conversion; defer both.
    if matches!(declared_p, Type::TypeType { .. }) {
        return None;
    }

    // meet.py:307-317: declared Instance.
    if let Type::Instance {
        type_ref: d_ref, ..
    } = declared_p
    {
        let d_snap = res.get(d_ref)?;
        // meet.py:308-310: declared type has an alt_promote (native int)
        // -> cannot narrow -> unchanged.
        if d_snap.alt_promote_fullname.is_some() {
            return Some(declared.clone());
        }
        // meet.py:311-314: `narrowed` is an Instance whose alt_promote
        // points back at the declared type (`int` -> `i64`) -> unchanged.
        if let Type::Instance {
            type_ref: n_ref, ..
        } = narrowed_p
        {
            let n_snap = res.get(n_ref)?;
            if n_snap.alt_promote_fullname.as_deref() == Some(d_ref) {
                return Some(declared.clone());
            }
        }
        // meet.py:315-316: fall to meet_types(original, original).
        let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
        let r = meet_types(declared, narrowed, &ctx, res)?;
        return materialize_meet_result(r, declared, narrowed, strict_optional, res);
    }

    // meet.py:318-320: declared (TupleType, TypeType, LiteralType) -> meet.
    if matches!(declared_p, Type::TupleType { .. })
        || matches!(declared_p, Type::LiteralType { .. })
    {
        let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
        let r = meet_types(declared, narrowed, &ctx, res)?;
        return materialize_meet_result(r, declared, narrowed, strict_optional, res);
    }

    // meet.py:322-329: declared TypedDictType + narrowed `builtins.dict`
    // with all-Any args -> original_declared. Rust defers (TypedDictType
    // needs live TypeInfo); fall through.

    // meet.py:322-329: declared TypedDictType + narrowed `builtins.dict`
    // with all-Any args -> original_declared. TypedDictType carries callsite
    // extra attrs we can't reproduce; defer to Python.
    if matches!(declared_p, Type::TypedDictType { .. }) {
        return None;
    }

    // meet.py:331-334: both CallableType + type vars in declared ret_type
    // -> copy_modified(ret_type=narrow(...)). Rust is_subtype returns None
    // for CallableType pairs; defer to Python rather than mis-narrowing.
    if matches!(declared_p, Type::CallableType { .. }) {
        return None;
    }

    // meet.py:335-337: default -> original_narrowed.
    Some(narrowed.clone())
}

/// Materialize a `SetOpResult` from `meet_types` for `narrow_declared_type`.
/// `meet_types` emits SameS/SameT/Bottom/Any only. Any-typed result picks
/// `TypeOfAny.special_form` (6), matching meet.py's `meet_types` fallback
/// (`AnyType(TypeOfAny.special_form)`).
fn materialize_meet_result(
    r: crate::setops::SetOpResult,
    declared: &Type,
    narrowed: &Type,
    strict_optional: bool,
    _res: &TypeResolver,
) -> Option<Type> {
    use crate::setops::SetOpResult;
    match r {
        SetOpResult::SameS => Some(declared.clone()),
        SetOpResult::SameT => Some(narrowed.clone()),
        // Bottom encodes Python's TypeMeetVisitor.default: UninhabitedType
        // under strict optional, NoneType otherwise (meet.py:1541-1548).
        // The top-level meet_types shim already branches on the flag.
        SetOpResult::Bottom => Some(if strict_optional {
            Type::UninhabitedType { ambiguous: false }
        } else {
            Type::NoneType
        }),
        SetOpResult::Any => Some(Type::AnyType {
            // TypeOfAny.special_form (types.py:309), matching meet.py's
            // meet_types fallback AnyType(TypeOfAny.special_form).
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        }),
        SetOpResult::Encoded(bytes) => decode_type(&bytes),
        _ => None,
    }
}

/// `#[pyfunction]` entry for `mypy.meet.narrow_declared_type`.
///
/// The Python shim guarantees: no `TypeGuardedType`, no recursive pair,
/// and it resolves the top-level `declared == narrowed` identity branch
/// before us. `TypeAliasType` operands resolve through the alias resolver
/// inside `narrow_rec`. Returns serialized result bytes, or `None` (defer
/// to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_narrow_declared_type(
    declared_bytes: &[u8],
    narrowed_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let declared = decode_type(declared_bytes)?;
    let narrowed = decode_type(narrowed_bytes)?;
    let result = narrow_rec(
        &declared,
        &narrowed,
        strict_optional,
        Some(resolver.alias_resolver()),
        resolver.resolver(),
    )?;
    let mut wbuf = WriteBuffer::new();
    crate::wire::write_type(&mut wbuf, &result).ok()?;
    Some(wbuf.into_bytes())
}

/// `#[pyfunction]` entry for `mypy.typeops.get_possible_variants`
/// (meet.py:384-431).
///
/// Issue #526 wire seam: caller passes the serialized type; we return the
/// serialized variant list, or `None` (defer to Python) when a case needs a
/// live `TypeInfo` our snapshot cannot back (ParamSpec upper-bound MRO
/// lookup) or a `TypeAliasType` needs expansion (`get_proper` -> None).
#[pyfunction]
#[allow(dead_code)]
pub(crate) fn rust_get_possible_variants(
    typ_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let typ = decode_type(typ_bytes)?;
    let variants =
        get_possible_variants(&typ, resolver.resolver(), Some(resolver.alias_resolver()))?;
    let mut wbuf = WriteBuffer::new();
    crate::wire::write_type_list(&mut wbuf, &variants).ok()?;
    Some(wbuf.into_bytes())
}

// -----------------------------------------------------------------
// Issue #525: wire-seam #[pyfunction] wrappers for meet.py helpers.
// Each takes serialized Type bytes and returns Option<T> (None =

// defer to Python). The internal functions above already implement
// the logic; these wrappers just decode/encode at the seam.
// -----------------------------------------------------------------

/// `mypy.meet.is_object` (meet.py:425-427).
#[pyfunction]
pub(crate) fn rust_is_object(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    Some(is_object(&t))
}

/// `mypy.meet.is_tuple` (meet.py:872-876).
#[pyfunction]
pub(crate) fn rust_is_tuple(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    Some(is_tuple(&t))
}

/// `mypy.meet.is_none_object_overlap` (meet.py:429-434).
#[pyfunction]
pub(crate) fn rust_is_none_object_overlap(t1_bytes: &[u8], t2_bytes: &[u8]) -> Option<bool> {
    let t1 = decode_type(t1_bytes)?;
    let t2 = decode_type(t2_bytes)?;
    Some(is_none_object_overlap(&t1, &t2))
}

/// `mypy.meet.is_literal_in_union` (meet.py:416-422).
#[pyfunction]
pub(crate) fn rust_is_literal_in_union(x_bytes: &[u8], y_bytes: &[u8]) -> Option<bool> {
    let x = decode_type(x_bytes)?;
    let y = decode_type(y_bytes)?;
    is_literal_in_union(&x, &y)
}

/// `mypy.meet.is_enum_overlapping_union` (meet.py:403-413).
#[pyfunction]
pub(crate) fn rust_is_enum_overlapping_union(
    x_bytes: &[u8],
    y_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let x = decode_type(x_bytes)?;
    let y = decode_type(y_bytes)?;
    is_enum_overlapping_union(&x, &y, resolver.resolver())
}

/// `mypy.meet.are_related_types` (meet.py:447-456).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_are_related_types(
    left_bytes: &[u8],
    right_bytes: &[u8],
    proper_subtype: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        return None;
    }
    let ctx = SubtypeContext::new(
        false,
        false,
        false,
        ignore_promotions,
        proper_subtype,
        strict_optional,
    );
    let a = is_subtype(&left, &right, &ctx, resolver.resolver())?;
    let b = is_subtype(&right, &left, &ctx, resolver.resolver())?;
    Some(a || b)
}

/// `mypy.meet.is_overlapping_erased_types` (meet.py:818-824).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_is_overlapping_erased_types(
    left_bytes: &[u8],
    right_bytes: &[u8],
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        return None;
    }
    let left_erased = crate::argapprox::erase_type(&left, strict_optional, resolver.resolver())?;
    let right_erased = crate::argapprox::erase_type(&right, strict_optional, resolver.resolver())?;
    overlap(
        &left_erased,
        &right_erased,
        strict_optional,
        ignore_promotions,
        false,
        resolver.resolver(),
        0,
    )
}

/// `mypy.meet.are_typed_dicts_overlapping` (meet.py:786-807).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_are_typed_dicts_overlapping(
    left_bytes: &[u8],
    right_bytes: &[u8],
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let left = get_proper(&left)?;
    let right = get_proper(&right)?;
    are_typed_dicts_overlapping(left, right, &|a, b| {
        overlap(
            a,
            b,
            strict_optional,
            ignore_promotions,
            overlap_for_overloads,
            resolver.resolver(),
            0,
        )
    })
}

/// `mypy.meet.are_tuples_overlapping` (meet.py:810-843).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_are_tuples_overlapping(
    left_bytes: &[u8],
    right_bytes: &[u8],
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let left = get_proper(&left)?;
    let right = get_proper(&right)?;
    are_tuples_overlapping(left, right, &|a, b| {
        overlap(
            a,
            b,
            strict_optional,
            ignore_promotions,
            overlap_for_overloads,
            resolver.resolver(),
            0,
        )
    })
}

/// `mypy.meet.expand_tuple_if_possible` (meet.py:846-864).
#[pyfunction]
pub(crate) fn rust_expand_tuple_if_possible(tup_bytes: &[u8], target: usize) -> Option<Vec<u8>> {
    let tup = decode_type(tup_bytes)?;
    let result = expand_tuple_if_possible(&tup, target)?;
    let mut wbuf = WriteBuffer::new();
    crate::wire::write_type(&mut wbuf, &result).ok()?;
    Some(wbuf.into_bytes())
}

/// `mypy.meet.adjust_tuple` (meet.py:867-872).
/// Returns serialized TupleType bytes, or None (defer to Python —
/// Python does `left = adjust_tuple(left, right) or left`, so a None
/// result correctly keeps the original `left`).
#[pyfunction]
pub(crate) fn rust_adjust_tuple(left_bytes: &[u8], r_bytes: &[u8]) -> Option<Vec<u8>> {
    let left = decode_type(left_bytes)?;
    let r = decode_type(r_bytes)?;
    match adjust_tuple(&left, &r) {
        Some(t) => {
            let mut wbuf = WriteBuffer::new();
            crate::wire::write_type(&mut wbuf, &t).ok()?;
            Some(wbuf.into_bytes())
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    fn make_resolver() -> TypeResolver {
        let mut r = TypeResolver::new();
        for name in ["builtins.int", "builtins.str", "builtins.object"] {
            let mut s = TypeInfoSnapshot {
                fullname: name.to_string(),
                name: name.to_string(),
                ..Default::default()
            };
            s.mro.push(name.to_string());
            s.has_base.insert(name.to_string());
            r.insert(name.to_string(), s);
        }
        r
    }

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn uninhabited() -> Type {
        Type::UninhabitedType { ambiguous: false }
    }

    fn type_alias() -> Type {
        Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
        }
    }

    fn guarded(guard: &Type, typ: &Type) -> Type {
        Type::Instance {
            type_ref: "mypy.types.TypeGuardedType".to_string(),
            args: vec![guard.clone(), typ.clone()],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn alias_snap(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, target).expect("alias target must encode");
        crate::aliases::TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: buf.into_bytes(),
            ..Default::default()
        }
    }

    fn alias_ref(name: &str) -> Type {
        Type::TypeAliasType {
            type_ref: name.to_string(),
            args: Vec::new(),
        }
    }

    #[test]
    fn equal_unknown_identity_defers_to_python_shim() {
        // meet.py:222: declared == narrowed -> original_declared is handled
        // in the Python shim before Rust is reached, so narrow_rec must NOT
        // special-case identity. For equal types whose class is unknown to

        // the resolver, the overlap probe needs a TypeInfo lookup and defers
        // (None) rather than guessing. This pins that contract: the identity
        // branch is Python's, not Rust's.
        let r = make_resolver();
        let a = instance("a.A");
        let out = narrow_rec(&a, &a, true, None, &r);
        assert!(out.is_none());
    }

    #[test]
    fn disjoint_int_str_uninhabited_strict() {
        // meet.py:271-276: disjoint strict-optional -> UninhabitedType.
        let r = make_resolver();
        let out = narrow_rec(
            &instance("builtins.int"),
            &instance("builtins.str"),
            true,
            None,
            &r,
        );
        assert_eq!(out, Some(uninhabited()));
    }

    #[test]
    fn disjoint_int_str_none_non_strict() {
        // meet.py:271-276: disjoint non-strict-optional -> NoneType.
        let r = make_resolver();
        let out = narrow_rec(
            &instance("builtins.int"),
            &instance("builtins.str"),
            false,
            None,
            &r,
        );
        assert_eq!(out, Some(Type::NoneType));
    }

    #[test]
    fn union_narrowed_maps_per_item() {
        // meet.py:277-280: narrowed UnionType -> per-item results, simplified.
        // Union[int, str] narrowed by int -> Union[int, Uninhabited] which
        // simplifies to int (empty/uninhabited members are dropped).
        let r = make_resolver();
        let n = Type::UnionType {
            items: vec![instance("builtins.int"), instance("builtins.str")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let out = narrow_rec(&n, &instance("builtins.int"), true, None, &r).unwrap();
        assert!(matches!(out, Type::Instance { ref type_ref, .. } if type_ref == "builtins.int"));
    }

    #[test]
    fn narrowed_any_returns_original_narrowed() {
        // meet.py:281-282: narrowed AnyType -> original_narrowed.
        let r = make_resolver();
        let d = instance("builtins.int");
        let out = narrow_rec(&d, &any_type(), true, None, &r).unwrap();
        assert!(matches!(out, Type::AnyType { .. }));
    }

    #[test]
    fn instance_instance_meet_same_branch() {
        // meet.py:315-316: Instance + Instance -> meet_types; SameS -> declared.
        let r = make_resolver();
        let d = instance("builtins.int");
        let out = narrow_rec(&d, &d, true, None, &r);
        assert_eq!(out, Some(d));
    }

    #[test]
    fn tuple_declared_defers_empty_resolver() {
        // meet.rs:320-325: declared TupleType hits meet_types, which needs
        // live tuple/TypeInfo lookups that an empty snapshot cannot back.
        // Real parity for TupleType narrowing therefore defers to Python.
        let r = make_resolver();
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![instance("builtins.int")],
            implicit: false,
        };
        let out = narrow_rec(&t, &instance("builtins.int"), true, None, &r);
        assert!(out.is_none());
    }

    #[test]
    fn type_var_declared_narrows_upper_bound() {
        // meet.py:257-263: declared TypeVarType with narrowed a subtype of
        // the upper bound -> copy_modified(upper_bound=narrow(...)).
        let r = make_resolver();
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "mod".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(Type::NoneType),
            variance: crate::subtypes::INVARIANT,
            meta_level: 0,
        };
        let out = narrow_rec(&tv, &instance("builtins.int"), true, None, &r).unwrap();
        match out {
            Type::TypeVarType { upper_bound, .. } => match *upper_bound {
                Type::Instance { ref type_ref, .. } => {
                    assert_eq!(type_ref, "builtins.int");
                }
                other => panic!("expected narrowed upper_bound, got {other:?}"),
            },
            other => panic!("expected TypeVarType, got {other:?}"),
        }
    }

    #[test]
    fn wrapper_delegates_type_guarded_to_python() {
        // TypeGuardedType handling needs live comparison of the guard type;
        // wrapper defers instead of mis-narrowing.
        let r = make_resolver();
        let d = instance("builtins.int");
        let g = guarded(&instance("builtins.bool"), &instance("builtins.str"));
        let out = narrow_rec(&d, &g, true, None, &r);
        assert!(out.is_none());
    }

    #[test]
    fn rust_narrow_declared_type_serializes_result() {
        // The wrapper encodes the serialized bytes for the seam. We test the
        // same path via narrow_rec (wire round-trip through the real
        // encoder/decoder, no pyclass needed) for an alias-free, non-recursive

        // proper pair.
        let r = make_resolver();
        let d = instance("builtins.int");
        let n = instance("builtins.str");
        let out = narrow_rec(&d, &n, true, None, &r).expect("disjoint pair should resolve");
        assert_eq!(out, Type::UninhabitedType { ambiguous: false });
        // The bytes the wrapper hands back must round-trip through the wire
        // encoder/decoder used by the real seam.
        let mut buf = WriteBuffer::new();
        crate::wire::write_type(&mut buf, &out).expect("write type");
        let bytes = buf.into_bytes();
        let mut rbuf = ReadBuffer::new(&bytes);
        let decoded = crate::wire::read_type(&mut rbuf, None).expect("decode");
        assert_eq!(decoded, uninhabited());
    }

    #[test]
    fn rust_narrow_declared_type_defers_alias() {
        // meet.rs:1243-1248: TypeAliasType on either side -> None (the Python
        // shim expands aliases itself; we must not cross a poisoned type).
        let r = make_resolver();
        let d = type_alias();
        let n = instance("builtins.int");
        let out = narrow_rec(&d, &n, true, None, &r);
        assert!(out.is_none());
    }

    #[test]
    fn rust_narrow_declared_type_defers_garbage_bytes() {
        // Seam-level decode_garbage -> None. narrow_rec only sees decoded
        // types, so the garbage-byte rejection lives in decode_type, the
        // decoder the wrapper calls first. Assert that path directly.
        assert!(decode_type(b"\xff\xff").is_none());
        assert!(decode_type(&[]).is_none());
    }

    // ------------------------------------------------------------------
    // alias expansion (get_proper_or_expand) on the meet seams
    // ------------------------------------------------------------------

    #[test]
    fn get_possible_variants_alias_expands_to_union_items() {
        // get_possible_variants ran `Type::TypeAliasType => None` before
        // #874; Python calls get_proper_type at the top (meet.py:384), so a
        // resolvable alias must yield its target's variants natively.
        let r = make_resolver();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        let target = Type::UnionType {
            items: vec![instance("builtins.int"), instance("builtins.str")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        aliases.insert("mod.U".to_string(), alias_snap("mod.U", &target));
        let alias = alias_ref("mod.U");
        assert_eq!(
            get_possible_variants(&alias, &r, Some(&aliases)),
            Some(vec![instance("builtins.int"), instance("builtins.str")])
        );
        // No snapshot -> the pre-#874 TypeAliasType defer is preserved.
        let empty = crate::aliases::TypeAliasResolver::new();
        assert!(get_possible_variants(&alias, &r, Some(&empty)).is_none());
    }

    #[test]
    fn overlap_erased_operand_is_true() {
        // meet.py:568: Unbound/Erased/Deleted operands overlap (True). The overlap
        // shim serializes Erased operands unfiltered, so the wire tag-122 leaf
        // decides step 1; pre-#1185 this pair fell to the tie check: Some(false).
        let r = make_resolver();
        assert_eq!(
            overlap_impl(
                &instance("builtins.int"),
                &Type::ErasedType,
                true,
                false,
                false,
                &r,
                None,
                0,
            ),
            Some(true)
        );
        assert_eq!(
            overlap_impl(
                &Type::ErasedType,
                &instance("builtins.int"),
                true,
                false,
                false,
                &r,
                None,
                0,
            ),
            Some(true)
        );
    }

    #[test]
    fn overlap_alias_resolves_via_resolver() {
        // A `Type::TypeAliasType` operand that used to defer now resolves
        // through the alias per recursion level (meet.py:556).
        let r = make_resolver();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.A".to_string(),
            alias_snap("mod.A", &instance("builtins.int")),
        );
        let alias = alias_ref("mod.A");
        // alias -> builtins.int overlapping int: complete via subtype.
        assert_eq!(
            overlap_impl(
                &alias,
                &instance("builtins.int"),
                true,
                false,
                false,
                &r,
                Some(&aliases),
                0,
            ),
            Some(true)
        );
        // Missing snapshot defers (matches get_proper_type's inability to
        // expand an unresolvable alias).
        let empty = crate::aliases::TypeAliasResolver::new();
        assert!(overlap_impl(
            &alias,
            &instance("builtins.str"),
            true,
            false,
            false,
            &r,
            Some(&empty),
            0,
        )
        .is_none());
        // The public alias-less `overlap` entry still defers (no resolver).
        assert!(overlap(&alias, &instance("builtins.int"), true, false, false, &r, 0).is_none());
    }

    #[test]
    fn narrow_alias_expands_to_target() {
        // narrow_declared_type runs get_proper_type at the top; an alias
        // operand now expands. narrowed Any returns original_narrowed
        // (meet.py:281-282), a meet_types-free branch -> deterministic.
        let r = make_resolver();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.A".to_string(),
            alias_snap("mod.A", &instance("builtins.int")),
        );
        let alias = alias_ref("mod.A");
        // Without the alias resolver: get_proper(alias) is None -> defer.
        assert!(narrow_rec(&alias, &any_type(), true, None, &r).is_none());
        // With it: declared expands to int, narrowed Any -> Any.
        assert_eq!(
            narrow_rec(&alias, &any_type(), true, Some(&aliases), &r),
            Some(any_type())
        );
    }

    #[test]
    fn materialize_meet_any_is_special_form() {
        // meet.py's meet_types fallback is AnyType(TypeOfAny.special_form)
        // (types.py:309), not unannotated (0).
        let r = TypeResolver::new();
        let out = materialize_meet_result(
            crate::setops::SetOpResult::Any,
            &instance("builtins.int"),
            &instance("builtins.str"),
            true,
            &r,
        )
        .unwrap();
        match out {
            Type::AnyType { type_of_any, .. } => assert_eq!(type_of_any, 6),
            other => panic!("expected AnyType, got {other:?}"),
        }
    }

    #[test]
    fn materialize_meet_encoded_roundtrips() {
        // Same-ref generic instances emit Encoded(bytes) from
        // visit_instance_meet_args; materialize_meet_result must decode
        // them instead of dropping to None (Python fallback).
        let r = TypeResolver::new();
        let list_i = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![instance("builtins.int")],
            last_known_value: None,
            extra_attrs: None,
        };
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, &list_i).expect("Instance must encode");
        let out = materialize_meet_result(
            crate::setops::SetOpResult::Encoded(buf.into_bytes()),
            &list_i,
            &any_type(),
            true,
            &r,
        );
        assert_eq!(out.as_ref(), Some(&list_i));
    }

    #[test]
    fn narrow_same_ref_generic_via_meet_materializes_encoded() {
        // G[object] ~ G[Any]: neither is a proper subtype of the other, so
        // visit_instance_meet_args runs and the result instance rides the
        // Encoded arm; narrow_declared_type(G[object], G[Any]) == G[object].
        let mut r = make_resolver();
        let mut g = TypeInfoSnapshot {
            fullname: "g.G".to_string(),
            name: "G".to_string(),
            ..Default::default()
        };
        // Python's mro and has_base include the class itself;
        // visit_instance_nominal keys the same-ref branch on
        // has_base(right_ref).
        g.mro.push("g.G".to_string());
        g.mro.push("builtins.object".to_string());
        g.has_base.insert("g.G".to_string());
        g.has_base.insert("builtins.object".to_string());
        g.type_vars_with_variance.push(("T".to_string(), 1, 0));
        r.insert("g.G".to_string(), g);
        let g_obj = Type::Instance {
            type_ref: "g.G".to_string(),
            args: vec![instance("builtins.object")],
            last_known_value: None,
            extra_attrs: None,
        };
        let g_any = Type::Instance {
            type_ref: "g.G".to_string(),
            args: vec![any_type()],
            last_known_value: None,
            extra_attrs: None,
        };
        let out = meet_types(&g_obj, &g_any, &SubtypeContext::default(), &r)
            .expect("same-ref G[object] ~ G[Any] must decide");
        let bytes = match out {
            crate::setops::SetOpResult::Encoded(bytes) => bytes,
            other => panic!("expected Encoded, got {other:?}"),
        };
        assert_eq!(decode_type(&bytes).as_ref(), Some(&g_obj));
    }
}

