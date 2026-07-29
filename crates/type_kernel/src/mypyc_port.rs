#![allow(non_local_definitions)]

//! Native port of pure helpers from `mypyc/irbuild/prepare.py`
//! (Stage 16, Issue #99).
//!
//! Ports the module-level functions that operate on primitive data
//! (strings, booleans) without needing mypyc IR types (RType, ClassIR,
//! FuncSignature, Mapper):
//! - `can_subclass_builtin` — set membership check against a fixed
//!   allowlist of builtins that mypyc can subclass.
//!
//! Deferred (need RType / IR types not yet in Rust):
//! `is_runtime_subtype`, `is_subtype` (RType visitor), `check_matching_args`
//! (FuncSignature), `prepare_class_def`, `prepare_init_method`,
//! `find_non_acyclic_base` (ClassDef/IR). These require porting the mypyc
//! RType enum and IR data structures first, which is a larger effort.

use pyo3::prelude::*;

/// `mypyc.irbuild.prepare.can_subclass_builtin` — check if a builtin base
/// can be subclassed by mypyc-compiled code.
///
/// Mirrors `can_subclass_builtin` (prepare.py:379-392). BaseException and
/// dict are special-cased (excluded). The allowlist is:
/// - builtins.Exception
/// - builtins.LookupError
/// - builtins.IndexError
/// - builtins.Warning
/// - builtins.UserWarning
/// - builtins.ValueError
/// - builtins.object
#[pyfunction]
pub(crate) fn rust_can_subclass_builtin(builtin_base: &str) -> PyResult<bool> {
    Ok(matches!(
        builtin_base,
        "builtins.Exception"
            | "builtins.LookupError"
            | "builtins.IndexError"
            | "builtins.Warning"
            | "builtins.UserWarning"
            | "builtins.ValueError"
            | "builtins.object"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_subclass_object() {
        assert!(rust_can_subclass_builtin("builtins.object").unwrap());
    }

    #[test]
    fn test_can_subclass_exception() {
        assert!(rust_can_subclass_builtin("builtins.Exception").unwrap());
    }

    #[test]
    fn test_can_subclass_value_error() {
        assert!(rust_can_subclass_builtin("builtins.ValueError").unwrap());
    }

    #[test]
    fn test_cannot_subclass_dict() {
        assert!(!rust_can_subclass_builtin("builtins.dict").unwrap());
    }

    #[test]
    fn test_cannot_subclass_base_exception() {
        assert!(!rust_can_subclass_builtin("builtins.BaseException").unwrap());
    }

    #[test]
    fn test_cannot_subclass_str() {
        assert!(!rust_can_subclass_builtin("builtins.str").unwrap());
    }

    #[test]
    fn test_cannot_subclass_int() {
        assert!(!rust_can_subclass_builtin("builtins.int").unwrap());
    }

    #[test]
    fn test_cannot_subclass_arbitrary() {
        assert!(!rust_can_subclass_builtin("foo.bar.Baz").unwrap());
    }

    #[test]
    fn test_can_subclass_warning() {
        assert!(rust_can_subclass_builtin("builtins.Warning").unwrap());
    }

    #[test]
    fn test_can_subclass_lookup_error() {
        assert!(rust_can_subclass_builtin("builtins.LookupError").unwrap());
    }

    #[test]
    fn test_can_subclass_index_error() {
        assert!(rust_can_subclass_builtin("builtins.IndexError").unwrap());
    }

    #[test]
    fn test_can_subclass_user_warning() {
        assert!(rust_can_subclass_builtin("builtins.UserWarning").unwrap());
    }
}
