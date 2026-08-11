#![allow(non_local_definitions)]

//! Native port of Type-query helpers and `TypeAnalyser.anal_type` hot path
//! from `mypy/typeanal.py`.
//!
//! Query helpers (no semantic context needed):
//! - `has_explicit_any` — does the type carry `AnyType(TypeOfAny.explicit)`?
//! - `has_any_from_unimported_type` — same for `from_unimported_type`.
//! - `collect_all_inner_types` — list of every type `t` contains.
//! - `make_optional_type` — `Optional[t]` without union simplification.
//!
//! Hot path (`rust_type_analyze`): mirrors `TypeAnalyser.anal_type`, analyzing
//! already-bound types (Instance, Callable, TypeVar, etc.) by recursing into
//! children and rebuilding. Returns `None` for types needing semantic context
//! (UnboundType, TypeAliasType, PlaceholderType) — exactly the deferral pattern
//! Python uses (lookup_qualified, plugin hooks, type alias expansion).
//!
//! All four queries defer on `TypeAliasType`; `rust_type_analyze` also defers on
//! TypeAliasType and UnboundType because their analysis requires the live alias
//! target or symbol lookup respectively.

use pyo3::prelude::*;

use crate::wire::{read_type, write_type, ExtraAttrs, Parameters, ReadBuffer, Type, WriteBuffer};

// TypeOfAny constants (mirror mypy/types.py:213-239).
const EXPLICIT: i64 = 2;
const FROM_UNIMPORTED_TYPE: i64 = 3;
const SPECIAL_FORM: i64 = 6;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

// ---------------------------------------------------------------------------
// has_explicit_any / has_any_from_unimported_type
// ---------------------------------------------------------------------------

/// `mypy.typeanal.has_explicit_any` — true if `t` is, or contains, an
/// `AnyType` with `type_of_any == explicit`.
///
/// Mirrors `HasExplicitAny` (typeanal.py:2561-2570): `BoolTypeQuery` with
/// `ANY_STRATEGY`, `visit_any` compares the explicit flag, and
/// `visit_typeddict_type` returns False (TypedDict is checked during its
/// declaration, not here).
#[pyfunction]
pub(crate) fn rust_has_explicit_any(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_inner(&t, EXPLICIT)),
        None => Ok(None),
    }
}

/// `mypy.typeanal.has_any_from_unimported_type` — like
/// `has_explicit_any`, but for `type_of_any == from_unimported_type`.
#[pyfunction]
pub(crate) fn rust_has_any_from_unimported_type(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_inner(&t, FROM_UNIMPORTED_TYPE)),
        None => Ok(None),
    }
}

