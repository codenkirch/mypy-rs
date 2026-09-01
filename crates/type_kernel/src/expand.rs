//! Stage 4 seam: per-formal actual-type expansion for `check_call`.
//!
//! Ports `mypy.argmap.ArgTypeExpander.expand_actual_type` (argmap.py:269-364)
//! to Rust, behind the strangler-fig per-call gate. The Python method is
//! stateful: a single `ArgTypeExpander` instance processes all formals of one
//! call, tracking the next tuple-*args item (`tuple_index`) and the TypedDict
//! `**kwargs` keys already consumed (`kwargs_used`).
//!
//! Wire-safety: the Python side keeps the original `Type` objects; Rust never
//! rebuilds types into the mypy type graph. This module inspects the wire
//! serialization of the (proper) actual type structurally and returns a
//! `decision` that the Python shim maps back onto its own objects plus the
//! updated per-call state. Only the *deterministic* branches are ported:
//! tuple `*args` item indexing and named-key TypedDict `**kwargs` lookup.
//! Anything that needs `is_subtype` (Iterable/Mapping unpacking, TypeVarTuple
//! upper bounds) or picks an arbitrary key (Python's `set.pop()` on unused
//! TypedDict keys is hash-order dependent) returns `None` so Python runs it.
//!
//! Deferral contract matches the other Stage 4 seams: return `None` when the
//! wire blob cannot be decoded or the shape needs graph state, so the caller
//! falls back to the Python implementation. Wrong answers degrade to
//! deferral, never to a behavior change.

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type};

// ArgKind integer values, mirroring `mypy.nodes.ARG_*` (nodes.py:2480-2517).
const ARG_STAR: i64 = 2;
const ARG_STAR2: i64 = 4;

// Decision tags returned to the Python shim; `expand_actual_type` maps each
// back to a branch of its own implementation:

//   * TUPLE: ARG_STAR tuple `*args`. Python sets `tuple_index` from the
//     returned state and yields `items[tuple_index - 1]`, including its own
//     UnpackType fallback when `allow_unpack` is false.

//   * KWARG: ARG_STAR2 TypedDict `**kwargs`. Python yields
//     `items[returned_name]` and records the key as consumed.
//   * PASSTHROUGH: 1:1 (non-star kinds) or ParamSpec, no state change.

//   * ANY_ERROR: an un-unpackable actual; Python yields
//     `AnyType(TypeOfAny.from_error)` (the deterministic Python else branch).
const DECISION_TUPLE: i64 = 0;
const DECISION_KWARG: i64 = 1;
const DECISION_PASSTHROUGH: i64 = 2;
const DECISION_ANY_ERROR: i64 = 3;

