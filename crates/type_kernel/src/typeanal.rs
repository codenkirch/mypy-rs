#![allow(non_local_definitions)]

//! Native port of the pure Type-algebra subset of `mypy/typeanal.py`
//! (Stage 13, Issue #97).
//!
//! Ports the pure functions that operate on `Type` values without needing
//! the semantic analyzer's mutable state (`api`, `tvar_scope`, `plugin`,
//! `Options`, `MsgCallback`, `TypeInfo`):
//! - `make_optional_type` — wraps a type in `Optional[t]`.
//! - `has_explicit_any` — `BoolTypeQuery` (ANY_STRATEGY) for explicit Any.
//! - `has_any_from_unimported_type` — same query, different `TypeOfAny`.
//! - `collect_all_inner_types` — collects every sub-type recursively.
//! - `unknown_unpack` — checks for an unpack of special-form Any.
//! - `find_self_type` — checks for a Self-typed unbound reference.
//!
//! Deferred (not portable without AST / `TypeInfo` / `Options`):
//! `analyze_type_alias`, `fix_instance`, `validate_instance`,
//! `set_any_tvars`, `instantiate_type_alias`, `fix_type_var_tuple_argument`,
//! `check_vec_type_args`, `is_typevar_default_recursive`,
//! `detect_diverging_alias`, `check_for_explicit_any`.

use pyo3::prelude::*;

use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// TypeOfAny values (mirrors mypy.types.TypeOfAny)
// ---------------------------------------------------------------------------

/// `TypeOfAny.unannotated` = 1.
#[allow(dead_code)]
const TYPE_OF_ANY_UNANNOTATED: i64 = 1;
/// `TypeOfAny.explicit` = 2.
const TYPE_OF_ANY_EXPLICIT: i64 = 2;
/// `TypeOfAny.from_unimported_type` = 3.
const TYPE_OF_ANY_FROM_UNIMPORTED: i64 = 3;
/// `TypeOfAny.from_omitted_generics` = 4.
#[allow(dead_code)]
const TYPE_OF_ANY_FROM_OMITTED: i64 = 4;
/// `TypeOfAny.from_error` = 5.
#[allow(dead_code)]
const TYPE_OF_ANY_FROM_ERROR: i64 = 5;
/// `TypeOfAny.special_form` = 6.
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

// ---------------------------------------------------------------------------
// Constant sets (mirror mypy/typeanal.py module-level Finals)
// ---------------------------------------------------------------------------

/// `type_constructors` (typeanal.py:124-132). Set of fullnames that are
/// type constructors (construct a new type from arguments).
pub(crate) const TYPE_CONSTRUCTORS: &[&str] = &[
    "typing.Callable",
    "typing.Optional",
    "typing.Tuple",
    "typing.Type",
    "typing.Union",
    "typing.Literal",
    "typing_extensions.Literal",
    "typing.Annotated",
    "typing_extensions.Annotated",
];

/// `SELF_TYPE_NAMES` (typeanal.py:143).
pub(crate) const SELF_TYPE_NAMES: &[&str] = &["typing.Self", "typing_extensions.Self"];

/// Check if a fullname is a type constructor.
#[pyfunction]
pub(crate) fn rust_is_type_constructor(fullname: &str) -> PyResult<bool> {
    Ok(TYPE_CONSTRUCTORS.contains(&fullname))
}

/// Check if a fullname is a Self type reference.
#[pyfunction]
pub(crate) fn rust_is_self_type_name(fullname: &str) -> PyResult<bool> {
    Ok(SELF_TYPE_NAMES.contains(&fullname))
}

// ---------------------------------------------------------------------------
// Wire format helpers (shared decode/encode)
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

fn encode_type_list(types: &[Type]) -> Vec<Vec<u8>> {
    types.iter().filter_map(encode_type).collect()
}

// ---------------------------------------------------------------------------
// make_optional_type: wrap t in Optional[t] = Union[t, None]
// ---------------------------------------------------------------------------

/// `mypy.typeanal.make_optional_type` — return the type corresponding to
/// `Optional[t]`.
///
/// Mirrors `make_optional_type` (typeanal.py:2609-2625). If `t` is already
/// `NoneType`, returns it unchanged. If `t` is a `UnionType`, filters out
/// existing `NoneType` items and appends a fresh `NoneType` (no double-wrap).
/// The wire `TypeAliasType` can't be expanded (no target), so we treat it
/// like any other non-union, non-none type and wrap it in a union with
/// `NoneType`, which matches the Python behavior when `t` is already a
/// `ProperType`.
#[pyfunction]
pub(crate) fn rust_make_optional_type(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = make_optional_type_inner(&typ);
    Ok(encode_type(&result))
}

