//! Native port of `mypy/checkmember.py` helpers (Stage M20).
//!
//! Ports the pure-logic subset of `analyze_member_access` that operates on
//! the wire-format `Type` enum without needing live Python checker state.
//! Each `#[pyfunction]` returns `None` for cases Rust cannot handle, so the
//! Python caller falls through to the pure-Python implementation (the
//! strangler-fig per-call gate).
//!
//! Ported functions:
//!   * `bind_self_fast` — strips the first positional argument from a
//!     CallableType or Overloaded and sets `is_bound=True`. Pure type
//!     manipulation, no checker state. Called on every trivial-self method
//!     access (hot path).
//!   * `classify_member_access` — classifies the `_analyze_member_access`
//!     dispatch branch from a wire-format type. Returns an int code so
//!     Python can skip the isinstance chain. Defers on TypeAliasType
//!     (needs alias expansion via `get_proper_type`).
//!
//! Deferred (return None):
//!   * `TypeAliasType` — the wire format carries no resolved alias target,
//!     so `get_proper_type` cannot expand it.
//!   * CallableType whose first arg is `*args`/`**kwargs` — `bind_self_fast`
//!     returns the method unchanged in Python; we defer so Python handles it.
//!   * Overloaded with zero items — degenerate; defer to Python.

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `ArgKind.ARG_POS` = 0.
#[cfg(test)]
const ARG_POS: i64 = 0;
/// `ArgKind.ARG_STAR` = 2.
const ARG_STAR: i64 = 2;
/// `ArgKind.ARG_STAR2` = 4.
const ARG_STAR2: i64 = 4;

/// Dispatch codes for `classify_member_access`. Mirror the `isinstance`
/// chain in `_analyze_member_access` (checkmember.py:242-281).
pub(crate) const MA_INSTANCE: i64 = 0;
pub(crate) const MA_ANY: i64 = 1;
pub(crate) const MA_UNION: i64 = 2;
pub(crate) const MA_TYPE_CALLABLE: i64 = 3;
pub(crate) const MA_TYPE_TYPE: i64 = 4;
pub(crate) const MA_TUPLE: i64 = 5;
pub(crate) const MA_LITERAL_OR_FUNC: i64 = 6;
pub(crate) const MA_TYPEDDICT: i64 = 7;
pub(crate) const MA_NONE: i64 = 8;
pub(crate) const MA_TYPEVAR: i64 = 9;
pub(crate) const MA_DELETED: i64 = 10;
pub(crate) const MA_UNINHABITED: i64 = 11;
pub(crate) const MA_MISSING: i64 = 12;

// ---------------------------------------------------------------------------
// Wire helpers
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

