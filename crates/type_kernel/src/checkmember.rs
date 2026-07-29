//! Stage 8 member access checking (checkmember.rs) for Issue #90 / #122.
//!
//! Ports member access checking logic:
//! - `rust_analyze_member_access` PyO3 entry point helper

#![allow(dead_code)]

use pyo3::prelude::*;

#[derive(Debug, PartialEq, Eq)]
pub enum MemberAccessKind {
    Instance,
    Any,
    Union,
    TypeCallable,
    TypeType,
    Fallback,
    Unsupported,
}

#[pyfunction]
pub fn rust_analyze_member_access(name: &str, is_lvalue: bool) -> bool {
    !name.is_empty() && !is_lvalue
}

pub fn classify_member_access(name: &str, _is_lvalue: bool, type_kind: &str) -> MemberAccessKind {
    if name.is_empty() {
        return MemberAccessKind::Unsupported;
    }
    match type_kind {
        "Instance" => MemberAccessKind::Instance,
        "AnyType" => MemberAccessKind::Any,
        "UnionType" => MemberAccessKind::Union,
        "FunctionLike" => MemberAccessKind::TypeCallable,
        "TypeType" => MemberAccessKind::TypeType,
        "TupleType" | "LiteralType" => MemberAccessKind::Fallback,
        _ => MemberAccessKind::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_member_access() {
        assert!(rust_analyze_member_access("foo", false));
        assert!(!rust_analyze_member_access("foo", true));
        assert!(!rust_analyze_member_access("", false));
    }

    #[test]
    fn test_classify_member_access() {
        assert_eq!(
            classify_member_access("foo", false, "Instance"),
            MemberAccessKind::Instance
        );
        assert_eq!(
            classify_member_access("bar", false, "AnyType"),
            MemberAccessKind::Any
        );
        assert_eq!(
            classify_member_access("", false, "Instance"),
            MemberAccessKind::Unsupported
        );
    }
}
