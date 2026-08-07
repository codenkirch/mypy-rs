//! Native port of pure helper functions from `mypy/checkpattern.py`
//! (M22, issue #300).
//!
//! `PatternChecker` itself is a visitor deeply intertwined with the type
//! checker (`chk.expr_checker.accept`, `conditional_types_with_intersection`,
//! `msg.fail`), so the full visitor cannot be ported as standalone Rust
//! functions without porting the entire checker. Instead, this module ports
//! the standalone pure-logic helpers that `PatternChecker` calls:
//!
//! * `rust_is_uninhabited` — mirrors `checkpattern.is_uninhabited`.
//! * `rust_get_match_arg_names` — mirrors
//!   `checkpattern.get_match_arg_names`.
//! * `rust_get_type_range` — mirrors `checkpattern.get_type_range`: whether a
//!   bool `last_known_value` should unwrap before wrapping in a `TypeRange`.
//! * `rust_should_self_match` — mirrors `PatternChecker.should_self_match`.
//! * `rust_can_match_sequence` — mirrors `PatternChecker.can_match_sequence`.
//!
//! Each function takes wire-format bytes (serialized `Type` objects) and a
//! `NativeTypeResolver` for subtyping checks. Returns `None` to defer to
//! Python when the wire form cannot be fully decoded or a subtyping check
//! is undecided (the strangler-fig per-call gate).

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, LiteralValue, ReadBuffer, Type};

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `checkpattern.is_uninhabited(typ)` — whether `get_proper_type(typ)` is an
/// `UninhabitedType`.
///
/// Mirrors checkpattern.py:884-885. On the wire side, `get_proper_type` is
/// already applied by the Python shim before serialization (the caller
/// passes a `ProperType`), so we just match on `Type::UninhabitedType`.
/// Returns `None` (defer) when the type is a `TypeAliasType` (cannot resolve
/// to a proper type on the wire).
#[pyfunction]
pub(crate) fn rust_is_uninhabited(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    Some(matches!(t, Type::UninhabitedType { .. }))
}

/// `checkpattern.get_match_arg_names(typ)` — extract match-arg names from a
/// `TupleType`'s items.
///
/// Mirrors checkpattern.py:851-859. For each item in `typ.items`, calls
/// `try_getting_str_literals_from_type`. If the result is `None` or has
/// length != 1, appends `None`; otherwise appends the single str value.
///
/// Returns `None` (defer to Python) when the wire form cannot be decoded or
/// contains a `TypeAliasType` (which `try_getting_str_literals_from_type`
/// cannot resolve). Returns `Some(list)` where each element is a string or
/// `None`.
#[pyfunction]
pub(crate) fn rust_get_match_arg_names(py: Python<'_>, t_bytes: &[u8]) -> Option<PyObject> {
    let t = decode_type(t_bytes)?;
    let items = match &t {
        Type::TupleType { items, .. } => items,
        _ => return None,
    };
    let mut names: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items {
        match extract_single_str_literal(item) {
            Some(s) => names.push(s.into_py(py)),
            None => names.push(py.None()),
        }
    }
    Some(pyo3::types::PyList::new(py, names).into())
}

