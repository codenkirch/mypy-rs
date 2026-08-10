//! Per-argument type diagnosis for `check_argument_types` / `check_arg`
//! (checkexpr.py:3356/3489). All-or-nothing per-call gate: emits per-arg
//! error records for 1:1 non-star arguments; returns None to defer the
//! whole call to Python for any undecidable input.

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, ReadBuffer, Type};

const ARG_STAR: i64 = 2;
const ARG_STAR2: i64 = 4;

const ERR_DELETED: i64 = 0;
const ERR_ABSTRACT: i64 = 1;
const ERR_INCOMPATIBLE: i64 = 2;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_check_arguments(
    resolver: &NativeTypeResolver,
    callee_bytes: &[u8],
    arg_types_bytes: Vec<Vec<u8>>,
    arg_kinds: Vec<i64>,
    formal_to_actual: Vec<Vec<i64>>,
    strict_optional: bool,
    allow_abstract_call: bool,
) -> Option<Vec<(i64, i64, i64)>> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    let Type::CallableType {
        arg_types: formal_types,
        ..
    } = callee
    else {
        return None;
    };

    let mut arg_types = Vec::with_capacity(arg_types_bytes.len());
    for bytes in &arg_types_bytes {
        let t = decode_type(bytes)?;
        if matches!(t, Type::TypeAliasType { .. }) {
            return None;
        }
        arg_types.push(t);
    }
    // Defer when a formal needs Python's Unpack/alias expansion.
    if formal_types
        .iter()
        .any(|t| matches!(t, Type::UnpackType { .. } | Type::TypeAliasType { .. }))
    {
        return None;
    }
    // Star actuals need the mapper (tuple/iterable expansion): defer.
    if arg_kinds.iter().any(|&k| k == ARG_STAR || k == ARG_STAR2) {
        return None;
    }
    // 1:1 mapping or defer (multi-actual formals / dup actuals).
    let mut seen = vec![false; arg_types.len()];
    for actuals in &formal_to_actual {
        if actuals.len() > 1 {
            return None;
        }
        if let Some(&ai) = actuals.first() {
            let ai = ai as usize;
            if ai >= seen.len() || seen[ai] {
                return None;
            }
            seen[ai] = true;
        }
    }
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let mut records = Vec::new();
    for (fi, actuals) in formal_to_actual.iter().enumerate() {
        let Some(&ai) = actuals.first() else { continue };
        let ai = ai as usize;
        let caller = arg_types.get(ai)?;
        let formal = formal_types.get(fi)?;
        if matches!(caller, Type::DeletedType { .. }) {
            records.push((ERR_DELETED, ai as i64, fi as i64));
            continue;
        }
        if !allow_abstract_call {
            match has_abstract_type_part(caller, formal, resolver.resolver()) {
                Some(true) => {
                    records.push((ERR_ABSTRACT, ai as i64, fi as i64));
                    continue;
                }
                Some(false) => {}
                None => return None,
            }
        }
        match is_subtype(caller, formal, &ctx, resolver.resolver()) {
            Some(true) => {}
            Some(false) => records.push((ERR_INCOMPATIBLE, ai as i64, fi as i64)),
            None => return None,
        }
    }
    Some(records)
}

/// `has_abstract_type_part` (checkexpr.py:7356-7363). Zip-one-level tuples,
/// short-circuit any `has_abstract_type` True (Python's `any` stops there,
/// so an undecidable later pair never matters once a True is found).
fn has_abstract_type_part(caller: &Type, callee: &Type, resolver: &TypeResolver) -> Option<bool> {
    if let (Type::TupleType { items: c, .. }, Type::TupleType { items: k, .. }) = (caller, callee) {
        for (c, k) in c.iter().zip(k.iter()) {
            if has_abstract_type(c, k, resolver)? {
                return Some(true);
            }
        }
        return Some(false);
    }
    has_abstract_type(caller, callee, resolver)
}

/// `has_abstract_type` (checkexpr.py:7365-7374).
/// Python: caller.is_type_obj() then type_object() abstract/protocol.
/// is_type_obj() needs fallback.type.is_metaclass() (not in snapshot);
/// approximate via instance_type (set on real type objects); defer when
/// instance_type is absent or not a resolvable Instance.
fn has_abstract_type(caller: &Type, callee: &Type, resolver: &TypeResolver) -> Option<bool> {
    let (is_fl, abs) = match caller {
        Type::CallableType {
            instance_type: Some(inner),
            ..
        } => {
            let Type::Instance { type_ref, .. } = inner.as_ref() else {
                return None;
            };
            let snap = resolver.get(type_ref)?;
            (true, snap.is_abstract || snap.is_protocol)
        }
        Type::CallableType {
            instance_type: None,
            ..
        } => (true, false),
        Type::Overloaded { .. } => {
            // is_type_obj() on an overload = items[0].is_type_obj(); can't read
            // fallback meta; defer when callee is TypeType (Python may decide
            // via items[0]).
            if matches!(callee, Type::TypeType { .. }) {
                return None;
            }
            (false, false)
        }
        _ => (false, false),
    };
    if !is_fl || !abs {
        return Some(false);
    }
    let Type::TypeType { item, .. } = callee else {
        return Some(false);
    };
    let Type::Instance { type_ref: t, .. } = item.as_ref() else {
        return None;
    };
    let snap = resolver.get(t)?;
    Some(snap.is_abstract || snap.is_protocol)
}
