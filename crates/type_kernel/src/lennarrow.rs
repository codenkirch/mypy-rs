//! Issue #493: len-based type narrowing for tuples.
//!
//! Ports the type-computation core of `mypy.checker`'s len-narrowing family:
//! * `narrow_with_len` (checker.py:7972-7995) — dispatch on TupleType /
//!   Instance / UnionType.
//! * `refine_tuple_type_with_len` (checker.py:8000-8060) — fixed-length,
//!   TypeVarTuple-unpack, and homogeneous-variadic cases.
//! * `refine_instance_type_with_len` (checker.py:8070-8100) — map Instance
//!   to `builtins.tuple` supertype, construct fixed/variadic tuples.
//! * `can_be_narrowed_with_len` (checker.py:7840-7855) — predicate used by
//!   the UnionType recursion to skip non-narrowable items.
//!
//! The heavy AST-walking logic (`find_tuple_len_narrowing`) stays in Python:
//! it groups comparison operands, detects `len()` calls, and resolves
//! literal-int expressions. Only the pure type algebra crosses the wire.
//!
//! Returns `Some((yes_bytes, no_bytes))` on success, `None` to defer to the
//! pure-Python path. The Python shim in `mypy.checker` gates each call
//! behind `Options.native_type_kernel`.

use pyo3::prelude::*;