/// Extract a single string literal from a wire `Type`, mirroring
/// `try_getting_str_literals_from_type` returning exactly one value.
///
/// Handles: Instance with str LKV, LiteralType with str fallback, UnionType
/// of str literals. Returns `None` if no single str literal can be extracted
/// (matching Python returning `None` or length != 1).
fn extract_single_str_literal(t: &Type) -> Option<String> {
    let candidates: Vec<&Type> = match t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => vec![lkv.as_ref()],
        Type::UnionType { items, .. } => items.iter().collect(),
        _ => vec![t],
    };
    let mut found: Option<String> = None;
    for c in candidates {
        match c {
            Type::LiteralType { fallback, value } => {
                let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                    return None;
                };
                if type_ref != "builtins.str" {
                    return None;
                }
                match value {
                    LiteralValue::Str(s) => {
                        if found.is_some() {
                            return None;
                        }
                        found = Some(s.clone());
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    found
}

/// `checkpattern.get_type_range(typ)` — determine whether a type's
/// `last_known_value` (if it's a bool) should be unwrapped before wrapping in
/// a `TypeRange`.
///
/// Mirrors checkpattern.py:873-881. The Python function:
///   ```text
///   typ = get_proper_type(typ)
///   if isinstance(typ, Instance) and typ.last_known_value
///       and isinstance(typ.last_known_value.value, bool):
///       typ = typ.last_known_value
///   return TypeRange(typ, is_upper_bound=False)
///   ```
///
/// Returns `Some(true)` when the type is an Instance with a bool LKV (the
/// caller should unwrap to `typ.last_known_value` before building the
/// TypeRange). Returns `Some(false)` when the type does not need unwrapping.
/// Returns `None` (defer) when the wire form is a `TypeAliasType`.
#[pyfunction]
pub(crate) fn rust_get_type_range(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    match &t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => {
            // Check if the LKV is a LiteralType with a bool value and a
            // builtins.bool fallback.
            match lkv.as_ref() {
                Type::LiteralType { fallback, value } => {
                    let is_bool = matches!(
                        fallback.as_ref(),
                        Type::Instance { type_ref, .. } if type_ref == "builtins.bool"
                    ) && matches!(value, LiteralValue::Bool(_));
                    Some(is_bool)
                }
                _ => Some(false),
            }
        }
        _ => Some(false),
    }
}

/// `PatternChecker.should_self_match(typ)` — whether a class pattern should
/// match against the type itself rather than its `__match_args__`.
///
/// Mirrors checkpattern.py:756-769. The Python method:
///   ```text
///   typ = get_proper_type(typ)
///   if isinstance(typ, TupleType):
///       typ = typ.partial_fallback
///   if isinstance(typ, AnyType):
///       return False
///   if isinstance(typ, Instance) and typ.type.get("__match_args__") is not None:
///       return False
///   for other in self.self_match_types:
///       if is_subtype(typ, other):
///           return True
///   return False
///   ```
///
/// The `__match_args__` check needs live `TypeInfo` (not on the wire), so it
/// is handled by the Python shim before calling this function. The shim
/// passes `has_match_args: bool` so Rust can short-circuit.
///
/// Returns `None` (defer) when the type is a `TypeAliasType` or any subtype
/// check returns `None`.
#[pyfunction]
pub(crate) fn rust_should_self_match(
    t_bytes: &[u8],
    has_match_args: bool,
    self_match_types_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    let self_match_types = decode_type(self_match_types_bytes)?;

    // Unwrap TupleType to its partial_fallback.
    let typ = match &t {
        Type::TupleType {
            partial_fallback, ..
        } => partial_fallback.as_ref(),
        _ => &t,
    };

    // AnyType -> False.
    if matches!(typ, Type::AnyType { .. }) {
        return Some(false);
    }

    // Instance with __match_args__ -> False (checked by Python shim).
    if has_match_args && matches!(typ, Type::Instance { .. }) {
        return Some(false);
    }

    // Check is_subtype(typ, other) for each self_match_type.
    let items = match &self_match_types {
        Type::UnionType { items, .. } => items.clone(),
        // Single type wrapped in a list.
        _ => vec![self_match_types.clone()],
    };

    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    for other in &items {
        match is_subtype(typ, other, &ctx, resolver.resolver()) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

/// `PatternChecker.can_match_sequence(typ)` — whether a type can match a
/// sequence pattern.
///
/// Mirrors checkpattern.py:771-783. The Python method:
///   ```text
///   if isinstance(typ, AnyType): return True
///   if isinstance(typ, UnionType):
///       return any(self.can_match_sequence(item) for item in typ.items)
///   for other in self.non_sequence_match_types:
///       if is_subtype(typ, other, ignore_promotions=True): return False
///   sequence = self.chk.named_type("typing.Sequence")
///   return is_subtype(typ, sequence) or is_subtype(sequence, typ)
///   ```
///
/// `self.non_sequence_match_types` and `typing.Sequence` are serialized as
/// wire bytes by the Python shim. Returns `None` (defer) when any subtype
/// check is undecided.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_can_match_sequence(
    t_bytes: &[u8],
    non_seq_types_bytes: &[u8],
    sequence_type_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    let non_seq_types = decode_type(non_seq_types_bytes)?;
    let sequence_type = decode_type(sequence_type_bytes)?;

    can_match_sequence_inner(&t, &non_seq_types, &sequence_type, resolver)
}

/// Recursive inner: mirrors the UnionType recursion in the Python method.
fn can_match_sequence_inner(
    typ: &Type,
    non_seq_types: &Type,
    sequence_type: &Type,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    // AnyType -> True.
    if matches!(typ, Type::AnyType { .. }) {
        return Some(true);
    }
    // UnionType -> any(item can match sequence).
    if let Type::UnionType { items, .. } = typ {
        for item in items {
            match can_match_sequence_inner(item, non_seq_types, sequence_type, resolver) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
        }
        return Some(false);
    }
    // non_sequence_match_types: if is_subtype(typ, other, ignore_promotions)
    // -> return False.
    let non_seq_items = match non_seq_types {
        Type::UnionType { items, .. } => items.clone(),
        _ => vec![non_seq_types.clone()],
    };
    // ignore_promotions=True, proper_subtype=False, strict_optional=True.
    let ctx = SubtypeContext::new(false, false, false, true, false, true);
    for other in &non_seq_items {
        match is_subtype(typ, other, &ctx, resolver.resolver()) {
            Some(true) => return Some(false),
            None => return None,
            Some(false) => {}
        }
    }
    // sequence check: is_subtype(typ, sequence) or is_subtype(sequence, typ).
    match is_subtype(typ, sequence_type, &ctx, resolver.resolver()) {
        Some(true) => Some(true),
        None => None,
        Some(false) => match is_subtype(sequence_type, typ, &ctx, resolver.resolver()) {
            Some(true) => Some(true),
            None => None,
            Some(false) => Some(false),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_uninhabited_uninhabited() {
        let t = Type::UninhabitedType { ambiguous: false };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_is_uninhabited(&bytes), Some(true));
    }

    #[test]
    fn test_is_uninhabited_not_uninhabited() {
        let t = Type::NoneType;
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_is_uninhabited(&bytes), Some(false));
    }

    #[test]
    fn test_get_type_range_bool_lkv() {
        // Instance with bool LKV should return Some(true).
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let t = Type::Instance {
            type_ref: "builtins.bool".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv)),
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(true));
    }

    #[test]
    fn test_get_type_range_no_lkv() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(false));
    }

    #[test]
    fn test_get_type_range_int_lkv_not_bool() {
        // Instance with int LKV should return Some(false) (not a bool).
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv)),
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(false));
    }
}
