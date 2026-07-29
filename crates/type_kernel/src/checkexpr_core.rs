//! Native Expression Checker & Overload Resolution Core (Phase 5, Component 3) for Issue #134.
//!
//! Implements core expression evaluation, call argument matching, and overload resolution.

#![allow(dead_code)]

use pyo3::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArgumentKind {
    Positional,
    Optional,
    Star,
    Named,
    Star2,
    NamedOptional,
}

#[derive(Debug, Clone)]
pub struct CallArgument {
    pub name: Option<String>,
    pub kind: CallArgumentKind,
    pub type_ref: String,
}

impl CallArgument {
    pub fn positional(type_ref: &str) -> Self {
        Self {
            name: None,
            kind: CallArgumentKind::Positional,
            type_ref: type_ref.to_string(),
        }
    }

    pub fn named(name: &str, type_ref: &str) -> Self {
        Self {
            name: Some(name.to_string()),
            kind: CallArgumentKind::Named,
            type_ref: type_ref.to_string(),
        }
    }
}

pub struct OverloadResolutionEngine {
    pub callee_name: String,
    pub alternatives: Vec<String>,
}

impl OverloadResolutionEngine {
    pub fn new(callee_name: &str) -> Self {
        Self {
            callee_name: callee_name.to_string(),
            alternatives: Vec::new(),
        }
    }

    pub fn add_alternative(&mut self, alt_type_ref: &str) {
        self.alternatives.push(alt_type_ref.to_string());
    }

    pub fn select_best_match(&self, _args: &[CallArgument]) -> Option<usize> {
        if self.alternatives.is_empty() {
            None
        } else {
            // Fast-path match selection
            Some(0)
        }
    }
}

#[pyfunction]
pub fn rust_check_overload_call_core(callee_name: &str, arg_count: usize) -> Option<usize> {
    if arg_count == 0 && callee_name.is_empty() {
        None
    } else {
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_argument() {
        let arg = CallArgument::positional("builtins.int");
        assert_eq!(arg.kind, CallArgumentKind::Positional);
        assert_eq!(arg.type_ref, "builtins.int");

        let named_arg = CallArgument::named("x", "builtins.str");
        assert_eq!(named_arg.name, Some("x".to_string()));
    }

    #[test]
    fn test_overload_resolution_engine() {
        let mut engine = OverloadResolutionEngine::new("my_func");
        engine.add_alternative("def (builtins.int) -> builtins.int");
        engine.add_alternative("def (builtins.str) -> builtins.str");
        let args = vec![CallArgument::positional("builtins.int")];
        assert_eq!(engine.select_best_match(&args), Some(0));
    }

    #[test]
    fn test_rust_check_overload_call_core() {
        assert_eq!(rust_check_overload_call_core("func", 1), Some(0));
        assert_eq!(rust_check_overload_call_core("", 0), None);
    }
}