use crate::checker_helpers::custom_special_method_inner;
use crate::setops;
use crate::subtypes::{map_instance_to_supertype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Wire codec helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// `get_proper_type` — a `TypeAliasType` has no proper form in the wire
/// representation (its target is unresolved), so defer. Otherwise the wire
/// type is already proper.
fn get_proper_type(t: &Type) -> Option<Type> {
    if let Type::TypeAliasType { .. } = t {
        return None;
    }
    Some(t.clone())
}

// ---------------------------------------------------------------------------
// Operator tables (mirror mypy.operators)
// ---------------------------------------------------------------------------

/// `int_op_to_method[op](a, b)` — comparison dispatch.
fn int_op_to_method(op: &str, a: i64, b: i64) -> bool {
    match op {
        "==" | "is" => a == b,
        "!=" | "is not" => a != b,
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        _ => false,
    }
}

/// `neg_ops[op]` — negated comparison operator (checker.py:7995, operators.py).
fn neg_op(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("!="),
        "!=" => Some("=="),
        "is" => Some("is not"),
        "is not" => Some("is"),
        "<" => Some(">="),
        "<=" => Some(">"),
        ">" => Some("<="),
        ">=" => Some("<"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// can_be_narrowed_with_len
// ---------------------------------------------------------------------------

/// `can_be_narrowed_with_len` (checker.py:7840-7855).
///
/// True for `TupleType` (no unpack, or unpack with `builtins.tuple` fallback),
/// `Instance` with `builtins.tuple` base, and unions of those. Returns `None`
/// to defer when a `custom_special_method` or `has_base` check needs a
/// snapshot that is missing from the resolver.
pub(crate) fn can_be_narrowed_with_len(typ: &Type, resolver: &TypeResolver) -> Option<bool> {
    // If user overrides builtin behavior, we can't do anything.
    if custom_special_method_inner(typ, "__len__", false, resolver)? {
        return Some(false);
    }
    let p_typ = get_proper_type(typ)?;
    match p_typ {
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            let has_unpack = items.iter().any(|t| matches!(t, Type::UnpackType { .. }));
            if has_unpack {
                let is_builtins_tuple = match partial_fallback.as_ref() {
                    Type::Instance { type_ref, .. } => type_ref == "builtins.tuple",
                    _ => false,
                };
                Some(is_builtins_tuple)
            } else {
                Some(true)
            }
        }
        Type::Instance { type_ref, .. } => {
            let snap = resolver.get(&type_ref)?;
            Some(snap.has_base("builtins.tuple"))
        }
        Type::UnionType { items, .. } => {
            let mut any_true = false;
            for t in &items {
                match can_be_narrowed_with_len(t, resolver) {
                    Some(true) => {
                        any_true = true;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any_true)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// refine_tuple_type_with_len
// ---------------------------------------------------------------------------

/// `refine_tuple_type_with_len` (checker.py:8000-8060).
///
/// Returns `Some((yes_type, no_type))` or `None` to defer.
fn refine_tuple_type_with_len(
    typ: &Type,
    op: &str,
    size: i64,
    _resolver: &TypeResolver,
) -> Option<(Type, Type)> {
    let Type::TupleType {
        items,
        partial_fallback,
        implicit,
    } = typ
    else {
        return None;
    };
    let items = items.clone();
    let partial_fallback = partial_fallback.clone();
    let implicit = *implicit;

    // find_unpack_in_list — single UnpackType assertion.
    let unpack_index = items
        .iter()
        .position(|t| matches!(t, Type::UnpackType { .. }));

    let tuple_len = items.len() as i64;

    let Some(unpack_index) = unpack_index else {
        // Fixed length tuple: trivially reachable or not.
        if int_op_to_method(op, tuple_len, size) {
            return Some((typ.clone(), Type::UninhabitedType { ambiguous: false }));
        }
        return Some((Type::UninhabitedType { ambiguous: false }, typ.clone()));
    };

    let Some(Type::UnpackType {
        typ: unpack_type, ..
    }) = items.get(unpack_index).cloned()
    else {
        return None;
    };
    let unpacked = get_proper_type(&unpack_type)?;

    if let Type::TypeVarTupleType {
        tuple_fallback,
        name,
        fullname,
        raw_id,
        namespace,
        upper_bound,
        default,
        min_len: tv_min_len,
    } = unpacked
    {
        // TypeVarTuple unpack: reachability + min_len restrictions.
        let min_len = tuple_len - 1 + tv_min_len;
        if op == "==" || op == "is" {
            if min_len <= size {
                return Some((typ.clone(), typ.clone()));
            }
            return Some((Type::UninhabitedType { ambiguous: false }, typ.clone()));
        } else if op == "<" || op == "<=" {
            let size = if op == "<=" { size + 1 } else { size };
            if min_len < size {
                let prefix = items[..unpack_index].to_vec();
                let suffix = items[unpack_index + 1..].to_vec();
                // UnpackType(unpacked.copy_modified(min_len=size - typ.length() + 1))
                let new_min_len = size - tuple_len + 1;
                let new_tvt = Type::TypeVarTupleType {
                    tuple_fallback,
                    name,
                    fullname,
                    raw_id,
                    namespace,
                    upper_bound,
                    default,
                    min_len: new_min_len,
                };
                let new_unpack = Type::UnpackType {
                    typ: Box::new(new_tvt),
                    from_star_syntax: false,
                };
                let new_items = [&prefix[..], &[new_unpack]].concat();
                let new_items = [new_items, suffix].concat();
                let no_type = Type::TupleType {
                    partial_fallback,
                    items: new_items,
                    implicit,
                };
                return Some((typ.clone(), no_type));
            }
            return Some((Type::UninhabitedType { ambiguous: false }, typ.clone()));
        } else {
            // op in (">", ">="): delegate to neg_op and swap.
            let neg = neg_op(op)?;
            let (yes_type, no_type) = refine_tuple_type_with_len(typ, neg, size, _resolver)?;
            return Some((no_type, yes_type));
        }
    }

    // Homogeneous variadic: unpacked is Instance with fullname "builtins.tuple".
    let Type::Instance {
        type_ref: unpacked_ref,
        args: unpacked_args,
        ..
    } = &unpacked
    else {
        return None;
    };
    if unpacked_ref != "builtins.tuple" {
        return None;
    }
    let arg = unpacked_args.first().cloned()?;
    let min_len = tuple_len - 1;
    let prefix = items[..unpack_index].to_vec();
    let suffix = items[unpack_index + 1..].to_vec();

    if op == "==" || op == "is" {
        if min_len <= size {
            let count = (size - min_len) as usize;
            let mut new_items = prefix.clone();
            new_items.extend(std::iter::repeat_n(arg.clone(), count));
            new_items.extend(suffix.clone());
            let yes_type = Type::TupleType {
                partial_fallback: partial_fallback.clone(),
                items: new_items,
                implicit,
            };
            Some((yes_type, typ.clone()))
        } else {
            Some((Type::UninhabitedType { ambiguous: false }, typ.clone()))
        }
    } else if op == "<" || op == "<=" {
        let size = if op == "<=" { size + 1 } else { size };
        if min_len < size {
            // no_type: prefix + [arg] * (size - min_len) + [unpack] + suffix
            let no_count = (size - min_len) as usize;
            let mut no_items = prefix.clone();
            no_items.extend(std::iter::repeat_n(arg.clone(), no_count));
            // Reconstruct the original unpack for the no_type.
            no_items.push(Type::UnpackType {
                typ: Box::new(unpacked.clone()),
                from_star_syntax: false,
            });
            no_items.extend(suffix.clone());
            let no_type = Type::TupleType {
                partial_fallback: partial_fallback.clone(),
                items: no_items,
                implicit,
            };
            // yes_items: for n in range(size - min_len): prefix + [arg]*n + suffix
            let range = (size - min_len) as usize;
            let mut yes_items: Vec<Type> = Vec::with_capacity(range);
            for n in 0..range {
                let mut items_n = prefix.clone();
                items_n.extend(std::iter::repeat_n(arg.clone(), n));
                items_n.extend(suffix.clone());
                yes_items.push(Type::TupleType {
                    partial_fallback: partial_fallback.clone(),
                    items: items_n,
                    implicit,
                });
            }
            let yes_type = setops::union_make_union(yes_items);
            Some((yes_type, no_type))
        } else {
            Some((Type::UninhabitedType { ambiguous: false }, typ.clone()))
        }
    } else {
        // op in (">", ">="): delegate to neg_op and swap.
        let neg = neg_op(op)?;
        let (yes_type, no_type) = refine_tuple_type_with_len(typ, neg, size, _resolver)?;
        Some((no_type, yes_type))
    }
}

// ---------------------------------------------------------------------------
// refine_instance_type_with_len
// ---------------------------------------------------------------------------

/// `refine_instance_type_with_len` (checker.py:8070-8100).
///
/// Returns `Some((yes_type, no_type))` or `None` to defer.
fn refine_instance_type_with_len(
    typ: &Type,
    op: &str,
    size: i64,
    precise_tuple: bool,
    resolver: &TypeResolver,
) -> Option<(Type, Type)> {
    let Type::Instance {
        type_ref,
        args: instance_args,
        ..
    } = typ
    else {
        return None;
    };
    // base = map_instance_to_supertype(typ, builtins.tuple)
    let mapped_args =
        map_instance_to_supertype(type_ref, instance_args, "builtins.tuple", resolver)?;
    let base = Type::Instance {
        type_ref: "builtins.tuple".to_string(),
        args: mapped_args,
        last_known_value: None,
        extra_attrs: None,
    };
    let arg = match &base {
        Type::Instance { args, .. } => args.first().cloned()?,
        _ => return None,
    };
    // allow_precise = PRECISE_TUPLE_TYPES in options and typ.fullname == "builtins.tuple"
    let allow_precise = precise_tuple && type_ref == "builtins.tuple";

    if op == "==" || op == "is" {
        // TupleType(items=[arg] * size, fallback=typ), typ
        let yes_items = std::iter::repeat_n(arg.clone(), size as usize).collect::<Vec<_>>();
        let yes_type = Type::TupleType {
            partial_fallback: Box::new(typ.clone()),
            items: yes_items,
            implicit: false,
        };
        Some((yes_type, typ.clone()))
    } else if op == "<" || op == "<=" {
        let size = if op == "<=" { size + 1 } else { size };
        let no_type = if allow_precise {
            // UnpackType(named_generic_type("builtins.tuple", [arg]))
            let tuple_inst = Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![arg.clone()],
                last_known_value: None,
                extra_attrs: None,
            };
            let unpack = Type::UnpackType {
                typ: Box::new(tuple_inst),
                from_star_syntax: false,
            };
            let mut no_items = std::iter::repeat_n(arg.clone(), size as usize).collect::<Vec<_>>();
            no_items.push(unpack);
            Type::TupleType {
                partial_fallback: Box::new(typ.clone()),
                items: no_items,
                implicit: false,
            }
        } else {
            typ.clone()
        };
        let yes_type = if allow_precise {
            let mut items: Vec<Type> = Vec::with_capacity(size as usize);
            for n in 0..size as usize {
                let yes_items = std::iter::repeat_n(arg.clone(), n).collect::<Vec<_>>();
                items.push(Type::TupleType {
                    partial_fallback: Box::new(typ.clone()),
                    items: yes_items,
                    implicit: false,
                });
            }
            setops::union_make_union(items)
        } else {
            typ.clone()
        };
        Some((yes_type, no_type))
    } else {
        // op in (">", ">="): delegate to neg_op and swap.
        let neg = neg_op(op)?;
        let (yes_type, no_type) =
            refine_instance_type_with_len(typ, neg, size, precise_tuple, resolver)?;
        Some((no_type, yes_type))
    }
}

// ---------------------------------------------------------------------------
// narrow_with_len — public entry point
// ---------------------------------------------------------------------------

/// `mypy.checker.narrow_with_len` (checker.py:7972-7995).
///
/// Dispatches to `refine_tuple_type_with_len` / `refine_instance_type_with_len`
/// or recurses over `UnionType` items. Returns `Some((yes_bytes, no_bytes))`
/// on success, `None` to defer to the pure-Python path.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_narrow_with_len(
    typ_bytes: &[u8],
    op: &str,
    size: i64,
    strict_optional: bool,
    precise_tuple: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(Vec<u8>, Vec<u8>)>> {
    let _ = strict_optional; // not used in len-narrowing

    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let p_typ = match get_proper_type(&typ) {
        Some(t) => t,
        None => return Ok(None),
    };

    let (yes_type, no_type) =
        match narrow_with_len_inner(&p_typ, op, size, precise_tuple, resolver.resolver()) {
            Some(pair) => pair,
            None => return Ok(None),
        };

    let yes_bytes = match encode_type(&yes_type) {
        Some(b) => b,
        None => return Ok(None),
    };
    let no_bytes = match encode_type(&no_type) {
        Some(b) => b,
        None => return Ok(None),
    };
    Ok(Some((yes_bytes, no_bytes)))
}

/// Inner driver for `narrow_with_len`. Returns `Some((yes_type, no_type))`
/// or `None` to defer.
fn narrow_with_len_inner(
    p_typ: &Type,
    op: &str,
    size: i64,
    precise_tuple: bool,
    resolver: &TypeResolver,
) -> Option<(Type, Type)> {
    match p_typ {
        Type::TupleType { .. } => refine_tuple_type_with_len(p_typ, op, size, resolver),
        Type::Instance { .. } => {
            refine_instance_type_with_len(p_typ, op, size, precise_tuple, resolver)
        }
        Type::UnionType { items, .. } => {
            let mut yes_types: Vec<Type> = Vec::new();
            let mut no_types: Vec<Type> = Vec::new();
            let mut other_types: Vec<Type> = Vec::new();
            for t in items {
                match can_be_narrowed_with_len(t, resolver) {
                    Some(true) => {}
                    Some(false) => {
                        other_types.push(t.clone());
                        continue;
                    }
                    None => return None,
                }
                let (yt, nt) = narrow_with_len_inner(t, op, size, precise_tuple, resolver)?;
                yes_types.push(yt);
                no_types.push(nt);
            }
            yes_types.extend(other_types.clone());
            no_types.extend(other_types);
            let ctx = SubtypeContext::new(false, false, false, true, true, true);
            let yes_type = setops::make_simplified_union(&yes_types, &ctx, resolver, false, false)?;
            let no_type = setops::make_simplified_union(&no_types, &ctx, resolver, false, false)?;
            Some((yes_type, no_type))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// rust_can_be_narrowed_with_len — exported gate predicate (#1065)
// ---------------------------------------------------------------------------

/// `TypeChecker.can_be_narrowed_with_len` (checker.py:9267).
///
/// Returns the bool decision, or `None` to defer to the pure-Python path
/// (undecodable wire bytes, an unresolved resolver snapshot, or an alias
/// target that has no proper form on the wire).
#[pyfunction]
pub(crate) fn rust_can_be_narrowed_with_len(
    typ_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(can_be_narrowed_with_len(&typ, resolver.resolver()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use std::collections::HashSet;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_snapshot(fullname: &str) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_string(),
            mro: vec![fullname.to_string()],
            has_base: HashSet::from([fullname.to_string()]),
            ..Default::default()
        }
    }

    fn make_tuple(fallback_ref: &str, items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(make_instance(fallback_ref, vec![])),
            items,
            implicit: false,
        }
    }

    #[test]
    fn test_fixed_tuple_true() {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        let t = make_tuple("builtins.tuple", vec![]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(true));
    }

    #[test]
    fn test_unpack_tuple_builtins_fallback_true() {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        // Mirrors tuple[int, ...]: the unpack wraps tuple[int].
        let unpack = Type::UnpackType {
            typ: Box::new(make_instance(
                "builtins.tuple",
                vec![Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                }],
            )),
            from_star_syntax: false,
        };
        let t = make_tuple("builtins.tuple", vec![unpack]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(true));
    }

    #[test]
    fn test_unpack_tuple_other_fallback_false() {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        r.insert("mymod.Fake".to_string(), make_snapshot("mymod.Fake"));
        let unpack = Type::UnpackType {
            typ: Box::new(make_instance(
                "builtins.tuple",
                vec![Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                }],
            )),
            from_star_syntax: false,
        };
        let t = make_tuple("mymod.Fake", vec![unpack]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(false));
    }

    #[test]
    fn test_tuple_instance_true() {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        let t = make_instance("builtins.tuple", vec![]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(true));
    }

    #[test]
    fn test_non_tuple_instance_false() {
        let mut r = TypeResolver::new();
        r.insert("builtins.int".to_string(), make_snapshot("builtins.int"));
        let t = make_instance("builtins.int", vec![]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(false));
    }

    #[test]
    fn test_custom_len_false() {
        let mut r = TypeResolver::new();
        let mut snap = make_snapshot("mymod.Len");
        snap.member_definers
            .insert("__len__".to_string(), (0, "mymod.Len".to_string()));
        r.insert("mymod.Len".to_string(), snap);
        let t = make_instance("mymod.Len", vec![]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(false));
    }

    #[test]
    fn test_missing_snapshot_defers() {
        let r = TypeResolver::new();
        let t = make_instance("mymod.Unknown", vec![]);
        assert_eq!(can_be_narrowed_with_len(&t, &r), None);
    }

    #[test]
    fn test_union_with_tuple_true() {
        let mut r = TypeResolver::new();
        r.insert(
            "builtins.tuple".to_string(),
            make_snapshot("builtins.tuple"),
        );
        r.insert("builtins.int".to_string(), make_snapshot("builtins.int"));
        let t = Type::UnionType {
            items: vec![
                make_instance("builtins.tuple", vec![]),
                make_instance("builtins.int", vec![]),
            ],
            uses_pep604_syntax: false,
            can_be_true: false,
            can_be_false: false,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(true));
    }

    #[test]
    fn test_union_without_tuple_false() {
        let mut r = TypeResolver::new();
        r.insert("builtins.int".to_string(), make_snapshot("builtins.int"));
        let t = Type::UnionType {
            items: vec![
                make_instance("builtins.int", vec![]),
                Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None,
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: false,
            can_be_false: false,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        // AnyType is treated as custom-__len__ (uncertain) -> False.
        assert_eq!(can_be_narrowed_with_len(&t, &r), Some(false));
    }

    #[test]
    fn test_wire_round_trip_through_export() {
        let make_native = || {
            let mut r = TypeResolver::new();
            r.insert(
                "builtins.tuple".to_string(),
                make_snapshot("builtins.tuple"),
            );
            NativeTypeResolver::from_resolver(r)
        };
        let bytes = encode_type(&make_tuple("builtins.tuple", vec![])).unwrap();
        let mut native = make_native();
        assert_eq!(
            rust_can_be_narrowed_with_len(&bytes, &mut native).unwrap(),
            Some(true)
        );
        // Undecodable wire bytes defer (shim falls back to pure Python).
        let mut native = make_native();
        assert_eq!(
            rust_can_be_narrowed_with_len(&[0xff, 0xff], &mut native).unwrap(),
            None
        );
    }
}