/// `get_proper_type` for the wire format. Expands `TypeAliasType` by
/// returning `None` (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// Whether a CallableType is a type object (i.e. its fallback is a
/// metaclass). Mirrors `CallableType.is_type_obj()` (types.py:2343).
/// Checks `fallback.type.is_metaclass()` via the resolver snapshot's
/// `metaclass_fullname` field. Also requires `ret_type` not to be
/// `UninhabitedType`.
fn is_type_obj(fallback: &Type, ret_type: &Type, resolver: &TypeResolver) -> bool {
    if matches!(ret_type, Type::UninhabitedType { .. }) {
        return false;
    }
    let type_ref = match fallback {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return false,
    };
    // is_metaclass() checks if the TypeInfo or any of its MRO bases is
    // builtins.type or abc.ABCMeta. The snapshot stores metaclass_fullname
    // only when metaclass_type is set. We check if the fallback's type_ref
    // appears as a metaclass in any snapshot (i.e. its own
    // metaclass_fullname is set and it has builtins.type in its MRO).
    // Simplified: check if type_ref has builtins.type in its MRO.
    if type_ref == "builtins.type" {
        return true;
    }
    if let Some(snap) = resolver.get(type_ref) {
        snap.mro.iter().any(|m| m == "builtins.type")
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// bind_self_fast
// ---------------------------------------------------------------------------

/// `mypy.checkmember.bind_self_fast` — strip the first positional argument
/// from a CallableType or Overloaded and set `is_bound=True`.
///
/// Mirrors `bind_self_fast` (checkmember.py:1503-1525). Returns `None` (Python
/// `None`) when Rust cannot handle the case so the Python caller falls
/// through. Deferred cases:
///   * Non-callable types (Instance, AnyType, etc.)
///   * CallableType with no arg_types
///   * CallableType whose first arg_kind is ARG_STAR or ARG_STAR2
///   * Overloaded with zero items
///
/// The `original_type` parameter from Python is unused here: `bind_self_fast`
/// only strips the first arg and sets `is_bound`; it does NOT substitute
/// type variables (that's `bind_self` in typeops.py).
#[pyfunction]
pub(crate) fn rust_bind_self_fast(method_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(method_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = bind_self_fast_inner(&typ)?;
    match result {
        Some(r) => Ok(encode_type(&r)),
        None => Ok(None),
    }
}

fn bind_self_fast_inner(typ: &Type) -> PyResult<Option<Type>> {
    match typ {
        Type::Overloaded { items } => {
            if items.is_empty() {
                return Ok(None);
            }
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                match bind_self_fast_inner(item)? {
                    Some(bound) => new_items.push(bound),
                    None => return Ok(None),
                }
            }
            Ok(Some(Type::Overloaded { items: new_items }))
        }
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
            if arg_types.is_empty() || arg_kinds.is_empty() {
                // Invalid method — Python returns it unchanged.
                return Ok(None);
            }
            let first_kind = arg_kinds[0];
            if first_kind == ARG_STAR || first_kind == ARG_STAR2 {
                // *args / **kwargs — Python returns unchanged. Defer.
                return Ok(None);
            }
            let new_callable = Type::CallableType {
                fallback: fallback.clone(),
                instance_type: instance_type.clone(),
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: true,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                arg_types: arg_types[1..].to_vec(),
                arg_kinds: arg_kinds[1..].to_vec(),
                arg_names: arg_names[1..].to_vec(),
                ret_type: ret_type.clone(),
                name: name.clone(),
                variables: variables.clone(),
                type_guard: type_guard.clone(),
                type_is: type_is.clone(),
            };
            // Suppress unused warning for the original is_bound.
            let _ = is_bound;
            Ok(Some(new_callable))
        }
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// classify_member_access
// ---------------------------------------------------------------------------

/// Classify the `_analyze_member_access` dispatch branch from a wire-format
/// type. Returns an int code (MA_* constant) so Python can skip the
/// isinstance chain. Returns `None` (Python `None`) for `TypeAliasType`
/// (needs alias expansion) or decode failure.
///
/// Mirrors `_analyze_member_access` (checkmember.py:242-281). The
/// `resolver` is used to check `is_type_obj()` for FunctionLike (CallableType
/// / Overloaded whose fallback is a metaclass).
#[pyfunction]
pub(crate) fn rust_classify_member_access(
    resolver: &NativeTypeResolver,
    typ_bytes: &[u8],
) -> PyResult<Option<i64>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let proper = match get_proper_or_none(&typ) {
        Some(p) => p,
        None => return Ok(None),
    };
    Ok(Some(classify_member_access_inner(
        proper,
        resolver.resolver(),
    )))
}

fn classify_member_access_inner(typ: &Type, resolver: &TypeResolver) -> i64 {
    match typ {
        Type::Instance { .. } => MA_INSTANCE,
        Type::AnyType { .. } => MA_ANY,
        Type::UnionType { .. } => MA_UNION,
        Type::TypeType { .. } => MA_TYPE_TYPE,
        Type::TupleType { .. } => MA_TUPLE,
        Type::TypedDictType { .. } => MA_TYPEDDICT,
        Type::NoneType => MA_NONE,
        Type::DeletedType { .. } => MA_DELETED,
        Type::UninhabitedType { .. } => MA_UNINHABITED,
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            MA_TYPEVAR
        }
        Type::LiteralType { .. } => MA_LITERAL_OR_FUNC,
        Type::CallableType {
            fallback, ret_type, ..
        } => {
            if is_type_obj(fallback, ret_type, resolver) {
                MA_TYPE_CALLABLE
            } else {
                MA_LITERAL_OR_FUNC
            }
        }
        Type::Overloaded { items } => {
            // FunctionLike.is_type_obj() checks the first item.
            if let Some(Type::CallableType {
                fallback, ret_type, ..
            }) = items.first()
            {
                if is_type_obj(fallback, ret_type, resolver) {
                    MA_TYPE_CALLABLE
                } else {
                    MA_LITERAL_OR_FUNC
                }
            } else {
                MA_LITERAL_OR_FUNC
            }
        }
        // TypeAliasType is deferred in get_proper_or_none; unreachable here.
        // Parameters, UnpackType, UnboundType: fall through to MISSING.
        _ => MA_MISSING,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_callable(arg_kinds: Vec<i64>, is_bound: bool) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }],
            arg_kinds,
            arg_names: vec![Some("self".to_string())],
            ret_type: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            name: Some("method".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn make_instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_overloaded(items: Vec<Type>) -> Type {
        Type::Overloaded { items }
    }

    #[test]
    fn test_bind_self_fast_strips_first_arg() {
        let method = make_callable(vec![ARG_POS], false);
        let result = bind_self_fast_inner(&method).unwrap().unwrap();
        match result {
            Type::CallableType {
                arg_types,
                arg_kinds,
                arg_names,
                is_bound,
                ..
            } => {
                assert!(arg_types.is_empty());
                assert!(arg_kinds.is_empty());
                assert!(arg_names.is_empty());
                assert!(is_bound);
            }
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_bind_self_fast_overloaded() {
        let item1 = make_callable(vec![ARG_POS], false);
        let item2 = make_callable(vec![ARG_POS], false);
        let overloaded = make_overloaded(vec![item1, item2]);
        let result = bind_self_fast_inner(&overloaded).unwrap().unwrap();
        match result {
            Type::Overloaded { items } => {
                assert_eq!(items.len(), 2);
                for item in &items {
                    match item {
                        Type::CallableType {
                            is_bound,
                            arg_types,
                            ..
                        } => {
                            assert!(*is_bound);
                            assert!(arg_types.is_empty());
                        }
                        _ => panic!("expected CallableType"),
                    }
                }
            }
            _ => panic!("expected Overloaded"),
        }
    }

    #[test]
    fn test_bind_self_fast_defers_star_args() {
        let method = make_callable(vec![ARG_STAR], false);
        let result = bind_self_fast_inner(&method).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_self_fast_defers_star2() {
        let method = make_callable(vec![ARG_STAR2], false);
        let result = bind_self_fast_inner(&method).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_self_fast_defers_empty_args() {
        let method = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int")),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let result = bind_self_fast_inner(&method).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_self_fast_defers_non_callable() {
        let inst = make_instance("builtins.int");
        let result = bind_self_fast_inner(&inst).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_self_fast_defers_empty_overloaded() {
        let overloaded = make_overloaded(vec![]);
        let result = bind_self_fast_inner(&overloaded).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_self_fast_round_trip() {
        let method = make_callable(vec![ARG_POS], false);
        let encoded = encode_type(&method).unwrap();
        let decoded = decode_type(&encoded).unwrap();
        let result = bind_self_fast_inner(&decoded).unwrap().unwrap();
        match result {
            Type::CallableType { is_bound, .. } => assert!(is_bound),
            _ => panic!("expected CallableType"),
        }
    }

    #[test]
    fn test_classify_instance() {
        let resolver = TypeResolver::new();
        let inst = make_instance("builtins.int");
        assert_eq!(classify_member_access_inner(&inst, &resolver), MA_INSTANCE);
    }

    #[test]
    fn test_classify_any() {
        let resolver = TypeResolver::new();
        let any = Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(classify_member_access_inner(&any, &resolver), MA_ANY);
    }

    #[test]
    fn test_classify_union() {
        let resolver = TypeResolver::new();
        let union = Type::UnionType {
            items: vec![make_instance("builtins.int"), make_instance("builtins.str")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(classify_member_access_inner(&union, &resolver), MA_UNION);
    }

    #[test]
    fn test_classify_none_type() {
        let resolver = TypeResolver::new();
        assert_eq!(
            classify_member_access_inner(&Type::NoneType, &resolver),
            MA_NONE
        );
    }

    #[test]
    fn test_classify_deleted() {
        let resolver = TypeResolver::new();
        let deleted = Type::DeletedType {
            source: Some("x".to_string()),
        };
        assert_eq!(
            classify_member_access_inner(&deleted, &resolver),
            MA_DELETED
        );
    }

    #[test]
    fn test_classify_uninhabited() {
        let resolver = TypeResolver::new();
        let uninh = Type::UninhabitedType { ambiguous: false };
        assert_eq!(
            classify_member_access_inner(&uninh, &resolver),
            MA_UNINHABITED
        );
    }

    #[test]
    fn test_classify_type_type() {
        let resolver = TypeResolver::new();
        let tt = Type::TypeType {
            item: Box::new(make_instance("builtins.int")),
            is_type_form: false,
        };
        assert_eq!(classify_member_access_inner(&tt, &resolver), MA_TYPE_TYPE);
    }

    #[test]
    fn test_classify_typed_dict() {
        let resolver = TypeResolver::new();
        let td = Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict")),
            items: vec![],
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        assert_eq!(classify_member_access_inner(&td, &resolver), MA_TYPEDDICT);
    }

    #[test]
    fn test_classify_typevar() {
        let resolver = TypeResolver::new();
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(make_instance("builtins.object")),
            default: Box::new(make_instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(classify_member_access_inner(&tv, &resolver), MA_TYPEVAR);
    }

    #[test]
    fn test_classify_tuple() {
        let resolver = TypeResolver::new();
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple")),
            items: vec![make_instance("builtins.int")],
            implicit: false,
        };
        assert_eq!(classify_member_access_inner(&tup, &resolver), MA_TUPLE);
    }

    #[test]
    fn test_classify_literal() {
        let resolver = TypeResolver::new();
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int")),
            value: crate::wire::LiteralValue::Int(42),
        };
        assert_eq!(
            classify_member_access_inner(&lit, &resolver),
            MA_LITERAL_OR_FUNC
        );
    }

    #[test]
    fn test_classify_plain_callable_not_type_obj() {
        let resolver = TypeResolver::new();
        let callable = make_callable(vec![ARG_POS], false);
        // fallback is builtins.function, not a metaclass -> MA_LITERAL_OR_FUNC
        assert_eq!(
            classify_member_access_inner(&callable, &resolver),
            MA_LITERAL_OR_FUNC
        );
    }
}
