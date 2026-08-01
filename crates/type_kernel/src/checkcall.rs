//! Stage 4 `check_call` dispatch classification (checkcall.rs).
//!
//! Ports the pure dispatch decision at the top of `mypy.checkexpr.check_call`:
//! given an already-proper callee type, classify it into the branch that
//! Python's `isinstance` chain would take. Purely structural; no mutation,
//! no checker state, so the kernel cannot emit/suppress errors here.

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type, WireError};

/// Dispatch kinds mirroring `check_call`'s isinstance chain.
pub(crate) const CALL_PLAIN: i64 = 0; // CallableType without variables
pub(crate) const CALL_WITH_VARS: i64 = 1; // CallableType with variables
pub(crate) const CALL_OVERLOADED: i64 = 2; // Overloaded
pub(crate) const CALL_ANY: i64 = 3; // AnyType (or not checked function)
pub(crate) const CALL_UNION: i64 = 4; // UnionType
pub(crate) const CALL_INSTANCE: i64 = 5; // Instance -> __call__ member access
pub(crate) const CALL_TYPE_TYPE: i64 = 6; // TypeType (falls through to member access)
pub(crate) const CALL_OTHER: i64 = 7;

/// Classify an already-proper callee type into the `check_call` dispatch
/// branch. Defer (None) on any wire/decode failure.
#[pyfunction]
pub(crate) fn rust_classify_call(callee_bytes: &[u8]) -> Option<i64> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    classify_call(&callee).ok()
}

/// Pure classification mirroring `check_call`'s isinstance chain.
fn classify_call(callee: &Type) -> Result<i64, WireError> {
    Ok(match callee {
        Type::CallableType { variables, .. } => {
            if variables.is_empty() {
                CALL_PLAIN
            } else {
                CALL_WITH_VARS
            }
        }
        Type::Overloaded { .. } => CALL_OVERLOADED,
        Type::AnyType { .. } => CALL_ANY,
        Type::UnionType { .. } => CALL_UNION,
        Type::Instance { .. } => CALL_INSTANCE,
        Type::TypeType { .. } => CALL_TYPE_TYPE,
        _ => CALL_OTHER,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn classify_bytes(t: &Type) -> Option<i64> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok()?;
        rust_classify_call(&buf.into_bytes())
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance() -> Type {
        Type::Instance {
            type_ref: "mod.C".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn type_var() -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "mod".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn callable(variables: usize) -> Type {
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(any_type()),
            name: None,
            variables: vec![type_var(); variables],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn classifies_plain_callable() {
        assert_eq!(classify_bytes(&callable(0)), Some(CALL_PLAIN));
    }

    #[test]
    fn classifies_callable_with_vars() {
        assert_eq!(classify_bytes(&callable(2)), Some(CALL_WITH_VARS));
    }

    #[test]
    fn classifies_any() {
        assert_eq!(classify_bytes(&any_type()), Some(CALL_ANY));
    }

    #[test]
    fn classifies_instance() {
        assert_eq!(classify_bytes(&instance()), Some(CALL_INSTANCE));
    }

    #[test]
    fn classifies_overloaded() {
        let t = Type::Overloaded {
            items: vec![callable(0)],
        };
        assert_eq!(classify_bytes(&t), Some(CALL_OVERLOADED));
    }

    #[test]
    fn classifies_union() {
        let t = Type::UnionType {
            items: vec![any_type(), instance()],
            uses_pep604_syntax: false,
        };
        assert_eq!(classify_bytes(&t), Some(CALL_UNION));
    }
}
