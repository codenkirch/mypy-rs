//! Native port of pure helper functions from `mypy/semanal.py` (Issue #209).
//!
//! These are the pure, non-plugin, non-mutating helpers used by the
//! `SemanticAnalyzer` visitor. Each mirrors the Python implementation and
//! operates on live Python AST/symbol objects via PyO3, following the same
//! strangler-fig pattern as `erase_type` (Stage 1): Rust handles the common
//! case and returns `None` / `false` for anything it cannot handle, so Python
//! falls back gracefully.
//!
//! Ported functions:
//! - `refers_to_fullname` — check whether a `RefExpr` node refers to a
//!   given fullname, possibly through a `TypeAlias` indirection.
//! - `refers_to_class_or_function` — check whether a `RefExpr` node refers
//!   to a class or function definition.
//! - `is_trivial_body` — check whether a `Block` body is trivial (pass,
//!   ellipsis, or `raise NotImplementedError()`).
//! - `find_duplicate` — return the first duplicate element in a list, if any.
//! - `is_valid_replacement` — can a `SymbolTableNode` replace an existing one?
//! - `is_same_symbol` — do two `SymbolNode` values refer to the same symbol?
//! - `names_modified_in_lvalue` / `names_modified_by_assignment` — collect
//!   `NameExpr` assignment targets.
//! - `remove_imported_names_from_symtable` — strip imported names from a
//!   symbol table (mutates the Python dict in-place).
//! - `apply_semantic_analyzer_patches` — call patch callbacks sorted by
//!   priority.

use std::collections::HashSet;

use pyo3::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple, PyType};

// ---------------------------------------------------------------------------
// refers_to_fullname
// ---------------------------------------------------------------------------

/// `mypy.semanal.refers_to_fullname` — is `node` a name or member expression
/// with the given full name?
///
/// Mirrors semanal.py:8308-8319. Checks `isinstance(node, RefExpr)` via
/// isinstance against the resolved `mypy.nodes.RefExpr` class, then compares
/// `node.fullname` against the provided fullnames. If the node's `.node`
/// attribute is a `TypeAlias` (and `python_3_12_type_alias` is False), recurses
/// into the alias target via `is_named_instance`.
///
/// Returns `false` when the node is not a `RefExpr` — the Python function also
/// returns `False` unconditionally, so there is no fallback. For the
/// `TypeAlias` recursion, returns `false` if any attribute access fails
/// (conservative: Python would never raise here).
#[pyfunction]
pub(crate) fn rust_refers_to_fullname(
    py: Python<'_>,
    node: &PyAny,
    fullnames: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;

    if !node.is_instance(ref_expr_cls)? {
        return Ok(false);
    }

    let fullname_set = normalize_fullnames(fullnames)?;

    let node_fullname = node.getattr("fullname")?;
    let node_fullname_str: &str = node_fullname.downcast::<PyString>()?.to_str()?;
    if fullname_set.contains(node_fullname_str) {
        return Ok(true);
    }

    // Check if node.node is a TypeAlias (and not python_3_12_type_alias).
    let node_attr = node.getattr("node")?;
    if node_attr.is_none() {
        return Ok(false);
    }

    let type_alias_cls: &PyType = nodes_mod.getattr("TypeAlias")?.downcast()?;
    if !node_attr.is_instance(type_alias_cls)? {
        return Ok(false);
    }

    let python_3_12 = node_attr.getattr("python_3_12_type_alias")?;
    if python_3_12.is_true()? {
        return Ok(false);
    }

    // Recurse: is_named_instance(node.node.target, fullnames).
    let target = node_attr.getattr("target")?;
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let proper = get_proper_type.call1((target,))?;

    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;
    if !proper.is_instance(instance_cls)? {
        return Ok(false);
    }

    let typ = proper.getattr("type")?;
    let typ_fullname = typ.getattr("fullname")?;
    let typ_fullname_str: &str = typ_fullname.downcast::<PyString>()?.to_str()?;
    Ok(fullname_set.contains(typ_fullname_str))
}

/// Normalize the `fullnames` argument (str or tuple of str) into a HashSet.
fn normalize_fullnames(fullnames: &PyAny) -> PyResult<HashSet<String>> {
    if let Ok(s) = fullnames.downcast::<PyString>() {
        return Ok([s.to_str()?.to_string()].into_iter().collect());
    }
    if let Ok(tup) = fullnames.downcast::<PyTuple>() {
        let mut result = HashSet::with_capacity(tup.len());
        for item in tup.iter() {
            let s = item.downcast::<PyString>()?;
            result.insert(s.to_str()?.to_string());
        }
        return Ok(result);
    }
    Ok([fullnames.str()?.to_str()?.to_string()]
        .into_iter()
        .collect())
}

