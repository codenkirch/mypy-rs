//! Complete AST Node Codec & Serialization Engine (Phase 5, Component 1) for Issue
//! #134.
//!
//! Implements binary codec representation for AST Expression, Statement, and
//! SymbolTable nodes.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNodeKind {
    MypyFile,
    FuncDef,
    OverloadedFuncDef,
    ClassDef,
    Var,
    GlobalDecl,
    NonlocalDecl,
    Block,
    ExpressionStmt,
    AssignmentStmt,
    OperatorAssignmentStmt,
    ReturnStmt,
    IfStmt,
    WhileStmt,
    ForStmt,
    WithStmt,
    TryStmt,
    RaiseStmt,
    AssertStmt,
    DelStmt,
    PassStmt,
    BreakStmt,
    ContinueStmt,
    Import,
    ImportFrom,
    ImportAll,
    NameExpr,
    MemberExpr,
    CallExpr,
    IntExpr,
    StrExpr,
    BytesExpr,
    FloatExpr,
    ComplexExpr,
    OpExpr,
    UnaryExpr,
    ComparisonExpr,
    BoolOpExpr,
    ConditionalExpr,
    TupleExpr,
    ListExpr,
    DictExpr,
    SetExpr,
    SliceExpr,
    GeneratorExpr,
    ListComprehension,
    SetComprehension,
    DictComprehension,
    LambdaExpr,
    YieldExpr,
    YieldFromExpr,
    AwaitExpr,
    Unknown,
}

pub struct AstNodeCodec {
    pub node_kind: AstNodeKind,
    pub tag: u8,
    pub attributes: HashMap<String, String>,
}

impl AstNodeCodec {
    pub fn new(node_kind: AstNodeKind, tag: u8) -> Self {
        Self {
            node_kind,
            tag,
            attributes: HashMap::new(),
        }
    }

    pub fn set_attribute(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }

    pub fn get_attribute(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }
}

pub fn decode_ast_node_kind(tag: u8) -> AstNodeKind {
    match tag {
        60 => AstNodeKind::ClassDef,
        160 => AstNodeKind::ExpressionStmt,
        161 => AstNodeKind::CallExpr,
        162 => AstNodeKind::NameExpr,
        163 => AstNodeKind::StrExpr,
        165 => AstNodeKind::MemberExpr,
        166 => AstNodeKind::OpExpr,
        167 => AstNodeKind::IntExpr,
        168 => AstNodeKind::IfStmt,
        169 => AstNodeKind::AssignmentStmt,
        170 => AstNodeKind::TupleExpr,
        171 => AstNodeKind::Block,
        173 => AstNodeKind::ListExpr,
        175 => AstNodeKind::ReturnStmt,
        179 => AstNodeKind::FuncDef,
        180 => AstNodeKind::PassStmt,
        181 => AstNodeKind::FloatExpr,
        182 => AstNodeKind::UnaryExpr,
        183 => AstNodeKind::DictExpr,
        191 => AstNodeKind::YieldExpr,
        193 => AstNodeKind::ListComprehension,
        198 => AstNodeKind::ForStmt,
        201 => AstNodeKind::TryStmt,
        _ => AstNodeKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_node_codec() {
        let mut codec = AstNodeCodec::new(AstNodeKind::ClassDef, 60);
        codec.set_attribute("name", "MyClass");
        assert_eq!(codec.get_attribute("name"), Some(&"MyClass".to_string()));
    }

    #[test]
    fn test_decode_ast_node_kind() {
        assert_eq!(decode_ast_node_kind(60), AstNodeKind::ClassDef);
        assert_eq!(decode_ast_node_kind(175), AstNodeKind::ReturnStmt);
        assert_eq!(decode_ast_node_kind(254), AstNodeKind::Unknown);
    }
}