/// ANY_STRATEGY (default False) bool query over the wire type, comparing
/// `AnyType.type_of_any` against `wanted`. Any child deferring (alias) makes
/// the whole result defer (`None`) to match the Python fallback.
fn has_explicit_any_inner(t: &Type, wanted: i64) -> Option<bool> {
    // TypeAliasType needs the live target to expand (mirrors
    // rust_has_recursive_types); can't conclude from the wire.
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    // visit_typeddict_type -- TypeQuery descends, but HasExplicitAny and
    // HasAnyFromUnimportedType override it to return False.
    if matches!(t, Type::TypedDictType { .. }) {
        return Some(false);
    }
    if let Type::AnyType { type_of_any, .. } = t {
        return Some(*type_of_any == wanted);
    }
    for child in query_children_bool(t) {
        match has_explicit_any_inner(child, wanted) {
            // ANY_STRATEGY: parent true if any child true.
            Some(true) => return Some(true),
            // Defer on any child that needs the alias target.
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

/// Direct children for a `BoolTypeQuery` traversal (ANY_STRATEGY default
/// False leaves return no children). Mirrors `BoolTypeQuery.visit_*`
/// (type_visitor.py:517-597), including the callable's unconditional
/// instance_type descent and the typevar upper_bound/default/values descent.
fn query_children_bool(t: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match t {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(values.iter());
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
        }
        Type::Parameters(p) => out.extend(p.arg_types.iter()),
        Type::Instance { args, .. } => out.extend(args.iter()),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            if let Some(it) = instance_type {
                out.push(it);
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        Type::AnyType {
            source_any: None, ..
        } => {}
        // TypeAliasType is handled by the caller (deferred). NoneType,
        // UninhabitedType, DeletedType: no children (default False).
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// collect_all_inner_types
// ---------------------------------------------------------------------------

/// `mypy.typeanal.collect_all_inner_types` — all types `t` contains, in
/// query order: each child's collected inners first, then the direct
/// children. The root is excluded. Deferral is by inner function (`None`)
/// rather than by list return, matching the Python `Option` convention.
#[pyfunction]
pub(crate) fn rust_collect_all_inner_types(type_bytes: &[u8]) -> PyResult<Option<Vec<Vec<u8>>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(collect_all_inner_types_inner(&t).map(|ts| ts.iter().filter_map(encode_type).collect()))
}

/// `CollectAllInnerTypesQuery` (typeanal.py:2601-2607): `query_types`
/// chains each child's collected inners, then appends the direct children.
/// Deferral (`None`) propagates on any TypeAliasType, since its target is
/// a live alias the wire cannot expand.
fn collect_all_inner_types_inner(t: &Type) -> Option<Vec<Type>> {
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    let children = query_children_type(t);
    let mut out = Vec::new();
    for child in &children {
        out.extend(collect_all_inner_types_inner(child)?);
    }
    // Direct children of `t` (not `t` itself) are the query result.
    for child in children {
        out.push(child.clone());
    }
    Some(out)
}

/// Direct children for a `TypeQuery` traversal. Mirrors
/// `TypeQuery.visit_*` (type_visitor.py:378-468): arg_types + ret (+
/// instance_type only when not equal to ret_type) for callables, partial
/// fallback + items for tuples, values only for typeddicts, upper +
/// default + values for typevars, prefix arg_types for param specs.
fn query_children_type(t: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match t {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(values.iter());
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
        }
        Type::Parameters(p) => out.extend(p.arg_types.iter()),
        Type::Instance { args, .. } => out.extend(args.iter()),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            if let Some(it) = instance_type {
                if it.as_ref() != ret_type.as_ref() {
                    out.push(it);
                }
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::TypedDictType { items, .. } => out.extend(items.iter().map(|(_, t)| t)),
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        // TypeAliasType is deferred by the caller. AnyType, NoneType,
        // UninhabitedType, DeletedType: no children.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// make_optional_type
// ---------------------------------------------------------------------------

/// `mypy.typeanal.make_optional_type` — `Optional[t]` without union
/// simplification (typeanal.py:2609-2623).
///
/// - NoneType returns itself.
/// - UnionType keeps items that are not None (via `get_proper_type` in
///   Python; via the wire's own NoneType variant here), prepends a fresh
///   NoneType, and reuses the union's line/column. Truthiness flags are
///   recomputed over the new items exactly like the Python constructor
///   computes after `flatten_nested_unions`.
/// - Any other type (including TypeAliasType, which is not a ProperType
///   and hits the `else` branch) wraps as `UnionType([t, NoneType])`.
///
/// Defers (`None`) when the input union contains an alias (Python filters
/// aliases by expanding them, which the wire cannot do), and when
/// truthiness reconstruction would need the live alias target.
#[pyfunction]
pub(crate) fn rust_make_optional_type(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(make_optional_type_inner(&t).and_then(|res| encode_type(&res)))
}

fn make_optional_type_inner(t: &Type) -> Option<Type> {
    // isinstance(t, ProperType) and isinstance(t, NoneType): return t.
    if matches!(t, Type::NoneType) {
        return Some(t.clone());
    }
    // isinstance(t, ProperType) and isinstance(t, UnionType): filter None
    // items, then append a fresh NoneType.
    if let Type::UnionType {
        items,
        uses_pep604_syntax,
        ..
    } = t
    {
        let mut kept = Vec::with_capacity(items.len() + 1);
        for item in items {
            // Python: `not isinstance(get_proper_type(item), NoneType)`.
            // Defer when expansion could change the answer (alias target
            // unknown on the wire).
            if matches!(item, Type::TypeAliasType { .. }) {
                return None;
            }
            if !matches!(item, Type::NoneType) {
                kept.push(item.clone());
            }
        }
        kept.push(Type::NoneType);
        let can_be_true = kept.iter().any(crate::setops::union_item_can_be_true);
        let can_be_false = kept.iter().any(crate::setops::union_item_can_be_false);
        return Some(Type::UnionType {
            items: kept,
            uses_pep604_syntax: *uses_pep604_syntax,
            can_be_true,
            can_be_false,
        });
    }
    // else: wrap Non-None-type values in a UnionType.
    let items = vec![t.clone(), Type::NoneType];
    let can_be_true = items.iter().any(crate::setops::union_item_can_be_true);
    let can_be_false = items.iter().any(crate::setops::union_item_can_be_false);
    Some(Type::UnionType {
        items,
        uses_pep604_syntax: false,
        can_be_true,
        can_be_false,
    })
}

// ---------------------------------------------------------------------------
// unknown_unpack
// ---------------------------------------------------------------------------

/// `mypy.typeanal.unknown_unpack` — true if `t` is an unpack of an unknown
/// type: `UnpackType` whose proper type is `AnyType(TypeOfAny.special_form)`.
///
/// Mirrors typeanal.py:2856-2867. Pure query: returns `None` (defer) when
/// the unpack target is a `TypeAliasType`, whose proper expansion is not
/// available on the wire (`get_proper_type` needs the live alias target).
#[pyfunction]
pub(crate) fn rust_unknown_unpack(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(unknown_unpack_inner(&t)),
        None => Ok(None),
    }
}

fn unknown_unpack_inner(t: &Type) -> Option<bool> {
    let Type::UnpackType { typ } = t else {
        return Some(false);
    };
    // get_proper_type(UnpackType.typ): non-alias wire types are already
    // proper, so only the alias case is indeterminate on the wire.
    match typ.as_ref() {
        Type::TypeAliasType { .. } => None,
        // isinstance(unpacked, AnyType) and type_of_any == special_form.
        Type::AnyType { type_of_any, .. } => Some(*type_of_any == SPECIAL_FORM),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
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

    fn make_callable(arg_types: Vec<Type>, ret_type: Type) -> Type {
        make_callable_with_instance(None, arg_types, ret_type)
    }

    fn make_callable_with_instance(
        instance_type: Option<Box<Type>>,
        arg_types: Vec<Type>,
        ret_type: Type,
    ) -> Type {
        Type::CallableType {
            fallback: Box::new(make_any(SPECIAL_FORM)),
            instance_type,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types,
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(ret_type),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn make_typeddict(items: Vec<(String, Type)>) -> Type {
        Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict", vec![])),
            items,
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: true,
        }
    }

    use std::collections::HashSet;

    #[test]
    fn test_has_explicit_any_true_for_explicit() {
        assert_eq!(
            has_explicit_any_inner(&make_any(EXPLICIT), EXPLICIT),
            Some(true)
        );
        assert_eq!(
            has_explicit_any_inner(&make_any(EXPLICIT), FROM_UNIMPORTED_TYPE),
            Some(false)
        );
    }

    #[test]
    fn test_has_explicit_any_false_for_special_and_unimported() {
        assert_eq!(
            has_explicit_any_inner(&make_any(SPECIAL_FORM), EXPLICIT),
            Some(false)
        );
        assert_eq!(
            has_explicit_any_inner(&make_any(FROM_UNIMPORTED_TYPE), EXPLICIT),
            Some(false)
        );
    }

    #[test]
    fn test_has_explicit_any_in_union() {
        let t = make_union(vec![make_any(SPECIAL_FORM), make_any(EXPLICIT)]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(true));
    }

    #[test]
    fn test_has_explicit_any_in_instance_args() {
        let t = make_instance("builtins.list", vec![make_any(EXPLICIT)]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(true));
    }

    #[test]
    fn test_has_explicit_any_typeddict_is_false() {
        let t = make_typeddict(vec![("x".to_string(), make_any(EXPLICIT))]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(false));
    }

    #[test]
    fn test_has_explicit_any_callable_instance_type() {
        let with_inst = make_callable_with_instance(
            Some(Box::new(make_any(EXPLICIT))),
            vec![make_any(SPECIAL_FORM)],
            make_any(SPECIAL_FORM),
        );
        assert_eq!(has_explicit_any_inner(&with_inst, EXPLICIT), Some(true));
        let no_inst = make_callable(vec![make_any(SPECIAL_FORM)], make_any(SPECIAL_FORM));
        assert_eq!(has_explicit_any_inner(&no_inst, EXPLICIT), Some(false));
    }

    #[test]
    fn test_has_any_unimported_true() {
        let t = make_union(vec![make_any(SPECIAL_FORM), make_any(FROM_UNIMPORTED_TYPE)]);
        assert_eq!(has_explicit_any_inner(&t, FROM_UNIMPORTED_TYPE), Some(true));
    }

    #[test]
    fn test_has_explicit_any_deferred_on_alias() {
        let t = Type::TypeAliasType {
            args: vec![make_any(EXPLICIT)],
            type_ref: "m.T".to_string(),
        };
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), None);
        // Alias nested inside a union propagates the deferral.
        let u = make_union(vec![make_any(SPECIAL_FORM), t]);
        assert_eq!(has_explicit_any_inner(&u, EXPLICIT), None);
    }

    #[test]
    fn test_collect_all_inner_union_children() {
        let t = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_any(SPECIAL_FORM),
        ]);
        let result = collect_all_inner_types_inner(&t).unwrap();
        // [int, any] -- both are leaves, so no inner, direct children only.
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Type::Instance { .. }));
        assert!(matches!(result[1], Type::AnyType { .. }));
    }

    #[test]
    fn test_collect_all_inner_nested_union_inner_ordering() {
        let inner = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        let t = make_instance("builtins.list", vec![inner.clone()]);
        let result = collect_all_inner_types_inner(&t).unwrap();
        assert_eq!(result.len(), 3);
        // Query order: inner union's children first, then the direct child.
        assert!(matches!(result[2], Type::UnionType { .. }));
    }

    #[test]
    fn test_collect_all_inner_children_none_does_not_include_root() {
        let t = make_instance("builtins.int", vec![]);
        assert_eq!(collect_all_inner_types_inner(&t).unwrap().len(), 0);
    }

    #[test]
    fn test_collect_all_inner_callable_instance_not_double_counted() {
        let ret = make_instance("builtins.str", vec![]);
        let t = make_callable_with_instance(
            Some(Box::new(ret.clone())),
            vec![make_instance("builtins.int", vec![])],
            ret.clone(),
        );
        let result = collect_all_inner_types_inner(&t).unwrap();
        // children: int (arg), str (ret), str (instance == ret, not dup).
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_collect_all_inner_deferred_on_alias() {
        let t = make_union(vec![
            make_any(SPECIAL_FORM),
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            },
        ]);
        assert_eq!(collect_all_inner_types_inner(&t), None);
    }

    #[test]
    fn test_make_optional_none_identity() {
        assert_eq!(
            make_optional_type_inner(&Type::NoneType),
            Some(Type::NoneType)
        );
    }

    #[test]
    fn test_make_optional_instance_wraps() {
        let t = make_instance("builtins.int", vec![]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::Instance { .. }));
                assert!(matches!(items[1], Type::NoneType));
                // int can be true and false.
                assert!(can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_union_strips_none() {
        let t = make_union(vec![make_instance("builtins.int", vec![]), Type::NoneType]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::Instance { .. }));
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_none_only_union_truthiness() {
        let t = make_union(vec![Type::NoneType]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], Type::NoneType));
                // Union[None] is always false-y: can be false, never true.
                assert!(!can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_alias_nested_union_defers() {
        let t = make_union(vec![
            make_any(SPECIAL_FORM),
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            },
        ]);
        assert_eq!(make_optional_type_inner(&t), None);
    }

    #[test]
    fn test_make_optional_alias_wraps_in_else_branch() {
        // A TypeAliasType is not a ProperType, so Python takes the else
        // branch and wraps it without inspecting its target.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "m.T".to_string(),
        };
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::TypeAliasType { .. }));
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_alias_wraps_truthiness_true() {
        // Else-branch wrap: setops truthiness helpers fall back to the
        // per-variant default for aliases, so a plain TypeAliasType has
        // can_be_true/can_be_false both true.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "m.T".to_string(),
        };
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 2);
                assert!(can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_unknown_unpack_false_for_non_unpack() {
        assert_eq!(unknown_unpack_inner(&make_any(SPECIAL_FORM)), Some(false));
        assert_eq!(
            unknown_unpack_inner(&make_instance("m.B", Vec::new())),
            Some(false)
        );
    }

    #[test]
    fn test_unknown_unpack_matches_special_form_any() {
        let unpacked = Type::UnpackType {
            typ: Box::new(make_any(SPECIAL_FORM)),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), Some(true));
    }

    #[test]
    fn test_unknown_unpack_other_any_is_false() {
        let unpacked = Type::UnpackType {
            typ: Box::new(make_any(EXPLICIT)),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), Some(false));
    }

    #[test]
    fn test_unknown_unpack_defers_on_alias_target() {
        let unpacked = Type::UnpackType {
            typ: Box::new(Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            }),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), None);
    }
}