/// Rust port of `ArgTypeExpander.expand_actual_type` (argmap.py:269-364).
///
/// The `actual_type` argument is the wire serialization of the *proper*
/// actual type (the Python shim applies `get_proper_type` before
/// serializing, matching the Python method's first step). Returns
/// `(decision, name or None, new_tuple_index, sorted new kwargs_used)`;
/// `None` defers to Python (graph-dependent branch or undecodable blob).
///
/// `kwargs_used` arrives as a sorted Vec (the Python set, sorted for
/// determinism) and is returned re-sorted. `tuple_index` is the raw Python
/// value; the wrap-around semantics (already-exhausted → start over at 1,
/// else increment) are mirrored exactly.
#[pyfunction]
#[pyo3(signature = (
    actual_type,
    actual_kind,
    formal_name,
    formal_kind,
    allow_unpack,
    tuple_index,
    kwargs_used,
))]
pub fn rust_expand_actual_type(
    actual_type: Vec<u8>,
    actual_kind: i64,
    formal_name: Option<String>,
    formal_kind: i64,
    allow_unpack: bool,
    tuple_index: i64,
    kwargs_used: Vec<String>,
) -> Option<(i64, Option<String>, i64, Vec<String>)> {
    // `allow_unpack` is accepted for Python-signature parity but unused: the
    // TUPLE decision lets Python handle the UnpackType fallback itself.
    let _ = allow_unpack;
    let mut buf = ReadBuffer::new(&actual_type);
    let actualt = read_type(&mut buf, None).ok()?;
    match actual_kind {
        ARG_STAR => match &actualt {
            Type::TupleType { items, .. } => {
                let len = items.len() as i64;
                let new_index = if tuple_index >= len {
                    1
                } else {
                    tuple_index + 1
                };
                Some((DECISION_TUPLE, None, new_index, kwargs_used))
            }
            Type::ParamSpecType { .. } => {
                Some((DECISION_PASSTHROUGH, None, tuple_index, kwargs_used))
            }
            // Iterable unpacking and TypeVarTuple upper-bound expansion need
            // `is_subtype` / graph state: defer to Python.
            Type::Instance { .. } | Type::TypeVarTupleType { .. } => None,
            _ => Some((DECISION_ANY_ERROR, None, tuple_index, kwargs_used)),
        },
        ARG_STAR2 => match &actualt {
            Type::TypedDictType { items, .. } => {
                if formal_kind == ARG_STAR2 {
                    // Python pops an arbitrary unused key (set iteration
                    // order): not deterministic, defer.
                    return None;
                }
                let chosen = formal_name.as_deref()?;
                if !items.iter().any(|(k, _)| k == chosen) {
                    // Name not among the TypedDict keys: Python pops
                    // arbitrarily, defer.
                    return None;
                }
                let mut new_used = kwargs_used;
                if !new_used.iter().any(|k| k == chosen) {
                    new_used.push(chosen.to_string());
                    new_used.sort_unstable();
                }
                Some((
                    DECISION_KWARG,
                    Some(chosen.to_string()),
                    tuple_index,
                    new_used,
                ))
            }
            Type::ParamSpecType { .. } => {
                Some((DECISION_PASSTHROUGH, None, tuple_index, kwargs_used))
            }
            // Mapping unpacking needs `is_subtype`: defer to Python.
            Type::Instance { .. } => None,
            _ => Some((DECISION_ANY_ERROR, None, tuple_index, kwargs_used)),
        },
        // No translation for other kinds -- 1:1 mapping.
        _ => Some((DECISION_PASSTHROUGH, None, tuple_index, kwargs_used)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn blob(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok();
        buf.into_bytes()
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn object_type() -> Type {
        Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tuple_type(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(object_type()),
            items,
            implicit: false,
        }
    }

    fn str_type() -> Type {
        Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn test_tuple_star_advances_index() {
        // *args: tuple[int, str]; first item.
        let t = tuple_type(vec![any_type(), str_type()]);
        let r = rust_expand_actual_type(blob(&t), ARG_STAR, None, 0, false, 0, vec![]);
        assert_eq!(r, Some((DECISION_TUPLE, None, 1, vec![])));
    }

    #[test]
    fn test_tuple_star_second_item() {
        let t = tuple_type(vec![any_type(), str_type()]);
        let r = rust_expand_actual_type(blob(&t), ARG_STAR, None, 0, false, 1, vec![]);
        assert_eq!(r, Some((DECISION_TUPLE, None, 2, vec![])));
    }

    #[test]
    fn test_tuple_star_wrap_after_exhaustion() {
        // tuple_index == len: exhausted, wrap to 1 (Python's reset).
        let t = tuple_type(vec![any_type(), str_type()]);
        let r = rust_expand_actual_type(blob(&t), ARG_STAR, None, 0, false, 2, vec![]);
        assert_eq!(r, Some((DECISION_TUPLE, None, 1, vec![])));
    }

    #[test]
    fn test_tuple_star_unpack_allowed_index() {
        // Unpack item with allow_unpack: Python uses items[new_index - 1] as-is.
        let t = tuple_type(vec![
            Type::UnpackType {
                typ: Box::new(any_type()),
                from_star_syntax: false,
            },
            str_type(),
        ]);
        let r = rust_expand_actual_type(blob(&t), ARG_STAR, None, 0, true, 0, vec![]);
        assert_eq!(r, Some((DECISION_TUPLE, None, 1, vec![])));
    }

    #[test]
    fn test_star_instance_defers() {
        // Iterable unpacking needs is_subtype: defer to Python.
        let inst = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![any_type()],
            last_known_value: None,
            extra_attrs: None,
        };
        let r = rust_expand_actual_type(blob(&inst), ARG_STAR, None, 0, false, 0, vec![]);
        assert_eq!(r, None);
    }

    #[test]
    fn test_star_any_error() {
        // A bare scalar (non-Tuple, non-Instance, non-ParamSpec) *arg: Python's
        // deterministic else branch yields error-Any.
        let r = rust_expand_actual_type(blob(&any_type()), ARG_STAR, None, 0, false, 0, vec![]);
        assert_eq!(r, Some((DECISION_ANY_ERROR, None, 0, vec![])));
    }

    #[test]
    fn test_star2_typeddict_named_key() {
        // **kwargs: {x: int, y: str} against formal "x": pick x.
        let td = Type::TypedDictType {
            fallback: Box::new(object_type()),
            items: vec![("x".to_string(), any_type()), ("y".to_string(), str_type())],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let r = rust_expand_actual_type(
            blob(&td),
            ARG_STAR2,
            Some("x".to_string()),
            0,
            false,
            0,
            vec![],
        );
        assert_eq!(
            r,
            Some((
                DECISION_KWARG,
                Some("x".to_string()),
                0,
                vec!["x".to_string()]
            ))
        );
    }

    #[test]
    fn test_star2_typeddict_unmatched_name_defers() {
        // Name not among the TypedDict keys: Python pops an arbitrary unused
        // key (set hash-order), not deterministic, defer.
        let td = Type::TypedDictType {
            fallback: Box::new(object_type()),
            items: vec![("x".to_string(), any_type()), ("y".to_string(), str_type())],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let r = rust_expand_actual_type(
            blob(&td),
            ARG_STAR2,
            Some("z".to_string()),
            0,
            false,
            0,
            vec!["x".to_string()],
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_star2_typeddict_star2_formal_defers() {
        // formal_kind == ARG_STAR2: Python pops an arbitrary unused key.
        let td = Type::TypedDictType {
            fallback: Box::new(object_type()),
            items: vec![("x".to_string(), any_type())],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let r = rust_expand_actual_type(
            blob(&td),
            ARG_STAR2,
            Some("x".to_string()),
            ARG_STAR2,
            false,
            0,
            vec![],
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_star2_typeddict_exhausted_defers() {
        // All keys consumed: Python would KeyError; defer so Python decides.
        let td = Type::TypedDictType {
            fallback: Box::new(object_type()),
            items: vec![("x".to_string(), any_type())],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let r = rust_expand_actual_type(
            blob(&td),
            ARG_STAR2,
            Some("z".to_string()),
            0,
            false,
            0,
            vec!["x".to_string()],
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_star2_mapping_instance_defers() {
        let inst = Type::Instance {
            type_ref: "builtins.dict".to_string(),
            args: vec![str_type(), any_type()],
            last_known_value: None,
            extra_attrs: None,
        };
        let r = rust_expand_actual_type(blob(&inst), ARG_STAR2, None, 0, false, 0, vec![]);
        assert_eq!(r, None);
    }

    #[test]
    fn test_star2_any_error() {
        // Non-TypedDict, non-Instance, non-ParamSpec **kwargs: error-Any.
        let r = rust_expand_actual_type(blob(&any_type()), ARG_STAR2, None, 0, false, 0, vec![]);
        assert_eq!(r, Some((DECISION_ANY_ERROR, None, 0, vec![])));
    }

    #[test]
    fn test_non_star_passthrough() {
        // Positional or named actual: 1:1, no state change.
        let r = rust_expand_actual_type(
            blob(&str_type()),
            0,
            None,
            0,
            false,
            5,
            vec!["x".to_string()],
        );
        assert_eq!(
            r,
            Some((DECISION_PASSTHROUGH, None, 5, vec!["x".to_string()]))
        );
    }

    #[test]
    fn test_garbage_blob_defers() {
        let r = rust_expand_actual_type(vec![0xFF; 16], ARG_STAR, None, 0, false, 0, vec![]);
        assert_eq!(r, None);
    }
}
