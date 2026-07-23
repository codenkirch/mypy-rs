//! Stage 3e: typeops helpers from `mypy/typeops.py`.
//!
//! Ports pure-algebra helpers as standalone `#[pyfunction]`s:
//! * `rust_make_simplified_union` — wraps the existing `setops::make_simplified_union`.
//! * `rust_simple_literal_type` — extracts fallback Instance for simple literals.
//! * `rust_is_simple_literal` — checks if a type is a simple literal.
//! * `rust_true_only` / `rust_false_only` / `rust_true_or_false` — truthiness
//!   narrowing via discriminators (Python shim performs the `copy_type` +
//!   flag mutation on live objects).
//!
//! Parity-only, default-off. The Python shim in `mypy/typeops.py` gates each
//! call behind `Options.native_type_kernel`. `None` means "Rust doesn't handle
//! this, let Python decide".

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

use crate::setops;
use crate::subtypes::SubtypeContext;

// ---------------------------------------------------------------------------
// Wire codec helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

// ---------------------------------------------------------------------------
// simple_literal_type / is_simple_literal
// ---------------------------------------------------------------------------

/// `simple_literal_type` (typeops.py:588-594): return the fallback `Instance`
/// for a simple literal. If `t` is an `Instance` with a `last_known_value`,
/// unwrap to that literal first. If `t` is a `LiteralType`, return its
/// fallback. Otherwise return `None`.
///
/// Returns the fallback as wire-encoded bytes, or `None` if `t` is not a
/// simple literal (Python `None`).
fn simple_literal_type(t: &Type) -> Option<Type> {
    let t = match t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => lkv.as_ref(),
        _ => t,
    };
    match t {
        Type::LiteralType { fallback, .. } => Some((**fallback).clone()),
        _ => None,
    }
}