pub(crate) fn make_optional_type_inner(t: &Type) -> Type {
    match t {
        Type::NoneType => t.clone(),
        Type::UnionType {
            items,
            uses_pep604_syntax,
        } => {
            // Filter out existing NoneType items to avoid double-wrapping.
            let filtered: Vec<Type> = items
                .iter()
                .filter(|item| !matches!(item, Type::NoneType))
                .cloned()
                .collect();
            let mut new_items = filtered;
            new_items.push(Type::NoneType);
            Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
            }
        }
        _ => Type::UnionType {
            items: vec![t.clone(), Type::NoneType],
            uses_pep604_syntax: false,
        },
    }
}

// ---------------------------------------------------------------------------
// has_explicit_any: BoolTypeQuery (ANY_STRATEGY), AnyType with explicit
// ---------------------------------------------------------------------------

/// `mypy.typeanal.has_explicit_any` — whether `t` or any type it contains is
/// an `Any` coming from an explicit annotation.
///
/// Mirrors `HasExplicitAny` (typeanal.py:2554-2571). `BoolTypeQuery` with
/// `ANY_STRATEGY`: returns `true` if any child is an explicit Any.
/// `TypedDictType` is skipped (checked at declaration, not here).
#[pyfunction]
pub(crate) fn rust_has_explicit_any(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(has_explicit_any_inner(&typ))
}

pub(crate) fn has_explicit_any_inner(t: &Type) -> bool {
    match t {
        Type::AnyType { type_of_any, .. } => *type_of_any == TYPE_OF_ANY_EXPLICIT,
        // TypedDictType is checked at declaration; skip its children.
        Type::TypedDictType { .. } => false,
        _ => children(t).iter().any(|c| has_explicit_any_inner(c)),
    }
}

// ---------------------------------------------------------------------------
// has_any_from_unimported_type: BoolTypeQuery, AnyType with from_unimported
// ---------------------------------------------------------------------------

/// `mypy.typeanal.has_any_from_unimported_type` — return true if `t` is an
/// `Any` because an import was not followed, or contains such an `Any`.
///
/// Mirrors `HasAnyFromUnimportedType` (typeanal.py:2573-2592).
#[pyfunction]
pub(crate) fn rust_has_any_from_unimported_type(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(has_any_from_unimported_type_inner(&typ))
}

pub(crate) fn has_any_from_unimported_type_inner(t: &Type) -> bool {
    match t {
        Type::AnyType { type_of_any, .. } => *type_of_any == TYPE_OF_ANY_FROM_UNIMPORTED,
        Type::TypedDictType { .. } => false,
        _ => children(t)
            .iter()
            .any(|c| has_any_from_unimported_type_inner(c)),
    }
}

// ---------------------------------------------------------------------------
// collect_all_inner_types: TypeQuery collecting all sub-types
// ---------------------------------------------------------------------------

/// `mypy.typeanal.collect_all_inner_types` — return all types that `t`
/// contains (including `t` itself and all transitive children).
///
/// Mirrors `CollectAllInnerTypesQuery` (typeanal.py:2594-2607). The query
/// returns each visited type plus all its children's results, flattened.
#[pyfunction]
pub(crate) fn rust_collect_all_inner_types(type_bytes: &[u8]) -> PyResult<Vec<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };
    let collected = collect_all_inner_types_inner(&typ);
    Ok(encode_type_list(&collected))
}

pub(crate) fn collect_all_inner_types_inner(t: &Type) -> Vec<Type> {
    let mut result = vec![t.clone()];
    for child in children(t) {
        result.extend(collect_all_inner_types_inner(child));
    }
    result
}

// ---------------------------------------------------------------------------
// unknown_unpack: UnpackType of special-form Any
// ---------------------------------------------------------------------------

/// `mypy.typeanal.unknown_unpack` — check if a type is an unpack of an
/// unknown type (special-form Any).
///
/// Mirrors `unknown_unpack` (typeanal.py:2716-2728). Returns true if `t` is
/// an `UnpackType` whose inner type is an `AnyType` with
/// `type_of_any == special_form`.
#[pyfunction]
pub(crate) fn rust_unknown_unpack(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(unknown_unpack_inner(&typ))
}

