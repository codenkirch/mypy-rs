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

use crate::setops::union_make_union;
use crate::subtypes::{is_subtype, map_instance_to_supertype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::find_unpack_in_list_inner;
use crate::wire::{self, LiteralValue, ReadBuffer, Type};

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

/// `mypy.typeops.get_possible_variants` (meet.py:353-400).
fn get_possible_variants(t: &Type, res: &TypeResolver) -> Option<Vec<Type>> {
    match t {
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
        Type::TypeAliasType { .. } => None,
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
#[allow(clippy::too_many_arguments)]
fn overlap(
    left: &Type,
    right: &Type,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    res: &TypeResolver,
    depth: i64,
) -> Option<bool> {
    if depth > MAX_DEPTH {
        return None;
    }

    // 1. illegal types: Unbound/Deleted in Python are an overlap (True).
    // ErasedType is not on the wire; PartialType corrupts serialization so
    // the shim asserts before we are reached. Shorthand with `||` = Rust's
    // operator (Python's `and`), correct boolean logic.
    if matches!(left, Type::UnboundType { .. } | Type::DeletedType { .. })
        || matches!(right, Type::UnboundType { .. } | Type::DeletedType { .. })
    {
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
    let lv = get_possible_variants(&left, res)?;
    let rv = get_possible_variants(&right, res)?;
    if lv.len() > 1 || rv.len() > 1 || is_type_var_like(&left) || is_type_var_like(&right) {
        for l in &lv {
            for r in &rv {
                if overlap(
                    l,
                    r,
                    strict_optional,
                    ignore_promotions,
                    overlap_for_overloads,
                    res,
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
            overlap(
                a,
                b,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
                depth + 1,
            )
        });
    }
    if typed_dict_mapping_pair(&left, &right, res)? || typed_dict_mapping_pair(&right, &left, res)?
    {
        // needs typed_dict_mapping_overlap (deferred).
        return None;
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
            overlap(
                a,
                b,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
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
            return overlap(
                li,
                ri,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
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
        // are_parameters_compatible not ported -> defer.
        return None;
    }
    if matches!(left, Type::Parameters(_)) || matches!(right, Type::Parameters(_)) {
        return Some(false);
    }
    if let (Type::CallableType { .. }, Type::CallableType { .. }) = (&left, &right) {
        // is_callable_compatible (bidirectional) not ported -> defer.
        return None;
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
    depth: i64,
) -> Option<bool> {
    let Type::TypeType { item, .. } = get_proper(left)? else {
        return Some(false);
    };
    match get_proper(right)? {
        Type::CallableType { ret_type, .. } => overlap(
            item,
            ret_type,
            strict_optional,
            ignore_promotions,
            overlap_for_overloads,
            res,
            depth + 1,
        ),
        Type::Instance {
            type_ref: r_ref, ..
        } => match get_proper(item)? {
            Type::Instance {
                type_ref: item_ref, ..
            } => {
                let snap = res.get(item_ref)?;
                if let Some(meta) = &snap.metaclass_fullname {
                    let meta_inst = Type::Instance {
                        type_ref: meta.clone(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    return overlap(
                        &meta_inst,
                        right,
                        strict_optional,
                        ignore_promotions,
                        overlap_for_overloads,
                        res,
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
            if !overlap(
                la,
                ra,
                strict_optional,
                ignore_promotions,
                overlap_for_overloads,
                res,
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
/// the `seen_types` recursion before calling; Rust only decides non-alias
/// cases. Returns `None` (defer to Python) for any deferred case.
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
    // TypeAliasType cannot be expanded without the alias resolver: let
    // Python (get_proper_types) handle it.
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        return None;
    }
    overlap(
        &left,
        &right,
        strict_optional,
        ignore_promotions,
        overlap_for_overloads,
        resolver.resolver(),
        0,
    )
}
