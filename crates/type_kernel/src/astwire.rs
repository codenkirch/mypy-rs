#![allow(dead_code)]

//! AST node wire format: Rust-side reader for the generic node serializer
//! (mirrors `mypy/astwire.py`). Provides a tree structure with variant tag
//! and child fields, sufficient for traversal-only visitors (the traverser
//! seekers/collectors that match on node kind and walk children).
//!
//! Wire format (see astwire.py):
//!   LITERAL_NONE (2)          → None node
//!   tag:u8 count:varint child* END_TAG
//!     child = LITERAL_NONE | node | LIST_GEN size item*
//!
//! This module does NOT parse scalar fields (line, column, name, kind) — the
//! Python serializer skips them. Only structure + node kind survives the
//! round-trip, which is what the seekers need.

use crate::wire::{read_int_bare, read_tag, ReadBuffer, WireError, WriteBuffer};

// Shared wire tags (mirror mypy.cache).
const LITERAL_NONE: u8 = 2;
const LIST_GEN: u8 = 20;
const END_TAG: u8 = 255;

// ---------------------------------------------------------------------------
// Node tags (mirror mypy/nodes.py Final[Tag] constants)
// ---------------------------------------------------------------------------

pub(crate) const MYPY_FILE: u8 = 50;
pub(crate) const OVERLOADED_FUNC_DEF: u8 = 51;
pub(crate) const FUNC_DEF: u8 = 52;
pub(crate) const DECORATOR: u8 = 53;
pub(crate) const VAR: u8 = 54;
pub(crate) const CLASS_DEF: u8 = 60;
pub(crate) const EXPR_STMT: u8 = 160;
pub(crate) const CALL_EXPR: u8 = 161;
pub(crate) const NAME_EXPR: u8 = 162;
pub(crate) const STR_EXPR: u8 = 163;
pub(crate) const IMPORT: u8 = 164;
pub(crate) const MEMBER_EXPR: u8 = 165;
pub(crate) const OP_EXPR: u8 = 166;
pub(crate) const INT_EXPR: u8 = 167;
pub(crate) const IF_STMT: u8 = 168;
pub(crate) const ASSIGNMENT_STMT: u8 = 169;
pub(crate) const TUPLE_EXPR: u8 = 170;
pub(crate) const BLOCK: u8 = 171;
pub(crate) const INDEX_EXPR: u8 = 172;
pub(crate) const LIST_EXPR: u8 = 173;
pub(crate) const SET_EXPR: u8 = 174;
pub(crate) const RETURN_STMT: u8 = 175;
pub(crate) const WHILE_STMT: u8 = 176;
pub(crate) const COMPARISON_EXPR: u8 = 177;
pub(crate) const FUNC_DEF_STMT: u8 = 179;
pub(crate) const PASS_STMT: u8 = 180;
pub(crate) const FLOAT_EXPR: u8 = 181;
pub(crate) const UNARY_EXPR: u8 = 182;
pub(crate) const DICT_EXPR: u8 = 183;
pub(crate) const COMPLEX_EXPR: u8 = 184;
pub(crate) const SLICE_EXPR: u8 = 185;
pub(crate) const TEMP_NODE: u8 = 186;
pub(crate) const RAISE_STMT: u8 = 187;
pub(crate) const BREAK_STMT: u8 = 188;
pub(crate) const CONTINUE_STMT: u8 = 189;
pub(crate) const GENERATOR_EXPR: u8 = 190;
pub(crate) const YIELD_EXPR: u8 = 191;
pub(crate) const YIELD_FROM_EXPR: u8 = 192;
pub(crate) const LIST_COMPREHENSION: u8 = 193;
pub(crate) const SET_COMPREHENSION: u8 = 194;
pub(crate) const DICT_COMPREHENSION: u8 = 195;
pub(crate) const IMPORT_FROM: u8 = 196;
pub(crate) const ASSERT_STMT: u8 = 197;
pub(crate) const FOR_STMT: u8 = 198;
pub(crate) const WITH_STMT: u8 = 199;
pub(crate) const OPERATOR_ASSIGNMENT_STMT: u8 = 200;
pub(crate) const TRY_STMT: u8 = 201;
pub(crate) const ELLIPSIS_EXPR: u8 = 202;
pub(crate) const CONDITIONAL_EXPR: u8 = 203;
pub(crate) const DEL_STMT: u8 = 204;
pub(crate) const LAMBDA_EXPR: u8 = 207;
pub(crate) const ASSIGNMENT_EXPR: u8 = 208;
pub(crate) const STAR_EXPR: u8 = 209;
pub(crate) const BYTES_EXPR: u8 = 210;
pub(crate) const GLOBAL_DECL: u8 = 211;
pub(crate) const NONLOCAL_DECL: u8 = 212;
pub(crate) const AWAIT_EXPR: u8 = 213;
pub(crate) const IMPORT_ALL: u8 = 215;
pub(crate) const MATCH_STMT: u8 = 216;
pub(crate) const AS_PATTERN: u8 = 217;
pub(crate) const OR_PATTERN: u8 = 218;
pub(crate) const VALUE_PATTERN: u8 = 219;
pub(crate) const SINGLETON_PATTERN: u8 = 220;
pub(crate) const SEQUENCE_PATTERN: u8 = 221;
pub(crate) const STARRED_PATTERN: u8 = 222;
pub(crate) const MAPPING_PATTERN: u8 = 223;
pub(crate) const CLASS_PATTERN: u8 = 224;
pub(crate) const TYPE_ALIAS_STMT: u8 = 225;

