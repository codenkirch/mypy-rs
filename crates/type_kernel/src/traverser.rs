#![allow(non_local_definitions)]

//! Native port of `mypy/traverser.py` seeker functions (Stage 14, Issue #98).
//!
//! Ports the pure traversal helpers that walk an AST subtree and match on
//! node kind:
//! - `has_return_statement` — find a non-trivial return (with expr).
//! - `has_str_expression` — find any StrExpr node.
//! - `has_yield_expression` — find YieldExpr (not recursing into nested
//!   functions, matching `FuncCollectorBase` semantics).
//! - `has_yield_from_expression` — find YieldFromExpr (same constraint).
//! - `has_await_expression` — find AwaitExpr.
//! - `all_return_statements` — collect all ReturnStmt nodes.
//! - `all_yield_expressions` — collect (YieldExpr, in_assignment) pairs.
//! - `all_name_and_member_expressions` — collect NameExpr + MemberExpr.
//!
//! The AST is serialized by `mypy/astwire.py` (generic `__slots__`
//! introspection) into the binary wire format read by `astwire.rs`. Only
//! node kind + tree structure survives the round-trip — scalar fields
//! (names, line numbers) are not serialized. The seekers that need scalar
//! data (NameExpr.name) return the node's wire bytes so Python can
//! deserialize the original node.
//!
//! `FuncCollectorBase` semantics: yield/return seekers do NOT recurse into
//! nested function definitions. The `inside_func` flag tracks this.

use pyo3::prelude::*;

use crate::astwire::{
    decode_node, is_assignment_stmt, is_await_expr, is_func_def, is_member_expr, is_name_expr,
    is_return_stmt, is_str_expr, is_yield_expr, is_yield_from_expr, AstNode, ChildField,
};

// ---------------------------------------------------------------------------
// has_return_statement: non-trivial return (expr is not None)
// ---------------------------------------------------------------------------

/// `mypy.traverser.has_return_statement` — find if a function has a
/// non-trivial return statement.
///
/// Mirrors `ReturnSeeker` (traverser.py:946-963). "Non-trivial" means the
/// return has an expression (plain `return` and `return None` don't count).
/// Since the wire format drops scalar values, we can't distinguish
/// `return None` from `return <expr>` — but we CAN distinguish `return`
/// (expr field is None/ChildField::None) from `return <expr>` (expr field
/// is a Node). This matches the Python seeker which checks
/// `stmt.expr is not None`.
#[pyfunction]
pub(crate) fn rust_has_return_statement(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(has_return_statement_inner(&node))
}

fn has_return_statement_inner(node: &AstNode) -> bool {
    if is_return_stmt(node.tag) {
        // Non-trivial if the expr child is a Node (not None).
        if node
            .children
            .first()
            .is_some_and(|f| matches!(f, ChildField::Node(_)))
        {
            return true;
        }
    }
    node.child_nodes()
        .iter()
        .any(|c| has_return_statement_inner(c))
}

// ---------------------------------------------------------------------------
// has_str_expression
// ---------------------------------------------------------------------------

/// `mypy.traverser.has_str_expression` — check if the subtree contains a
/// StrExpr.
#[pyfunction]
pub(crate) fn rust_has_str_expression(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(has_str_expression_inner(&node))
}

fn has_str_expression_inner(node: &AstNode) -> bool {
    if is_str_expr(node.tag) {
        return true;
    }
    node.child_nodes()
        .iter()
        .any(|c| has_str_expression_inner(c))
}

// ---------------------------------------------------------------------------
// has_yield_expression (FuncCollectorBase: no nested func recursion)
// ---------------------------------------------------------------------------

/// `mypy.traverser.has_yield_expression` — find YieldExpr without recursing
/// into nested function definitions.
#[pyfunction]
pub(crate) fn rust_has_yield_expression(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(yield_seek(&node, false))
}