/// `is_simple_literal` (typeops.py:597-602): check if `t` is a simple literal.
///
/// A `LiteralType` is simple if its fallback is an enum or `builtins.str`.
/// An `Instance` is simple if it has a `last_known_value` whose value is a
/// string. The `is_enum` check needs the resolver snapshot; if the snapshot
/// is missing, conservatively return `false` (defer to Python).
fn is_simple_literal(t: &Type, resolver: &TypeResolver) -> Option<bool> {
    match t {
        Type::LiteralType { fallback, .. } => {
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(false);
            };
            if type_ref == "builtins.str" {
                return Some(true);
            }
            // enum check needs the snapshot
            let snap = resolver.get(type_ref)?;
            Some(snap.is_enum)
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => {
            if let Type::LiteralType { value, .. } = lkv.as_ref() {
                Some(matches!(value, LiteralValue::Str(_)))
            } else {
                Some(false)
            }
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// Truthiness helpers: can_be_true_default / can_be_false_default
// ---------------------------------------------------------------------------

/// Mirrors `Type.can_be_true_default()` for each variant (types.py:295-3459).
/// Returns `None` for variants where the default depends on live Python state
/// (TypeAliasType needs `alias.target`, TupleType needs `can_be_any_bool`).
fn can_be_true_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType => Some(false),
        Type::NoneType => Some(false),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            // Enum literal: depends on fallback's truthiness. For enum, the
            // Python code returns `self.fallback.can_be_true`, which for an
            // Instance defaults to True. Defer to be safe (snapshot lookup
            // might reveal is_enum, but the enum Instance's own truthiness
            // is the default True anyway).
            // Non-enum: bool(value)
            match value {
                LiteralValue::Bool(b) => Some(*b),
                LiteralValue::Int(i) => Some(*i != 0),
                LiteralValue::Str(s) => Some(!s.is_empty()),
                LiteralValue::Bytes(b) => Some(!b.is_empty()),
                LiteralValue::Float(f) => Some(*f != 0.0),
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_true_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        // All other variants default to True (Instance, AnyType, CallableType,
        // Overloaded, UnboundType, DeletedType, TypeType, TypeVarType,
        // ParamSpecType, TypeVarTupleType, Parameters, UnpackType).
        _ => Some(true),
    }
}

/// Mirrors `Type.can_be_false_default()` for each variant (types.py:298-3459).
fn can_be_false_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType => Some(false),
        Type::NoneType => Some(true),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            match value {
                LiteralValue::Bool(b) => Some(!*b),
                LiteralValue::Int(i) => Some(*i == 0),
                LiteralValue::Str(s) => Some(s.is_empty()),
                LiteralValue::Bytes(b) => Some(b.is_empty()),
                LiteralValue::Float(f) => Some(*f == 0.0),
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_false_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        _ => Some(true),
    }
}

// ---------------------------------------------------------------------------
// Truthiness discriminators
// ---------------------------------------------------------------------------

/// Result of `true_only` / `false_only` / `true_or_false`.
///
/// The Python shim maps each variant to a live `Type`:
/// * `Uninhabited` -> `UninhabitedType(line=t.line)`
/// * `None` (strict_optional off) -> `NoneType(line=t.line)`
/// * `SameType` -> `t` unchanged
/// * `CopyTrueOnly` -> `copy_type(t)` with `can_be_false=False`
/// * `CopyFalseOnly` -> `copy_type(t)` with `can_be_true=False`
/// * `CopyReset` -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
/// * `LiteralEmptyStr(fallback_bytes)` -> `LiteralType("", fallback)`
/// * `LiteralZero(fallback_bytes)` -> `LiteralType(0, fallback)`
/// * `UnionNarrow(item_discs)` -> recurse on each union item (discs[i] is
///   the discriminator for items[i])
#[derive(Clone)]
#[allow(dead_code)]
enum TruthinessResult {
    Uninhabited,
    NoneType,
    SameType,
    CopyTrueOnly,
    CopyFalseOnly,
    CopyReset,
    LiteralEmptyStr(Vec<u8>),
    LiteralZero(Vec<u8>),
    /// For unions: one TruthinessResult per item, so the Python shim can
    /// recurse. The outer shape is always a `make_simplified_union`.
    UnionNarrow(Vec<TruthinessResult>),
}

// ---------------------------------------------------------------------------
// true_only
// ---------------------------------------------------------------------------

/// `true_only` (typeops.py:790-817): restrict `t` to only True-ish values.
///
/// Logic:
/// 1. If `not can_be_true` -> `UninhabitedType`
/// 2. If `not can_be_false` -> `t` (already all-true)
/// 3. If `UnionType` -> union of `true_only` on each item, filtered to
///    `can_be_true` items, via `make_simplified_union`
/// 4. Else -> `copy_type(t)` with `can_be_false=False` (unless `__bool__`/
///    `__len__` ret_type says all-false, then UninhabitedType)
///
/// Step 4's `__bool__`/`__len__` lookup needs live TypeInfo -> defer (None).
/// Union recursion (step 3) recurses via discriminators.
fn true_only(t: &Type) -> Option<TruthinessResult> {
    let cbt = can_be_true_default(t)?;
    if !cbt {
        return Some(TruthinessResult::Uninhabited);
    }
    let cbf = can_be_false_default(t)?;
    if !cbf {
        return Some(TruthinessResult::SameType);
    }
    if let Type::UnionType { items, .. } = t {
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            let r = true_only(item)?;
            // Filter: only keep items that can_be_true.
            let item_cbt = can_be_true_default(item)?;
            if item_cbt {
                item_results.push(r);
            }
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    // Step 4: __bool__/__len__ lookup needs live TypeInfo -> defer.
    None
}

// ---------------------------------------------------------------------------
// false_only
// ---------------------------------------------------------------------------

/// `false_only` (typeops.py:820-862): restrict `t` to only False-ish values.
///
/// Logic:
/// 1. If `not can_be_false`:
///    - strict_optional -> `UninhabitedType`
///    - non-strict -> `NoneType`
/// 2. If `not can_be_true` -> `t` (already all-false)
/// 3. If `UnionType` -> union of `false_only` on each item, filtered to
///    `can_be_false` items, via `make_simplified_union`
/// 4. If `Instance(builtins.str)` or `Instance(builtins.bytes)` ->
///    `LiteralType("", fallback=t)`
/// 5. If `Instance(builtins.int)` -> `LiteralType(0, fallback=t)`
/// 6. Else -> `__bool__`/`__len__` lookup, or `copy_type(t)` with
///    `can_be_true=False`
///
/// Step 6's method lookup and `is_final`/`is_enum` checks need live TypeInfo
/// -> defer (None). Steps 4-5 return the literal directly.
fn false_only(t: &Type, strict_optional: bool) -> Option<TruthinessResult> {
    let cbf = can_be_false_default(t)?;
    if !cbf {
        if strict_optional {
            return Some(TruthinessResult::Uninhabited);
        } else {
            return Some(TruthinessResult::NoneType);
        }
    }
    let cbt = can_be_true_default(t)?;
    if !cbt {
        return Some(TruthinessResult::SameType);
    }
    if let Type::UnionType { items, .. } = t {
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            let r = false_only(item, strict_optional)?;
            let item_cbf = can_be_false_default(item)?;
            if item_cbf {
                item_results.push(r);
            }
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    // Steps 4-5: str/bytes/int Instance -> LiteralType("", fallback) or
    // LiteralType(0, fallback). Only fire for plain Instances (no args, no
    // last_known_value) matching the Python `isinstance(t, Instance)` check.
    if let Type::Instance { type_ref, .. } = t {
        if type_ref == "builtins.str" || type_ref == "builtins.bytes" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralEmptyStr(fb_bytes));
        }
        if type_ref == "builtins.int" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralZero(fb_bytes));
        }
    }
    // Step 6: __bool__/__len__ lookup + is_final/is_enum checks need live
    // TypeInfo -> defer.
    None
}