// ---------------------------------------------------------------------------
// Node tree representation
// ---------------------------------------------------------------------------

/// A child field: either None, a single node, or a list of nodes.
#[derive(Debug, Clone)]
pub(crate) enum ChildField {
    None,
    Node(AstNode),
    List(Vec<AstNode>),
}

/// An AST node: variant tag + child fields. Structure only (no scalar data).
#[derive(Debug, Clone)]
pub(crate) struct AstNode {
    pub tag: u8,
    pub children: Vec<ChildField>,
}

impl AstNode {
    /// Yield all direct child nodes (flattening list fields).
    pub fn child_nodes(&self) -> Vec<&AstNode> {
        let mut out = Vec::new();
        for field in &self.children {
            match field {
                ChildField::Node(n) => out.push(n),
                ChildField::List(items) => out.extend(items.iter()),
                ChildField::None => {}
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Reader (mirrors astwire.py serialize_node)
// ---------------------------------------------------------------------------

/// Read a node from the buffer. Returns `None` if the next tag is
/// `LITERAL_NONE` (the Python serializer writes None nodes as LITERAL_NONE).
pub(crate) fn read_node(buf: &mut ReadBuffer<'_>) -> Result<Option<AstNode>, WireError> {
    let tag = read_tag(buf)?;
    if tag == LITERAL_NONE {
        return Ok(None);
    }
    let count = read_int_bare(buf)? as usize;
    let mut children = Vec::with_capacity(count);
    for _ in 0..count {
        let field = read_child_field(buf)?;
        children.push(field);
    }
    let end = read_tag(buf)?;
    if end != END_TAG {
        return Err(WireError::invalid(format!("expected END_TAG, got {end}")));
    }
    Ok(Some(AstNode { tag, children }))
}

fn read_child_field(buf: &mut ReadBuffer<'_>) -> Result<ChildField, WireError> {
    // Peek: if next is a node tag, read it as Node. If LIST_GEN, read list.
    // If LITERAL_NONE, it's None. The serializer writes LITERAL_NONE for
    // None fields, LIST_GEN for lists, or a node tag for single nodes.
    let tag = read_tag(buf)?;
    if tag == LITERAL_NONE {
        return Ok(ChildField::None);
    }
    if tag == LIST_GEN {
        let size = read_int_bare(buf)? as usize;
        let mut items = Vec::with_capacity(size);
        for _ in 0..size {
            if let Some(node) = read_node(buf)? {
                items.push(node);
            }
        }
        return Ok(ChildField::List(items));
    }
    // It's a node tag — re-read the node with this tag already consumed.
    // We need to read count + children + END_TAG.
    let count = read_int_bare(buf)? as usize;
    let mut children = Vec::with_capacity(count);
    for _ in 0..count {
        let field = read_child_field(buf)?;
        children.push(field);
    }
    let end = read_tag(buf)?;
    if end != END_TAG {
        return Err(WireError::invalid(format!("expected END_TAG, got {end}")));
    }
    Ok(ChildField::Node(AstNode { tag, children }))
}

// ---------------------------------------------------------------------------
// Writer (for round-trip tests)
// ---------------------------------------------------------------------------

pub(crate) fn write_node(buf: &mut WriteBuffer, node: &AstNode) {
    buf.push(node.tag);
    crate::wire::write_int_bare(buf, node.children.len() as i64).expect("write child count");
    for field in &node.children {
        write_child_field(buf, field);
    }
    buf.push(END_TAG);
}

fn write_child_field(buf: &mut WriteBuffer, field: &ChildField) {
    match field {
        ChildField::None => buf.push(LITERAL_NONE),
        ChildField::Node(n) => write_node(buf, n),
        ChildField::List(items) => {
            buf.push(LIST_GEN);
            crate::wire::write_int_bare(buf, items.len() as i64).expect("write list size");
            for item in items {
                write_node(buf, item);
            }
        }
    }
}

/// Decode a node from bytes (PyO3 entry point helper).
pub(crate) fn decode_node(bytes: &[u8]) -> Option<AstNode> {
    let mut buf = ReadBuffer::new(bytes);
    read_node(&mut buf).ok().flatten()
}

// ---------------------------------------------------------------------------
// Node kind predicates (for seekers)
// ---------------------------------------------------------------------------

/// Check if a node is a ReturnStmt.
pub(crate) fn is_return_stmt(tag: u8) -> bool {
    tag == RETURN_STMT
}

/// Check if a node is a YieldExpr.
pub(crate) fn is_yield_expr(tag: u8) -> bool {
    tag == YIELD_EXPR
}

/// Check if a node is a YieldFromExpr.
pub(crate) fn is_yield_from_expr(tag: u8) -> bool {
    tag == YIELD_FROM_EXPR
}

/// Check if a node is an AwaitExpr.
pub(crate) fn is_await_expr(tag: u8) -> bool {
    tag == AWAIT_EXPR
}

/// Check if a node is a StrExpr.
pub(crate) fn is_str_expr(tag: u8) -> bool {
    tag == STR_EXPR
}

/// Check if a node is a NameExpr.
pub(crate) fn is_name_expr(tag: u8) -> bool {
    tag == NAME_EXPR
}

/// Check if a node is a MemberExpr.
pub(crate) fn is_member_expr(tag: u8) -> bool {
    tag == MEMBER_EXPR
}

/// Check if a node is a FuncDef (the FUNC_DEF tag, used by both FuncDef
/// and the FUNC_DEF_STMT alias).
pub(crate) fn is_func_def(tag: u8) -> bool {
    tag == FUNC_DEF || tag == FUNC_DEF_STMT || tag == OVERLOADED_FUNC_DEF
}

/// Check if a node is an AssignmentStmt.
pub(crate) fn is_assignment_stmt(tag: u8) -> bool {
    tag == ASSIGNMENT_STMT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_return(expr: Option<AstNode>) -> AstNode {
        let children = match expr {
            Some(n) => vec![ChildField::Node(n)],
            None => vec![ChildField::None],
        };
        AstNode {
            tag: RETURN_STMT,
            children,
        }
    }

    fn make_int() -> AstNode {
        AstNode {
            tag: INT_EXPR,
            children: vec![],
        }
    }

    fn make_yield() -> AstNode {
        AstNode {
            tag: YIELD_EXPR,
            children: vec![ChildField::None],
        }
    }

    fn make_block(stmts: Vec<AstNode>) -> AstNode {
        AstNode {
            tag: BLOCK,
            children: vec![ChildField::List(stmts)],
        }
    }

    fn round_trip(node: &AstNode) -> AstNode {
        let mut buf = WriteBuffer::new();
        write_node(&mut buf, node);
        let bytes = buf.into_bytes();
        decode_node(&bytes).expect("decode failed")
    }

    #[test]
    fn test_round_trip_return_with_expr() {
        let node = make_return(Some(make_int()));
        let result = round_trip(&node);
        assert_eq!(result.tag, RETURN_STMT);
        assert_eq!(result.children.len(), 1);
        match &result.children[0] {
            ChildField::Node(n) => assert_eq!(n.tag, INT_EXPR),
            _ => panic!("expected Node child"),
        }
    }

    #[test]
    fn test_round_trip_return_none() {
        let node = make_return(None);
        let result = round_trip(&node);
        assert_eq!(result.tag, RETURN_STMT);
        assert!(matches!(result.children[0], ChildField::None));
    }

    #[test]
    fn test_round_trip_block_with_list() {
        let block = make_block(vec![make_return(Some(make_int())), make_yield()]);
        let result = round_trip(&block);
        assert_eq!(result.tag, BLOCK);
        match &result.children[0] {
            ChildField::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].tag, RETURN_STMT);
                assert_eq!(items[1].tag, YIELD_EXPR);
            }
            _ => panic!("expected List child"),
        }
    }

    #[test]
    fn test_child_nodes_flattens() {
        let block = make_block(vec![make_return(None), make_yield()]);
        let children = block.child_nodes();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_predicates() {
        assert!(is_return_stmt(RETURN_STMT));
        assert!(!is_return_stmt(YIELD_EXPR));
        assert!(is_yield_expr(YIELD_EXPR));
        assert!(is_yield_from_expr(YIELD_FROM_EXPR));
        assert!(is_await_expr(AWAIT_EXPR));
        assert!(is_str_expr(STR_EXPR));
        assert!(is_name_expr(NAME_EXPR));
        assert!(is_member_expr(MEMBER_EXPR));
        assert!(is_func_def(FUNC_DEF));
        assert!(is_assignment_stmt(ASSIGNMENT_STMT));
    }

    #[test]
    fn test_empty_node() {
        let node = AstNode {
            tag: INT_EXPR,
            children: vec![],
        };
        let result = round_trip(&node);
        assert_eq!(result.tag, INT_EXPR);
        assert!(result.children.is_empty());
        assert!(result.child_nodes().is_empty());
    }
}