// ---------------------------------------------------------------------------
// refers_to_class_or_function
// ---------------------------------------------------------------------------

/// `mypy.semanal.refers_to_class_or_function` — does semantically analyzed
/// `node` refer to a class or function?
///
/// Mirrors semanal.py:8322-8326. Returns `True` when `node` is a `RefExpr` and
/// `node.node` is a `TypeInfo`, `FuncDef`, or `OverloadedFuncDef`.
#[pyfunction]
pub(crate) fn rust_refers_to_class_or_function(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !node.is_instance(ref_expr_cls)? {
        return Ok(false);
    }

    let node_attr = node.getattr("node")?;
    if node_attr.is_none() {
        return Ok(false);
    }

    let type_info_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;
    let func_def_cls: &PyType = nodes_mod.getattr("FuncDef")?.downcast()?;
    let overloaded_cls: &PyType = nodes_mod.getattr("OverloadedFuncDef")?.downcast()?;

    Ok(node_attr.is_instance(type_info_cls)?
        || node_attr.is_instance(func_def_cls)?
        || node_attr.is_instance(overloaded_cls)?)
}

// ---------------------------------------------------------------------------
// is_trivial_body
// ---------------------------------------------------------------------------

/// `mypy.semanal.is_trivial_body` — is the given block body "trivial"?
///
/// Mirrors semanal.py:8498-8546. A body is trivial if it contains just a
/// `pass`, `...` (ellipsis), or `raise NotImplementedError()`. A trivial body
/// may also start with a docstring (a string expression statement).
#[pyfunction]
pub(crate) fn rust_is_trivial_body(py: Python<'_>, block: &PyAny) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let body = block.getattr("body")?;
    let body_list = body.downcast::<PyList>()?;

    if body_list.is_empty() {
        return Ok(false);
    }

    let all_items: Vec<&PyAny> = body_list.iter().collect();
    let mut start = 0usize;

    // Skip a docstring if present (first stmt is ExpressionStmt(StrExpr)).
    // Python skips unconditionally — even when the docstring is the only
    // statement in the body.
    if !all_items.is_empty() && is_expression_stmt_with_str_expr(all_items[0], nodes_mod)? {
        start = 1;
    }

    let remaining = &all_items[start..];
    if remaining.is_empty() {
        return Ok(true);
    }
    if remaining.len() > 1 {
        return Ok(false);
    }

    let stmt = remaining[0];
    let pass_stmt_cls: &PyType = nodes_mod.getattr("PassStmt")?.downcast()?;
    let expression_stmt_cls: &PyType = nodes_mod.getattr("ExpressionStmt")?.downcast()?;
    let raise_stmt_cls: &PyType = nodes_mod.getattr("RaiseStmt")?.downcast()?;

    if stmt.is_instance(pass_stmt_cls)? {
        return Ok(true);
    }
    if stmt.is_instance(expression_stmt_cls)? {
        let expr = stmt.getattr("expr")?;
        let ellipsis_expr_cls: &PyType = nodes_mod.getattr("EllipsisExpr")?.downcast()?;
        return expr.is_instance(ellipsis_expr_cls);
    }
    if stmt.is_instance(raise_stmt_cls)? {
        return is_raise_not_implemented(stmt, nodes_mod);
    }
    Ok(false)
}

/// Check if `stmt` is a `RaiseStmt` raising `NotImplementedError()`.
fn is_raise_not_implemented(stmt: &PyAny, nodes_mod: &PyModule) -> PyResult<bool> {
    let expr = stmt.getattr("expr")?;
    if expr.is_none() {
        return Ok(false);
    }

    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;

    let mut callee = expr;
    if callee.is_instance(call_expr_cls)? {
        callee = callee.getattr("callee")?;
    }

    if callee.is_instance(name_expr_cls)? {
        let fullname = callee.getattr("fullname")?;
        let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
        return Ok(fullname_str == "builtins.NotImplementedError");
    }
    Ok(false)
}

/// Check if `stmt` is an `ExpressionStmt` wrapping a `StrExpr`.
fn is_expression_stmt_with_str_expr(stmt: &PyAny, nodes_mod: &PyModule) -> PyResult<bool> {
    let expression_stmt_cls: &PyType = nodes_mod.getattr("ExpressionStmt")?.downcast()?;
    if !stmt.is_instance(expression_stmt_cls)? {
        return Ok(false);
    }
    let expr = stmt.getattr("expr")?;
    let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;
    expr.is_instance(str_expr_cls)
}

// ---------------------------------------------------------------------------
// find_duplicate
// ---------------------------------------------------------------------------

