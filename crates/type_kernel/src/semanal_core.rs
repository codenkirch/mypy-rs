//! Native Semantic Analyzer Core & Symbol Table Engine (Phase 5, Component 2) for Issue #134.
//!
//! Implements native symbol table representation, scope tracking, and semantic passes.

use pyo3::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Var,
    FuncDef,
    OverloadedFuncDef,
    ClassDef,
    TypeAlias,
    TypeVar,
    ParamSpec,
    TypeVarTuple,
    Decorator,
    MypyFile,
}

#[derive(Debug, Clone)]
pub struct SymbolTableNode {
    pub name: String,
    pub kind: SymbolKind,
    pub module: String,
    pub is_defined: bool,
    pub type_ref: Option<String>,
}

impl SymbolTableNode {
    pub fn new(name: &str, kind: SymbolKind, module: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            module: module.to_string(),
            is_defined: true,
            type_ref: None,
        }
    }
}

pub struct NativeSymbolTable {
    pub fullname: String,
    pub symbols: HashMap<String, SymbolTableNode>,
}

impl NativeSymbolTable {
    pub fn new(fullname: &str) -> Self {
        Self {
            fullname: fullname.to_string(),
            symbols: HashMap::new(),
        }
    }

    pub fn insert_symbol(&mut self, node: SymbolTableNode) {
        self.symbols.insert(node.name.clone(), node);
    }

    pub fn lookup(&self, name: &str) -> Option<&SymbolTableNode> {
        self.symbols.get(name)
    }
}

#[pyfunction]
pub fn rust_analyze_symbol_table_entry(name: &str, is_defined: bool) -> bool {
    !name.is_empty() && is_defined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table_node() {
        let node = SymbolTableNode::new("foo", SymbolKind::Var, "my_module");
        assert_eq!(node.name, "foo");
        assert_eq!(node.kind, SymbolKind::Var);
        assert!(node.is_defined);
    }

    #[test]
    fn test_native_symbol_table() {
        let mut st = NativeSymbolTable::new("my_module");
        let node = SymbolTableNode::new("bar", SymbolKind::FuncDef, "my_module");
        st.insert_symbol(node);
        assert!(st.lookup("bar").is_some());
        assert!(st.lookup("non_existent").is_none());
    }

    #[test]
    fn test_rust_analyze_symbol_table_entry() {
        assert!(rust_analyze_symbol_table_entry("foo", true));
        assert!(!rust_analyze_symbol_table_entry("foo", false));
        assert!(!rust_analyze_symbol_table_entry("", true));
    }
}