/// Traverse, skipping nested function defs. `inside_func` starts false;
/// the top-level FuncDef sets it true (but still recurses into its body).
fn yield_seek(node: &AstNode, inside_func: bool) -> bool {
    if is_yield_expr(node.tag) {
        return true;
    }
    // Don't recurse into nested function definitions.
    if inside_func && is_func_def(node.tag) {
        return false;
    }
    node.child_nodes()
        .iter()
        .any(|c| yield_seek(c, inside_func || is_func_def(node.tag)))
}

// ---------------------------------------------------------------------------
// has_yield_from_expression (FuncCollectorBase)
// ---------------------------------------------------------------------------

/// `mypy.traverser.has_yield_from_expression` — find YieldFromExpr without
/// recursing into nested function definitions.
#[pyfunction]
pub(crate) fn rust_has_yield_from_expression(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(yield_from_seek(&node, false))
}

fn yield_from_seek(node: &AstNode, inside_func: bool) -> bool {
    if is_yield_from_expr(node.tag) {
        return true;
    }
    if inside_func && is_func_def(node.tag) {
        return false;
    }
    node.child_nodes()
        .iter()
        .any(|c| yield_from_seek(c, inside_func || is_func_def(node.tag)))
}

// ---------------------------------------------------------------------------
// has_await_expression
// ---------------------------------------------------------------------------

/// `mypy.traverser.has_await_expression` — find AwaitExpr in the subtree.
#[pyfunction]
pub(crate) fn rust_has_await_expression(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(has_await_inner(&node))
}

fn has_await_inner(node: &AstNode) -> bool {
    if is_await_expr(node.tag) {
        return true;
    }
    node.child_nodes().iter().any(|c| has_await_inner(c))
}

// ---------------------------------------------------------------------------
// all_return_statements (FuncCollectorBase)
// ---------------------------------------------------------------------------

/// `mypy.traverser.all_return_statements` — collect all ReturnStmt nodes.
/// Returns the count (the nodes themselves are Python objects; we can only
/// count them since the wire format doesn't carry identity).
#[pyfunction]
pub(crate) fn rust_count_return_statements(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    collect_returns(&node, false, &mut count);
    Ok(count)
}

fn collect_returns(node: &AstNode, inside_func: bool, count: &mut i64) {
    if is_return_stmt(node.tag) {
        *count += 1;
    }
    if inside_func && is_func_def(node.tag) {
        return;
    }
    for child in node.child_nodes() {
        collect_returns(child, inside_func || is_func_def(node.tag), count);
    }
}

// ---------------------------------------------------------------------------
// all_yield_expressions (FuncCollectorBase + AssignmentStmt tracking)
// ---------------------------------------------------------------------------

/// `mypy.traverser.all_yield_expressions` — collect (YieldExpr,
/// in_assignment) pairs. Returns the count of yield expressions found.
#[pyfunction]
pub(crate) fn rust_count_yield_expressions(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    collect_yields(&node, false, false, &mut count);
    Ok(count)
}

fn collect_yields(node: &AstNode, inside_func: bool, in_assignment: bool, count: &mut i64) {
    if is_yield_expr(node.tag) {
        *count += 1;
    }
    if inside_func && is_func_def(node.tag) {
        return;
    }
    let new_in_assignment = in_assignment || is_assignment_stmt(node.tag);
    for child in node.child_nodes() {
        collect_yields(
            child,
            inside_func || is_func_def(node.tag),
            new_in_assignment,
            count,
        );
    }
}

// ---------------------------------------------------------------------------
// all_name_and_member_expressions
// ---------------------------------------------------------------------------

/// `mypy.traverser.all_name_and_member_expressions` — count NameExpr and
/// MemberExpr nodes. Returns (name_count, member_count).
#[pyfunction]
pub(crate) fn rust_count_name_and_member_expressions(node_bytes: &[u8]) -> PyResult<(i64, i64)> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok((0, 0)),
    };
    let mut names = 0i64;
    let mut members = 0i64;
    collect_name_member(&node, &mut names, &mut members);
    Ok((names, members))
}