/// `mypy.semanal.find_duplicate` — if the list has duplicates, return one.
///
/// Mirrors semanal.py:8329-8337. Iterates from index 1 and checks if the
/// current element appears in the prefix slice. Uses Python equality semantics
/// (`==`).
///
/// Returns `None` if no duplicate is found.
#[pyfunction]
pub(crate) fn rust_find_duplicate(_py: Python<'_>, list: &PyAny) -> PyResult<Option<PyObject>> {
    let pylist = list.downcast::<PyList>()?;
    let len = pylist.len();
    if len < 2 {
        return Ok(None);
    }

    let items: Vec<&PyAny> = pylist.iter().collect();
    for i in 1..len {
        for j in 0..i {
            let is_eq = items[i].rich_compare(items[j], CompareOp::Eq)?;
            if is_eq.is_true()? {
                return Ok(Some(items[i].into()));
            }
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// is_valid_replacement
// ---------------------------------------------------------------------------

/// `mypy.semanal.is_valid_replacement` — can `new` replace `old` in a symbol
/// table?
///
/// Mirrors semanal.py:8473-8487. Valid cases:
/// 1. `old.node` is a `PlaceholderNode` and `new.node` is not.
/// 2. Both are `PlaceholderNode`s, `old.becomes_typeinfo` is False and
///    `new.becomes_typeinfo` is True.
#[pyfunction]
pub(crate) fn rust_is_valid_replacement(
    py: Python<'_>,
    old: &PyAny,
    new: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let placeholder_cls: &PyType = nodes_mod.getattr("PlaceholderNode")?.downcast()?;

    let old_node = old.getattr("node")?;
    if !old_node.is_instance(placeholder_cls)? {
        return Ok(false);
    }

    let new_node = new.getattr("node")?;
    let new_is_placeholder = new_node.is_instance(placeholder_cls)?;

    if !new_is_placeholder {
        return Ok(true);
    }

    let old_becomes = old_node.getattr("becomes_typeinfo")?;
    let new_becomes = new_node.getattr("becomes_typeinfo")?;
    Ok(!old_becomes.is_true()? && new_becomes.is_true()?)
}

// ---------------------------------------------------------------------------
// is_same_symbol
// ---------------------------------------------------------------------------

/// `mypy.semanal.is_same_symbol` — do two `SymbolNode` values refer to the
/// same symbol?
///
/// Mirrors semanal.py:8490-8495. Returns `True` when:
/// - `a == b` (Python equality),
/// - both are `PlaceholderNode`s, or
/// - both are `Var`s from module-level `__getattr__` with the same `fullname`.
#[pyfunction]
pub(crate) fn rust_is_same_symbol(py: Python<'_>, a: &PyAny, b: &PyAny) -> PyResult<bool> {
    // a == b
    let eq_result = a.rich_compare(b, CompareOp::Eq)?;
    if eq_result.is_true()? {
        return Ok(true);
    }

    if a.is_none() || b.is_none() {
        return Ok(false);
    }

    let nodes_mod = py.import("mypy.nodes")?;
    let placeholder_cls: &PyType = nodes_mod.getattr("PlaceholderNode")?.downcast()?;
    if a.is_instance(placeholder_cls)? && b.is_instance(placeholder_cls)? {
        return Ok(true);
    }

    // is_same_var_from_getattr
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
    if a.is_instance(var_cls)? && b.is_instance(var_cls)? {
        let a_getattr = a.getattr("from_module_getattr")?;
        let b_getattr = b.getattr("from_module_getattr")?;
        if a_getattr.is_true()? && b_getattr.is_true()? {
            let a_full = a.getattr("fullname")?;
            let b_full = b.getattr("fullname")?;
            let a_str: &str = a_full.downcast::<PyString>()?.to_str()?;
            let b_str: &str = b_full.downcast::<PyString>()?.to_str()?;
            return Ok(a_str == b_str);
        }
    }

    Ok(false)
}

// ---------------------------------------------------------------------------
// names_modified_in_lvalue / names_modified_by_assignment
// ---------------------------------------------------------------------------

/// `mypy.semanal.names_modified_in_lvalue` — return all `NameExpr` assignment
/// targets in an lvalue.
///
/// Mirrors semanal.py:8444-8455. Handles `NameExpr`, `StarExpr`, and
/// `ListExpr`/`TupleExpr` (recursing into items).
#[pyfunction]
pub(crate) fn rust_names_modified_in_lvalue(
    py: Python<'_>,
    lvalue: &PyAny,
) -> PyResult<Vec<PyObject>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    let star_expr_cls: &PyType = nodes_mod.getattr("StarExpr")?.downcast()?;
    let list_expr_cls: &PyType = nodes_mod.getattr("ListExpr")?.downcast()?;
    let tuple_expr_cls: &PyType = nodes_mod.getattr("TupleExpr")?.downcast()?;

    collect_name_exprs(
        lvalue,
        name_expr_cls,
        star_expr_cls,
        list_expr_cls,
        tuple_expr_cls,
    )
}

fn collect_name_exprs(
    lvalue: &PyAny,
    name_expr_cls: &PyType,
    star_expr_cls: &PyType,
    list_expr_cls: &PyType,
    tuple_expr_cls: &PyType,
) -> PyResult<Vec<PyObject>> {
    if lvalue.is_instance(name_expr_cls)? {
        return Ok(vec![lvalue.into()]);
    }
    if lvalue.is_instance(star_expr_cls)? {
        let inner = lvalue.getattr("expr")?;
        return collect_name_exprs(
            inner,
            name_expr_cls,
            star_expr_cls,
            list_expr_cls,
            tuple_expr_cls,
        );
    }
    if lvalue.is_instance(list_expr_cls)? || lvalue.is_instance(tuple_expr_cls)? {
        let items = lvalue.getattr("items")?;
        let items_list = items.downcast::<PyList>()?;
        let mut result = Vec::new();
        for item in items_list.iter() {
            let sub = collect_name_exprs(
                item,
                name_expr_cls,
                star_expr_cls,
                list_expr_cls,
                tuple_expr_cls,
            )?;
            result.extend(sub);
        }
        return Ok(result);
    }
    Ok(Vec::new())
}

/// `mypy.semanal.names_modified_by_assignment` — return all unqualified (short)
/// names assigned to in an assignment statement.
///
/// Mirrors semanal.py:8436-8441.
#[pyfunction]
pub(crate) fn rust_names_modified_by_assignment(
    py: Python<'_>,
    s: &PyAny,
) -> PyResult<Vec<PyObject>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    let star_expr_cls: &PyType = nodes_mod.getattr("StarExpr")?.downcast()?;
    let list_expr_cls: &PyType = nodes_mod.getattr("ListExpr")?.downcast()?;
    let tuple_expr_cls: &PyType = nodes_mod.getattr("TupleExpr")?.downcast()?;

    let lvalues = s.getattr("lvalues")?;
    let lvalues_list = lvalues.downcast::<PyList>()?;
    let mut result = Vec::new();
    for lvalue in lvalues_list.iter() {
        let sub = collect_name_exprs(
            lvalue,
            name_expr_cls,
            star_expr_cls,
            list_expr_cls,
            tuple_expr_cls,
        )?;
        result.extend(sub);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// remove_imported_names_from_symtable
// ---------------------------------------------------------------------------

/// `mypy.semanal.remove_imported_names_from_symtable` — remove all imported
/// names from the symbol table of a module.
///
/// Mirrors semanal.py:8340-8351. Mutates the `names` dict in-place: removes
/// entries whose `node.fullname` prefix doesn't match `module`.
#[pyfunction]
pub(crate) fn rust_remove_imported_names_from_symtable(
    py: Python<'_>,
    names: &PyAny,
    module: &str,
) -> PyResult<()> {
    let names_dict = names.downcast::<PyDict>()?;
    let keys: Vec<PyObject> = names_dict.keys().into_iter().map(|k| k.into()).collect();
    let mut to_remove: Vec<PyObject> = Vec::new();

    for key in &keys {
        let node_entry = match names_dict.get_item(key)? {
            Some(n) => n,
            None => continue,
        };
        let inner_node = node_entry.getattr("node")?;
        if inner_node.is_none() {
            continue;
        }
        let fullname = inner_node.getattr("fullname")?;
        let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
        let prefix = match fullname_str.rfind('.') {
            Some(idx) => &fullname_str[..idx],
            None => "",
        };
        if prefix != module {
            to_remove.push(key.clone_ref(py));
        }
    }

    for key in &to_remove {
        names_dict.del_item(key)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// apply_semantic_analyzer_patches
// ---------------------------------------------------------------------------

/// `mypy.semanal.apply_semantic_analyzer_patches` — call patch callbacks in
/// the right order (sorted by priority).
///
/// Mirrors semanal.py:8426-8433. Takes a list of `(priority, callback)` tuples,
/// sorts by priority, then calls each callback.
#[pyfunction]
pub(crate) fn rust_apply_semantic_analyzer_patches(
    py: Python<'_>,
    patches: &PyAny,
) -> PyResult<()> {
    let patches_list = patches.downcast::<PyList>()?;
    let mut items: Vec<(i64, PyObject)> = Vec::with_capacity(patches_list.len());
    for patch in patches_list.iter() {
        let tuple = patch.downcast::<PyTuple>()?;
        let priority: i64 = tuple.get_item(0)?.extract()?;
        let callback: PyObject = tuple.get_item(1)?.into();
        items.push((priority, callback));
    }
    items.sort_by_key(|(p, _)| *p);
    for (_, callback) in items {
        let cb: &PyAny = callback.as_ref(py);
        cb.call0()?;
    }
    Ok(())
}
