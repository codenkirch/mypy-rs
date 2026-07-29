//! Native check_call dispatch & fast-path checker (Stage 4, #147).
//!
//! Ports the high-level call dispatch and single-callable positional matching
//! logic of `mypy.checkexpr.Calling.check_call` to Rust.
//!
//! Defer-Mutation Protocol (Option A):
//! - Rust receives: callee type bytes, actual arg count, positional arg names,
//!   and whether star actuals are present.
//! - Rust evaluates: callee dispatch kind ("callable", "overloaded", "any", "union"),
//!   arity constraints, and positional arg-binding viability.
//! - Rust returns: `CheckCallResult` enum (FastPass, DeferToPython, etc.).
//! - Python applies: full call checking or falls back to Python `check_call`.

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type};

/// Fast-path classification result for `rust_check_call_fast_path`.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckCallKind {
    /// Non-generic single CallableType with valid positional arity.
    SingleCallablePositional,
    /// Overloaded callable.
    Overloaded,
    /// AnyType callee.
    Any,
    /// UnionType callee.
    Union,
    /// Needs full Python handling (generics, star actuals, named args, etc.).
    Defer,
}

#[pyfunction]
pub fn rust_check_call_fast_path(
    callee_bytes: &[u8],
    actual_pos_count: usize,
    actual_named_count: usize,
    has_star_actuals: bool,
) -> PyResult<Option<String>> {
    if has_star_actuals || actual_named_count > 0 {
        return Ok(None);
    }
    let mut buf = ReadBuffer::new(callee_bytes);
    let typ = match read_type(&mut buf, None) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    let kind = match typ {
        Type::CallableType {
            arg_kinds,
            variables,
            ..
        } => {
            // If function is generic (has TypeVar variables), defer to Python
            if !variables.is_empty() {
                CheckCallKind::Defer
            } else {
                // Check positional argument count against required positional formals (ARG_POS = 0)
                let min_pos = arg_kinds.iter().filter(|&&k| k == 0).count();
                let max_pos = arg_kinds.iter().filter(|&&k| k == 0 || k == 1).count();
                let has_star_formal = arg_kinds.contains(&2);

                if actual_pos_count >= min_pos && (has_star_formal || actual_pos_count <= max_pos) {
                    CheckCallKind::SingleCallablePositional
                } else {
                    CheckCallKind::Defer
                }
            }
        }
        Type::Overloaded { .. } => CheckCallKind::Overloaded,
        Type::AnyType { .. } => CheckCallKind::Any,
        Type::UnionType { .. } => CheckCallKind::Union,
        _ => CheckCallKind::Defer,
    };

    let res_str = match kind {
        CheckCallKind::SingleCallablePositional => "single_pos",
        CheckCallKind::Overloaded => "overloaded",
        CheckCallKind::Any => "any",
        CheckCallKind::Union => "union",
        CheckCallKind::Defer => return Ok(None),
    };
    Ok(Some(res_str.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    #[test]
    fn test_single_callable_positional_match() {
        let callable = Type::CallableType {
            arg_types: vec![Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }],
            arg_kinds: vec![0], // ARG_POS
            arg_names: vec![Some("x".to_string())],
            ret_type: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.function".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            name: Some("foo".to_string()),
            variables: vec![],
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            type_guard: None,
            type_is: None,
        };

        let bytes = encode(&callable);
        let res = rust_check_call_fast_path(&bytes, 1, 0, false).unwrap();
        assert_eq!(res, Some("single_pos".to_string()));

        // Too few args
        let res_few = rust_check_call_fast_path(&bytes, 0, 0, false).unwrap();
        assert_eq!(res_few, None);

        // Too many args
        let res_many = rust_check_call_fast_path(&bytes, 2, 0, false).unwrap();
        assert_eq!(res_many, None);
    }
}