// ---------------------------------------------------------------------------
// true_or_false
// ---------------------------------------------------------------------------

/// `true_or_false` (typeops.py:865-878): unrestricted version of `t`.
///
/// Logic:
/// 1. If `UnionType` -> union of `true_or_false` on each item via
///    `make_simplified_union`
/// 2. Else -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
fn true_or_false(t: &Type) -> Option<TruthinessResult> {
    if let Type::UnionType { items, .. } = t {
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            item_results.push(true_or_false(item)?);
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    Some(TruthinessResult::CopyReset)
}

// ---------------------------------------------------------------------------
// Discriminator serialization for Python
// ---------------------------------------------------------------------------

/// Serialize a `TruthinessResult` to a Python-friendly tuple.
///
/// The encoding is a nested structure:
/// `(tag: i64, payload)` where:
/// * 0 = `Uninhabited`
/// * 1 = `NoneType`
/// * 2 = `SameType`
/// * 3 = `CopyTrueOnly`
/// * 4 = `CopyFalseOnly`
/// * 5 = `CopyReset`
/// * 6 = `LiteralEmptyStr(fallback_bytes)`
/// * 7 = `LiteralZero(fallback_bytes)`
/// * 8 = `UnionNarrow(item_discs)` — payload is a `Vec<TruthinessOut>`
type TruthinessOut = (i64, PyObject);

fn truthiness_to_py(py: Python<'_>, r: TruthinessResult) -> TruthinessOut {
    match r {
        TruthinessResult::Uninhabited => (0, py.None()),
        TruthinessResult::NoneType => (1, py.None()),
        TruthinessResult::SameType => (2, py.None()),
        TruthinessResult::CopyTrueOnly => (3, py.None()),
        TruthinessResult::CopyFalseOnly => (4, py.None()),
        TruthinessResult::CopyReset => (5, py.None()),
        TruthinessResult::LiteralEmptyStr(bytes) => {
            (6, pyo3::types::PyBytes::new(py, &bytes).into())
        }
        TruthinessResult::LiteralZero(bytes) => (7, pyo3::types::PyBytes::new(py, &bytes).into()),
        TruthinessResult::UnionNarrow(items) => {
            let py_items: Vec<TruthinessOut> =
                items.into_iter().map(|r| truthiness_to_py(py, r)).collect();
            (8, pyo3::types::PyList::new(py, py_items).into())
        }
    }
}

// ---------------------------------------------------------------------------
// #pyfunction entry points
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for `make_simplified_union`. Takes serialized items
/// + line/column/flags + `NativeTypeResolver`. Returns encoded result bytes
/// or `None` (defer to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_make_simplified_union(
    items_bytes: &[u8],
    line: i64,
    column: i64,
    keep_erased: bool,
    contract_literals: bool,
    handle_recursive: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    // items_bytes is a LIST_GEN-tagged list of serialized types.
    let mut buf = ReadBuffer::new(items_bytes);
    let items = wire::read_type_list(&mut buf).ok()?;
    let _ = (
        line,
        column,
        keep_erased,
        contract_literals,
        handle_recursive,
    );
    let ctx = SubtypeContext::new(false, false, false, true, false, true);
    let result = setops::make_simplified_union(&items, &ctx, resolver.resolver())?;
    encode_type(&result)
}