// ---------------------------------------------------------------------------
// rust_type_analyze — hot path mirroring TypeAnalyser.anal_type
// ---------------------------------------------------------------------------

/// Mirrors `TypeAnalyser.anal_type` for types that can be analyzed without
/// semantic context (Instance, Callable, TypeVar, Tuple, TypedDict, Union,
/// TypeType, Literal, etc.). Returns `None` for types requiring symbol lookup
/// (UnboundType), alias expansion (TypeAliasType), or placeholder resolution
/// (PlaceholderType), matching Python's deferral semantics exactly.
///
/// Flags control tuple literal syntax, ParamSpec literal syntax, and unpack
/// handling — these are the same options the Python visitor carries.
#[pyfunction]
#[pyo3(signature = (type_bytes, allow_tuple_literal=false, allow_param_spec_literals=false, allow_unpack=false))]
pub(crate) fn rust_type_analyze(
    type_bytes: &[u8],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> PyResult<Option<Vec<u8>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let analyzed = match analyze_type_inner(
        &t,
        allow_tuple_literal,
        allow_param_spec_literals,
        allow_unpack,
    ) {
        Some(result) => result,
        None => return Ok(None),
    };
    Ok(encode_type(&analyzed))
}

/// Core analysis of a single Type value. Returns `None` when the type requires
/// semantic context (symbol lookup, alias expansion, etc.).
fn analyze_type_inner(
    t: &Type,
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Type> {
    match t {
        // Already-bound types: analyze children.
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            let args = analyze_type_list(
                args,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let lkv = match last_known_value {
                Some(v) => Some(Box::new(analyze_type_inner(
                    v,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args,
                last_known_value: lkv,
                extra_attrs: extra_attrs.as_ref().map(|ea| ExtraAttrs {
                    attrs: ea
                        .attrs
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                analyze_type_inner(
                                    v,
                                    allow_tuple_literal,
                                    allow_param_spec_literals,
                                    allow_unpack,
                                )
                                .unwrap(),
                            )
                        })
                        .collect(),
                    immutable: ea.immutable.clone(),
                    mod_name: ea.mod_name.clone(),
                }),
            })
        }

        Type::TypeAliasType {
            args: _,
            type_ref: _,
        } => {
            // Type alias expansion needs the live alias target (symbol lookup,
            // type parameter substitution). Defer to Python.
            None
        }

        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => {
            let values = analyze_type_list(
                values,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeVarType {
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                values,
                upper_bound,
                default,
                variance: *variance,
                meta_level: *meta_level,
            })
        }

        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
        } => {
            let prefix = Box::new(Parameters {
                arg_types: analyze_type_list(
                    &prefix.arg_types,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?,
                arg_kinds: prefix.arg_kinds.clone(),
                arg_names: prefix.arg_names.clone(),
                variables: analyze_type_var_likes(
                    &prefix.variables,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?,
                imprecise_arg_kinds: prefix.imprecise_arg_kinds,
            });
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::ParamSpecType {
                prefix,
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: *flavor,
                upper_bound,
                default,
            })
        }

        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
        } => {
            let tuple_fallback = Box::new(analyze_type_inner(
                tuple_fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeVarTupleType {
                tuple_fallback,
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                upper_bound,
                default,
                min_len: *min_len,
            })
        }

        Type::UnpackType { typ } => {
            // Unpack analysis needs allow_unpack context and nesting_level tracking.
            // Defer to Python unless allow_unpack is set.
            if !allow_unpack {
                return None;
            }
            let typ = Box::new(analyze_type_inner(
                typ,
                allow_tuple_literal,
                allow_param_spec_literals,
                true,
            )?);
            Some(Type::UnpackType { typ })
        }

        Type::Parameters(p) => Some(Type::Parameters(Parameters {
            arg_types: analyze_type_list(
                &p.arg_types,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?,
            arg_kinds: p.arg_kinds.clone(),
            arg_names: p.arg_names.clone(),
            variables: analyze_type_var_likes(
                &p.variables,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?,
            imprecise_arg_kinds: p.imprecise_arg_kinds,
        })),

        Type::UnboundType { .. } => {
            // UnboundType needs symbol lookup (lookup_qualified) in the
            // semantic analyzer. Python handles this by looking up the name
            // and dispatching to ParamSpecExpr/TypeVarExpr/TypeInfo/etc.
            // Rust has no access to the symbol table.
            None
        }

        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } => {
            let source_any = match source_any {
                Some(sa) => {
                    let analyzed = analyze_type_inner(
                        sa,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    )?;
                    Some(Box::new(analyzed))
                }
                None => None,
            };
            Some(Type::AnyType {
                type_of_any: *type_of_any,
                source_any,
                missing_import_name: missing_import_name.clone(),
            })
        }

        Type::NoneType => Some(Type::NoneType),

        Type::UninhabitedType { ambiguous } => Some(Type::UninhabitedType {
            ambiguous: *ambiguous,
        }),

        Type::DeletedType { source } => Some(Type::DeletedType {
            source: source.clone(),
        }),

        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            // Callable analysis: analyze arg_types, ret_type, variables, and
            // optional fields. This is the most complex branch — mirroring
            // visit_callable_type which binds type vars, handles type guards,
            // and analyzes star args.
            let ret = analyze_type_inner(
                ret_type,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let arg_types = analyze_type_list(
                arg_types,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let variables = analyze_type_var_likes(
                variables,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let type_guard = match type_guard {
                Some(g) => Some(Box::new(analyze_type_inner(
                    g,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let type_is = match type_is {
                Some(ti) => Some(Box::new(analyze_type_inner(
                    ti,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let instance_type = match instance_type {
                Some(it) => Some(Box::new(analyze_type_inner(
                    it,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::CallableType {
                fallback,
                instance_type,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                arg_types,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(ret),
                name: name.clone(),
                variables,
                type_guard,
                type_is,
            })
        }

        Type::Overloaded { items } => {
            // Each overloaded item is a CallableType (Python asserts this).
            let items: Vec<Type> = items
                .iter()
                .map(|item| match item {
                    Type::CallableType { .. } => analyze_type_inner(
                        item,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    ),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::Overloaded { items })
        }

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            let partial_fallback = Box::new(analyze_type_inner(
                partial_fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let items = analyze_type_list(
                items,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            Some(Type::TupleType {
                partial_fallback,
                items,
                implicit: *implicit,
            })
        }

        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let items_out: Vec<_> = items
                .iter()
                .map(|(k, v)| {
                    analyze_type_inner(
                        v,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    )
                    .map(|t| (k.clone(), t))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::TypedDictType {
                fallback,
                items: items_out,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

        Type::LiteralType { fallback, value } => {
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::LiteralType {
                fallback,
                value: value.clone(),
            })
        }

        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true,
            can_be_false,
        } => {
            let items = analyze_type_list(
                items,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            Some(Type::UnionType {
                items,
                uses_pep604_syntax: *uses_pep604_syntax,
                can_be_true: *can_be_true,
                can_be_false: *can_be_false,
            })
        }

        Type::TypeType { item, is_type_form } => {
            let item = Box::new(analyze_type_inner(
                item,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeType {
                item,
                is_type_form: *is_type_form,
            })
        }
    }
}

/// Analyze a list of types, returning `None` if any child defers.
fn analyze_type_list(
    types: &[Type],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        out.push(analyze_type_inner(
            t,
            allow_tuple_literal,
            allow_param_spec_literals,
            allow_unpack,
        )?);
    }
    Some(out)
}

/// Analyze TypeVar-like types (TypeVarType, ParamSpecType, TypeVarTupleType).
fn analyze_type_var_likes(
    types: &[Type],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        out.push(analyze_type_inner(
            t,
            allow_tuple_literal,
            allow_param_spec_literals,
            allow_unpack,
        )?);
    }
    Some(out)
}
