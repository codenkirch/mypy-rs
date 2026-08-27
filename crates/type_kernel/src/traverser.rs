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
    decode_node, is_assignment_stmt, is_await_expr, is_class_def, is_func_def, is_global_decl,
    is_literal_expr, is_member_expr, is_name_expr, is_return_stmt, is_slice_expr, is_str_expr,
    is_try_stmt, is_yield_expr, is_yield_from_expr, AstNode, ChildField,
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
///
/// Returns `None` (defer) when the root does not decode: the Python
/// serializer emits a bare LITERAL_NONE for a node kind it has no wire
/// tag for (e.g. a bare `FuncItem`, whose `body` therefore never reaches
/// us), and answering `false` there would diverge from the pure-Python
/// seeker. The Python shim falls back to `ReturnSeeker` on `None`.
#[pyfunction]
pub(crate) fn rust_has_return_statement(node_bytes: &[u8]) -> PyResult<Option<bool>> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(None),
    };
    Ok(Some(has_return_statement_inner(&node)))
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

/// `mypy.checkexpr.visit_generator_expr` — combined has-await for a
/// GeneratorExpr: walks left_expr, sequences[1:], and all condlists.
/// Mirrors the pure-Python check at checkexpr.py:7417-7422 exactly:
/// the first sequence and the indices are NOT checked.
#[pyfunction]
pub(crate) fn rust_has_await_in_generator(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    // GeneratorExpr slots order: left_expr, sequences, condlists,
    // is_async, indices. is_async is list[bool] and drops to a None
    // child field, so positions survive for the rest.
    let children = &node.children;
    if children.len() < 3 {
        return Ok(false);
    }
    let any_has_await = |n: &AstNode| has_await_inner(n);
    // left_expr (field 0).
    if let ChildField::Node(left) = &children[0] {
        if any_has_await(left) {
            return Ok(true);
        }
    }
    // sequences[1:] (field 1, flat list).
    if let ChildField::List(seqs) = &children[1] {
        for seq in seqs.iter().skip(1) {
            if any_has_await(seq) {
                return Ok(true);
            }
        }
    }
    // condlists (field 2, nested list).
    if let ChildField::NestedList(rows) = &children[2] {
        for row in rows {
            for cond in row {
                if any_has_await(cond) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
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
// all_yield_from_expressions (FuncCollectorBase + AssignmentStmt tracking)
// ---------------------------------------------------------------------------

/// `mypy.traverser.all_yield_from_expressions` — collect (YieldFromExpr,
/// in_assignment) pairs. Returns the count of yield-from expressions.
#[pyfunction]
pub(crate) fn rust_count_yield_from_expressions(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    collect_yield_from(&node, false, false, &mut count);
    Ok(count)
}

fn collect_yield_from(node: &AstNode, inside_func: bool, in_assignment: bool, count: &mut i64) {
    if is_yield_from_expr(node.tag) {
        *count += 1;
    }
    if inside_func && is_func_def(node.tag) {
        return;
    }
    let new_in_assignment = in_assignment || is_assignment_stmt(node.tag);
    for child in node.child_nodes() {
        collect_yield_from(
            child,
            inside_func || is_func_def(node.tag),
            new_in_assignment,
            count,
        );
    }
}

// ---------------------------------------------------------------------------
// all_return_statements_and_flags (FuncCollectorBase + finally tracking)
// ---------------------------------------------------------------------------

/// `mypy.traverser.all_return_statements_and_flags` — like
/// `all_return_statements` but also tracks whether each return is inside
/// a `finally` block. Returns (total_count, in_finally_count).
#[pyfunction]
pub(crate) fn rust_count_return_statements_and_flags(node_bytes: &[u8]) -> PyResult<(i64, i64)> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok((0, 0)),
    };
    let mut total = 0i64;
    let mut in_finally = 0i64;
    collect_returns_and_flags(&node, false, false, &mut total, &mut in_finally);
    Ok((total, in_finally))
}

/// TryStmt slots: body, types, vars, handlers, else_body, finally_body.
/// `finally_body` is the 6th slot (index 5).
const TRY_FINALLY_BODY_INDEX: usize = 5;

fn collect_returns_and_flags(
    node: &AstNode,
    inside_func: bool,
    in_finally: bool,
    total: &mut i64,
    in_finally_count: &mut i64,
) {
    if is_return_stmt(node.tag) {
        *total += 1;
        if in_finally {
            *in_finally_count += 1;
        }
    }
    if inside_func && is_func_def(node.tag) {
        return;
    }
    // For TryStmt, only children of the finally_body (6th field,
    // index 5) are "in finally". Other children (body, handlers,
    // else_body) are not.
    if is_try_stmt(node.tag) {
        for (i, field) in node.children.iter().enumerate() {
            let child_in_finally =
                in_finally || (i == TRY_FINALLY_BODY_INDEX && matches!(field, ChildField::Node(_)));
            match field {
                ChildField::Node(n) => {
                    collect_returns_and_flags(
                        n,
                        inside_func || is_func_def(node.tag),
                        child_in_finally,
                        total,
                        in_finally_count,
                    );
                }
                ChildField::List(items) => {
                    for item in items {
                        collect_returns_and_flags(
                            item,
                            inside_func || is_func_def(node.tag),
                            child_in_finally,
                            total,
                            in_finally_count,
                        );
                    }
                }
                ChildField::NestedList(rows) => {
                    for row in rows {
                        for item in row {
                            collect_returns_and_flags(
                                item,
                                inside_func || is_func_def(node.tag),
                                child_in_finally,
                                total,
                                in_finally_count,
                            );
                        }
                    }
                }
                ChildField::None => {}
            }
        }
        return;
    }
    for child in node.child_nodes() {
        collect_returns_and_flags(
            child,
            inside_func || is_func_def(node.tag),
            in_finally,
            total,
            in_finally_count,
        );
    }
}

// ---------------------------------------------------------------------------
// count_returns (count ALL returns, including nested funcs)
// ---------------------------------------------------------------------------

/// Count all ReturnStmt nodes in the subtree, including those in nested
/// function definitions (no FuncCollectorBase skip).
#[pyfunction]
pub(crate) fn rust_count_all_returns(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    count_all_returns(&node, &mut count);
    Ok(count)
}

fn count_all_returns(node: &AstNode, count: &mut i64) {
    if is_return_stmt(node.tag) {
        *count += 1;
    }
    for child in node.child_nodes() {
        count_all_returns(child, count);
    }
}

// ---------------------------------------------------------------------------
// has_yield_return (return statement whose expr is a YieldExpr)
// ---------------------------------------------------------------------------

/// Check if the subtree contains a `return yield <expr>` — a ReturnStmt
/// whose expression child is a YieldExpr.
#[pyfunction]
pub(crate) fn rust_has_yield_return(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(has_yield_return_inner(&node))
}

fn has_yield_return_inner(node: &AstNode) -> bool {
    if is_return_stmt(node.tag) {
        // ReturnStmt's first child is the expr field.
        if let Some(ChildField::Node(expr)) = node.children.first() {
            if is_yield_expr(expr.tag) {
                return true;
            }
        }
    }
    node.child_nodes().iter().any(|c| has_yield_return_inner(c))
}

// ---------------------------------------------------------------------------
// has_complex_slice (slice with a non-None stride)
// ---------------------------------------------------------------------------

/// Check if the subtree contains a SliceExpr with a stride (step)
/// that is not None. A slice like `::2` or `1:10:2` is complex;
/// `1:10` or `:` is not.
#[pyfunction]
pub(crate) fn rust_has_complex_slice(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(has_complex_slice_inner(&node))
}

/// SliceExpr slots: begin_index, end_index, stride.
/// `stride` is the 3rd slot (index 2).
const SLICE_STRIDE_INDEX: usize = 2;

fn has_complex_slice_inner(node: &AstNode) -> bool {
    if is_slice_expr(node.tag) {
        // Complex if stride (3rd field) is a Node (not None).
        if node
            .children
            .get(SLICE_STRIDE_INDEX)
            .is_some_and(|f| matches!(f, ChildField::Node(_)))
        {
            return true;
        }
    }
    node.child_nodes()
        .iter()
        .any(|c| has_complex_slice_inner(c))
}

// ---------------------------------------------------------------------------
// find_non_extension_handlers (methods without @extension decorator)
// ---------------------------------------------------------------------------

/// Count methods (FuncDef) inside a class that are NOT wrapped in a
/// Decorator node. A Decorator wraps a decorated FuncDef; bare `def`
/// methods appear directly. Since the wire format drops decorator names,
/// we structurally check: a FuncDef directly inside a ClassDef's defs
/// block (not wrapped in Decorator) is a "non-extension handler".
/// Returns the count.
#[pyfunction]
pub(crate) fn rust_count_non_extension_handlers(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    count_non_extension_handlers(&node, &mut count);
    Ok(count)
}

fn count_non_extension_handlers(node: &AstNode, count: &mut i64) {
    if is_class_def(node.tag) {
        // ClassDef slots: name, _fullname, defs, type_args, type_vars,
        // base_type_exprs, removed_base_type_exprs, info, metaclass,
        // decorators, keywords, analyzed, ...

        // `defs` is the 3rd slot (index 2) — a Block.
        if let Some(ChildField::Node(defs)) = node.children.get(2) {
            count_bare_funcs_in_block(defs, count);
        }
        // Don't recurse further — we only want direct methods.
        return;
    }
    for child in node.child_nodes() {
        count_non_extension_handlers(child, count);
    }
}

/// Walk a Block's statement list and count FuncDef nodes that are not
/// wrapped in a Decorator.
fn count_bare_funcs_in_block(block: &AstNode, count: &mut i64) {
    for stmt in block.child_nodes() {
        if is_func_def(stmt.tag) {
            *count += 1;
        }
        // Decorator-wrapped funcs have tag DECORATOR; skip those.
    }
}

// ---------------------------------------------------------------------------
// is_global_expr (subtree contains a GlobalDecl)
// ---------------------------------------------------------------------------

/// Check if the subtree contains a `global` declaration (GlobalDecl).
#[pyfunction]
pub(crate) fn rust_is_global_expr(node_bytes: &[u8]) -> PyResult<bool> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(is_global_expr_inner(&node))
}

fn is_global_expr_inner(node: &AstNode) -> bool {
    if is_global_decl(node.tag) {
        return true;
    }
    node.child_nodes().iter().any(|c| is_global_expr_inner(c))
}

// ---------------------------------------------------------------------------
// find_non_literal_handlers (methods whose body is non-literal)
// ---------------------------------------------------------------------------

/// Count methods (FuncDef) inside a class whose body contains at least
/// one non-literal expression. A "literal handler" is a method whose
/// body consists entirely of literal expressions (IntExpr, StrExpr,
/// FloatExpr, BytesExpr, ComplexExpr, EllipsisExpr). Returns the count
/// of methods that are NOT literal handlers.
#[pyfunction]
pub(crate) fn rust_count_non_literal_handlers(node_bytes: &[u8]) -> PyResult<i64> {
    let node = match decode_node(node_bytes) {
        Some(n) => n,
        None => return Ok(0),
    };
    let mut count = 0i64;
    count_non_literal_handlers(&node, &mut count);
    Ok(count)
}

fn count_non_literal_handlers(node: &AstNode, count: &mut i64) {
    if is_class_def(node.tag) {
        if let Some(ChildField::Node(defs)) = node.children.get(2) {
            for stmt in defs.child_nodes() {
                if is_func_def(stmt.tag) && !is_literal_handler(stmt) {
                    *count += 1;
                }
            }
        }
        return;
    }
    for child in node.child_nodes() {
        count_non_literal_handlers(child, count);
    }
}

/// A FuncDef is a "literal handler" if every statement in its body is
/// a literal expression statement (ExpressionStmt whose expr is literal)
/// or a PassStmt.
fn is_literal_handler(func: &AstNode) -> bool {
    // FuncItem slots include `body` — we need to find the Block.
    // FuncDef inherits from FuncItem, which has slots: arguments,
    // arg_names, arg_kinds, min_args, max_pos, type_args, body, ...

    // `body` is at index 6 in FuncItem's own slots. But FuncBase has
    // 12 slots before FuncItem's. So body is at index 12+6 = 18.
    // That's fragile — instead, scan all child fields for a Block node

    // (the body is the Block child of a FuncDef).
    let body = find_body_block(func);
    match body {
        Some(block) => block_is_literal_only(block),
        None => true,
    }
}

/// Find the body Block of a FuncDef by looking for a Block child.
fn find_body_block(func: &AstNode) -> Option<&AstNode> {
    // The body is typically the last Block child in the FuncDef's
    // children list. Scan from the end.
    for field in func.children.iter().rev() {
        if let ChildField::Node(n) = field {
            if n.tag == crate::astwire::BLOCK {
                return Some(n);
            }
        }
        if let ChildField::List(items) = field {
            for item in items.iter().rev() {
                if item.tag == crate::astwire::BLOCK {
                    return Some(item);
                }
            }
        }
    }
    None
}

/// Check if a Block contains only literal expression statements
/// and PassStmts.
fn block_is_literal_only(block: &AstNode) -> bool {
    for stmt in block.child_nodes() {
        let tag = stmt.tag;
        if tag == crate::astwire::PASS_STMT {
            continue;
        }
        if tag == crate::astwire::EXPR_STMT {
            // ExpressionStmt: its first child is the expression.
            if let Some(ChildField::Node(expr)) = stmt.children.first() {
                if !is_literal_expr(expr.tag) {
                    return false;
                }
            }
        } else if is_literal_expr(tag) {
            // A bare literal expression as a statement.
            continue;
        } else {
            return false;
        }
    }
    true
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
    use crate::astwire::{
        AstNode, ChildField, ASSIGNMENT_STMT, BLOCK, FUNC_DEF, INT_EXPR, MEMBER_EXPR, NAME_EXPR,
        RETURN_STMT, STR_EXPR, YIELD_EXPR, YIELD_FROM_EXPR,
    };

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

    fn make_yield_from() -> AstNode {
        AstNode {
            tag: YIELD_FROM_EXPR,
            children: vec![ChildField::Node(make_int())],
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

    fn make_func_def(body: AstNode) -> AstNode {
        AstNode {
            tag: FUNC_DEF,
            children: vec![ChildField::Node(body)],
        }
    }

    fn make_assignment(rvalue: AstNode) -> AstNode {
        AstNode {
            tag: ASSIGNMENT_STMT,
            children: vec![ChildField::Node(rvalue)],
        }
    }

    #[test]
    fn test_has_return_statement_with_expr() {
        let block = make_block(vec![make_return(Some(make_int()))]);
        assert!(has_return_statement_inner(&block));
    }

    #[test]
    fn test_decode_bare_literal_none_defers() {
        // A bare FuncItem has no wire tag, so the Python serializer emits
        // a bare LITERAL_NONE (tag 2). decode_node must return None so
        // rust_has_return_statement defers instead of answering false (#1030).
        assert!(decode_node(&[2]).is_none());
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
    fn test_has_yield_from_expression() {
        let block = make_block(vec![make_yield_from()]);
        assert!(yield_from_seek(&block, false));
    }

    #[test]
    fn test_has_yield_from_expression_none() {
        let block = make_block(vec![make_int()]);
        assert!(!yield_from_seek(&block, false));
    }

    #[test]
    fn test_has_await_expression() {
        use crate::astwire::AWAIT_EXPR;
        let node = AstNode {
            tag: AWAIT_EXPR,
            children: vec![ChildField::Node(make_int())],
        };
        let block = make_block(vec![node]);
        assert!(has_await_inner(&block));
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
    fn test_count_returns_skips_nested_func() {
        let inner_func = make_func_def(make_block(vec![make_return(Some(make_int()))]));
        let block = make_func_def(make_block(vec![make_return(Some(make_int())), inner_func]));
        let mut count = 0i64;
        collect_returns(&block, false, &mut count);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_yields() {
        let block = make_block(vec![make_yield(), make_int(), make_yield()]);
        let mut count = 0i64;
        collect_yields(&block, false, false, &mut count);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_yields_skips_nested_func() {
        let inner_func = make_func_def(make_block(vec![make_yield()]));
        let block = make_func_def(make_block(vec![make_yield(), inner_func]));
        let mut count = 0i64;
        collect_yields(&block, false, false, &mut count);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_yield_from() {
        let block = make_block(vec![make_yield_from(), make_int(), make_yield_from()]);
        let mut count = 0i64;
        collect_yield_from(&block, false, false, &mut count);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_yield_from_skips_nested_func() {
        let inner_func = make_func_def(make_block(vec![make_yield_from()]));
        let block = make_func_def(make_block(vec![make_yield_from(), inner_func]));
        let mut count = 0i64;
        collect_yield_from(&block, false, false, &mut count);
        assert_eq!(count, 1);
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

    #[test]
    fn test_collect_yields_tracks_assignment() {
        // yield inside an assignment should be counted with in_assignment=true
        let assignment = make_assignment(make_yield());
        let block = make_block(vec![assignment]);
        let mut count = 0i64;
        collect_yields(&block, false, false, &mut count);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_collect_yield_from_tracks_assignment() {
        let assignment = make_assignment(make_yield_from());
        let block = make_block(vec![assignment]);
        let mut count = 0i64;
        collect_yield_from(&block, false, false, &mut count);
        assert_eq!(count, 1);
    }

    // --- new seeker tests (Issue #541) ---

    use crate::astwire::{
        CLASS_DEF, DECORATOR, EXPR_STMT, GLOBAL_DECL, PASS_STMT, SLICE_EXPR, TRY_STMT,
    };

    fn make_slice(
        begin: Option<AstNode>,
        end: Option<AstNode>,
        stride: Option<AstNode>,
    ) -> AstNode {
        AstNode {
            tag: SLICE_EXPR,
            children: vec![
                match begin {
                    Some(n) => ChildField::Node(n),
                    None => ChildField::None,
                },
                match end {
                    Some(n) => ChildField::Node(n),
                    None => ChildField::None,
                },
                match stride {
                    Some(n) => ChildField::Node(n),
                    None => ChildField::None,
                },
            ],
        }
    }

    fn make_try_stmt(body: AstNode, finally_body: Option<AstNode>) -> AstNode {
        // TryStmt slots: body, types, vars, handlers, else_body, finally_body
        let children = vec![
            ChildField::Node(body),
            ChildField::None, // types
            ChildField::None, // vars
            ChildField::None, // handlers
            ChildField::None, // else_body
            match finally_body {
                Some(n) => ChildField::Node(n),
                None => ChildField::None,
            },
        ];
        AstNode {
            tag: TRY_STMT,
            children,
        }
    }

    fn make_expr_stmt(expr: AstNode) -> AstNode {
        AstNode {
            tag: EXPR_STMT,
            children: vec![ChildField::Node(expr)],
        }
    }

    fn make_pass_stmt() -> AstNode {
        AstNode {
            tag: PASS_STMT,
            children: vec![],
        }
    }

    fn make_global_decl() -> AstNode {
        AstNode {
            tag: GLOBAL_DECL,
            children: vec![],
        }
    }

    fn make_decorator(func: AstNode) -> AstNode {
        AstNode {
            tag: DECORATOR,
            children: vec![
                ChildField::Node(func),
                ChildField::None, // decorators
            ],
        }
    }

    fn make_class_def(defs: AstNode) -> AstNode {
        // ClassDef slots: name, _fullname, defs, ...
        AstNode {
            tag: CLASS_DEF,
            children: vec![
                ChildField::None, // name (scalar, not serialized)
                ChildField::None, // _fullname
                ChildField::Node(defs),
            ],
        }
    }

    #[test]
    fn test_count_all_returns_includes_nested() {
        let inner_func = make_func_def(make_block(vec![make_return(Some(make_int()))]));
        let block = make_func_def(make_block(vec![make_return(Some(make_int())), inner_func]));
        let mut count = 0i64;
        count_all_returns(&block, &mut count);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_has_yield_return_true() {
        let ret = make_return(Some(make_yield()));
        let block = make_block(vec![ret]);
        assert!(has_yield_return_inner(&block));
    }

    #[test]
    fn test_has_yield_return_false() {
        let ret = make_return(Some(make_int()));
        let block = make_block(vec![ret]);
        assert!(!has_yield_return_inner(&block));
    }

    #[test]
    fn test_has_complex_slice_with_stride() {
        let slice = make_slice(None, None, Some(make_int()));
        let block = make_block(vec![slice]);
        assert!(has_complex_slice_inner(&block));
    }

    #[test]
    fn test_has_complex_slice_without_stride() {
        let slice = make_slice(Some(make_int()), Some(make_int()), None);
        let block = make_block(vec![slice]);
        assert!(!has_complex_slice_inner(&block));
    }

    #[test]
    fn test_returns_and_flags_no_finally() {
        let block = make_block(vec![make_return(Some(make_int()))]);
        let mut total = 0i64;
        let mut in_finally = 0i64;
        collect_returns_and_flags(&block, false, false, &mut total, &mut in_finally);
        assert_eq!(total, 1);
        assert_eq!(in_finally, 0);
    }

    #[test]
    fn test_returns_and_flags_in_finally() {
        // Return in try body (not in finally) + return in finally body.
        let try_body = make_block(vec![make_return(Some(make_int()))]);
        let finally_block = make_block(vec![make_return(Some(make_int()))]);
        let try_stmt = make_try_stmt(try_body, Some(finally_block));
        let block = make_block(vec![try_stmt]);
        let mut total = 0i64;
        let mut in_finally = 0i64;
        collect_returns_and_flags(&block, false, false, &mut total, &mut in_finally);
        assert_eq!(total, 2);
        assert_eq!(in_finally, 1);
    }

    #[test]
    fn test_is_global_expr_true() {
        let block = make_block(vec![make_global_decl()]);
        assert!(is_global_expr_inner(&block));
    }

    #[test]
    fn test_is_global_expr_false() {
        let block = make_block(vec![make_int()]);
        assert!(!is_global_expr_inner(&block));
    }

    #[test]
    fn test_count_non_extension_handlers() {
        // Class with one bare func and one decorated func.
        let bare_func = make_func_def(make_block(vec![]));
        let decorated = make_decorator(make_func_def(make_block(vec![])));
        let class_block = make_block(vec![bare_func, decorated]);
        let class_def = make_class_def(class_block);
        let mut count = 0i64;
        count_non_extension_handlers(&class_def, &mut count);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_non_literal_handlers_literal_body() {
        // Func with only a literal expr stmt → literal handler.
        let func = make_func_def(make_block(vec![make_expr_stmt(make_int())]));
        let class_block = make_block(vec![func]);
        let class_def = make_class_def(class_block);
        let mut count = 0i64;
        count_non_literal_handlers(&class_def, &mut count);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_non_literal_handlers_non_literal_body() {
        // Func with a non-literal expr stmt (NameExpr) → non-literal.
        let func = make_func_def(make_block(vec![make_expr_stmt(make_name())]));
        let class_block = make_block(vec![func]);
        let class_def = make_class_def(class_block);
        let mut count = 0i64;
        count_non_literal_handlers(&class_def, &mut count);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_non_literal_handlers_pass_only() {
        // Func with only PassStmt → literal handler.
        let func = make_func_def(make_block(vec![make_pass_stmt()]));
        let class_block = make_block(vec![func]);
        let class_def = make_class_def(class_block);
        let mut count = 0i64;
        count_non_literal_handlers(&class_def, &mut count);
        assert_eq!(count, 0);
    }
}