fn collect_name_member(node: &AstNode, names: &mut i64, members: &mut i64) {
    if is_name_expr(node.tag) {
        *names += 1;
    }
    if is_member_expr(node.tag) {
        *members += 1;
    }
    for child in node.child_nodes() {
        collect_name_member(child, names, members);
    }
}

// ---------------------------------------------------------------------------
// Traversal helper: visit all nodes (for future extenders)
// ---------------------------------------------------------------------------

/// Visit all nodes in pre-order. The callback receives each node.
#[allow(dead_code)]
fn visit_all<F: FnMut(&AstNode)>(node: &AstNode, f: &mut F) {
    f(node);
    for child in node.child_nodes() {
        visit_all(child, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astwire::{AstNode, ChildField, RETURN_STMT, YIELD_EXPR};
    use crate::astwire::{BLOCK, INT_EXPR, MEMBER_EXPR, NAME_EXPR, STR_EXPR};

    fn make_int() -> AstNode {
        AstNode {
            tag: INT_EXPR,
            children: vec![],
        }
    }

    fn make_str() -> AstNode {
        AstNode {
            tag: STR_EXPR,
            children: vec![],
        }
    }

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

    fn make_yield() -> AstNode {
        AstNode {
            tag: YIELD_EXPR,
            children: vec![ChildField::None],
        }
    }

    fn make_name() -> AstNode {
        AstNode {
            tag: NAME_EXPR,
            children: vec![],
        }
    }

    fn make_member() -> AstNode {
        AstNode {
            tag: MEMBER_EXPR,
            children: vec![],
        }
    }

    fn make_block(stmts: Vec<AstNode>) -> AstNode {
        AstNode {
            tag: BLOCK,
            children: vec![ChildField::List(stmts)],
        }
    }

    #[test]
    fn test_has_return_statement_with_expr() {
        let block = make_block(vec![make_return(Some(make_int()))]);
        assert!(has_return_statement_inner(&block));
    }

    #[test]
    fn test_has_return_statement_bare_return() {
        let block = make_block(vec![make_return(None)]);
        assert!(!has_return_statement_inner(&block));
    }

    #[test]
    fn test_has_return_statement_nested() {
        let inner = make_block(vec![make_return(Some(make_int()))]);
        let outer = make_block(vec![inner]);
        assert!(has_return_statement_inner(&outer));
    }

    #[test]
    fn test_has_return_statement_none() {
        let block = make_block(vec![make_int()]);
        assert!(!has_return_statement_inner(&block));
    }

    #[test]
    fn test_has_str_expression_true() {
        let block = make_block(vec![make_str()]);
        assert!(has_str_expression_inner(&block));
    }

    #[test]
    fn test_has_str_expression_false() {
        let block = make_block(vec![make_int()]);
        assert!(!has_str_expression_inner(&block));
    }

    #[test]
    fn test_has_yield_expression() {
        let block = make_block(vec![make_yield()]);
        assert!(yield_seek(&block, false));
    }

    #[test]
    fn test_has_yield_expression_none() {
        let block = make_block(vec![make_int()]);
        assert!(!yield_seek(&block, false));
    }

    #[test]
    fn test_count_returns() {
        let block = make_block(vec![
            make_return(Some(make_int())),
            make_return(None),
            make_int(),
        ]);
        let mut count = 0i64;
        collect_returns(&block, false, &mut count);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_yields() {
        let block = make_block(vec![make_yield(), make_int(), make_yield()]);
        let mut count = 0i64;
        collect_yields(&block, false, false, &mut count);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_name_and_member() {
        let block = make_block(vec![make_name(), make_member(), make_int(), make_name()]);
        let mut names = 0i64;
        let mut members = 0i64;
        collect_name_member(&block, &mut names, &mut members);
        assert_eq!(names, 2);
        assert_eq!(members, 1);
    }
}