/// `#[pyfunction]` entry for `simple_literal_type`. Returns encoded fallback
/// Instance bytes, or `None`.
#[pyfunction]
pub(crate) fn rust_simple_literal_type(t_bytes: &[u8]) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let fallback = simple_literal_type(&t)?;
    encode_type(&fallback)
}

/// `#[pyfunction]` entry for `is_simple_literal`. Returns `Some(true)`/
/// `Some(false)` or `None` (defer to Python when snapshot lookup is needed
/// but missing).
#[pyfunction]
pub(crate) fn rust_is_simple_literal(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_simple_literal(&t, resolver.resolver())
}

/// `#[pyfunction]` entry for `true_only`. Returns a truthiness discriminator
/// tuple or `None` (defer to Python).
#[pyfunction]
pub(crate) fn rust_true_only(t_bytes: &[u8]) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = true_only(&t)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

/// `#[pyfunction]` entry for `false_only`.
#[pyfunction]
pub(crate) fn rust_false_only(t_bytes: &[u8], strict_optional: bool) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = false_only(&t, strict_optional)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

/// `#[pyfunction]` entry for `true_or_false`.
#[pyfunction]
pub(crate) fn rust_true_or_false(t_bytes: &[u8]) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = true_or_false(&t)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{LiteralValue, Type};

    #[test]
    fn simple_literal_type_extracts_fallback() {
        let fallback = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let lit = Type::LiteralType {
            fallback: Box::new(fallback.clone()),
            value: LiteralValue::Str("hello".to_string()),
        };
        let result = simple_literal_type(&lit).unwrap();
        assert_eq!(result, fallback);
    }

    #[test]
    fn simple_literal_type_unwraps_last_known_value() {
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv.clone())),
            extra_attrs: None,
        };
        let result = simple_literal_type(&inst).unwrap();
        // simple_literal_type unwraps Instance->lkv->lkv.fallback (the int Instance).
        assert_eq!(
            result,
            Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }
        );
    }

    #[test]
    fn simple_literal_type_returns_none_for_non_literal() {
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(simple_literal_type(&inst).is_none());
    }

    #[test]
    fn true_only_none_type_returns_uninhabited() {
        let t = Type::NoneType;
        let result = true_only(&t).unwrap();
        assert!(matches!(result, TruthinessResult::Uninhabited));
    }

    #[test]
    fn true_only_literal_true_returns_same() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let result = true_only(&t).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn false_only_none_type_returns_same() {
        let t = Type::NoneType;
        let result = false_only(&t, true).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn false_only_literal_false_returns_same() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(false),
        };
        let result = false_only(&t, true).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn true_or_false_instance_returns_copy_reset() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = true_or_false(&t).unwrap();
        assert!(matches!(result, TruthinessResult::CopyReset));
    }

    #[test]
    fn true_only_union_narrows_items() {
        // NoneType can_be_true=False -> filtered out.
        // LiteralType(True) can_be_true=True, can_be_false=False -> SameType.
        // The Instance(builtins.int) would defer (step 4 needs live TypeInfo),
        // so use a literal to avoid deferral.
        let t = Type::UnionType {
            items: vec![
                Type::NoneType,
                Type::LiteralType {
                    fallback: Box::new(Type::Instance {
                        type_ref: "builtins.bool".to_string(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    }),
                    value: LiteralValue::Bool(true),
                },
            ],
            uses_pep604_syntax: false,
        };
        let result = true_only(&t).unwrap();
        match result {
            TruthinessResult::UnionNarrow(items) => {
                // NoneType filtered out (can_be_true=False).
                // LiteralType(True) kept -> SameType (can_be_false=False).
                assert_eq!(items.len(), 1);
            }
            _ => panic!("expected UnionNarrow"),
        }
    }

    #[test]
    fn false_only_str_returns_literal_empty() {
        let t = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = false_only(&t, true).unwrap();
        match result {
            TruthinessResult::LiteralEmptyStr(_) => {}
            _ => panic!("expected LiteralEmptyStr"),
        }
    }

    #[test]
    fn false_only_int_returns_literal_zero() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = false_only(&t, true).unwrap();
        match result {
            TruthinessResult::LiteralZero(_) => {}
            _ => panic!("expected LiteralZero"),
        }
    }
}