pub(crate) fn unknown_unpack_inner(t: &Type) -> bool {
    if let Type::UnpackType { typ } = t {
        // get_proper_type on the wire is identity (no alias target to expand).
        if let Type::AnyType { type_of_any, .. } = typ.as_ref() {
            return *type_of_any == TYPE_OF_ANY_SPECIAL_FORM;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// find_self_type: BoolTypeQuery, unbound name in SELF_TYPE_NAMES
// ---------------------------------------------------------------------------

/// `mypy.typeanal.find_self_type` — check if `typ` contains a Self-typed
/// reference.
///
/// Mirrors `find_self_type` / `HasSelfType` (typeanal.py:2700-2714). The
/// Python version takes a `lookup` callback to resolve unbound names; we
/// take the set of known Self-type fullnames directly (the Python shim
/// passes the resolved names). `ANY_STRATEGY`: any child matching returns
/// true.
#[pyfunction]
pub(crate) fn rust_find_self_type(type_bytes: &[u8]) -> PyResult<bool> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(false),
    };
    Ok(find_self_type_inner(&typ))
}

pub(crate) fn find_self_type_inner(t: &Type) -> bool {
    if let Type::UnboundType { name, .. } = t {
        if SELF_TYPE_NAMES.contains(&name.as_str()) {
            return true;
        }
    }
    children(t).iter().any(|c| find_self_type_inner(c))
}

// ---------------------------------------------------------------------------
// children: yield direct child types (mirrors BoolTypeQuery.query_types)
// ---------------------------------------------------------------------------

/// Yield the direct child types of `typ`. Shared with the visitor module's
/// `children` to mirror `BoolTypeQuery` / `TypeQuery` traversal over the
/// wire `Type` enum.
fn children(typ: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match typ {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            out.extend(args.iter());
            if let Some(lkv) = last_known_value {
                out.push(lkv);
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            out.extend(variables.iter());
            if let Some(it) = instance_type {
                out.push(it);
            }
        }
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            out.push(fallback);
            out.extend(items.iter().map(|(_, t)| t));
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        Type::AnyType {
            source_any: None, ..
        } => {}
        Type::TypeAliasType { args, .. } => out.extend(args.iter()),
        // Leaves: NoneType, UninhabitedType, ErasedType, DeletedType,
        // Parameters, TypeVarType, ParamSpecType, TypeVarTupleType.
        // TypeVar-like types have upper_bound/default/values, but
        // BoolTypeQuery does not traverse into them by default (they are
        // treated as leaves for the queries ported here, matching the
        // Python visitor's default visit methods that don't recurse into
        // TypeVar bounds).
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// has_explicit_any without TypedDict skip (for collect-all-style queries)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
        }
    }

    fn make_typeddict() -> Type {
        Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict", vec![])),
            items: vec![("x".to_string(), make_any(TYPE_OF_ANY_EXPLICIT))],
            required_keys: ["x".to_string()].into_iter().collect(),
            readonly_keys: [].into_iter().collect(),
            is_closed: true,
        }
    }

    // --- make_optional_type ---

    #[test]
    fn test_make_optional_wraps_instance() {
        let inst = make_instance("builtins.int", vec![]);
        let result = make_optional_type_inner(&inst);
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_none_is_identity() {
        let result = make_optional_type_inner(&Type::NoneType);
        assert!(matches!(result, Type::NoneType));
    }

    #[test]
    fn test_make_optional_union_filters_existing_none() {
        let u = make_union(vec![make_instance("builtins.int", vec![]), Type::NoneType]);
        let result = make_optional_type_inner(&u);
        match result {
            Type::UnionType { items, .. } => {
                // int + none, no duplicate none.
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::Instance { .. }));
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    // --- has_explicit_any ---

    #[test]
    fn test_has_explicit_any_true() {
        assert!(has_explicit_any_inner(&make_any(TYPE_OF_ANY_EXPLICIT)));
    }

    #[test]
    fn test_has_explicit_any_false_unannotated() {
        assert!(!has_explicit_any_inner(&make_any(TYPE_OF_ANY_UNANNOTATED)));
    }

    #[test]
    fn test_has_explicit_any_in_instance_args() {
        let inst = make_instance("Foo", vec![make_any(TYPE_OF_ANY_EXPLICIT)]);
        assert!(has_explicit_any_inner(&inst));
    }

    #[test]
    fn test_has_explicit_any_skips_typeddict() {
        // TypedDictType children are not traversed.
        let td = make_typeddict();
        assert!(!has_explicit_any_inner(&td));
    }

    #[test]
    fn test_has_explicit_any_false_simple() {
        assert!(!has_explicit_any_inner(&make_instance(
            "builtins.int",
            vec![]
        )));
    }

    // --- has_any_from_unimported_type ---

    #[test]
    fn test_has_any_from_unimported_true() {
        assert!(has_any_from_unimported_type_inner(&make_any(
            TYPE_OF_ANY_FROM_UNIMPORTED
        )));
    }

    #[test]
    fn test_has_any_from_unimported_false_explicit() {
        assert!(!has_any_from_unimported_type_inner(&make_any(
            TYPE_OF_ANY_EXPLICIT
        )));
    }

    #[test]
    fn test_has_any_from_unimported_nested() {
        let inst = make_instance("Foo", vec![make_any(TYPE_OF_ANY_FROM_UNIMPORTED)]);
        assert!(has_any_from_unimported_type_inner(&inst));
    }

    // --- collect_all_inner_types ---

    #[test]
    fn test_collect_all_inner_types_simple() {
        let inst = make_instance("builtins.int", vec![]);
        let result = collect_all_inner_types_inner(&inst);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_collect_all_inner_types_with_args() {
        let inner = make_instance("builtins.str", vec![]);
        let outer = make_instance("Foo", vec![inner]);
        let result = collect_all_inner_types_inner(&outer);
        // outer + inner = 2.
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_collect_all_inner_types_union() {
        let a = make_instance("A", vec![]);
        let b = make_instance("B", vec![]);
        let u = make_union(vec![a, b]);
        let result = collect_all_inner_types_inner(&u);
        // union + a + b = 3.
        assert_eq!(result.len(), 3);
    }

    // --- unknown_unpack ---

    #[test]
    fn test_unknown_unpack_true() {
        let t = Type::UnpackType {
            typ: Box::new(make_any(TYPE_OF_ANY_SPECIAL_FORM)),
        };
        assert!(unknown_unpack_inner(&t));
    }

    #[test]
    fn test_unknown_unpack_false_regular_any() {
        let t = Type::UnpackType {
            typ: Box::new(make_any(TYPE_OF_ANY_EXPLICIT)),
        };
        assert!(!unknown_unpack_inner(&t));
    }

    #[test]
    fn test_unknown_unpack_false_not_unpack() {
        assert!(!unknown_unpack_inner(&make_instance("A", vec![])));
    }

    // --- find_self_type ---

    #[test]
    fn test_find_self_type_direct() {
        let t = Type::UnboundType {
            name: "typing.Self".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert!(find_self_type_inner(&t));
    }

    #[test]
    fn test_find_self_type_extensions() {
        let t = Type::UnboundType {
            name: "typing_extensions.Self".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert!(find_self_type_inner(&t));
    }

    #[test]
    fn test_find_self_type_nested_in_instance() {
        let inner = Type::UnboundType {
            name: "typing.Self".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        let inst = make_instance("Foo", vec![inner]);
        assert!(find_self_type_inner(&inst));
    }

    #[test]
    fn test_find_self_type_false_regular_name() {
        let t = Type::UnboundType {
            name: "builtins.int".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert!(!find_self_type_inner(&t));
    }

    // --- constants ---

    #[test]
    fn test_is_type_constructor_true() {
        assert!(TYPE_CONSTRUCTORS.contains(&"typing.Union"));
        assert!(TYPE_CONSTRUCTORS.contains(&"typing.Literal"));
        assert!(TYPE_CONSTRUCTORS.contains(&"typing_extensions.Annotated"));
    }

    #[test]
    fn test_is_type_constructor_false() {
        assert!(!TYPE_CONSTRUCTORS.contains(&"builtins.int"));
    }

    #[test]
    fn test_self_type_names() {
        assert!(SELF_TYPE_NAMES.contains(&"typing.Self"));
        assert!(SELF_TYPE_NAMES.contains(&"typing_extensions.Self"));
    }
}
