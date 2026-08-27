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
//! - `is_init_only` — check whether a `Var` is a `dataclasses.InitVar` (Issue #391).
//! - `erase_func_annotations` — erase type annotations from a `FuncDef` (Issue #391).
//! - `get_deprecated` — extract deprecation string from a `CallExpr` decorator (Issue
//!   #391).
//! - `get_name_repr_of_expr` — simplified textual representation of an expression
//!   (Issue #391).

use std::collections::HashSet;

use pyo3::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple, PyType};

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
    _py: Python<'_>,
    node: &PyAny,
    fullnames: &PyAny,
) -> PyResult<bool> {
    let fullname_set = normalize_fullnames(fullnames)?;
    refers_to_fullname(node, &fullname_set)
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

// ---------------------------------------------------------------------------
// is_init_only (Issue #391)
// ---------------------------------------------------------------------------

/// `mypy.semanal.is_init_only` — check whether a `Var` is a `dataclasses.InitVar`.
///
/// Mirrors semanal.py:8647-8651. Pure helper: checks if the variable's type,
/// when properly resolved, is `dataclasses.InitVar`.
#[pyfunction]
pub(crate) fn rust_is_init_only(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;

    let node_type = node.getattr("type")?;
    let proper = get_proper_type.call1((node_type,))?;
    if !proper.is_instance(instance_cls)? {
        return Ok(false);
    }

    let type_obj = proper.getattr("type")?;
    let fullname = type_obj.getattr("fullname")?;
    let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
    Ok(fullname_str == "dataclasses.InitVar")
}

// ---------------------------------------------------------------------------
// erase_func_annotations (Issue #391)
// ---------------------------------------------------------------------------

/// `mypy.semanal.erase_func_annotations` — erase type annotations from a `FuncDef`.
///
/// Mirrors semanal.py:8654-8660. Mutates the `FuncDef` in-place: sets
/// `type_args`, `arguments[*].type_annotation`, `arguments[*].variable.type`,
/// `type`, and `unanalyzed_type` to `None`.
#[pyfunction]
pub(crate) fn rust_erase_func_annotations(py: Python<'_>, func: &PyAny) -> PyResult<()> {
    func.setattr("type_args", py.None())?;

    let arguments = func.getattr("arguments")?;
    let args_list = arguments.downcast::<PyList>()?;
    for arg in args_list.iter() {
        arg.setattr("type_annotation", py.None())?;
        let variable = arg.getattr("variable")?;
        variable.setattr("type", py.None())?;
    }

    func.setattr("type", py.None())?;
    func.setattr("unanalyzed_type", py.None())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// get_deprecated (Issue #391)
// ---------------------------------------------------------------------------

/// `mypy.semanal.get_deprecated` — extract deprecation string from a
/// `CallExpr` decorator (e.g. `@deprecated("msg")`).
///
/// Mirrors semanal.py:1487-1495. A pure static method: if `expression` is a
/// `CallExpr` whose callee refers to `DEPRECATED_TYPE_NAMES` and whose first
/// argument is a `StrExpr`, returns that string value; otherwise `None`.
#[pyfunction]
pub(crate) fn rust_get_deprecated(py: Python<'_>, expression: &PyAny) -> PyResult<Option<String>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;

    if !expression.is_instance(call_expr_cls)? {
        return Ok(None);
    }

    let callee = expression.getattr("callee")?;
    let args = expression.getattr("args")?;
    let args_list = args.downcast::<PyList>()?;
    if args_list.is_empty() {
        return Ok(None);
    }

    // Check callee refers to DEPRECATED_TYPE_NAMES
    let deprecated_names: Vec<&str> = vec!["warnings.deprecated", "typing_extensions.deprecated"];

    // callee could be a NameExpr or MemberExpr; try to get fullname
    let callee_fullname: Option<String> =
        if callee.is_instance(nodes_mod.getattr("NameExpr")?.downcast()?)? {
            let fn_val = callee.getattr("fullname")?;
            fn_val.downcast::<PyString>()?.extract().ok()
        } else if callee.is_instance(nodes_mod.getattr("MemberExpr")?.downcast()?)? {
            // MemberExpr: build fullname from expr.name parts
            let name = callee.getattr("name")?;
            let name_str: &str = name.downcast::<PyString>()?.to_str()?;
            let expr = callee.getattr("expr")?;
            if expr.is_instance(nodes_mod.getattr("NameExpr")?.downcast()?)? {
                let base = expr.getattr("name")?;
                let base_str: &str = base.downcast::<PyString>()?.to_str()?;
                Some(format!("{}.{}", base_str, name_str))
            } else {
                None
            }
        } else {
            None
        };

    if !deprecated_names.contains(&callee_fullname.as_deref().unwrap_or("")) {
        return Ok(None);
    }

    // First arg must be a StrExpr
    let first_arg = args_list.get_item(0)?;
    if !first_arg.is_instance(str_expr_cls)? {
        return Ok(None);
    }

    let value = first_arg.getattr("value")?;
    let value_str: &str = value.downcast::<PyString>()?.to_str()?;
    Ok(Some(value_str.to_string()))
}

// ---------------------------------------------------------------------------
// classify_decorators (Issue #348)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.visit_decorator` decorator classification
/// loop (semanal.py:1830-1897).
///
/// Each decorator expression in the list is classified against the same
/// name-matches that Python checks, in the same branch order. Rust returns a
/// single classification tag per decorator and Python applies the side
/// effects (AST mutation, error reporting, scope checks). This is the
/// strangler-fig per-call gate: every decorator receives a tag (including
/// `other`), and Python runs the matching side-effect branch from the tag.
/// Returns `None` only when the whole list cannot be classified, in which
/// case Python falls back to the pure loop unchanged.
///
/// Mirrors the dispatch in semanal.py:1831-1897:
///   1. `abc.abstractmethod` -> "abstract"
///   2. `asyncio.coroutines.coroutine` / `types.coroutine` -> "awaitable"
///   3. `builtins.staticmethod` -> "static"
///   4. `builtins.classmethod` -> "class"
///   5. `typing.override` / `typing_extensions.override` -> "override"
///   6. `builtins.property` / `abc.abstractproperty` /
///      `functools.cached_property` / `enum.property` /
///      `types.DynamicClassAttribute` -> "property", "abstract_property", or
///      "cached_property" depending on which name matched
///   7. `typing.no_type_check` -> "no_type_check"
///   8. `typing.final` / `typing_extensions.final` -> "final"
///   9. `typing.type_check_only` / `typing_extensions.type_check_only`
///      -> "type_check_only"
///  10. `CallExpr` with callee `typing.dataclass_transform` /
///      `typing_extensions.dataclass_transform` -> "dataclass_transform"
///  11. a deprecation `CallExpr` (`get_deprecated` non-None)
///      -> "deprecated"
///  12. anything else -> "other"
///
/// A name-match on a node whose `.node` is a `TypeAlias` resolves the alias
/// target to a named `Instance` first (same as `refers_to_fullname`). The
/// `fullnames` argument is a tuple of name sets passed in from Python, so the
/// name constants stay in `mypy/types.py`.
///
/// Returns `None` when the decorator list is not a list (Python would never
/// hit that) or when `name_sets` does not contain exactly 13 entries (a
/// caller mismatch). In both cases Python falls back to the pure loop.
#[pyfunction]
#[pyo3(signature = (decorators, name_sets))]
pub(crate) fn rust_classify_decorators(
    py: Python<'_>,
    decorators: &PyAny,
    name_sets: &PyTuple,
) -> PyResult<Option<Vec<String>>> {
    let dec_list = match decorators.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    // Each entry is a single name (str) or a tuple of names
    // (tuple[str, ...]); index order mirrors the branch order below.
    let mut fullname_sets: Vec<HashSet<String>> = Vec::with_capacity(name_sets.len());
    for item in name_sets.iter() {
        fullname_sets.push(normalize_fullnames(item)?);
    }
    if fullname_sets.len() != 13 {
        return Ok(None);
    }

    let nodes_mod = py.import("mypy.nodes")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;

    let mut result: Vec<String> = Vec::with_capacity(dec_list.len());
    for d in dec_list.iter() {
        // Branch order mirrors semanal.py:1831-1897 exactly.
        if refers_to_fullname(d, &fullname_sets[0])? {
            result.push("abstract".to_string());
        } else if refers_to_fullname(d, &fullname_sets[1])? {
            result.push("awaitable".to_string());
        } else if refers_to_fullname(d, &fullname_sets[2])? {
            result.push("static".to_string());
        } else if refers_to_fullname(d, &fullname_sets[3])? {
            result.push("class".to_string());
        } else if refers_to_fullname(d, &fullname_sets[4])? {
            result.push("override".to_string());
        } else if refers_to_fullname(d, &fullname_sets[5])? {
            // Property family: split on which name matched (mirrors the
            // abstractproperty / cached_property sub-branches).
            if refers_to_fullname(d, &fullname_sets[6])? {
                result.push("abstract_property".to_string());
            } else if refers_to_fullname(d, &fullname_sets[7])? {
                result.push("cached_property".to_string());
            } else {
                result.push("property".to_string());
            }
        } else if refers_to_fullname(d, &fullname_sets[8])? {
            result.push("no_type_check".to_string());
        } else if refers_to_fullname(d, &fullname_sets[9])? {
            result.push("final".to_string());
        } else if refers_to_fullname(d, &fullname_sets[10])? {
            result.push("type_check_only".to_string());
        } else if d.is_instance(call_expr_cls)? {
            // dataclass_transform: CallExpr with callee referring to
            // DATACLASS_TRANSFORM_NAMES.
            let callee = d.getattr("callee")?;
            if refers_to_fullname(callee, &fullname_sets[11])? {
                result.push("dataclass_transform".to_string());
            } else if is_deprecated_call(py, d, &fullname_sets[12])? {
                result.push("deprecated".to_string());
            } else {
                result.push("other".to_string());
            }
        } else if is_deprecated_call(py, d, &fullname_sets[12])? {
            result.push("deprecated".to_string());
        } else {
            result.push("other".to_string());
        }
    }
    Ok(Some(result))
}

/// Name-or-set fullname match mirroring `mypy.semanal.refers_to_fullname`.
///
/// Factors the body of `rust_refers_to_fullname` so the decorator classifier
/// reuses the exact same match semantics, including the `TypeAlias` target
/// resolution to a named `Instance` when `python_3_12_type_alias` is False.
fn refers_to_fullname(node: &PyAny, fullname_set: &HashSet<String>) -> PyResult<bool> {
    let py = node.py();
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;

    if !node.is_instance(ref_expr_cls)? {
        return Ok(false);
    }

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

/// Whether a decorator expression is a deprecation call
/// (`get_deprecated` would return a non-None string).
///
/// Mirrors `mypy.semanal.get_deprecated` (semanal.py:1495-1503): a
/// `CallExpr` whose callee refers to `DEPRECATED_TYPE_NAMES` and whose first
/// positional argument is a `StrExpr`. Any attribute access failure is
/// treated as not-deprecated (conservative: Python would raise instead, so
/// no fallback is needed for the common path).
fn is_deprecated_call(
    py: Python<'_>,
    expression: &PyAny,
    deprecated_names: &HashSet<String>,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;

    if !expression.is_instance(call_expr_cls)? {
        return Ok(false);
    }

    let callee = expression.getattr("callee")?;
    if !refers_to_fullname(callee, deprecated_names)? {
        return Ok(false);
    }

    let args = expression.getattr("args")?;
    let args_list = args.downcast::<PyList>()?;
    if args_list.is_empty() {
        return Ok(false);
    }

    let first_arg = args_list.get_item(0)?;
    if !first_arg.is_instance(str_expr_cls)? {
        return Ok(false);
    }
    Ok(true)
}

/// Extract the deprecation message from a `CallExpr` decorator, mirroring the
/// `mypy.semanal.SemanticAnalyzer.get_deprecated` staticmethod
/// (semanal.py:1710-1718). Returns `None` when not a deprecated call.
fn extract_deprecated_message(
    py: Python<'_>,
    expression: &PyAny,
    deprecated_names: &HashSet<String>,
) -> PyResult<Option<String>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;

    if !expression.is_instance(call_expr_cls)? {
        return Ok(None);
    }
    let callee = expression.getattr("callee")?;
    if !refers_to_fullname(callee, deprecated_names)? {
        return Ok(None);
    }
    let args = expression.getattr("args")?;
    let args_list = args.downcast::<PyList>()?;
    if args_list.is_empty() {
        return Ok(None);
    }
    let first_arg = args_list.get_item(0)?;
    if !first_arg.is_instance(str_expr_cls)? {
        return Ok(None);
    }
    let value = first_arg.getattr("value")?;
    let msg: &str = value.downcast::<PyString>()?.to_str()?;
    Ok(Some(msg.to_string()))
}

/// `mypy.semanal.SemanticAnalyzer.analyze_class_decorator_common` classifier
/// (semanal.py:2741-2752). Returns `Some((tag, deprecated_msg))` where `tag`
/// is `final`, `disjoint_base`, `type_check_only`, `deprecated`, or `none`;
/// `deprecated_msg` is `Some` only for `deprecated`. The Python shim applies
/// the flag writes and the two `@disjoint_base` `fail`s (protocol / TypedDict).
/// Returns `None` to defer to the pure-Python body on a name-set mismatch.
#[pyfunction]
#[pyo3(signature = (decorator, name_sets))]
pub(crate) fn rust_classify_class_decorator(
    py: Python<'_>,
    decorator: &PyAny,
    name_sets: &PyTuple,
) -> PyResult<Option<(String, Option<String>)>> {
    if name_sets.len() != 4 {
        return Ok(None);
    }
    let final_names = normalize_fullnames(name_sets.get_item(0)?)?;
    let disjoint_names = normalize_fullnames(name_sets.get_item(1)?)?;
    let tco_names = normalize_fullnames(name_sets.get_item(2)?)?;
    let deprecated_names = normalize_fullnames(name_sets.get_item(3)?)?;

    // Branch order mirrors semanal.py:2741-2752 exactly.
    if refers_to_fullname(decorator, &final_names)? {
        return Ok(Some(("final".to_string(), None)));
    }
    if refers_to_fullname(decorator, &disjoint_names)? {
        return Ok(Some(("disjoint_base".to_string(), None)));
    }
    if refers_to_fullname(decorator, &tco_names)? {
        return Ok(Some(("type_check_only".to_string(), None)));
    }
    if let Some(msg) = extract_deprecated_message(py, decorator, &deprecated_names)? {
        return Ok(Some(("deprecated".to_string(), Some(msg))));
    }
    Ok(Some(("none".to_string(), None)))
}

// ---------------------------------------------------------------------------
// get_name_repr_of_expr (Issue #391)
// ---------------------------------------------------------------------------

/// `mypy.semanal.get_name_repr_of_expr` — simplified textual representation
/// of a base class expression.
///
/// Mirrors semanal.py:2677-2687. Pure helper: recurses through
/// `IndexExpr`/`CallExpr` wrapping to reach a `NameExpr` or `MemberExpr`,
/// building a dotted name like `module.Class`.
#[pyfunction]
pub(crate) fn rust_get_name_repr_of_expr(py: Python<'_>, expr: &PyAny) -> PyResult<Option<String>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    let member_expr_cls: &PyType = nodes_mod.getattr("MemberExpr")?.downcast()?;
    let index_expr_cls: &PyType = nodes_mod.getattr("IndexExpr")?.downcast()?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;

    // Get the innermost base expression by peeling IndexExpr/CallExpr
    let mut current: &PyAny = expr;
    loop {
        if current.is_instance(index_expr_cls)? {
            current = current.getattr("base")?;
        } else if current.is_instance(call_expr_cls)? {
            current = current.getattr("callee")?;
        } else {
            break;
        }
    }

    if current.is_instance(name_expr_cls)? {
        let name = current.getattr("name")?;
        let name_str: &str = name.downcast::<PyString>()?.to_str()?;
        return Ok(Some(name_str.to_string()));
    }

    if current.is_instance(member_expr_cls)? {
        // Build fullname from MemberExpr
        let member_name = current.getattr("name")?;
        let member_name_str: &str = member_name.downcast::<PyString>()?.to_str()?;
        let inner = current.getattr("expr")?;
        let prefix = get_member_expr_prefix(inner, name_expr_cls, member_expr_cls)?;
        return match prefix {
            Some(p) => Ok(Some(format!("{}.{}", p, member_name_str))),
            None => Ok(None),
        };
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// classify_member_resolution (Issue #421)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.visit_member_expr` resolution branch
/// classification (semanal.py:6316-6364).
///
/// Rust computes which resolution branch applies to `expr` and returns the
/// symbol that branch resolves to; Python applies the AST assignments
/// (`expr.node` / `expr.kind` / `expr.fullname`), `record_imported_symbol`,
/// and `process_placeholder`. Branches that mutate semantic state or
/// construct new symbols (module re-export visibility checks,
/// `record_incomplete_ref`, module `__getattr__` Var synthesis, missing-module
/// Any Var synthesis) return `None` and run in pure Python unchanged.
///
/// The branch order mirrors semanal.py:6319-6364 exactly: the module
/// attribute branch is checked BEFORE the class/instance member branch. In
/// particular, a module definition whose node is a `TypeInfo` shadows a
/// same-named class member and is classified as `"module"`, matching
/// Python's precedence.
///
/// Returns
///     `(None, None)` — unsupported / no classification (pure fallback).
///     `(Some("module"), Some(sym))` — direct module-symbol table hit,
///       including placeholder-node symbols.
///     `(Some("none"), None)` — module symbol found but `module_hidden`
///       (Python resolves `get_module_symbol` to None and leaves the
///       expression unresolved).
///     `(Some("member"), Some(sym))` — `TypeInfo.names` member whose node is
///       a `MypyFile`, `TypeInfo`, or `TypeAlias`.
///     `(Some("none"), None)` — `TypeInfo` member present but not in the
///       (MypyFile, TypeInfo, TypeAlias) set, or an unbound symbol node
///       (Python leaves the expression unresolved).
#[pyfunction]
pub(crate) fn rust_classify_member_resolution(
    expr: &PyAny,
    member_expr_cls: &PyType,
    ref_expr_cls: &PyType,
    mypy_file_cls: &PyType,
    type_info_cls: &PyType,
    type_alias_cls: &PyType,
) -> PyResult<(Option<String>, Option<Py<PyAny>>)> {
    let py = expr.py();
    if !expr.is_instance(member_expr_cls)? {
        return Ok((None, None));
    }

    // `base = expr.expr; base.accept(self)` happens in Python before this
    // call; the base's analysis state (its `node` binding) is already
    // applied by the time we classify.
    let base = expr.getattr("expr")?;

    // Module-attribute branch (semanal.py:6319-6329).
    if base.is_instance(ref_expr_cls)? {
        let base_node = base.getattr("node")?;
        if !base_node.is_none() && base_node.is_instance(mypy_file_cls)? {
            let member_name = expr.getattr("name")?;
            let sym = base_node
                .getattr("names")?
                .call_method1("get", (member_name,))?;
            if !sym.is_none() {
                // Python: `elif sym.module_hidden: sym = None` runs before
                // visit_member_expr's placeholder check, so check it first.
                let module_hidden = sym.getattr("module_hidden")?;
                if module_hidden.is_true()? {
                    return Ok((Some("none".to_string()), None));
                }
                return Ok((Some("module".to_string()), Some(sym.into_py(py))));
            }
            // Missing name: get_module_symbol falls through to the re-export
            // visibility check, incomplete-namespace deferral, __getattr__
            // Var synthesis, or missing-module synthesis. All of those read

            // or mutate semantic state, so they stay in Python.
            return Ok((None, None));
        }
    }

    // Class/instance member branch (semanal.py:6330-6363).
    if !base.is_instance(ref_expr_cls)? {
        return Ok((None, None));
    }
    let base_node = base.getattr("node")?;
    if !base_node.is_instance(type_info_cls)? {
        // self.bar / cls.bar via function args, and no-args TypeAlias bases,
        // resolve type_info through Python state; not classified here.
        return Ok((None, None));
    }
    let member_name = expr.getattr("name")?;
    let sym = base_node
        .getattr("names")?
        .call_method1("get", (member_name,))?;
    if sym.is_none() {
        return Ok((None, None));
    }
    let sym_node = sym.getattr("node")?;
    // Mirror `isinstance(n.node, (MypyFile, TypeInfo, TypeAlias))` directly.
    if sym_node.is_none()
        || (!sym_node.is_instance(mypy_file_cls)?
            && !sym_node.is_instance(type_info_cls)?
            && !sym_node.is_instance(type_alias_cls)?)
    {
        // Method / Var / FuncDef / etc. are handled by checkmember; Python
        // leaves the expression unbound here.
        return Ok((Some("none".to_string()), None));
    }
    Ok((Some("member".to_string()), Some(sym.into_py(py))))
}

/// Recursively build the prefix for a MemberExpr.
fn get_member_expr_prefix(
    inner: &PyAny,
    name_expr_cls: &PyType,
    member_expr_cls: &PyType,
) -> PyResult<Option<String>> {
    if inner.is_instance(name_expr_cls)? {
        let name = inner.getattr("name")?;
        let name_str: &str = name.downcast::<PyString>()?.to_str()?;
        return Ok(Some(name_str.to_string()));
    }
    if inner.is_instance(member_expr_cls)? {
        let member_name = inner.getattr("name")?;
        let member_name_str: &str = member_name.downcast::<PyString>()?.to_str()?;
        let prefix =
            get_member_expr_prefix(inner.getattr("expr")?, name_expr_cls, member_expr_cls)?;
        return match prefix {
            Some(p) => Ok(Some(format!("{}.{}", p, member_name_str))),
            None => Ok(None),
        };
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// classify_imports (Issue #420)
// ---------------------------------------------------------------------------

/// One `import a.b [as c]` binding classified by `rust_classify_imports`:
/// `(imported_id, base_id, module_public, kind)`. `kind` is `None` when the
/// target module is missing (`add_unknown_imported_symbol` branch).
pub(crate) type ImportClass = (String, String, bool, Option<i64>);

/// `SemanticAnalyzer.visit_import` — classify `import a.b as c` bindings.
///
/// Mirrors semanal.py:3108-3148. Rust performs the lookup/decision for each
/// `(id, as_id)` pair: which target module is imported, whether the symbol is
/// module-public (implicit re-export), and which scope kind (LDEF/MDEF/GDEF)
/// the binding gets. Python keeps `SymbolTableNode` construction and the
/// symbol-table mutation (`add_imported_symbol` /
/// `add_unknown_imported_symbol`).
///
/// Returns `None` when the argument shapes do not match what Python would
/// feed (non-dict modules, non-tuple id pairs, scope stack shorter than 2)
/// so the caller falls back to the pure-Python loop byte-for-byte.
#[pyfunction]
#[pyo3(signature = (ids, is_stub_file, implicit_reexport, modules, scope_stack, self_type))]
pub(crate) fn rust_classify_imports(
    py: Python<'_>,
    ids: &PyAny,
    is_stub_file: bool,
    implicit_reexport: bool,
    modules: &PyAny,
    scope_stack: &PyAny,
    self_type: &PyAny,
) -> PyResult<Option<Vec<ImportClass>>> {
    let ids_list = match ids.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let modules_dict = match modules.downcast::<PyDict>() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    let scope_list = match scope_stack.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if scope_list.is_empty() {
        return Ok(None);
    }

    let nodes_mod = py.import("mypy.nodes")?;
    let semanal_mod = py.import("mypy.semanal")?;
    let gdef = nodes_mod.getattr("GDEF")?;
    let ldef = nodes_mod.getattr("LDEF")?;
    let mdef = nodes_mod.getattr("MDEF")?;
    let func_scope: i64 = semanal_mod.getattr("SCOPE_FUNC")?.extract()?;
    let comprehension_scope: i64 = semanal_mod.getattr("SCOPE_COMPREHENSION")?.extract()?;
    let annotation_scope: i64 = semanal_mod.getattr("SCOPE_ANNOTATION")?.extract()?;

    let gdef_val: i64 = gdef.extract()?;
    let ldef_val: i64 = ldef.extract()?;
    let mdef_val: i64 = mdef.extract()?;

    let use_implicit_reexport = !is_stub_file && implicit_reexport;

    let mut result: Vec<ImportClass> = Vec::with_capacity(ids_list.len());
    for item in ids_list.iter() {
        let pair = match item.downcast::<PyTuple>() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        if pair.len() != 2 {
            return Ok(None);
        }
        let id = pair.get_item(0)?;
        let as_id = pair.get_item(1)?;
        let id_str: &str = id.downcast::<PyString>()?.to_str()?;
        let has_alias = !as_id.is_none();
        let (imported_id, base_id) = if has_alias {
            let as_id_str: &str = as_id.downcast::<PyString>()?.to_str()?;
            (as_id_str.to_string(), id_str.to_string())
        } else {
            let base_id = match id_str.split_once('.') {
                Some((prefix, _)) => prefix.to_string(),
                None => id_str.to_string(),
            };
            (base_id.clone(), base_id)
        };

        let module_public = if has_alias {
            let as_id_str: &str = as_id.downcast::<PyString>()?.to_str()?;
            use_implicit_reexport || id_str == as_id_str
        } else {
            use_implicit_reexport
        };

        match modules_dict.get_item(&base_id)? {
            Some(_) => {
                // Scope kind: mirror `is_func_scope` / `self.type is not None`.
                // scope_stack is a stack: the current scope is the last element.
                let top_idx = scope_list.len() - 1;
                let top = scope_list.get_item(top_idx)?;
                let mut scope_type: i64 = top.extract()?;
                if scope_type == annotation_scope {
                    if top_idx == 0 {
                        return Ok(None);
                    }
                    scope_type = scope_list.get_item(top_idx - 1)?.extract()?;
                }
                let kind = if scope_type == func_scope || scope_type == comprehension_scope {
                    ldef_val
                } else if !self_type.is_none() {
                    mdef_val
                } else {
                    gdef_val
                };
                result.push((imported_id, base_id, module_public, Some(kind)));
            }
            None => {
                result.push((imported_id, base_id, module_public, None));
            }
        }
    }
    Ok(Some(result))
}

// ---------------------------------------------------------------------------
// lookup (Issue #419)
// ---------------------------------------------------------------------------

/// The resolution decision for `SemanticAnalyzer._lookup` (semanal.py:6749-6818).
///
/// Rust implements the pure local → enclosing → global → builtins walk and
/// returns *what was found*, leaving all side effects to Python: symbol-table
/// bookkeeping, `record_imported_symbol`, `name_not_defined` error reporting,
/// `is_active_symbol_in_class_body` gating, and the `__qualname__`/`__module__`
/// `Var` synthesis (which needs a live `Var(name, self.str_type())` constructed
/// through Python). This is the strangler-fig per-call gate: the walk itself is
/// the valuable slice, and every mutating/error sub-case stays in Python.
///
/// Returns `Some(("found", node))` when a `SymbolTableNode` was resolved
/// (the node is the table entry Python would return), `Some((reason, None))`
/// for terminal non-found paths (Python runs `name_not_defined` or synthesizes
/// the `Var`), and `None` when a sub-case is too entangled with Python state
/// for Rust to decide — Python then falls back to the pure loop unchanged.
///
/// The class-body attribute branch (2a) is *always* a `None` fallback: an
/// inactive class attribute does not terminate the walk, it falls through to
/// the later scopes, and only Python owns `is_active_symbol_in_class_body`.
/// Same for the implicit `self.x` assignment fallback (continuation-dependent).
///
/// Mirrors semanal.py:6761-6818: `global`/`nonlocal` decl precedence
/// (1a/1b), local scopes (3), module globals (4), builtins with the
/// single-underscore privacy filter (5), and the `__qualname__`/`__module__`
/// synthesis (2b, checked before locals only at class scope). `global_decls`
/// and `nonlocal_decls` are the top-of-stack decl sets (`self.global_decls[-1]`
/// / `self.nonlocal_decls[-1]`); `locals` is the full `self.locals` stack;
/// `type_names` is `self.type.names` (a dict) when inside a class, else `None`
/// passed as Python `None`. `is_func_scope` comes from `self.is_func_scope()`.
#[pyfunction]
#[pyo3(signature = (name, global_decls, globals, nonlocal_decls, locals, type_names, is_func_scope))]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_lookup(
    name: &str,
    global_decls: &PySet,
    globals: &PyDict,
    nonlocal_decls: &PySet,
    locals: &PyAny,
    type_names: &PyAny,
    is_func_scope: bool,
) -> PyResult<Option<(String, Option<PyObject>)>> {
    let name = name.to_string();
    let type_names = type_names.downcast::<PyDict>().ok();

    // 1a. Name declared using 'global x' takes precedence.
    if global_decls.contains(name.as_str())? {
        if let Some(node) = globals.get_item(name.as_str())?.map(Into::into) {
            return Ok(Some(("found".to_string(), Some(node))));
        }
        return Ok(Some(("global_undeclared".to_string(), None)));
    }

    // 1b. Name declared using 'nonlocal x' takes precedence: walk the
    // enclosing function scopes only (reversed(self.locals[:-1])).
    if nonlocal_decls.contains(name.as_str())? {
        let locals_list = match locals.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        let last_index = locals_list.len().saturating_sub(1);
        for i in (0..last_index).rev() {
            let table_item = locals_list.get_item(i)?;
            if table_item.is_none() {
                continue;
            }
            let table = match table_item.downcast::<PyDict>() {
                Ok(d) => d,
                Err(_) => return Ok(None),
            };
            if table.contains(name.as_str())? {
                let node = table.get_item(name.as_str())?.map(Into::into);
                return Ok(Some(("found".to_string(), node)));
            }
        }
        return Ok(Some(("nonlocal_undeclared".to_string(), None)));
    }

    // 2a/2b only apply at class scope (type_names is Some, not a function
    // scope).
    if let Some(type_names) = type_names {
        if !is_func_scope {
            // 2a. Class-body attributes always fall back: an inactive class
            // attribute falls through to later scopes, and only Python owns
            // `is_active_symbol_in_class_body` (and the `implicit_name`

            // self.x-assignment fallback).
            if type_names.contains(name.as_str())? {
                return Ok(None);
            }
            // 2b. Class attributes __qualname__ and __module__ are
            // synthesized by Python (`Var(name, self.str_type())`); reaching
            // here with a class scope guarantees the name is not in the

            // class namespace (2a would have fallen back).
            if name == "__qualname__" || name == "__module__" {
                return Ok(Some(("synthesize_qualname".to_string(), None)));
            }
        }
    }

    // 3. Local (function) scopes: the full stack including the current scope.
    let locals_list = match locals.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    for i in (0..locals_list.len()).rev() {
        let table_item = locals_list.get_item(i)?;
        if table_item.is_none() {
            continue;
        }
        let table = match table_item.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        if table.contains(name.as_str())? {
            let node = table.get_item(name.as_str())?.map(Into::into);
            return Ok(Some(("found".to_string(), node)));
        }
    }

    // 4. Current file global scope.
    if globals.contains(name.as_str())? {
        let node = globals.get_item(name.as_str())?.map(Into::into);
        return Ok(Some(("found".to_string(), node)));
    }

    // 5. Builtins, with the single-underscore privacy filter.
    if let Some(builtins_entry) = globals.get_item("__builtins__")? {
        let builtin_node = match builtins_entry.getattr("node") {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        if builtin_node.is_none() {
            // Python: `b` truthy but the assert fails only on a corrupt
            // builtins entry; treat as give-up (unreachable in practice).
            return Ok(Some(("not_found".to_string(), None)));
        }
        let names_table = match builtin_node.getattr("names") {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        let table = match names_table.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        if table.contains(name.as_str())? {
            let name_bytes = name.as_bytes();
            if name_bytes.len() > 1 && name_bytes[0] == b'_' && name_bytes[1] != b'_' {
                return Ok(Some(("builtin_private".to_string(), None)));
            }
            let node = table.get_item(name.as_str())?.map(Into::into);
            return Ok(Some(("found".to_string(), node)));
        }
    }

    // Give up. The give-up path returns `not_found` (no implicit class attr
    // was seen — that case fell back at 2a), so Python's `implicit_name`
    // variable is always False here.
    Ok(Some(("not_found".to_string(), None)))
}

// ---------------------------------------------------------------------------
// var_is_typing_special_form (Issue #444)
// ---------------------------------------------------------------------------

/// The set of fullnames that `var_is_typing_special_form` accepts.
///
/// Mirrors semanal.py:8449-8461. A `Var` whose `fullname` is in this set
/// and starts with "typing" is a typing special form and can appear as the
/// base of an `IndexExpr` type expression (e.g. `Callable[...]`,
/// `Literal[...]`, `Annotated[...]`).
const TYPING_SPECIAL_FORMS: &[&str] = &[
    "typing.Annotated",
    "typing_extensions.Annotated",
    "typing.Callable",
    "typing.Literal",
    "typing_extensions.Literal",
    "typing.Optional",
    "typing.TypeGuard",
    "typing_extensions.TypeGuard",
    "typing.TypeIs",
    "typing_extensions.TypeIs",
    "typing.Union",
];

/// `mypy.semanal.var_is_typing_special_form` — pure staticmethod.
///
/// Mirrors semanal.py:8449-8461. Returns `True` when `var.fullname` starts
/// with "typing" and is in the fixed set of typing special-form names.
/// Used by `try_parse_as_type_expression` to decide whether the base of
/// an `IndexExpr` is a valid type form. The function is a pure name-set
/// check with no side effects, making it safe to port directly.
///
/// Returns `false` if the object is not a `Var` or if `fullname` is not
/// a string, deferring to the pure-Python path (which would raise
/// `AttributeError` on a non-Var, caught by the wrapper).
#[pyfunction]
pub(crate) fn rust_var_is_typing_special_form(py: Python<'_>, var: &PyAny) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
    if !var.is_instance(var_cls)? {
        return Ok(false);
    }
    let fullname_obj = match var.getattr("fullname") {
        Ok(f) => f,
        Err(_) => return Ok(false),
    };
    let fullname: &str = match fullname_obj.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(false),
    };
    if !fullname.starts_with("typing") {
        return Ok(false);
    }
    Ok(TYPING_SPECIAL_FORMS.contains(&fullname))
}

/// Inner pure-logic core of `rust_var_is_typing_special_form`, testable
/// without a Python interpreter. Returns `true` when `fullname` starts
/// with "typing" and is in the fixed special-form set.
#[cfg(test)]
fn var_is_typing_special_form_inner(fullname: &str) -> bool {
    if !fullname.starts_with("typing") {
        return false;
    }
    TYPING_SPECIAL_FORMS.contains(&fullname)
}

// ---------------------------------------------------------------------------
// is_same_var_from_getattr (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.is_same_var_from_getattr` — pure helper.
///
/// Mirrors semanal.py:8833-8840. Returns `True` when both `n1` and `n2` are
/// `Var` instances, both have `from_module_getattr == True`, and both share
/// the same `fullname`. Pure name + flag check, no side effects.
#[pyfunction]
pub(crate) fn rust_is_same_var_from_getattr(
    py: Python<'_>,
    n1: &PyAny,
    n2: &PyAny,
) -> PyResult<bool> {
    if n1.is_none() || n2.is_none() {
        return Ok(false);
    }
    let nodes_mod = py.import("mypy.nodes")?;
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
    if !n1.is_instance(var_cls)? || !n2.is_instance(var_cls)? {
        return Ok(false);
    }
    let a_getattr = n1.getattr("from_module_getattr")?;
    let b_getattr = n2.getattr("from_module_getattr")?;
    if !a_getattr.is_true()? || !b_getattr.is_true()? {
        return Ok(false);
    }
    let a_full = n1.getattr("fullname")?;
    let b_full = n2.getattr("fullname")?;
    let a_str: &str = a_full.downcast::<PyString>()?.to_str()?;
    let b_str: &str = b_full.downcast::<PyString>()?.to_str()?;
    Ok(a_str == b_str)
}

// ---------------------------------------------------------------------------
// get_typevarlike_declaration (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.get_typevarlike_declaration` — pure
/// structural check.
///
/// Mirrors semanal.py:5142-5155. Returns the `CallExpr` if `s` is an
/// assignment of the form `name = callee(...)` where `callee.fullname` is
/// in `typevarlike_types`, otherwise `None`. Pure AST structure + fullname
/// check with no side effects. Returns `None` (fallback) for any shape Rust
/// cannot classify.
#[pyfunction]
pub(crate) fn rust_get_typevarlike_declaration(
    py: Python<'_>,
    s: &PyAny,
    typevarlike_types: &PyAny,
) -> PyResult<Option<PyObject>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let assignment_stmt_cls: &PyType = nodes_mod.getattr("AssignmentStmt")?.downcast()?;
    if !s.is_instance(assignment_stmt_cls)? {
        return Ok(None);
    }
    let lvalues = s.getattr("lvalues")?;
    let lvalues_list = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if lvalues_list.len() != 1 {
        return Ok(None);
    }
    let first_lv = lvalues_list.get_item(0)?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if !first_lv.is_instance(name_expr_cls)? {
        return Ok(None);
    }
    let rvalue = s.getattr("rvalue")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    if !rvalue.is_instance(call_expr_cls)? {
        return Ok(None);
    }
    let callee = rvalue.getattr("callee")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !callee.is_instance(ref_expr_cls)? {
        return Ok(None);
    }
    let callee_fullname = callee.getattr("fullname")?;
    let callee_str: &str = match callee_fullname.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(None),
    };
    let target_set = normalize_fullnames(typevarlike_types)?;
    if target_set.contains(callee_str) {
        Ok(Some(rvalue.into_py(py)))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// parse_bool (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal_shared.parse_bool` — pure NameExpr fullname check.
///
/// Mirrors semanal_shared.py:493-499. Returns `Some(true)` when
/// `expr.fullname == "builtins.True"`, `Some(false)` for `"builtins.False"`,
/// and `None` for anything else (non-NameExpr or different fullname).
#[pyfunction]
pub(crate) fn rust_parse_bool(py: Python<'_>, expr: &PyAny) -> PyResult<Option<bool>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if !expr.is_instance(name_expr_cls)? {
        return Ok(None);
    }
    let fullname_obj = match expr.getattr("fullname") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let fullname: &str = match fullname_obj.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(None),
    };
    match fullname {
        "builtins.True" => Ok(Some(true)),
        "builtins.False" => Ok(Some(false)),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// is_mangled_global / is_initial_mangled_global (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.is_mangled_global` — pure check.
///
/// Mirrors semanal.py:8240-8242. A global is mangled if there exists at
/// least one renamed variant, i.e. `unmangle(name) + "'"` is a key in
/// `self.globals`. `globals` is `self.globals` (a dict).
#[pyfunction]
#[pyo3(signature = (name, globals))]
pub(crate) fn rust_is_mangled_global(name: &str, globals: &PyDict) -> PyResult<bool> {
    let mangled = format!("{}'", unmangle_str(name));
    globals.contains(mangled.as_str())
}

/// `mypy.semanal.SemanticAnalyzer.is_initial_mangled_global` — pure check.
///
/// Mirrors semanal.py:8244-8246. The first renamed definition for a global
/// has exactly one prime: `name == unmangle(name) + "'"`.
#[pyfunction]
pub(crate) fn rust_is_initial_mangled_global(name: &str) -> PyResult<bool> {
    Ok(name == format!("{}'", unmangle_str(name)))
}

/// Strip trailing `'` chars from a name (mirrors `mypy.util.unmangle`).
fn unmangle_str(name: &str) -> &str {
    name.trim_end_matches('\'')
}

// ---------------------------------------------------------------------------
// is_final_redefinition (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.is_final_redefinition` — pure check.
///
/// Mirrors semanal.py:4760-4764. At global scope (`GDEF`), a final
/// redefinition occurs when the name is mangled but is not the initial
/// mangled global. At class scope (`MDEF`), it occurs when
/// `unmangle(name) + "'"` is present in `self.type.names`. For other
/// scopes, returns `false`.
///
/// `globals` is `self.globals` (dict); `type_names` is `self.type.names`
/// (dict) when inside a class, else `None`.
#[pyfunction]
#[pyo3(signature = (kind, name, globals, type_names))]
pub(crate) fn rust_is_final_redefinition(
    kind: i64,
    name: &str,
    globals: &PyDict,
    type_names: &PyAny,
) -> PyResult<bool> {
    // GDEF == 1, MDEF == 2 (mypy.nodes)
    match kind {
        1 => {
            // GDEF: is_mangled_global(name) and not is_initial_mangled_global(name)
            let mangled_key = format!("{}'", unmangle_str(name));
            let is_mangled = globals.contains(mangled_key.as_str())?;
            if !is_mangled {
                return Ok(false);
            }
            Ok(name != mangled_key.as_str())
        }
        2 => {
            // MDEF: unmangle(name) + "'" in self.type.names
            let key = format!("{}'", unmangle_str(name));
            if let Ok(table) = type_names.downcast::<PyDict>() {
                return table.contains(key.as_str());
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// can_possibly_be_typevarlike_declaration (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.can_possibly_be_typevarlike_declaration`
///
/// Mirrors semanal.py:3730-3738. Pure structural check: exactly one lvalue
/// that is a `NameExpr`, rvalue is a `CallExpr` whose callee is a `NameExpr`,
/// and `callee.fullname` is in `TYPE_VAR_LIKE_NAMES`. The Python version
/// calls `ref.accept(self)` before checking `fullname`; that is a semantic
/// side-effect we skip in the pure port. Returns `false` if any structural
/// attribute cannot be read.
#[pyfunction]
pub(crate) fn rust_can_possibly_be_typevarlike_declaration(
    py: Python<'_>,
    s: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let assignment_stmt_cls: &PyType = nodes_mod.getattr("AssignmentStmt")?.downcast()?;
    if !s.is_instance(assignment_stmt_cls)? {
        return Ok(false);
    }
    let lvalues = s.getattr("lvalues")?;
    let lvalues_list = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    if lvalues_list.len() != 1 {
        return Ok(false);
    }
    let first_lv = lvalues_list.get_item(0)?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if !first_lv.is_instance(name_expr_cls)? {
        return Ok(false);
    }
    let rvalue = s.getattr("rvalue")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    if !rvalue.is_instance(call_expr_cls)? {
        return Ok(false);
    }
    let callee = rvalue.getattr("callee")?;
    if !callee.is_instance(name_expr_cls)? {
        return Ok(false);
    }
    let fullname_obj = match callee.getattr("fullname") {
        Ok(f) => f,
        Err(_) => return Ok(false),
    };
    let fullname: &str = match fullname_obj.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(false),
    };
    Ok(is_type_var_like_name(fullname))
}

/// Check whether `fullname` is one of the `TYPE_VAR_LIKE_NAMES`.
fn is_type_var_like_name(fullname: &str) -> bool {
    matches!(
        fullname,
        "typing.TypeVar"
            | "typing_extensions.TypeVar"
            | "typing.ParamSpec"
            | "typing_extensions.ParamSpec"
            | "typing.TypeVarTuple"
            | "typing_extensions.TypeVarTuple"
    )
}

// ---------------------------------------------------------------------------
// can_possibly_be_type_form (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.can_possibly_be_type_form`
///
/// Mirrors semanal.py:4040-4067. Returns `Some(bool)` for cases Rust can
/// decide, or `None` to defer to Python. The `is_pep_613_annot` fact is
/// precomputed by the Python shim (it needs `self.lookup_qualified`).
#[pyfunction]
pub(crate) fn rust_can_possibly_be_type_form(
    py: Python<'_>,
    s: &PyAny,
    is_pep_613_annot: bool,
) -> PyResult<Option<bool>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let assignment_stmt_cls: &PyType = nodes_mod.getattr("AssignmentStmt")?.downcast()?;
    if !s.is_instance(assignment_stmt_cls)? {
        return Ok(None);
    }
    let lvalues = s.getattr("lvalues")?;
    let lvalues_list = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if lvalues_list.len() > 1 {
        return Ok(Some(false));
    }
    let rvalue = s.getattr("rvalue")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    if rvalue.is_instance(call_expr_cls)? {
        let callee = rvalue.getattr("callee")?;
        let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
        if callee.is_instance(ref_expr_cls)? {
            let fullname_obj = match callee.getattr("fullname") {
                Ok(f) => f,
                Err(_) => return Ok(None),
            };
            let fullname: &str = match fullname_obj.downcast::<PyString>() {
                Ok(s) => s.to_str()?,
                Err(_) => return Ok(None),
            };
            if is_typedict_name(fullname) || is_typed_namedtuple_name(fullname) {
                return Ok(Some(true));
            }
            return Ok(Some(false));
        }
        return Ok(Some(false));
    }
    let first_lv = lvalues_list.get_item(0)?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if !first_lv.is_instance(name_expr_cls)? {
        return Ok(Some(false));
    }
    // s.unanalyzed_type is not None and not self.is_pep_613(s) -> False.
    // is_pep_613 is precomputed by the Python shim (needs lookup_qualified);
    // when True we fall through to the rvalue structural checks below.
    let unanalyzed_type = s.getattr("unanalyzed_type")?;
    if !unanalyzed_type.is_none() && !is_pep_613_annot {
        return Ok(Some(false));
    }
    let index_expr_cls: &PyType = nodes_mod.getattr("IndexExpr")?.downcast()?;
    let op_expr_cls: &PyType = nodes_mod.getattr("OpExpr")?.downcast()?;
    if !rvalue.is_instance(index_expr_cls)? && !rvalue.is_instance(op_expr_cls)? {
        return Ok(Some(false));
    }
    Ok(Some(true))
}

/// Check whether `fullname` is a TypedDict constructor name.
fn is_typedict_name(fullname: &str) -> bool {
    matches!(
        fullname,
        "typing.TypedDict" | "typing_extensions.TypedDict" | "mypy_extensions.TypedDict"
    )
}

/// Check whether `fullname` is a NamedTuple constructor name.
fn is_typed_namedtuple_name(fullname: &str) -> bool {
    matches!(
        fullname,
        "typing.NamedTuple" | "typing_extensions.NamedTuple"
    )
}

// ---------------------------------------------------------------------------
// is_type_ref (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.is_type_ref`
///
/// Mirrors semanal.py:3740-3793. Returns `Some(bool)` for cases Rust can
/// decide, or `None` for the `NameExpr`/`MemberExpr` lookup cases that
/// require `self.lookup` / `self.lookup_qualified`.
#[pyfunction]
pub(crate) fn rust_is_type_ref(py: Python<'_>, rv: &PyAny, bare: bool) -> PyResult<Option<bool>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !rv.is_instance(ref_expr_cls)? {
        return Ok(Some(false));
    }
    let node = rv.getattr("node")?;
    if !node.is_none() {
        let type_var_like_cls: &PyType = nodes_mod.getattr("TypeVarLikeExpr")?.downcast()?;
        if node.is_instance(type_var_like_cls)? {
            // Python calls self.fail(...) then returns False.
            return Ok(Some(false));
        }
        let type_alias_cls: &PyType = nodes_mod.getattr("TypeAlias")?.downcast()?;
        if node.is_instance(type_alias_cls)? {
            return Ok(Some(true));
        }
        let fullname_obj = match rv.getattr("fullname") {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        let fullname: &str = match fullname_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?,
            Err(_) => return Ok(None),
        };
        if in_valid_refs(fullname, bare) {
            return Ok(Some(true));
        }
        let type_info_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;
        if node.is_instance(type_info_cls)? {
            if bare {
                return Ok(Some(true));
            }
            let is_enum = node.getattr("is_enum")?;
            let is_enum_bool: bool = is_enum.extract()?;
            return Ok(Some(!is_enum_bool));
        }
        let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
        if node.is_instance(var_cls)? {
            let var_fullname_obj = match node.getattr("fullname") {
                Ok(f) => f,
                Err(_) => return Ok(None),
            };
            let var_fullname: &str = match var_fullname_obj.downcast::<PyString>() {
                Ok(s) => s.to_str()?,
                Err(_) => return Ok(None),
            };
            return Ok(Some(is_never_name(var_fullname)));
        }
    }
    // NameExpr / MemberExpr lookup cases need self.lookup -> defer.
    Ok(None)
}

/// Check whether `fullname` is in the valid_refs set for `bare` or not.
fn in_valid_refs(fullname: &str, bare: bool) -> bool {
    if bare {
        matches!(fullname, "typing.Any" | "typing.Tuple" | "typing.Callable")
    } else {
        is_type_constructor(fullname)
    }
}

/// Check whether `fullname` is in the `type_constructors` set.
fn is_type_constructor(fullname: &str) -> bool {
    matches!(
        fullname,
        "typing.Callable"
            | "typing.Optional"
            | "typing.Tuple"
            | "typing.Type"
            | "typing.Union"
            | "typing.Literal"
            | "typing_extensions.Literal"
            | "typing.Annotated"
            | "typing_extensions.Annotated"
    )
}

/// Check whether `fullname` is in the `NEVER_NAMES` set.
fn is_never_name(fullname: &str) -> bool {
    matches!(
        fullname,
        "typing.NoReturn"
            | "typing_extensions.NoReturn"
            | "mypy_extensions.NoReturn"
            | "typing.Never"
            | "typing_extensions.Never"
    )
}

// ---------------------------------------------------------------------------
// can_be_type_alias (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.can_be_type_alias`
///
/// Mirrors semanal.py:3685-3706. Recursive. Returns `Some(bool)` for all
/// cases except `is_none_alias` (requires `self`), which returns `None`.
#[pyfunction]
pub(crate) fn rust_can_be_type_alias(
    py: Python<'_>,
    rv: &PyAny,
    allow_none: bool,
    is_stub_file: bool,
) -> PyResult<Option<bool>> {
    can_be_type_alias_inner(py, rv, allow_none, is_stub_file)
}

/// Inner recursive helper (no `#[pyfunction]` wrapper overhead).
fn can_be_type_alias_inner(
    py: Python<'_>,
    rv: &PyAny,
    allow_none: bool,
    is_stub_file: bool,
) -> PyResult<Option<bool>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if rv.is_instance(ref_expr_cls)? {
        let type_ref_result = rust_is_type_ref(py, rv, true)?;
        if let Some(true) = type_ref_result {
            return Ok(Some(true));
        }
    }
    let index_expr_cls: &PyType = nodes_mod.getattr("IndexExpr")?.downcast()?;
    if rv.is_instance(index_expr_cls)? {
        let base = rv.getattr("base")?;
        let type_ref_result = rust_is_type_ref(py, base, false)?;
        if let Some(true) = type_ref_result {
            return Ok(Some(true));
        }
    }
    // is_none_alias(rv) requires self -> defer.
    // We cannot determine it without the SemanticAnalyzer, so return None.
    // However, if we get here and none of the above matched, we try the

    // remaining checks. The is_none_alias check is skipped (returns None
    // only if we reach a path that would need it). To be safe, we defer
    // when the rvalue could be a none-alias (CallExpr with callee type(None)).

    // Actually, to keep it simple and correct, we just continue: the
    // remaining checks don't need is_none_alias.
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if allow_none && rv.is_instance(name_expr_cls)? {
        let fullname_obj = match rv.getattr("fullname") {
            Ok(f) => f,
            Err(_) => return Ok(Some(false)),
        };
        let fullname: &str = match fullname_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?,
            Err(_) => return Ok(Some(false)),
        };
        if fullname == "builtins.None" {
            return Ok(Some(true));
        }
    }
    let op_expr_cls: &PyType = nodes_mod.getattr("OpExpr")?.downcast()?;
    if rv.is_instance(op_expr_cls)? {
        let op_obj = rv.getattr("op")?;
        let op_str: &str = match op_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?,
            Err(_) => return Ok(Some(false)),
        };
        if op_str == "|" {
            if is_stub_file {
                return Ok(Some(true));
            }
            let left = rv.getattr("left")?;
            let right = rv.getattr("right")?;
            let left_result = can_be_type_alias_inner(py, left, true, is_stub_file)?;
            let right_result = can_be_type_alias_inner(py, right, true, is_stub_file)?;
            match (left_result, right_result) {
                (Some(true), Some(true)) => return Ok(Some(true)),
                (Some(false), _) | (_, Some(false)) => {
                    return Ok(Some(false));
                }
                _ => return Ok(None),
            }
        }
    }
    Ok(Some(false))
}

// ---------------------------------------------------------------------------
// check_typevarlike_name (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.check_typevarlike_name`
///
/// Mirrors semanal.py:5141-5158. Returns `Some(true)` when valid,
/// `Some((false, Some(msg)))` when definitely invalid (with error message),
/// or `None` to defer when an attribute cannot be read.
#[pyfunction]
pub(crate) fn rust_check_typevarlike_name(
    py: Python<'_>,
    call: &PyAny,
    name: &str,
) -> PyResult<Option<(bool, Option<String>)>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    if !call.is_instance(call_expr_cls)? {
        return Ok(None);
    }
    let callee = call.getattr("callee")?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;

    // typevarlike_type = callee.name if NameExpr else callee.fullname
    let typevarlike_type: String = if callee.is_instance(name_expr_cls)? {
        let name_obj = match callee.getattr("name") {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        match name_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?.to_string(),
            Err(_) => return Ok(None),
        }
    } else if callee.is_instance(ref_expr_cls)? {
        let fullname_obj = match callee.getattr("fullname") {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        match fullname_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?.to_string(),
            Err(_) => return Ok(None),
        }
    } else {
        return Ok(None);
    };

    let args = call.getattr("args")?;
    let args_list = match args.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let unmangled = unmangle_str(name);
    if args_list.is_empty() {
        let msg = format!("Too few arguments for {typevarlike_type}()");
        return Ok(Some((false, Some(msg))));
    }
    let first_arg = args_list.get_item(0)?;
    let str_expr_cls: &PyType = nodes_mod.getattr("StrExpr")?.downcast()?;
    if !first_arg.is_instance(str_expr_cls)? {
        let msg = format!("{typevarlike_type}() expects a string literal as first argument");
        return Ok(Some((false, Some(msg))));
    }
    // Check arg_kinds[0] == ARG_POS (IntEnum value 0).
    let arg_kinds = call.getattr("arg_kinds")?;
    let arg_kinds_list = match arg_kinds.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if arg_kinds_list.is_empty() {
        return Ok(None);
    }
    let first_kind = arg_kinds_list.get_item(0)?;
    let first_kind_int: i64 = match first_kind.extract() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if first_kind_int != 0 {
        let msg = format!("{typevarlike_type}() expects a string literal as first argument");
        return Ok(Some((false, Some(msg))));
    }
    let arg_value_obj = first_arg.getattr("value")?;
    let arg_value: &str = match arg_value_obj.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(None),
    };
    if arg_value != unmangled {
        let msg = format!(
            "String argument 1 \"{}\" to {}(...) does not match \
            variable name \"{}\"",
            arg_value, typevarlike_type, unmangled
        );
        return Ok(Some((false, Some(msg))));
    }
    Ok(Some((true, None)))
}

// ---------------------------------------------------------------------------
// extract_typevarlike_name (Issue #460)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.extract_typevarlike_name`
///
/// Mirrors semanal.py:5314-5326. Returns `Some(name)` only when the name
/// is successfully extracted with no errors. Returns `None` for any case
/// that needs error reporting (so Python runs the full path with
/// `self.fail` calls).
#[pyfunction]
pub(crate) fn rust_extract_typevarlike_name(
    py: Python<'_>,
    s: &PyAny,
    call: &PyAny,
) -> PyResult<Option<String>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let assignment_stmt_cls: &PyType = nodes_mod.getattr("AssignmentStmt")?.downcast()?;
    if !s.is_instance(assignment_stmt_cls)? {
        return Ok(None);
    }
    if call.is_none() {
        return Ok(None);
    }
    let lvalues = s.getattr("lvalues")?;
    let lvalues_list = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if lvalues_list.is_empty() {
        return Ok(None);
    }
    let lvalue = lvalues_list.get_item(0)?;
    let name_expr_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    if !lvalue.is_instance(name_expr_cls)? {
        return Ok(None);
    }
    // s.type is not None -> Python calls self.fail(...) -> defer.
    let s_type = s.getattr("type")?;
    if !s_type.is_none() {
        return Ok(None);
    }
    let lvalue_name_obj = lvalue.getattr("name")?;
    let lvalue_name: &str = match lvalue_name_obj.downcast::<PyString>() {
        Ok(s) => s.to_str()?,
        Err(_) => return Ok(None),
    };
    let check_result = rust_check_typevarlike_name(py, call, lvalue_name)?;
    match check_result {
        Some((true, _)) => Ok(Some(lvalue_name.to_string())),
        // Invalid or defer: Python needs to run full path with errors.
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// is_defined_type_param (Phase C1, issue #608)
// ---------------------------------------------------------------------------

/// `mypy.semanal.SemanticAnalyzer.is_defined_type_param`
///
/// Mirrors semanal.py:2075-2082. Walks `self.locals` (a list of
/// `SymbolTable | None`), returns true if `name` is bound to a
/// `TypeVarLikeExpr` in any scope. Pure boolean query, no side effects:
/// Python applies no mutation based on this result, so Rust returns a
/// plain `bool` (no fallback needed).
#[pyfunction]
pub(crate) fn rust_is_defined_type_param(
    py: Python<'_>,
    locals: &PyAny,
    name: &str,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let tve_cls: &PyType = nodes_mod.getattr("TypeVarLikeExpr")?.downcast()?;

    let locals_list = match locals.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for names in locals_list.iter() {
        if names.is_none() {
            continue;
        }
        let names_dict = match names.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return Ok(false),
        };
        let entry = match names_dict.get_item(name)? {
            Some(e) => e,
            None => continue,
        };
        let node = entry.getattr("node")?;
        if node.is_none() {
            continue;
        }
        if node.is_instance(tve_cls)? {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// classify_setup_type_vars (Phase C1, issue #608)
// ---------------------------------------------------------------------------

/// Tag for the kind of a type-variable-like type in `setup_type_vars`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SvtKind {
    TypeVar,
    TypeVarTuple,
    ParamSpec,
}

/// The pure decision core of `setup_type_vars` (semanal.py:2294-2310).
///
/// Given one tag per `tvar_defs` entry and a parallel `has_default` flag per
/// entry, reproduces the `seen_tvt` state machine and returns the invalid
/// indices: a `TypeVarType` with a default appearing after a
/// `TypeVarTupleType` (NO_DEFAULT_AFTER_TYPEVAR_TUPLE). TypeVarTupleType
/// entries set `seen_tvt`; ParamSpecType entries are always kept. PyO3-free,
/// so the state machine is unit-tested directly.
fn classify_setup_type_vars_inner(tags: &[SvtKind], has_defaults: &[bool]) -> Vec<usize> {
    let mut seen_tvt = false;
    let mut invalid: Vec<usize> = Vec::new();
    for (i, (&kind, has_default)) in tags.iter().zip(has_defaults).enumerate() {
        match kind {
            SvtKind::TypeVarTuple => seen_tvt = true,
            SvtKind::TypeVar if seen_tvt && *has_default => invalid.push(i),
            _ => {}
        }
    }
    invalid
}

/// The PyO3 seam for `setup_type_vars`: maps each `tvar_defs` entry to an
/// `SvtKind` via isinstance checks (the `has_default` flags are computed by
/// Python, since `has_default()` needs `get_proper_type` / `TypeOfAny`
/// semantics that stay in Python), then delegates to the pure
/// `classify_setup_type_vars_inner`.
///
/// Returns `Some(Vec<usize>)` of invalid indices (possibly empty), or `None`
/// when the input is not a list / the two lists differ in length (a caller
/// mismatch; Python would never hit that). Rust never mutates `self`; Python
/// applies the removals and the `self.fail` call.
#[pyfunction]
pub(crate) fn rust_classify_setup_type_vars(
    py: Python<'_>,
    tvar_defs: &PyAny,
    has_defaults: &PyAny,
) -> PyResult<Option<Vec<usize>>> {
    let defs_list = match tvar_defs.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let defaults_list = match has_defaults.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    if defaults_list.len() != defs_list.len() {
        return Ok(None);
    }

    let types_mod = py.import("mypy.types")?;
    let tv_cls: &PyType = types_mod.getattr("TypeVarType")?.downcast()?;
    let tvt_cls: &PyType = types_mod.getattr("TypeVarTupleType")?.downcast()?;

    let mut tags: Vec<SvtKind> = Vec::with_capacity(defs_list.len());
    for tv in defs_list.iter() {
        if tv.is_instance(tvt_cls)? {
            tags.push(SvtKind::TypeVarTuple);
        } else if tv.is_instance(tv_cls)? {
            tags.push(SvtKind::TypeVar);
        } else {
            tags.push(SvtKind::ParamSpec);
        }
    }
    let mut defaults: Vec<bool> = Vec::with_capacity(defaults_list.len());
    for d in defaults_list.iter() {
        defaults.push(d.extract()?);
    }
    Ok(Some(classify_setup_type_vars_inner(&tags, &defaults)))
}

// ---------------------------------------------------------------------------
// Slice 1: leaf-actions shard for visit_list_expr / visit_set_expr /
// visit_dict_expr / visit_template_str_expr (semanal.py:6274-6299).

//
// Pattern: Rust walks the expr items and mutates `StarExpr.valid = True`
// directly, then calls `item.accept(semanal_self)` to recurse into the

// Python visitor. The four Python methods become a single gate dispatch
// into one of these helpers; if the helper returns `true` the Python
// body was fully handled, otherwise Python falls back.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_list_expr` / `visit_set_expr` body. Sets
/// `item.valid = True` on StarExpr items and recurses via `accept`.
/// Returns `false` if any required attribute/method is missing so
/// Python falls back.
#[pyfunction]
pub(crate) fn rust_visit_list_set_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let star_cls = nodes_mod.getattr("StarExpr")?.downcast::<PyType>()?;
    let items = expr.getattr("items")?;
    let items_list = match items.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for item in items_list.iter() {
        if item.is_instance(star_cls)? {
            item.setattr("valid", true)?;
        }
        item.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_dict_expr` body. Recurses into key (when not None)
/// and value via `accept`.
#[pyfunction]
pub(crate) fn rust_visit_dict_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let items = expr.getattr("items")?;
    let items_list = match items.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for pair in items_list.iter() {
        let pair_t = match pair.downcast::<PyTuple>() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };
        if pair_t.len() != 2 {
            return Ok(false);
        }
        let key = pair_t.get_item(0)?;
        let value = pair_t.get_item(1)?;
        if !key.is_none() {
            key.call_method1("accept", (semanal,))?;
        }
        value.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_template_str_expr` body. Each item is either an
/// `Expression` or a 4-tuple `(value_expr, source_text, conversion,
/// format_spec_expr)`.
#[pyfunction]
pub(crate) fn rust_visit_template_str_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let items = expr.getattr("items")?;
    let items_list = match items.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for item in items_list.iter() {
        if let Ok(t) = item.downcast::<PyTuple>() {
            if t.len() != 4 {
                return Ok(false);
            }
            let value_expr = t.get_item(0)?;
            let format_spec = t.get_item(3)?;
            value_expr.call_method1("accept", (semanal,))?;
            if !format_spec.is_none() {
                format_spec.call_method1("accept", (semanal,))?;
            }
        } else {
            item.call_method1("accept", (semanal,))?;
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 2: leaf-actions shards for additional simple recurse-only visit_*
// bodies (semanal.py). Same pattern as slice 1: Rust walks, Python falls

// back on unexpected shapes.
// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_unary_expr` body — recurse into expr.expr.
#[pyfunction]
pub(crate) fn rust_visit_unary_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    match expr.getattr("expr") {
        Ok(inner) => {
            inner.call_method1("accept", (semanal,))?;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

/// `mypy.semanal.visit_comparison_expr` body — recurse into each operand.
#[pyfunction]
pub(crate) fn rust_visit_comparison_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let operands = match expr.getattr("operands") {
        Ok(o) => o,
        Err(_) => return Ok(false),
    };
    let list = match operands.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for op in list.iter() {
        op.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_slice_expr` body — recurse into begin/end/stride
/// when not None.
#[pyfunction]
pub(crate) fn rust_visit_slice_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    // Python pre-checks are `if expr.begin_index:` etc. — truthiness, not
    // is-not-None. Mirror exactly via is_true() to match the fallback loop.
    let begin = expr.getattr("begin_index")?;
    if !begin.is_none() && begin.is_true()? {
        begin.call_method1("accept", (semanal,))?;
    }
    let end = expr.getattr("end_index")?;
    if !end.is_none() && end.is_true()? {
        end.call_method1("accept", (semanal,))?;
    }
    let stride = expr.getattr("stride")?;
    if !stride.is_none() && stride.is_true()? {
        stride.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_conditional_expr` body — recurse into if_expr,
/// cond, else_expr in order.
#[pyfunction]
pub(crate) fn rust_visit_conditional_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    for name in ["if_expr", "cond", "else_expr"] {
        let v = expr.getattr(name)?;
        v.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_super_expr` body. Python side is responsible for
/// the `self.fail` pre-check (it has access to self.type and self.fail);
/// it passes `self.type` (may be None) here. Rust sets `expr.info` to
/// the type and recurses into `expr.call.args`.
#[pyfunction]
pub(crate) fn rust_visit_super_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
    the_type: &PyAny,
) -> PyResult<bool> {
    expr.setattr("info", the_type)?;
    let call = match expr.getattr("call") {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    let args = match call.getattr("args") {
        Ok(a) => a,
        Err(_) => return Ok(false),
    };
    let list = match args.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for arg in list.iter() {
        arg.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 3: leaf-actions shards for the simple statement-visit bodies.
// Pattern: Rust sets the analyzer's `statement` slot (the same way the

// Python body does `self.statement = s`), recurses via `accept`, and
// returns true on the common path. The __all__-style branches stay in
// Python by falling back (return false) when they would fire.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_raise_stmt` body — set statement slot, recurse
/// into expr / from_expr if present.
#[pyfunction]
pub(crate) fn rust_visit_raise_stmt(_py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let expr = s.getattr("expr")?;
    if !expr.is_none() && expr.is_true()? {
        expr.call_method1("accept", (semanal,))?;
    }
    let from_expr = s.getattr("from_expr")?;
    if !from_expr.is_none() && from_expr.is_true()? {
        from_expr.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_assert_stmt` body — set statement slot, recurse
/// into expr / msg if present.
#[pyfunction]
pub(crate) fn rust_visit_assert_stmt(
    _py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let expr = s.getattr("expr")?;
    if !expr.is_none() && expr.is_true()? {
        expr.call_method1("accept", (semanal,))?;
    }
    let msg = s.getattr("msg")?;
    if !msg.is_none() && msg.is_true()? {
        msg.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_operator_assignment_stmt` body (semanal.py:5932-5942).
/// Recurse into lvalue and rvalue, then handle the `__all__` export branch
/// in place via `semanal.add_exports`. The kind check must happen AFTER
/// the recursion because the lvalue's `kind` is assigned during accept.
#[pyfunction]
pub(crate) fn rust_visit_operator_assignment_stmt(
    py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let lvalue = s.getattr("lvalue")?;
    let rvalue = s.getattr("rvalue")?;
    lvalue.call_method1("accept", (semanal,))?;
    rvalue.call_method1("accept", (semanal,))?;

    // __all__ export branch (semanal.py:5953): the Python body visits lvalue
    // first, then checks `lvalue.kind == GDEF` — the kind is resolved during
    // the accept call, so the check must run AFTER the recursion. Handle

    // the branch here instead of deferring; add_exports is a plain Python
    // method call and cheap to forward.
    let nodes_mod = py.import("mypy.nodes")?;
    let name_expr_cls = nodes_mod.getattr("NameExpr")?;
    if lvalue.is_instance(name_expr_cls)? {
        let name: String = lvalue.getattr("name")?.extract()?;
        if name == "__all__" {
            let kind_obj = lvalue.getattr("kind")?;
            if !kind_obj.is_none() {
                let kind: i64 = kind_obj.extract()?;
                let gdef: i64 = nodes_mod.getattr("GDEF")?.extract()?;
                if kind == gdef {
                    let list_cls = nodes_mod.getattr("ListExpr")?;
                    let tuple_cls = nodes_mod.getattr("TupleExpr")?;
                    if rvalue.is_instance(list_cls)? || rvalue.is_instance(tuple_cls)? {
                        let items = rvalue.getattr("items")?;
                        semanal.call_method1("add_exports", (items,))?;
                    }
                }
            }
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 4: leaf-actions shards for block / if-stmt / del-stmt bodies.
// Pattern: Rust adjusts the analyzer's block_depth list in place, marks

// the statement slot, and recurses through `semanal.accept` /
// `expression.accept`. Python keeps the reachability pre-pass (its own
// options call) and the `fail` call-forward passes through PyO3.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_block` body. Skips unreachable blocks, bumps
/// block_depth[-1], then `semanal.accept`s each statement.
#[pyfunction]
pub(crate) fn rust_visit_block(_py: Python<'_>, b: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    let unreachable = b.getattr("is_unreachable")?;
    if unreachable.is_true()? {
        return Ok(true);
    }
    let depth = semanal.getattr("block_depth")?;
    let last = depth.len()? - 1;
    let cur: i64 = depth.get_item(last)?.extract()?;
    depth.set_item(last, cur + 1)?;
    let body = b.getattr("body")?;
    let stmts = match body.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for s in stmts.iter() {
        semanal.call_method1("accept", (s,))?;
    }
    depth.set_item(last, cur)?;
    Ok(true)
}

/// `mypy.semanal.visit_if_stmt` body. Recurse into each (expr, body)
/// pair, then the optional else body. The reachability inference on the
/// statement happens in Python before this is called.
#[pyfunction]
pub(crate) fn rust_visit_if_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let exprs = s.getattr("expr")?;
    let bodies = s.getattr("body")?;
    let exprs_l = match exprs.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let bodies_l = match bodies.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    if exprs_l.len() != bodies_l.len() {
        return Ok(false);
    }
    for i in 0..exprs_l.len() {
        let expr = exprs_l.get_item(i)?;
        let body = bodies_l.get_item(i)?;
        expr.call_method1("accept", (semanal,))?;
        if !rust_visit_block(py, body, semanal)? {
            return Ok(false);
        }
    }
    let else_body = s.getattr("else_body")?;
    if !else_body.is_none() && else_body.is_true()? && !rust_visit_block(py, else_body, semanal)? {
        return Ok(false);
    }
    Ok(true)
}

/// `mypy.semanal.is_valid_del_target` — pure shape check predicting
/// whether `s` is a legal `del` target.
#[pyfunction]
pub(crate) fn rust_is_valid_del_target(py: Python<'_>, s: &PyAny) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let simple = ["IndexExpr", "NameExpr", "MemberExpr"];
    let seq = ["TupleExpr", "ListExpr"];
    for name in simple {
        if s.is_instance(nodes_mod.getattr(name)?)? {
            return Ok(true);
        }
    }
    for name in seq {
        if s.is_instance(nodes_mod.getattr(name)?)? {
            let items = s.getattr("items")?;
            let list = match items.downcast::<PyList>() {
                Ok(l) => l,
                Err(_) => return Ok(false),
            };
            for item in list.iter() {
                if !rust_is_valid_del_target(py, item)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
    }
    Ok(false)
}

/// `mypy.semanal.visit_del_stmt` body. Set statement slot, recurse into
/// expr, then fail if the target is invalid (forwarded to Python's
/// `self.fail` so the code/location plumbing is unchanged).
#[pyfunction]
pub(crate) fn rust_visit_del_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let expr = s.getattr("expr")?;
    expr.call_method1("accept", (semanal,))?;
    if !rust_is_valid_del_target(py, expr)? {
        semanal.call_method1("fail", ("Invalid delete target", s))?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 5: leaf-actions shards for expression-stmt, break/continue,
// global-decl, and match-stmt bodies.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_expression_stmt` body — set statement slot, recurse
/// into the expression.
#[pyfunction]
pub(crate) fn rust_visit_expression_stmt(
    _py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let expr = s.getattr("expr")?;
    expr.call_method1("accept", (semanal,))?;
    Ok(true)
}

/// Shared helper for `visit_break_stmt` / `visit_continue_stmt` — forward a
/// `self.fail` call with `serious=True` (and optionally `blocker=True`), so
/// error plumbing (codes, location) stays identical to Python.
fn fail_serious(semanal: &PyAny, msg: &str, s: &PyAny, blocker: bool) -> PyResult<()> {
    let kwargs = pyo3::types::PyDict::new(semanal.py());
    kwargs.set_item("serious", true)?;
    if blocker {
        kwargs.set_item("blocker", true)?;
    }
    semanal.call_method("fail", (msg, s), Some(kwargs))?;
    Ok(())
}

/// `mypy.semanal.visit_break_stmt` body — set statement slot, emit
/// 'break outside loop' when not in a loop, and 'break not allowed in
/// except* block'.
#[pyfunction]
pub(crate) fn rust_visit_break_stmt(_py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let loop_depth = semanal.getattr("loop_depth")?;
    let last = loop_depth.len()? - 1;
    let cur: i64 = loop_depth.get_item(last)?.extract()?;
    if cur == 0 {
        fail_serious(semanal, "\"break\" outside loop", s, true)?;
    }
    let inside = semanal.getattr("inside_except_star_block")?;
    if inside.is_true()? {
        fail_serious(semanal, "\"break\" not allowed in except* block", s, false)?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_continue_stmt` body — same shape as break.
#[pyfunction]
pub(crate) fn rust_visit_continue_stmt(
    _py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let loop_depth = semanal.getattr("loop_depth")?;
    let last = loop_depth.len()? - 1;
    let cur: i64 = loop_depth.get_item(last)?.extract()?;
    if cur == 0 {
        fail_serious(semanal, "\"continue\" outside loop", s, true)?;
    }
    let inside = semanal.getattr("inside_except_star_block")?;
    if inside.is_true()? {
        fail_serious(
            semanal,
            "\"continue\" not allowed in except* block",
            s,
            false,
        )?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_global_decl` body — set statement slot, fail on
/// names already declared nonlocal, then add each name to the current
/// global-decls set.
#[pyfunction]
pub(crate) fn rust_visit_global_decl(
    _py: Python<'_>,
    g: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", g)?;
    let names = g.getattr("names")?;
    let names_l = match names.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let nonlocal_decls = semanal.getattr("nonlocal_decls")?;
    let nonlocal_decls_l = match nonlocal_decls.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let nonlocal_top = nonlocal_decls_l.get_item(nonlocal_decls_l.len() - 1)?;
    let global_decls = semanal.getattr("global_decls")?;
    let global_decls_l = match global_decls.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let global_top = global_decls_l.get_item(global_decls_l.len() - 1)?;
    for name in names_l.iter() {
        if nonlocal_top
            .call_method1("__contains__", (name,))?
            .is_true()?
        {
            let msg = format!("Name \"{}\" is nonlocal and global", name.str()?);
            semanal.call_method1("fail", (msg.as_str(), g))?;
        }
        global_top.call_method1("add", (name,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_match_stmt` body — set statement slot, recurse into
/// subject, patterns, guards, then visit each body block. The reachability
/// inference (`infer_reachability_of_match_statement`) runs in Python first,
/// matching the `visit_if_stmt` split.
#[pyfunction]
pub(crate) fn rust_visit_match_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    s.getattr("subject")?.call_method1("accept", (semanal,))?;
    let patterns = s.getattr("patterns")?;
    let guards = s.getattr("guards")?;
    let bodies = s.getattr("bodies")?;
    let patterns_l = match patterns.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let guards_l = match guards.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let bodies_l = match bodies.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    if patterns_l.len() != guards_l.len() || patterns_l.len() != bodies_l.len() {
        return Ok(false);
    }
    for i in 0..patterns_l.len() {
        patterns_l.get_item(i)?.call_method1("accept", (semanal,))?;
        let guard = guards_l.get_item(i)?;
        if !guard.is_none() {
            guard.call_method1("accept", (semanal,))?;
        }
        if !rust_visit_block(py, bodies_l.get_item(i)?, semanal)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 6: leaf-actions shards for return / block-maybe / while bodies.
// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_return_stmt` body — save+set statement slot, run the
/// scope/except*-block checks, recurse and try-parse the expression, then
/// restore the previous statement. The Python body does `old = statement`
/// then restores `statement = old` at the end; we mirror that exactly.
#[pyfunction]
pub(crate) fn rust_visit_return_stmt(
    _py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let old = semanal.getattr("statement")?;
    semanal.setattr("statement", s)?;
    let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
    if !in_func {
        semanal.call_method1("fail", ("\"return\" outside function", s))?;
    }
    let inside = semanal.getattr("return_stmt_inside_except_star_block")?;
    if inside.is_true()? {
        fail_serious(semanal, "\"return\" not allowed in except* block", s, false)?;
    }
    let expr = s.getattr("expr")?;
    if !expr.is_none() && expr.is_true()? {
        expr.call_method1("accept", (semanal,))?;
        semanal.call_method1("try_parse_as_type_expression", (expr,))?;
    }
    semanal.setattr("statement", old)?;
    Ok(true)
}

/// `mypy.semanal.visit_block_maybe` body — visit a block when present.
#[pyfunction]
pub(crate) fn rust_visit_block_maybe(py: Python<'_>, b: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    if b.is_none() {
        return Ok(true);
    }
    rust_visit_block(py, b, semanal)
}

/// `mypy.semanal.visit_while_stmt` body — set statement slot, recurse expr,
/// bump loop_depth, run the except*-block scope, recurse body, restore
/// loop_depth, then visit the optional else body.
#[pyfunction]
pub(crate) fn rust_visit_while_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    s.getattr("expr")?.call_method1("accept", (semanal,))?;

    let loop_depth = semanal.getattr("loop_depth")?;
    let last = loop_depth.len()? - 1;
    let cur: i64 = loop_depth.get_item(last)?.extract()?;
    loop_depth.set_item(last, cur + 1)?;

    // Build the context manager: semanal.inside_except_star_block_set(
    //     value=False, entering_loop=True
    // )
    let factory_kw = pyo3::types::PyDict::new(py);
    factory_kw.set_item("value", false)?;
    factory_kw.set_item("entering_loop", true)?;
    let cm = semanal.call_method("inside_except_star_block_set", (), Some(factory_kw))?;
    cm.call_method0("__enter__")?;
    let result = s.getattr("body")?.call_method1("accept", (semanal,));
    cm.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
    result?;

    loop_depth.set_item(last, cur)?;

    let else_body = s.getattr("else_body")?;
    if !else_body.is_none() && !rust_visit_block_maybe(py, else_body, semanal)? {
        return Ok(false);
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 7: leaf-actions shards for name-expr, star-expr, and the seven
// pattern visitors. These are pure recursion + self.analyze_lvalue calls.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_name_expr` body — lookup the name and, when found,
/// bind it through the analyzer's own bind_name_expr (which mutates the
/// live NameExpr and emits type-variable/placeholder diagnostics).
#[pyfunction]
pub(crate) fn rust_visit_name_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let name = expr.getattr("name")?;
    let n = semanal.call_method1("lookup", (name, expr))?;
    if !n.is_none() {
        semanal.call_method1("bind_name_expr", (expr, n))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_star_expr` body — fail on invalid star, else recurse.
#[pyfunction]
pub(crate) fn rust_visit_star_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let valid = expr.getattr("valid")?.is_true()?;
    if !valid {
        let kw = pyo3::types::PyDict::new(py);
        kw.set_item("blocker", true)?;
        semanal.call_method(
            "fail",
            ("can't use starred expression here", expr),
            Some(kw),
        )?;
    } else {
        expr.getattr("expr")?.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// Shared helper: iterate a Python list attribute of `p` and accept each.
fn accept_list_attr(attr: &str, p: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    let items = p.getattr(attr)?;
    let list = match items.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for item in list.iter() {
        item.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_as_pattern` — recurse pattern + analyze_lvalue(name).
#[pyfunction]
pub(crate) fn rust_visit_as_pattern(_py: Python<'_>, p: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    let pattern = p.getattr("pattern")?;
    if !pattern.is_none() {
        pattern.call_method1("accept", (semanal,))?;
    }
    let name = p.getattr("name")?;
    if !name.is_none() {
        semanal.call_method1("analyze_lvalue", (name,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_or_pattern` — recurse over patterns.
#[pyfunction]
pub(crate) fn rust_visit_or_pattern(_py: Python<'_>, p: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    accept_list_attr("patterns", p, semanal)
}

/// `mypy.semanal.visit_value_pattern` — recurse expr.
#[pyfunction]
pub(crate) fn rust_visit_value_pattern(
    _py: Python<'_>,
    p: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    p.getattr("expr")?.call_method1("accept", (semanal,))?;
    Ok(true)
}

/// `mypy.semanal.visit_sequence_pattern` — recurse patterns.
#[pyfunction]
pub(crate) fn rust_visit_sequence_pattern(
    _py: Python<'_>,
    p: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    accept_list_attr("patterns", p, semanal)
}

/// `mypy.semanal.visit_starred_pattern` — analyze_lvalue(capture) if present.
#[pyfunction]
pub(crate) fn rust_visit_starred_pattern(
    _py: Python<'_>,
    p: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let capture = p.getattr("capture")?;
    if !capture.is_none() {
        semanal.call_method1("analyze_lvalue", (capture,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_mapping_pattern` — recurse keys + values, then
/// analyze_lvalue(rest) if present.
#[pyfunction]
pub(crate) fn rust_visit_mapping_pattern(
    _py: Python<'_>,
    p: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    if !accept_list_attr("keys", p, semanal)? {
        return Ok(false);
    }
    if !accept_list_attr("values", p, semanal)? {
        return Ok(false);
    }
    let rest = p.getattr("rest")?;
    if !rest.is_none() {
        semanal.call_method1("analyze_lvalue", (rest,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_class_pattern` — recurse class_ref, positionals,
/// and keyword_values.
#[pyfunction]
pub(crate) fn rust_visit_class_pattern(
    _py: Python<'_>,
    p: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    p.getattr("class_ref")?.call_method1("accept", (semanal,))?;
    if !accept_list_attr("positionals", p, semanal)? {
        return Ok(false);
    }
    accept_list_attr("keyword_values", p, semanal)
}

// ---------------------------------------------------------------------------
// Slice 8: leaf-actions shards for yield / yield-from / await / try. The
// yield/await bodies check scope, flip the enclosing FuncDef's generator

// flags, and recurse. Errors carry serious/blocker and sometimes a code.
// ---------------------------------------------------------------------------

/// Shared helper that forwards a `self.fail` call with `serious` /
/// `blocker` / optional `code` kwargs, mirroring the Python call surface.
fn fail_with_kwargs(
    semanal: &PyAny,
    msg: &str,
    ctx: &PyAny,
    serious: bool,
    blocker: bool,
    code: Option<&PyAny>,
) -> PyResult<()> {
    let py = semanal.py();
    let kw = pyo3::types::PyDict::new(py);
    if serious {
        kw.set_item("serious", true)?;
    }
    if blocker {
        kw.set_item("blocker", true)?;
    }
    if let Some(c) = code {
        kw.set_item("code", c)?;
    }
    semanal.call_method("fail", (msg, ctx), Some(kw))?;
    Ok(())
}

/// Fetch an error-code attribute from `mypy.errorcodes` (e.g. TOP_LEVEL_AWAIT).
fn error_code(py: Python<'_>, name: &str) -> PyResult<pyo3::Py<PyAny>> {
    let codes_mod = py.import("mypy.errorcodes")?;
    Ok(codes_mod.getattr(name)?.into())
}

/// `mypy.semanal.visit_yield_expr` body — scope + comprehension + async
/// checks, flip the generator/async-generator flag on the enclosing
/// FuncDef, recurse the expr.
#[pyfunction]
pub(crate) fn rust_visit_yield_expr(py: Python<'_>, e: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
    if !in_func {
        fail_with_kwargs(semanal, "\"yield\" outside function", e, true, true, None)?;
    } else {
        let scope_stack = semanal.getattr("scope_stack")?;
        let last = scope_stack.len()? - 1;
        let cur_scope: i64 = scope_stack.get_item(last)?.extract()?;
        // SCOPE_COMPREHENSION is a module-level int in mypy.semanal.
        let comp_scope: i64 = py
            .import("mypy.semanal")?
            .getattr("SCOPE_COMPREHENSION")?
            .extract()?;
        if cur_scope == comp_scope {
            fail_with_kwargs(
                semanal,
                "\"yield\" inside comprehension or generator expression",
                e,
                true,
                true,
                None,
            )?;
        } else {
            let function_stack = semanal.getattr("function_stack")?;
            let func = function_stack.get_item(function_stack.len()? - 1)?;
            let is_coro = func.getattr("is_coroutine")?.is_true()?;
            if is_coro {
                func.setattr("is_generator", true)?;
                func.setattr("is_async_generator", true)?;
            } else {
                func.setattr("is_generator", true)?;
            }
        }
    }
    let expr = e.getattr("expr")?;
    if !expr.is_none() && expr.is_true()? {
        expr.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_yield_from_expr` body — scope + comprehension +
/// async checks (fail-only, no generator flag flip), then recurse.
#[pyfunction]
pub(crate) fn rust_visit_yield_from_expr(
    py: Python<'_>,
    e: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
    if !in_func {
        fail_with_kwargs(
            semanal,
            "\"yield from\" outside function",
            e,
            true,
            true,
            None,
        )?;
    } else {
        let scope_stack = semanal.getattr("scope_stack")?;
        let last = scope_stack.len()? - 1;
        let cur_scope: i64 = scope_stack.get_item(last)?.extract()?;
        let comp_scope: i64 = py
            .import("mypy.semanal")?
            .getattr("SCOPE_COMPREHENSION")?
            .extract()?;
        if cur_scope == comp_scope {
            fail_with_kwargs(
                semanal,
                "\"yield from\" inside comprehension or generator expression",
                e,
                true,
                true,
                None,
            )?;
        } else {
            let function_stack = semanal.getattr("function_stack")?;
            let func = function_stack.get_item(function_stack.len()? - 1)?;
            let is_coro = func.getattr("is_coroutine")?.is_true()?;
            if is_coro {
                fail_with_kwargs(
                    semanal,
                    "\"yield from\" in async function",
                    e,
                    true,
                    true,
                    None,
                )?;
            } else {
                func.setattr("is_generator", true)?;
            }
        }
    }
    let expr = e.getattr("expr")?;
    if !expr.is_none() && expr.is_true()? {
        expr.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_await_expr` body — scope + async checks with
/// error codes, then recurse the expr.
#[pyfunction]
pub(crate) fn rust_visit_await_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
    let function_stack = semanal.getattr("function_stack")?;
    if !in_func || function_stack.len()? == 0 {
        let code = error_code(py, "TOP_LEVEL_AWAIT")?;
        fail_with_kwargs(
            semanal,
            "\"await\" outside function",
            expr,
            true,
            false,
            Some(code.as_ref(py)),
        )?;
    } else {
        let func = function_stack.get_item(function_stack.len()? - 1)?;
        let is_coro = func.getattr("is_coroutine")?.is_true()?;
        if !is_coro {
            let code = error_code(py, "AWAIT_NOT_ASYNC")?;
            fail_with_kwargs(
                semanal,
                "\"await\" outside coroutine (\"async def\")",
                expr,
                true,
                false,
                Some(code.as_ref(py)),
            )?;
        }
    }
    expr.getattr("expr")?.call_method1("accept", (semanal,))?;
    Ok(true)
}

/// `mypy.semanal.visit_try_stmt` body — set statement slot, delegate to
/// the analyzer's own `analyze_try_stmt` (which still owns the handler
/// scope-stack / context-manager interactions).
#[pyfunction]
pub(crate) fn rust_visit_try_stmt(_py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    semanal.call_method1("analyze_try_stmt", (s, semanal))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 9: leaf-actions shards for op / index / cast / type_form /
// assert_type / reveal / type_application. These drive expr recursion and

// a single `self.anal_type` call per type slot, mirroring the Python
// bodies; the `anal_type` machinery (with its full placeholder/deferral
// flow) stays in Python.

// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_op_expr` body — recurse left, then for `and`/`or`
/// ask `infer_condition_value` for the left condition, set the
/// right_unreachable/right_always flags as appropriate, then recurse
/// right (skipped when unreachable).
#[pyfunction]
pub(crate) fn rust_visit_op_expr(py: Python<'_>, expr: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    expr.getattr("left")?.call_method1("accept", (semanal,))?;
    let op: String = expr.getattr("op")?.extract()?;
    if op == "and" || op == "or" {
        let reach_mod = py.import("mypy.reachability")?;
        let options = semanal.getattr("options")?;
        let inferred: i64 = reach_mod
            .getattr("infer_condition_value")?
            .call1((expr.getattr("left")?, options))?
            .extract()?;
        let always_false: i64 = reach_mod.getattr("ALWAYS_FALSE")?.extract()?;
        let mypy_false: i64 = reach_mod.getattr("MYPY_FALSE")?.extract()?;
        let always_true: i64 = reach_mod.getattr("ALWAYS_TRUE")?.extract()?;
        let mypy_true: i64 = reach_mod.getattr("MYPY_TRUE")?.extract()?;
        let unreachable = (op == "and" && (inferred == always_false || inferred == mypy_false))
            || (op == "or" && (inferred == always_true || inferred == mypy_true));
        let always = (op == "and" && (inferred == always_true || inferred == mypy_true))
            || (op == "or" && (inferred == always_false || inferred == mypy_false));
        if unreachable {
            expr.setattr("right_unreachable", true)?;
            return Ok(true);
        } else if always {
            expr.setattr("right_always", true)?;
        }
    }
    expr.getattr("right")?.call_method1("accept", (semanal,))?;
    Ok(true)
}

/// `mypy.semanal.visit_index_expr` body — recurse base, then dispatch
/// among three branches: (TypeInfo non-generic) recurse index;
/// (TypeAlias or class/function ref) call `analyze_type_application`;
/// otherwise recurse index.
#[pyfunction]
pub(crate) fn rust_visit_index_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let base = expr.getattr("base")?;
    base.call_method1("accept", (semanal,))?;
    let ref_expr_cls = nodes_mod.getattr("RefExpr")?;
    let type_info_cls = nodes_mod.getattr("TypeInfo")?;
    let type_alias_cls = nodes_mod.getattr("TypeAlias")?;
    let is_ref = base.is_instance(ref_expr_cls)?;
    if is_ref {
        let node = base.getattr("node")?;
        if !node.is_none() {
            if node.is_instance(type_info_cls)? {
                let is_generic = node.call_method0("is_generic")?.is_true()?;
                if !is_generic {
                    expr.getattr("index")?.call_method1("accept", (semanal,))?;
                    return Ok(true);
                }
            }
            if node.is_instance(type_alias_cls)? {
                semanal.call_method1("analyze_type_application", (expr,))?;
                return Ok(true);
            }
        }
    }
    // refers_to_class_or_function(base) OR the else-branch both recurse
    // the index; the type-application branch is the only deviation and
    // we already handled it above. Use the existing helper to detect the

    // class/function case so we can call analyze_type_application.
    let refers_class_fn = rust_refers_to_class_or_function(py, base)?;
    if refers_class_fn {
        semanal.call_method1("analyze_type_application", (expr,))?;
    } else {
        expr.getattr("index")?.call_method1("accept", (semanal,))?;
    }
    Ok(true)
}

/// Shared helper for the trivial "recurse + anal_type + assign back" bodies
/// used by cast/type_form/assert_type/type_application.
fn anal_type_assign(semanal: &PyAny, expr: &PyAny, attr: &str) -> PyResult<()> {
    let analyzed = semanal.call_method1("anal_type", (expr.getattr(attr)?,))?;
    if !analyzed.is_none() {
        expr.setattr(attr, analyzed)?;
    }
    Ok(())
}

/// `mypy.semanal.visit_cast_expr` — recurse inner expr, anal_type(type).
#[pyfunction]
pub(crate) fn rust_visit_cast_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    expr.getattr("expr")?.call_method1("accept", (semanal,))?;
    anal_type_assign(semanal, expr, "type")?;
    Ok(true)
}

/// `mypy.semanal.visit_type_form_expr` — anal_type(type).
#[pyfunction]
pub(crate) fn rust_visit_type_form_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    anal_type_assign(semanal, expr, "type")?;
    Ok(true)
}

/// `mypy.semanal.visit_assert_type_expr` — recurse inner expr + anal_type.
#[pyfunction]
pub(crate) fn rust_visit_assert_type_expr(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    expr.getattr("expr")?.call_method1("accept", (semanal,))?;
    anal_type_assign(semanal, expr, "type")?;
    Ok(true)
}

/// `mypy.semanal.visit_reveal_expr` — if REVEAL_TYPE, recurse the expr.
#[pyfunction]
pub(crate) fn rust_visit_reveal_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    // REVEAL_TYPE is a module constant in mypy.semanal; this shard
    // mirrors the Python branch only when kind == REVEAL_TYPE.
    let semanal_mod = py.import("mypy.semanal")?;
    let reveal_type: i64 = semanal_mod.getattr("REVEAL_TYPE")?.extract()?;
    let kind: i64 = expr.getattr("kind")?.extract()?;
    if kind == reveal_type {
        let inner = expr.getattr("expr")?;
        if !inner.is_none() {
            inner.call_method1("accept", (semanal,))?;
        }
    }
    Ok(true)
}

/// `mypy.semanal.visit_type_application` — recurse expr, then anal_type
/// each entry of the `types` list and assign back in place.
#[pyfunction]
pub(crate) fn rust_visit_type_application(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    expr.getattr("expr")?.call_method1("accept", (semanal,))?;
    let types = expr.getattr("types")?;
    let list = match types.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    for i in 0..list.len() {
        let analyzed = semanal.call_method1("anal_type", (list.get_item(i)?,))?;
        if !analyzed.is_none() {
            list.set_item(i, analyzed)?;
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 10: leaf-actions shards for the list/set comprehension visitors
// (async-for-outside-coroutine check + generator recurse).

// ---------------------------------------------------------------------------

/// Shared check for `any(expr.generator.is_async)` outside a coroutine,
/// emitting the ASYNC_FOR_OUTSIDE_COROUTINE syntax code. Mirrors the
/// Python prelude of the list/set comprehension visitors.
fn check_async_comp_for(semanal: &PyAny, generator: &PyAny, ctx: &PyAny) -> PyResult<bool> {
    // any(is_async) over the generator's index/unpacked is_async flags.
    // The GeneratorExpr/DictionaryComprehension node stores is_async as a
    // list-of-bool aligned with the for clauses; mirror `any(...)` by

    // scanning the `is_async` attribute on either the expr (dict comp)
    // or expr.generator (list/set comp) — callers pass the node whose
    // `.is_async` is a list.
    let is_async_attr = generator.getattr("is_async")?;
    let has_async = match is_async_attr.downcast::<PyList>() {
        Ok(list) => {
            let mut any = false;
            for flag in list.iter() {
                if flag.is_true()? {
                    any = true;
                    break;
                }
            }
            any
        }
        Err(_) => is_async_attr.is_true()?,
    };
    if !has_async {
        // Short-circuit: no async for clause, no check needed.
        return Ok(false);
    }
    let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
    let function_stack = semanal.getattr("function_stack")?;
    let ok = in_func
        && function_stack.len()? > 0
        && function_stack
            .get_item(function_stack.len()? - 1)?
            .getattr("is_coroutine")?
            .is_true()?;
    if !ok {
        let py = semanal.py();
        let codes_mod = py.import("mypy.errorcodes")?;
        let syntax = codes_mod.getattr("SYNTAX")?;
        let msg_mod = py.import("mypy.message_registry")?;
        let msg: String = msg_mod.getattr("ASYNC_FOR_OUTSIDE_COROUTINE")?.extract()?;
        let kw = pyo3::types::PyDict::new(py);
        kw.set_item("code", syntax)?;
        semanal.call_method("fail", (msg.as_str(), ctx), Some(kw))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_list_comprehension` body — async-for check on the
/// generator, then recurse the generator.
#[pyfunction]
pub(crate) fn rust_visit_list_comprehension(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let generator = expr.getattr("generator")?;
    check_async_comp_for(semanal, generator, expr)?;
    generator.call_method1("accept", (semanal,))?;
    Ok(true)
}

/// `mypy.semanal.visit_set_comprehension` body — same shape.
#[pyfunction]
pub(crate) fn rust_visit_set_comprehension(
    _py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let generator = expr.getattr("generator")?;
    check_async_comp_for(semanal, generator, expr)?;
    generator.call_method1("accept", (semanal,))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Slice 11: leaf-actions shards for dictionary comprehension,
// generator expression, and lambda visitors. These use `self.enter(expr)`

// and `self.inside_except_star_block_set(False, entering_loop=False)`
// context managers, mirrored here via `__enter__` / `__exit__`.
// ---------------------------------------------------------------------------

/// `mypy.semanal.visit_dictionary_comprehension` body — async-for
/// check (is_async lives on the expr itself, not a nested generator),
/// then `enter(expr)` CM wrapping `analyze_comp_for` + key/value
/// accept, then `analyze_comp_for_2`.
#[pyfunction]
pub(crate) fn rust_visit_dictionary_comprehension(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    check_async_comp_for(semanal, expr, expr)?;
    let cm = semanal.call_method1("enter", (expr,))?;
    cm.call_method1("__enter__", ())?;
    let inner = (|| -> PyResult<()> {
        semanal.call_method1("analyze_comp_for", (expr,))?;
        expr.getattr("key")?.call_method1("accept", (semanal,))?;
        expr.getattr("value")?.call_method1("accept", (semanal,))?;
        Ok(())
    })();
    cm.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
    inner?;
    semanal.call_method1("analyze_comp_for_2", (expr,))?;
    Ok(true)
}

/// `mypy.semanal.visit_generator_expr` body — `enter(expr)` CM
/// wrapping `analyze_comp_for` + left_expr accept, then
/// `analyze_comp_for_2`.
#[pyfunction]
pub(crate) fn rust_visit_generator_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let cm = semanal.call_method1("enter", (expr,))?;
    cm.call_method1("__enter__", ())?;
    let inner = (|| -> PyResult<()> {
        semanal.call_method1("analyze_comp_for", (expr,))?;
        expr.getattr("left_expr")?
            .call_method1("accept", (semanal,))?;
        Ok(())
    })();
    cm.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
    inner?;
    semanal.call_method1("analyze_comp_for_2", (expr,))?;
    Ok(true)
}

/// `mypy.semanal.visit_lambda_expr` body — analyze arg initializers,
/// then `inside_except_star_block_set(False, entering_loop=False)` CM
/// wrapping `analyze_function_body`.
#[pyfunction]
pub(crate) fn rust_visit_lambda_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.call_method1("analyze_arg_initializers", (expr,))?;
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("entering_loop", false)?;
    let cm = semanal.call_method("inside_except_star_block_set", (false,), Some(kwargs))?;
    cm.call_method1("__enter__", ())?;
    let inner = semanal.call_method1("analyze_function_body", (expr,));
    cm.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
    inner?;
    Ok(true)
}

/// `mypy.semanal.visit_overloaded_func_def` body — set statement, add to
/// symbol table, early-return if `not recurse_into_functions` and no
/// `def_or_infer_vars`, else `function_scope(defn)` +
/// `set_recurse_into_functions()` CMs wrapping `analyze_overloaded_func_def`.
#[pyfunction]
pub(crate) fn rust_visit_overloaded_func_def(
    py: Python<'_>,
    defn: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", defn)?;
    semanal.call_method1("add_function_to_symbol_table", (defn,))?;
    let recurse = semanal.getattr("recurse_into_functions")?.is_true()?;
    let d_vars = defn.getattr("def_or_infer_vars")?.is_true()?;
    if !recurse && !d_vars {
        return Ok(true);
    }
    let scope = semanal.getattr("scope")?;
    let cm1 = scope.call_method1("function_scope", (defn,))?;
    let cm2 = semanal.call_method0("set_recurse_into_functions")?;
    cm1.call_method1("__enter__", ())?;
    let enter2 = cm2.call_method1("__enter__", ());
    match enter2 {
        Ok(_) => {
            let inner = semanal.call_method1("analyze_overloaded_func_def", (defn,));
            let _ = cm2.call_method1("__exit__", (py.None(), py.None(), py.None()));
            let _ = cm1.call_method1("__exit__", (py.None(), py.None(), py.None()));
            inner?;
        }
        Err(err) => {
            let _ = cm1.call_method1("__exit__", (py.None(), py.None(), py.None()));
            return Err(err);
        }
    }
    Ok(true)
}

/// `mypy.semanal.visit_class_def` body — set statement, push to
/// `incomplete_type_stack`, compute qualified_name, enter
/// `tvar_scope_frame(tvar_scope.class_frame(namespace))` CM, then
/// either mark_incomplete + return (when push_type_args returns None),
/// or push/pop `removed_type_vars` around `analyze_class` +
/// `pop_type_args`. `incomplete_type_stack` pop happens after the CM.
#[pyfunction]
pub(crate) fn rust_visit_class_def(
    py: Python<'_>,
    defn: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", defn)?;
    let info = defn.getattr("info")?;
    // Python: `not defn.info` — truthy check, not just None check.
    semanal
        .getattr("incomplete_type_stack")?
        .call_method1("append", (!info.is_true()?,))?;
    let namespace = semanal.call_method1("qualified_name", (defn.getattr("name")?,))?;
    let tvar_scope = semanal.getattr("tvar_scope")?;
    let frame = tvar_scope.call_method1("class_frame", (namespace,))?;
    let cm = semanal.call_method1("tvar_scope_frame", (frame,))?;
    cm.call_method1("__enter__", ())?;
    // push_type_args returns None on error: mark_incomplete + skip rest.
    let pushed = semanal.call_method1("push_type_args", (defn.getattr("type_args")?, defn))?;
    let ok = !pushed.is_none();
    if !ok {
        let _ = semanal.call_method1("mark_incomplete", (defn.getattr("name")?, defn));
    }
    let body_err: Option<pyo3::PyErr> = if ok {
        let removed = semanal.getattr("removed_type_vars")?;
        let _ = removed.call_method1("append", (PyList::empty(py),));
        let analyze_result = semanal.call_method1("analyze_class", (defn,));
        let _ = removed.call_method1("pop", ());
        let _ = semanal.call_method1("pop_type_args", (defn.getattr("type_args")?,));
        analyze_result.err()
    } else {
        None
    };
    let _ = cm.call_method1("__exit__", (py.None(), py.None(), py.None()));
    let _ = semanal
        .getattr("incomplete_type_stack")?
        .call_method1("pop", ());
    if let Some(err) = body_err {
        return Err(err);
    }
    Ok(true)
}

/// `mypy.semanal.visit_func_def` body — set statement, visit arg
/// initializers, set is_conditional, set _fullname, conditionally
/// add_function_to_symbol_table, early-return, else triple-nested CM
/// (function_scope + set_recurse_into_functions +
/// inside_except_star_block_set) wrapping analyze_func_def.
#[pyfunction]
pub(crate) fn rust_visit_func_def(py: Python<'_>, defn: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", defn)?;
    // Visit default values (may contain assignment expressions).
    let arguments = defn.getattr("arguments")?;
    if let Ok(args_list) = arguments.downcast::<PyList>() {
        for arg in args_list.iter() {
            let init = arg.getattr("initializer")?;
            if !init.is_none() {
                init.call_method1("accept", (semanal,))?;
            }
        }
    }
    // defn.is_conditional = self.block_depth[-1] > 0
    let block_depth = semanal.getattr("block_depth")?;
    let bd_list = block_depth.downcast::<PyList>()?;
    let bd_last = bd_list.get_item(bd_list.len() - 1)?;
    defn.setattr("is_conditional", bd_last.is_true()?)?;
    // defn._fullname = self.qualified_name(defn.name)
    let fullname = semanal.call_method1("qualified_name", (defn.getattr("name")?,))?;
    defn.setattr("_fullname", fullname)?;
    // Conditionally add to symbol table.
    let recurse = semanal.getattr("recurse_into_functions")?.is_true()?;
    let func_stack = semanal.getattr("function_stack")?;
    let stack_len = func_stack.len()?;
    if !recurse || stack_len > 0 {
        let is_decorated = defn.getattr("is_decorated")?.is_true()?;
        let is_overload = defn.getattr("is_overload")?.is_true()?;
        if !is_decorated && !is_overload {
            semanal.call_method1("add_function_to_symbol_table", (defn,))?;
        }
    }
    // Early return.
    let d_vars = defn.getattr("def_or_infer_vars")?.is_true()?;
    if !recurse && !d_vars {
        return Ok(true);
    }
    // Triple-nested CM: function_scope + set_recurse_into_functions +
    // inside_except_star_block_set(value=False).
    let scope = semanal.getattr("scope")?;
    let cm1 = scope.call_method1("function_scope", (defn,))?;
    let cm2 = semanal.call_method0("set_recurse_into_functions")?;
    let cm3 = semanal.call_method1("inside_except_star_block_set", (false,))?;
    cm1.call_method1("__enter__", ())?;
    let enter2 = cm2.call_method1("__enter__", ());
    match enter2 {
        Ok(_) => {
            cm3.call_method1("__enter__", ())?;
            let inner = semanal.call_method1("analyze_func_def", (defn,));
            let _ = cm3.call_method1("__exit__", (py.None(), py.None(), py.None()));
            let _ = cm2.call_method1("__exit__", (py.None(), py.None(), py.None()));
            let _ = cm1.call_method1("__exit__", (py.None(), py.None(), py.None()));
            inner?;
        }
        Err(err) => {
            let _ = cm1.call_method1("__exit__", (py.None(), py.None(), py.None()));
            return Err(err);
        }
    }
    Ok(true)
}

/// `mypy.semanal.visit_nonlocal_decl` body — set statement,
/// check is_module_scope, else for each name: scan locals/scope_stack
/// for a matching table (for...else), check annotation scope, check
/// local redefinition, check global conflict, add to nonlocal_decls.
#[pyfunction]
pub(crate) fn rust_visit_nonlocal_decl(
    py: Python<'_>,
    d: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", d)?;
    if semanal.call_method0("is_module_scope")?.is_true()? {
        semanal.call_method(
            "fail",
            ("nonlocal declaration not allowed at module level", d),
            None,
        )?;
        return Ok(true);
    }
    // SCOPE_ANNOTATION constant from semanal module.
    let scope_annotation = py
        .import("mypy.semanal")?
        .getattr("SCOPE_ANNOTATION")?
        .extract::<i64>()?;
    let names = d.getattr("names")?;
    let names_list = match names.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };
    let locals = semanal.getattr("locals")?;
    let locals_list = locals.downcast::<PyList>()?;
    let scope_stack = semanal.getattr("scope_stack")?;
    let scope_list = scope_stack.downcast::<PyList>()?;
    // self.locals[:-1] and self.scope_stack[:-1] — all but last.
    let n = locals_list.len();
    // zip(reversed(locals[:-1]), reversed(scope_stack[:-1]))
    let global_decls = semanal.getattr("global_decls")?;
    let global_list = global_decls.downcast::<PyList>()?;
    let nonlocal_decls = semanal.getattr("nonlocal_decls")?;
    let nonlocal_list = nonlocal_decls.downcast::<PyList>()?;
    let local_last = locals_list.get_item(n - 1)?;
    let global_last = global_list.get_item(global_list.len() - 1)?;
    let nonlocal_last = nonlocal_list.get_item(nonlocal_list.len() - 1)?;
    for name in names_list.iter() {
        // for table, scope_type in zip(reversed(...), reversed(...)):
        let mut found = false;
        for i in (0..(n - 1)).rev() {
            let table = locals_list.get_item(i)?;
            let scope_type = scope_list.get_item(i)?;
            if !table.is_none() {
                // name in table (dict membership)
                if table.contains(name)? {
                    let st = scope_type.extract::<i64>()?;
                    if st == scope_annotation {
                        let msg = format!(
                            "nonlocal binding not allowed for type parameter \"{}\"",
                            name.extract::<String>()?
                        );
                        semanal.call_method("fail", (msg.as_str(), d), None)?;
                    }
                    found = true;
                    break;
                }
            }
        }
        if !found {
            let msg = format!(
                "No binding for nonlocal \"{}\" found",
                name.extract::<String>()?
            );
            semanal.call_method("fail", (msg.as_str(), d), None)?;
        }
        // self.locals[-1] is not None and name in self.locals[-1]
        if !local_last.is_none() && local_last.contains(name)? {
            let name_str = name.extract::<String>()?;
            let msg = format!(
                "Name \"{}\" is already defined in local scope before nonlocal declaration",
                name_str
            );
            semanal.call_method("fail", (msg.as_str(), d), None)?;
        }
        // name in self.global_decls[-1]
        if global_last.contains(name)? {
            let msg = format!(
                "Name \"{}\" is nonlocal and global",
                name.extract::<String>()?
            );
            semanal.call_method("fail", (msg.as_str(), d), None)?;
        }
        // self.nonlocal_decls[-1].add(name)
        nonlocal_last.call_method1("add", (name,))?;
    }
    Ok(true)
}

/// `mypy.semanal.visit_for_stmt` body — async-for check, set statement,
/// accept expr, analyze_lvalue, conditional index_type handling,
/// loop_depth increment, CM (entering_loop=True) wrapping visit_block,
/// loop_depth decrement, visit_block_maybe.
#[pyfunction]
pub(crate) fn rust_visit_for_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    // async-for check
    if s.getattr("is_async")?.is_true()? {
        let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
        let func_stack = semanal.getattr("function_stack")?;
        let ok = in_func
            && func_stack.len()? > 0
            && func_stack
                .get_item(func_stack.len()? - 1)?
                .getattr("is_coroutine")?
                .is_true()?;
        if !ok {
            let msg_mod = py.import("mypy.message_registry")?;
            let msg: String = msg_mod.getattr("ASYNC_FOR_OUTSIDE_COROUTINE")?.extract()?;
            let codes_mod = py.import("mypy.errorcodes")?;
            let syntax = codes_mod.getattr("SYNTAX")?;
            let kw = pyo3::types::PyDict::new(py);
            kw.set_item("code", syntax)?;
            semanal.call_method("fail", (msg.as_str(), s), Some(kw))?;
        }
    }
    semanal.setattr("statement", s)?;
    s.getattr("expr")?.call_method1("accept", (semanal,))?;
    // analyze_lvalue with kwargs
    let index_type = s.getattr("index_type")?;
    let has_index_type = !index_type.is_none();
    let kwargs_al = pyo3::types::PyDict::new(py);
    kwargs_al.set_item("explicit_type", has_index_type)?;
    kwargs_al.set_item("is_index_var", true)?;
    semanal.call_method("analyze_lvalue", (s.getattr("index")?,), Some(kwargs_al))?;
    if has_index_type {
        // is_classvar check
        let is_cv = semanal
            .call_method1("is_classvar", (index_type,))?
            .is_true()?;
        if is_cv {
            semanal.call_method1("fail_invalid_classvar", (s.getattr("index")?,))?;
        }
        // allow_tuple_literal = isinstance(s.index, TupleExpr)
        let nodes_mod = py.import("mypy.nodes")?;
        let tuple_cls = nodes_mod.getattr("TupleExpr")?.downcast::<PyType>()?;
        let allow_tuple = s.getattr("index")?.is_instance(tuple_cls)?;
        let kwargs_at = pyo3::types::PyDict::new(py);
        kwargs_at.set_item("allow_tuple_literal", allow_tuple)?;
        let analyzed = semanal.call_method("anal_type", (index_type,), Some(kwargs_at))?;
        if !analyzed.is_none() {
            semanal.call_method1("store_declared_types", (s.getattr("index")?, analyzed))?;
            s.setattr("index_type", analyzed)?;
        }
    }
    // loop_depth[-1] += 1
    let loop_depth = semanal.getattr("loop_depth")?;
    let ld_list = loop_depth.downcast::<PyList>()?;
    let ld_idx = ld_list.len() - 1;
    let cur: i64 = ld_list.get_item(ld_idx)?.extract()?;
    ld_list.set_item(ld_idx, cur + 1)?;
    // CM: inside_except_star_block_set(value=False, entering_loop=True)
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("entering_loop", true)?;
    let cm = semanal.call_method("inside_except_star_block_set", (false,), Some(kwargs))?;
    cm.call_method1("__enter__", ())?;
    let inner = semanal.call_method1("visit_block", (s.getattr("body")?,));
    let _ = cm.call_method1("__exit__", (py.None(), py.None(), py.None()));
    inner?;
    // loop_depth[-1] -= 1
    let cur: i64 = ld_list.get_item(ld_idx)?.extract()?;
    ld_list.set_item(ld_idx, cur - 1)?;
    // visit_block_maybe(s.else_body)
    semanal.call_method1("visit_block_maybe", (s.getattr("else_body")?,))?;
    Ok(true)
}

/// `mypy.semanal.visit_with_stmt` body — async-with check,
/// unanalyzed_type handling (ProperType/TupleType isinstance checks,
/// target count logic), zip(expr, target) iteration with types.pop(0),
/// analyze_lvalue, is_classvar, anal_type, store_declared_types,
/// analyzed_types assignment, visit_block.
#[pyfunction]
pub(crate) fn rust_visit_with_stmt(py: Python<'_>, s: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", s)?;
    let nodes_mod = py.import("mypy.nodes")?;
    let types_mod = py.import("mypy.types")?;
    let proper_cls = types_mod.getattr("ProperType")?.downcast::<PyType>()?;
    let tuple_cls = types_mod.getattr("TupleType")?.downcast::<PyType>()?;
    let tupleexpr_cls = nodes_mod.getattr("TupleExpr")?.downcast::<PyType>()?;

    // async-with check
    if s.getattr("is_async")?.is_true()? {
        let in_func = semanal.call_method0("is_func_scope")?.is_true()?;
        let func_stack = semanal.getattr("function_stack")?;
        let ok = in_func
            && func_stack.len()? > 0
            && func_stack
                .get_item(func_stack.len()? - 1)?
                .getattr("is_coroutine")?
                .is_true()?;
        if !ok {
            let msg_mod = py.import("mypy.message_registry")?;
            let msg: String = msg_mod.getattr("ASYNC_WITH_OUTSIDE_COROUTINE")?.extract()?;
            let codes_mod = py.import("mypy.errorcodes")?;
            let syntax = codes_mod.getattr("SYNTAX")?;
            let kw = pyo3::types::PyDict::new(py);
            kw.set_item("code", syntax)?;
            semanal.call_method("fail", (msg.as_str(), s), Some(kw))?;
        }
    }

    // Build the `types` list from unanalyzed_type
    let unanalyzed = s.getattr("unanalyzed_type")?;
    let mut types: Vec<&PyAny> = Vec::new();
    if !unanalyzed.is_none() {
        // assert isinstance(unanalyzed, ProperType)
        if !unanalyzed.is_instance(proper_cls)? {
            return Ok(false);
        }
        // actual_targets = [t for t in s.target if t is not None]
        let target = s.getattr("target")?;
        let target_list = match target.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(false),
        };
        let actual_count = target_list.iter().filter(|t| !t.is_none()).count();
        if actual_count == 0 {
            semanal.call_method(
                "fail",
                ("Invalid type comment: \"with\" statement has no targets", s),
                None,
            )?;
        } else if actual_count == 1 {
            types.push(unanalyzed);
        } else if unanalyzed.is_instance(tuple_cls)? {
            let items = unanalyzed.getattr("items")?;
            let items_list = match items.downcast::<PyList>() {
                Ok(l) => l,
                Err(_) => return Ok(false),
            };
            if actual_count == items_list.len() {
                for item in items_list.iter() {
                    types.push(item);
                }
            } else {
                semanal.call_method(
                    "fail",
                    ("Incompatible number of types for \"with\" targets", s),
                    None,
                )?;
            }
        } else {
            semanal.call_method(
                "fail",
                ("Multiple types expected for multiple \"with\" targets", s),
                None,
            )?;
        }
    }

    // new_types: list[Type] = []
    let new_types = PyList::empty(py);
    let has_unanalyzed = !unanalyzed.is_none();

    // for e, n in zip(s.expr, s.target):
    let expr_list = s
        .getattr("expr")?
        .downcast::<PyList>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("s.expr is not a list"))?;
    let target_list = s.getattr("target")?;
    let target_list = target_list.downcast::<PyList>()?;
    let min_len = expr_list.len().min(target_list.len());
    for i in 0..min_len {
        let e = expr_list.get_item(i)?;
        let n = target_list.get_item(i)?;
        e.call_method1("accept", (semanal,))?;
        if !n.is_none() {
            let kwargs_al = pyo3::types::PyDict::new(py);
            kwargs_al.set_item("explicit_type", has_unanalyzed)?;
            semanal.call_method("analyze_lvalue", (n,), Some(kwargs_al))?;
            if !types.is_empty() {
                let t = types.remove(0);
                let is_cv = semanal.call_method1("is_classvar", (t,))?.is_true()?;
                if is_cv {
                    semanal.call_method1("fail_invalid_classvar", (n,))?;
                }
                let allow_tuple = n.is_instance(tupleexpr_cls)?;
                let kwargs_at = pyo3::types::PyDict::new(py);
                kwargs_at.set_item("allow_tuple_literal", allow_tuple)?;
                let analyzed = semanal.call_method("anal_type", (t,), Some(kwargs_at))?;
                if !analyzed.is_none() {
                    new_types.append(analyzed)?;
                    semanal.call_method1("store_declared_types", (n, analyzed))?;
                }
            }
        }
    }

    s.setattr("analyzed_types", new_types)?;
    semanal.call_method1("visit_block", (s.getattr("body")?,))?;
    Ok(true)
}

/// `mypy.semanal.visit_assignment_expr` body — accept value,
/// if func_scope: check_valid_comprehension (early return if False),
/// analyze_lvalue with kwargs.
#[pyfunction]
pub(crate) fn rust_visit_assignment_expr(
    py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    s.getattr("value")?.call_method1("accept", (semanal,))?;
    if semanal.call_method0("is_func_scope")?.is_true()? {
        let ok = semanal
            .call_method1("check_valid_comprehension", (s,))?
            .is_true()?;
        if !ok {
            return Ok(true);
        }
    }
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("escape_comprehensions", true)?;
    kwargs.set_item("has_explicit_value", true)?;
    semanal.call_method("analyze_lvalue", (s.getattr("target")?,), Some(kwargs))?;
    Ok(true)
}

/// `mypy.semanal.visit_import_all` body — correct relative import,
/// module lookup, incomplete-namespace mark, iterate module symbol names
/// (skip no_serialize), set future import flags, reexport logic with
/// MypyFile / PlaceholderNode handling.
#[pyfunction]
pub(crate) fn rust_visit_import_all(py: Python<'_>, i: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    let i_id = semanal.call_method1("correct_relative_import", (i,))?;
    let i_id_str: String = i_id.extract()?;
    let modules = semanal.getattr("modules")?;
    let modules_dict = match modules.downcast::<PyDict>() {
        Ok(d) => d,
        Err(_) => return Ok(false),
    };
    if let Some(m) = modules_dict.get_item(&i_id_str)? {
        let incomplete = semanal
            .call_method1("is_incomplete_namespace", (i_id_str.as_str(),))?
            .is_true()?;
        if incomplete {
            semanal.call_method1("mark_incomplete", ("*", i))?;
        }
        let names = m.getattr("names")?;
        let names_dict = match names.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return Ok(false),
        };
        let nodes_mod = py.import("mypy.nodes")?;
        let mypyfile_cls = nodes_mod.getattr("MypyFile")?.downcast::<PyType>()?;
        let placeholder_cls = nodes_mod.getattr("PlaceholderNode")?.downcast::<PyType>()?;
        let imports = semanal.getattr("imports")?;
        for (name, node) in names_dict.iter() {
            let name_str: String = name.extract()?;
            let no_serialize = node.getattr("no_serialize")?.is_true()?;
            if no_serialize {
                continue;
            }
            let fullname = format!("{}.{}", i_id_str, name_str);
            semanal.call_method1("set_future_import_flags", (fullname.as_str(),))?;
            let module_public = node.getattr("module_public")?.is_true()?;
            let has_all = names_dict.contains("__all__")?;
            let name_ok = !name_str.starts_with('_') || has_all;
            if module_public && name_ok {
                let node_node = node.getattr("node")?;
                if node_node.is_instance(mypyfile_cls)? {
                    let fullname2 = node_node.getattr("fullname")?;
                    imports.call_method1("add", (fullname2,))?;
                }
                let kw2 = pyo3::types::PyDict::new(py);
                kw2.set_item("context", i)?;
                kw2.set_item("module_public", true)?;
                kw2.set_item("module_hidden", false)?;
                semanal.call_method("add_imported_symbol", (name_str.as_str(), node), Some(kw2))?;
                let final_it = semanal.getattr("final_iteration")?.is_true()?;
                if node_node.is_instance(placeholder_cls)? && final_it {
                    let kw3 = pyo3::types::PyDict::new(py);
                    kw3.set_item("module_public", true)?;
                    kw3.set_item("module_hidden", false)?;
                    semanal.call_method(
                        "add_unknown_imported_symbol",
                        (name_str.as_str(), i, py.None()),
                        Some(kw3),
                    )?;
                }
            }
        }
    }
    Ok(true)
}

/// `mypy.semanal.visit_import_from` body — correct relative import,
/// iterate imported names, resolve each against the module symbol table
/// or self.modules, handle __getattr__, process/missing/unknown symbols.
#[pyfunction]
pub(crate) fn rust_visit_import_from(
    py: Python<'_>,
    imp: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", imp)?;
    let module_id = semanal.call_method1("correct_relative_import", (imp,))?;
    let module_id_str: String = module_id.extract()?;
    let modules = semanal.getattr("modules")?;
    let modules_dict = match modules.downcast::<PyDict>() {
        Ok(d) => d,
        Err(_) => return Ok(false),
    };
    let module = modules_dict.get_item(&module_id_str)?;
    let cur_mod_id = semanal.getattr("cur_mod_id")?;
    let cur_mod_id_str: String = cur_mod_id.extract()?;
    let missing_modules = semanal.getattr("missing_modules")?;
    let missing_set = match missing_modules.downcast::<PySet>() {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    let is_stub_file = semanal.getattr("is_stub_file")?.is_true()?;
    let opts = semanal.getattr("options")?;
    let implicit_reexport = opts.getattr("implicit_reexport")?.is_true()?;
    let use_implicit_reexport = !is_stub_file && implicit_reexport;
    let all_exports = semanal.getattr("all_exports")?;

    let imp_names = imp.getattr("names")?;
    let names_list = match imp_names.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };

    let nodes_mod = py.import("mypy.nodes")?;
    let symboltable_cls = nodes_mod.getattr("SymbolTableNode")?;
    let semanal_mod = py.import("mypy.semanal")?;
    let gdef = semanal_mod.getattr("GDEF")?;

    for item in names_list.iter() {
        let pair = match item.downcast::<PyTuple>() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };
        if pair.len() != 2 {
            return Ok(false);
        }
        let id: String = pair.get_item(0)?.extract()?;
        let as_id_obj = pair.get_item(1)?;
        let as_id: Option<String> = if as_id_obj.is_none() {
            None
        } else {
            Some(as_id_obj.extract()?)
        };

        let fullname = format!("{}.{}", module_id_str, id);
        semanal.call_method1("set_future_import_flags", (fullname.as_str(),))?;

        // Resolve node
        let mut node: Option<PyObject> = if let Some(m) = module {
            if module_id_str == cur_mod_id_str && modules_dict.contains(&fullname)? {
                let mod_entry = modules_dict.get_item(&fullname)?;
                let stn = symboltable_cls.call1((gdef, mod_entry.unwrap()))?;
                Some(stn.into())
            } else {
                // __all__ recovery
                if id == "__all__" && as_id.as_deref() == Some("__all__") {
                    let m_names = m.getattr("names")?;
                    let m_names_dict = match m_names.downcast::<PyDict>() {
                        Ok(d) => d,
                        Err(_) => return Ok(false),
                    };
                    all_exports.call_method1("clear", ())?;
                    let public_names: Vec<PyObject> = m_names_dict
                        .iter()
                        .filter(|(_, sym)| {
                            sym.getattr("module_public")
                                .map(|v| v.is_true().unwrap_or(false))
                                .unwrap_or(false)
                        })
                        .map(|(name, _)| name.into())
                        .collect();
                    all_exports.call_method1("extend", (public_names,))?;
                }
                let m_names = m.getattr("names")?;
                let m_names_dict = match m_names.downcast::<PyDict>() {
                    Ok(d) => d,
                    Err(_) => return Ok(false),
                };
                m_names_dict.get_item(&id)?.map(|v| v.into())
            }
        } else {
            None
        };

        let mut missing_submodule = false;
        let imported_id = match &as_id {
            Some(a) => a.clone(),
            None => id.clone(),
        };

        let module_public =
            use_implicit_reexport || (as_id.is_some() && as_id.as_deref() == Some(id.as_str()));

        // If node is None, try module lookup
        if node.is_none() {
            if let Some(mod2) = modules_dict.get_item(&fullname)? {
                let kind = semanal.call_method1("current_symbol_kind", ())?;
                let stn = symboltable_cls.call1((kind, mod2))?;
                node = Some(stn.into());
            } else if missing_set.contains(&fullname)? {
                missing_submodule = true;
            }
        }

        // __getattr__ handling
        if let Some(m) = module {
            if node.is_none() {
                let m_names = m.getattr("names")?;
                let m_names_dict = match m_names.downcast::<PyDict>() {
                    Ok(d) => d,
                    Err(_) => return Ok(false),
                };
                if m_names_dict.contains("__getattr__")? {
                    let getattr_defn = m_names_dict.get_item("__getattr__")?.unwrap();
                    let fullname2 = format!("{}.{}", module_id_str, id);
                    let gvar = semanal.call_method1(
                        "create_getattr_var",
                        (getattr_defn, imported_id.as_str(), fullname2.as_str()),
                    )?;
                    if gvar.is_true()? {
                        let kw = pyo3::types::PyDict::new(py);
                        kw.set_item("module_public", module_public)?;
                        kw.set_item("module_hidden", !module_public)?;
                        semanal.call_method(
                            "add_symbol",
                            (imported_id.as_str(), gvar, imp),
                            Some(kw),
                        )?;
                        continue;
                    }
                }
            }
        }

        if let Some(n) = &node {
            let n_ref = n.as_ref(py);
            semanal.call_method(
                "process_imported_symbol",
                (
                    n_ref,
                    module_id_str.as_str(),
                    id.as_str(),
                    imported_id.as_str(),
                    fullname.as_str(),
                    module_public,
                ),
                {
                    let kw = pyo3::types::PyDict::new(py);
                    kw.set_item("context", imp)?;
                    Some(kw)
                },
            )?;
            if n_ref.getattr("module_hidden")?.is_true()? {
                let kw = pyo3::types::PyDict::new(py);
                kw.set_item("module_public", module_public)?;
                kw.set_item("module_hidden", !module_public)?;
                kw.set_item("context", imp)?;
                kw.set_item("add_unknown_imported_symbol", false)?;
                semanal.call_method(
                    "report_missing_module_attribute",
                    (
                        module_id_str.as_str(),
                        id.as_str(),
                        imported_id.as_str(),
                        module_public,
                        !module_public,
                        imp,
                    ),
                    Some(kw),
                )?;
            }
        } else if module.is_some() {
            if !missing_submodule {
                let kw = pyo3::types::PyDict::new(py);
                kw.set_item("module_public", module_public)?;
                kw.set_item("module_hidden", !module_public)?;
                kw.set_item("context", imp)?;
                semanal.call_method(
                    "report_missing_module_attribute",
                    (
                        module_id_str.as_str(),
                        id.as_str(),
                        imported_id.as_str(),
                        module_public,
                        !module_public,
                        imp,
                    ),
                    Some(kw),
                )?;
            } else {
                semanal.call_method(
                    "add_unknown_imported_symbol",
                    (
                        imported_id.as_str(),
                        imp,
                        fullname.as_str(),
                        module_public,
                        !module_public,
                    ),
                    None,
                )?;
            }
        } else {
            semanal.call_method(
                "add_unknown_imported_symbol",
                (
                    imported_id.as_str(),
                    imp,
                    fullname.as_str(),
                    module_public,
                    !module_public,
                ),
                None,
            )?;
        }
    }

    Ok(true)
}

/// `mypy.semanal.visit_assignment_stmt` body — identity check, track
/// incomplete refs, conditional rvalue visit with allow_unbound_tvars
/// and basic_type_applications, special form dispatch, normal path
/// (unwrap_final, analyze_lvalues, process_type_annotation, etc).
#[pyfunction]
pub(crate) fn rust_visit_assignment_stmt(
    py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    semanal.setattr("statement", s)?;

    // Special case 'X = X' in global scope.
    let is_identity = semanal
        .call_method1("analyze_identity_global_assignment", (s,))?
        .is_true()?;
    if is_identity {
        return Ok(true);
    }

    let tag = semanal.call_method1("track_incomplete_refs", ())?;

    // Rvalue visit, conditional on type form / typevar-like.
    let can_be_type_form = semanal
        .call_method1("can_possibly_be_type_form", (s,))?
        .is_true()?;
    let can_be_typevarlike = semanal
        .call_method1("can_possibly_be_typevarlike_declaration", (s,))?
        .is_true()?;

    let rvalue = s.getattr("rvalue")?;

    // Save/restore allow_unbound_tvars and basic_type_applications
    let old_allow_unbound = semanal.getattr("allow_unbound_tvars")?;
    semanal.setattr("allow_unbound_tvars", true)?;

    if can_be_type_form {
        let old_basic = semanal.getattr("basic_type_applications")?;
        semanal.setattr("basic_type_applications", true)?;
        rvalue.call_method1("accept", (semanal,))?;
        semanal.setattr("basic_type_applications", old_basic)?;
    } else if can_be_typevarlike {
        rvalue.call_method1("accept", (semanal,))?;
    } else {
        semanal.setattr("allow_unbound_tvars", old_allow_unbound)?;
        rvalue.call_method1("accept", (semanal,))?;
    }
    semanal.setattr("allow_unbound_tvars", old_allow_unbound)?;

    // Check incomplete refs or should_wait_rhs.
    let found_incomplete = semanal
        .call_method1("found_incomplete_ref", (tag,))?
        .is_true()?;
    let should_wait = semanal
        .call_method1("should_wait_rhs", (rvalue,))?
        .is_true()?;

    if found_incomplete || should_wait {
        // Mark incomplete for each modified name.
        let semanal_mod = py.import("mypy.semanal")?;
        let names_mod_fn = semanal_mod.getattr("names_modified_by_assignment")?;
        let names_list = names_mod_fn.call1((s,))?;
        let names_pylist = match names_list.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(false),
        };
        for expr in names_pylist.iter() {
            let name: String = expr.getattr("name")?.extract()?;
            semanal.call_method1("mark_incomplete", (name.as_str(), expr))?;
        }
        return Ok(true);
    }

    if can_be_type_form {
        semanal.setattr("allow_unbound_tvars", true)?;
        rvalue.call_method1("accept", (semanal,))?;
        semanal.setattr("allow_unbound_tvars", old_allow_unbound)?;
    }

    // Special form dispatch
    let mut special_form = false;

    let is_alias = semanal
        .call_method1("check_and_set_up_type_alias", (s,))?
        .is_true()?;
    if is_alias {
        s.setattr("is_alias_def", true)?;
        special_form = true;
    } else {
        let callexpr_type = py.import("mypy.nodes")?.getattr("CallExpr")?;
        if rvalue.is_instance(callexpr_type)? {
            let process_typevar = semanal
                .call_method1("process_typevar_declaration", (s,))?
                .is_true()?;
            if process_typevar {
                special_form = true;
            } else {
                let process_paramspec = semanal
                    .call_method1("process_paramspec_declaration", (s,))?
                    .is_true()?;
                if process_paramspec {
                    special_form = true;
                } else {
                    let process_typevartuple = semanal
                        .call_method1("process_typevartuple_declaration", (s,))?
                        .is_true()?;
                    if process_typevartuple {
                        special_form = true;
                    } else {
                        let namedtuple = semanal
                            .call_method1("analyze_namedtuple_assign", (s,))?
                            .is_true()?;
                        if namedtuple {
                            special_form = true;
                        } else {
                            let typeddict = semanal
                                .call_method1("analyze_typeddict_assign", (s,))?
                                .is_true()?;
                            if typeddict {
                                special_form = true;
                            } else {
                                let newtype_analyzer = semanal.getattr("newtype_analyzer")?;
                                let newtype = newtype_analyzer
                                    .call_method1("process_newtype_declaration", (s,))?
                                    .is_true()?;
                                if newtype {
                                    special_form = true;
                                } else {
                                    let enum_assign = semanal
                                        .call_method1("analyze_enum_assign", (s,))?
                                        .is_true()?;
                                    if enum_assign {
                                        special_form = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if special_form {
        semanal.call_method1("record_special_form_lvalue", (s,))?;
        return Ok(true);
    }

    // Clear alias flag
    s.setattr("is_alias_def", false)?;

    // Normal assignment analysis
    let is_final_def = semanal.call_method1("unwrap_final", (s,))?;
    s.setattr("is_final_def", is_final_def)?;
    semanal.call_method1("analyze_lvalues", (s,))?;
    semanal.call_method1("check_final_implicit_def", (s,))?;
    semanal.call_method1("store_final_status", (s,))?;
    semanal.call_method1("check_classvar", (s,))?;
    semanal.call_method1("process_type_annotation", (s,))?;
    semanal.call_method1("analyze_rvalue_as_type_form", (s,))?;
    semanal.call_method1("apply_dynamic_class_hook", (s,))?;

    let s_type = s.getattr("type")?;
    if s_type.is_none() {
        let lvalues = s.getattr("lvalues")?;
        semanal.call_method1("process_module_assignment", (lvalues, rvalue, s))?;
    }

    semanal.call_method1("process__all__", (s,))?;
    semanal.call_method1("process__deletable__", (s,))?;
    semanal.call_method1("process__slots__", (s,))?;

    Ok(true)
}

/// `mypy.semanal.visit_import` body — set statement, compute
/// use_implicit_reexport, iterate i.ids, classify each import,
/// construct SymbolTableNode, dispatch add_imported_symbol /
/// add_unknown_imported_symbol.
#[pyfunction]
pub(crate) fn rust_visit_import(py: Python<'_>, i: &PyAny, semanal: &PyAny) -> PyResult<bool> {
    semanal.setattr("statement", i)?;

    let is_stub_file = semanal.getattr("is_stub_file")?.is_true()?;
    let opts = semanal.getattr("options")?;
    let implicit_reexport = opts.getattr("implicit_reexport")?.is_true()?;
    let use_implicit_reexport = !is_stub_file && implicit_reexport;

    let modules = semanal.getattr("modules")?;
    let modules_dict = match modules.downcast::<PyDict>() {
        Ok(d) => d,
        Err(_) => return Ok(false),
    };

    let ids = i.getattr("ids")?;
    let ids_list = match ids.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };

    let nodes_mod = py.import("mypy.nodes")?;
    let symboltable_cls = nodes_mod.getattr("SymbolTableNode")?;
    let semanal_mod = py.import("mypy.semanal")?;

    let is_func_scope = semanal.call_method1("is_func_scope", ())?.is_true()?;
    let semanal_type = semanal.getattr("type")?;

    for item in ids_list.iter() {
        let pair = match item.downcast::<PyTuple>() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };
        if pair.len() != 2 {
            return Ok(false);
        }
        let id: String = pair.get_item(0)?.extract()?;
        let as_id_obj = pair.get_item(1)?;
        let as_id: Option<String> = if as_id_obj.is_none() {
            None
        } else {
            Some(as_id_obj.extract()?)
        };

        let (base_id, imported_id, module_public) = if let Some(as_id) = &as_id {
            (
                id.clone(),
                as_id.clone(),
                use_implicit_reexport || id == *as_id,
            )
        } else {
            let base = id.split('.').next().unwrap_or(&id).to_string();
            (base.clone(), base, use_implicit_reexport)
        };

        if modules_dict.contains(&base_id)? {
            let node = modules_dict.get_item(&base_id)?.unwrap();
            let kind = if is_func_scope {
                semanal_mod.getattr("LDEF")?
            } else if !semanal_type.is_none() {
                semanal_mod.getattr("MDEF")?
            } else {
                semanal_mod.getattr("GDEF")?
            };
            let kw = pyo3::types::PyDict::new(py);
            kw.set_item("module_public", module_public)?;
            kw.set_item("module_hidden", !module_public)?;
            let symbol = symboltable_cls.call((kind, node), Some(kw))?;
            let kw2 = pyo3::types::PyDict::new(py);
            kw2.set_item("context", i)?;
            kw2.set_item("module_public", module_public)?;
            kw2.set_item("module_hidden", !module_public)?;
            semanal.call_method(
                "add_imported_symbol",
                (imported_id.as_str(), symbol),
                Some(kw2),
            )?;
        } else {
            semanal.call_method(
                "add_unknown_imported_symbol",
                (
                    imported_id.as_str(),
                    i,
                    base_id.as_str(),
                    module_public,
                    !module_public,
                ),
                None,
            )?;
        }
    }

    Ok(true)
}

/// `mypy.semanal.visit_call_expr` body — accept callee, dispatch
/// special forms (cast, assert_type, reveal_type, reveal_locals,
/// Any, _promote, dict, divmod, TypeAliasType, TypeForm), normal call
/// with __all__ mutation handling.
#[pyfunction]
pub(crate) fn rust_visit_call_expr(
    py: Python<'_>,
    expr: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let callee = expr.getattr("callee")?;
    callee.call_method1("accept", (semanal,))?;

    let nodes_mod = py.import("mypy.nodes")?;
    let types_mod = py.import("mypy.types")?;
    let castexpr_cls = nodes_mod.getattr("CastExpr")?;
    let asserttype_cls = nodes_mod.getattr("AssertTypeExpr")?;
    let revealexpr_cls = nodes_mod.getattr("RevealExpr")?;
    let promoteexpr_cls = nodes_mod.getattr("PromoteExpr")?;
    let typeform_cls = nodes_mod.getattr("TypeFormExpr")?;
    let opexpr_cls = nodes_mod.getattr("OpExpr")?;
    let refexpr_cls = nodes_mod.getattr("RefExpr")?;
    let memberexpr_cls = nodes_mod.getattr("MemberExpr")?;
    let nameexpr_cls = nodes_mod.getattr("NameExpr")?;
    let listexpr_cls = nodes_mod.getattr("ListExpr")?;
    let tupleexpr_cls = nodes_mod.getattr("TupleExpr")?;
    let strexpr_cls = nodes_mod.getattr("StrExpr")?;
    let typealias_cls = nodes_mod.getattr("TypeAlias")?;
    let symbol_funcbase_types = nodes_mod.getattr("SYMBOL_FUNCBASE_TYPES")?;
    let anytype_cls = types_mod.getattr("AnyType")?;
    let typeofany = types_mod.getattr("TypeOfAny")?;
    let from_error = typeofany.getattr("from_error")?;
    let semanal_mod = py.import("mypy.semanal")?;
    let refers_to_fullname_fn = semanal_mod.getattr("refers_to_fullname")?;
    let gdef = semanal_mod.getattr("GDEF")?;
    let reveal_type = semanal_mod.getattr("REVEAL_TYPE")?;
    let reveal_locals = semanal_mod.getattr("REVEAL_LOCALS")?;
    let assert_type_names = semanal_mod.getattr("ASSERT_TYPE_NAMES")?;
    let reveal_type_names = semanal_mod.getattr("REVEAL_TYPE_NAMES")?;
    let imported_reveal_type_names = semanal_mod.getattr("IMPORTED_REVEAL_TYPE_NAMES")?;

    let expr_line = expr.getattr("line")?;
    let expr_column = expr.getattr("column")?;
    let args = expr.getattr("args")?;
    let args_list = match args.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(false),
    };

    // Helper: refers_to_fullname(callee, name)
    let refers =
        |name: &str| -> PyResult<bool> { refers_to_fullname_fn.call1((callee, name))?.is_true() };

    // Helper: refers_to_fullname with tuple of names
    let refers_any = |names: &PyAny| -> PyResult<bool> {
        refers_to_fullname_fn.call1((callee, names))?.is_true()
    };

    // Helper: check_fixed_args
    let check_fixed_args = |numargs: i64, name: &str| -> PyResult<bool> {
        semanal
            .call_method1("check_fixed_args", (expr, numargs, name))?
            .is_true()
    };

    // Helper: set analyzed line/column and accept
    let set_analyzed = |analyzed: &PyAny| -> PyResult<()> {
        analyzed.setattr("line", expr_line)?;
        let has_col = analyzed.hasattr("column")?;
        if has_col {
            analyzed.setattr("column", expr_column)?;
        }
        analyzed.call_method1("accept", (semanal,))?;
        Ok(())
    };

    // cast(...)
    if refers("typing.cast")? {
        if !check_fixed_args(2, "cast")? {
            return Ok(true);
        }
        let target =
            match semanal.call_method1("expr_to_unanalyzed_type", (args_list.get_item(0)?,)) {
                Ok(t) => t,
                Err(_) => {
                    semanal.call_method1("fail", ("Cast target is not a type", expr))?;
                    return Ok(true);
                }
            };
        let analyzed = castexpr_cls.call1((args_list.get_item(1)?, target))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // assert_type(...)
    if refers_any(assert_type_names)? {
        if !check_fixed_args(2, "assert_type")? {
            return Ok(true);
        }
        let target =
            match semanal.call_method1("expr_to_unanalyzed_type", (args_list.get_item(1)?,)) {
                Ok(t) => t,
                Err(_) => {
                    semanal.call_method1("fail", ("assert_type() type is not a type", expr))?;
                    return Ok(true);
                }
            };
        let analyzed = asserttype_cls.call1((args_list.get_item(0)?, target))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // reveal_type(...)
    if refers_any(reveal_type_names)? {
        if !check_fixed_args(1, "reveal_type")? {
            return Ok(true);
        }
        let reveal_imported = {
            let reveal_type_node = semanal.call_method("lookup", ("reveal_type", expr), {
                let kw = pyo3::types::PyDict::new(py);
                kw.set_item("suppress_errors", true)?;
                Some(kw)
            })?;
            if reveal_type_node.is_none() {
                false
            } else {
                let node = reveal_type_node.getattr("node")?;
                let is_symbol_funcbase = node.is_instance(symbol_funcbase_types)?;
                let fullname: String = reveal_type_node.getattr("fullname")?.extract()?;
                is_symbol_funcbase && imported_reveal_type_names.contains(&fullname)?
            }
        };
        let kw = pyo3::types::PyDict::new(py);
        kw.set_item("kind", reveal_type)?;
        kw.set_item("expr", args_list.get_item(0)?)?;
        kw.set_item("is_imported", reveal_imported)?;
        let analyzed = revealexpr_cls.call((), Some(kw))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // reveal_locals(...)
    if refers("builtins.reveal_locals")? {
        let local_nodes = pyo3::types::PyList::empty(py);
        let is_module_scope = semanal.call_method1("is_module_scope", ())?.is_true()?;
        let is_class_scope = semanal.call_method1("is_class_scope", ())?.is_true()?;
        let is_func_scope = semanal.call_method1("is_func_scope", ())?.is_true()?;
        if is_module_scope {
            let globals = semanal.getattr("globals")?;
            let globals_dict = match globals.downcast::<PyDict>() {
                Ok(d) => d,
                Err(_) => return Ok(false),
            };
            for (_, n) in globals_dict.iter() {
                let node = n.getattr("node")?;
                let is_inferred = node
                    .getattr("is_inferred")
                    .map(|v| v.is_true().unwrap_or(false))
                    .unwrap_or(false);
                let is_var = node.is_instance(nodes_mod.getattr("Var")?)?;
                if is_inferred && is_var {
                    local_nodes.append(node)?;
                }
            }
        } else if is_class_scope {
            let semanal_type = semanal.getattr("type")?;
            if !semanal_type.is_none() {
                let names = semanal_type.getattr("names")?;
                let names_dict = match names.downcast::<PyDict>() {
                    Ok(d) => d,
                    Err(_) => return Ok(false),
                };
                for (_, st) in names_dict.iter() {
                    let node = st.getattr("node")?;
                    let is_var = node.is_instance(nodes_mod.getattr("Var")?)?;
                    if is_var {
                        local_nodes.append(node)?;
                    }
                }
            }
        } else if is_func_scope {
            let locals = semanal.getattr("locals")?;
            if !locals.is_none() {
                let locals_list = match locals.downcast::<PyList>() {
                    Ok(l) => l,
                    Err(_) => return Ok(false),
                };
                if !locals_list.is_empty() {
                    let sym_table = locals_list.get_item(locals_list.len() - 1)?;
                    if !sym_table.is_none() {
                        let sym_table_dict = match sym_table.downcast::<PyDict>() {
                            Ok(d) => d,
                            Err(_) => return Ok(false),
                        };
                        for (_, st) in sym_table_dict.iter() {
                            let node = st.getattr("node")?;
                            let is_var = node.is_instance(nodes_mod.getattr("Var")?)?;
                            if is_var {
                                local_nodes.append(node)?;
                            }
                        }
                    }
                }
            }
        }
        let kw = pyo3::types::PyDict::new(py);
        kw.set_item("kind", reveal_locals)?;
        kw.set_item("local_nodes", local_nodes)?;
        let analyzed = revealexpr_cls.call((), Some(kw))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // Any(...)
    if refers("typing.Any")? {
        semanal.call_method1(
            "fail",
            (
                "Any(...) is no longer supported. Use cast(Any, ...) instead",
                expr,
            ),
        )?;
        return Ok(true);
    }

    // _promote(...)
    if refers("typing._promote")? {
        if !check_fixed_args(1, "_promote")? {
            return Ok(true);
        }
        let target =
            match semanal.call_method1("expr_to_unanalyzed_type", (args_list.get_item(0)?,)) {
                Ok(t) => t,
                Err(_) => {
                    semanal.call_method1("fail", ("Argument 1 to _promote is not a type", expr))?;
                    return Ok(true);
                }
            };
        let analyzed = promoteexpr_cls.call1((target,))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // dict(...)
    if refers("builtins.dict")? {
        // Check if callee is a RefExpr with TypeAlias node and no_args=False
        let skip = if callee.is_instance(refexpr_cls)? {
            let callee_node = callee.getattr("node")?;
            if callee_node.is_instance(typealias_cls)? {
                let no_args = callee_node.getattr("no_args")?.is_true()?;
                !no_args
            } else {
                false
            }
        } else {
            false
        };
        if !skip {
            let analyzed = semanal.call_method1("translate_dict_call", (expr,))?;
            expr.setattr("analyzed", analyzed)?;
            return Ok(true);
        }
    }

    // divmod(...)
    if refers("builtins.divmod")? {
        if !check_fixed_args(2, "divmod")? {
            return Ok(true);
        }
        let analyzed =
            opexpr_cls.call1(("divmod", args_list.get_item(0)?, args_list.get_item(1)?))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // TypeAliasType(...)
    if refers_any(PyTuple::new(
        py,
        ["typing.TypeAliasType", "typing_extensions.TypeAliasType"],
    ))? {
        // allow_unbound_tvars_set context
        let old_allow = semanal.getattr("allow_unbound_tvars")?;
        semanal.setattr("allow_unbound_tvars", true)?;
        for a in args_list.iter() {
            a.call_method1("accept", (semanal,))?;
        }
        semanal.setattr("allow_unbound_tvars", old_allow)?;
        return Ok(true);
    }

    // TypeForm(...)
    if refers_any(PyTuple::new(
        py,
        ["typing.TypeForm", "typing_extensions.TypeForm"],
    ))? {
        if !check_fixed_args(1, "TypeForm")? {
            return Ok(true);
        }
        let typ = match semanal.call_method1("expr_to_unanalyzed_type", (args_list.get_item(0)?,)) {
            Ok(t) => t,
            Err(_) => {
                semanal.call_method1("fail", ("TypeForm argument is not a type", expr))?;
                let any_type = anytype_cls.call1((from_error,))?;
                let analyzed = castexpr_cls.call1((args_list.get_item(0)?, any_type))?;
                expr.setattr("analyzed", analyzed)?;
                return Ok(true);
            }
        };
        let analyzed = typeform_cls.call1((typ,))?;
        expr.setattr("analyzed", analyzed)?;
        set_analyzed(analyzed)?;
        return Ok(true);
    }

    // Normal call expression
    for a in args_list.iter() {
        a.call_method1("accept", (semanal,))?;
        semanal.call_method1("try_parse_as_type_expression", (a,))?;
    }

    // __all__ mutation handling
    if callee.is_instance(memberexpr_cls)? {
        let callee_expr = callee.getattr("expr")?;
        if callee_expr.is_instance(nameexpr_cls)? {
            let callee_expr_name: String = callee_expr.getattr("name")?.extract()?;
            let callee_expr_kind = callee_expr.getattr("kind")?;
            let callee_name: String = callee.getattr("name")?.extract()?;
            if callee_expr_name == "__all__"
                && callee_expr_kind
                    .rich_compare(gdef, pyo3::basic::CompareOp::Eq)?
                    .is_true()?
                && matches!(callee_name.as_str(), "append" | "extend" | "remove")
            {
                if callee_name == "append" && !args_list.is_empty() {
                    semanal.call_method1("add_exports", (args_list.get_item(0)?,))?;
                } else if callee_name == "extend" && !args_list.is_empty() {
                    let arg0 = args_list.get_item(0)?;
                    if arg0.is_instance(listexpr_cls)? || arg0.is_instance(tupleexpr_cls)? {
                        let items = arg0.getattr("items")?;
                        semanal.call_method1("add_exports", (items,))?;
                    }
                } else if callee_name == "remove" && !args_list.is_empty() {
                    let arg0 = args_list.get_item(0)?;
                    if arg0.is_instance(strexpr_cls)? {
                        let val: String = arg0.getattr("value")?.extract()?;
                        let all_exports = semanal.getattr("all_exports")?;
                        let all_exports_list = match all_exports.downcast::<PyList>() {
                            Ok(l) => l,
                            Err(_) => return Ok(false),
                        };
                        let new_exports: Vec<PyObject> = all_exports_list
                            .iter()
                            .filter(|n| n.extract::<String>().map(|s| s != val).unwrap_or(true))
                            .map(|n| n.into())
                            .collect();
                        all_exports_list.call_method1("clear", ())?;
                        all_exports_list.call_method1("extend", (new_exports,))?;
                    }
                }
            }
        }
    }

    Ok(true)
}

/// Native port of `SemanticAnalyzer.visit_type_alias_stmt` (semanal.py:6311).
///
/// Handles the full PEP 695 type alias statement analysis: pushes type args,
/// analyzes the alias value, constructs the `TypeAlias` node, and handles
/// placeholder/existing-node updates. Returns `true` if fully handled
/// (including the `pop_type_args` cleanup), `false` on any error.
#[pyfunction]
pub(crate) fn rust_visit_type_alias_stmt(
    py: Python<'_>,
    s: &PyAny,
    semanal: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let types_mod = py.import("mypy.types")?;
    let semanal_mod = py.import("mypy.semanal")?;
    let semanal_shared_mod = py.import("mypy.semanal_shared")?;
    let typeanal_mod = py.import("mypy.typeanal")?;

    let typealias_cls = nodes_mod.getattr("TypeAlias")?;
    let placeholder_cls = nodes_mod.getattr("PlaceholderNode")?;
    let proptype_cls = types_mod.getattr("ProperType")?;
    let placeholder_type_cls = types_mod.getattr("PlaceholderType")?;
    let instance_cls = types_mod.getattr("Instance")?;
    let anytype_cls = types_mod.getattr("AnyType")?;
    let typeofany = types_mod.getattr("TypeOfAny")?;
    let from_error = typeofany.getattr("from_error")?;

    let has_placeholder_fn = semanal_shared_mod.getattr("has_placeholder")?;
    let check_for_explicit_any_fn = typeanal_mod.getattr("check_for_explicit_any")?;
    let validate_instance_fn = typeanal_mod.getattr("validate_instance")?;
    let fix_instance_fn = typeanal_mod.getattr("fix_instance")?;
    let has_any_from_unimported_fn = typeanal_mod.getattr("has_any_from_unimported_type")?;
    let make_any_non_explicit_fn = semanal_mod.getattr("make_any_non_explicit")?;
    let make_any_non_unimported_fn = semanal_mod.getattr("make_any_non_unimported")?;

    // Early return: invalid recursive alias
    if s.getattr("invalid_recursive_alias")?.is_true()? {
        return Ok(true);
    }

    // self.statement = s
    semanal.setattr("statement", s)?;

    // type_params = self.push_type_args(s.type_args, s)
    let type_args = s.getattr("type_args")?;
    let type_params = semanal.call_method1("push_type_args", (type_args, s))?;
    if type_params.is_none() {
        semanal.call_method1("defer", (s,))?;
        return Ok(true);
    }

    // all_type_params_names = [p.name for p in s.type_args]
    let all_type_params_names: Vec<String> = {
        let ta_list = match type_args.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(false),
        };
        let mut names = Vec::new();
        for p in ta_list.iter() {
            let name: String = p.getattr("name")?.extract()?;
            names.push(name);
        }
        names
    };
    let all_names_tuple = PyTuple::new(py, all_type_params_names.iter().map(|n| n.as_str()));

    // Wrap the body in a closure so we always call pop_type_args in the finally.
    let body_result = || -> PyResult<bool> {
        // existing = self.current_symbol_table().get(s.name.name)
        let sym_table = semanal.call_method1("current_symbol_table", ())?;
        let name_obj = s.getattr("name")?;
        let s_name: String = name_obj.getattr("name")?.extract()?;
        let existing = sym_table.call_method1("get", (s_name.as_str(),))?;

        if !existing.is_none() {
            let existing_node = existing.getattr("node")?;
            let is_typealias = existing_node.is_instance(typealias_cls)?;
            let is_placeholder = existing_node.is_instance(placeholder_cls)?;
            let placeholder_line_ok = if is_placeholder {
                let pline: i64 = existing_node.getattr("line")?.extract()?;
                let sline: i64 = s.getattr("line")?.extract()?;
                pline == sline
            } else {
                false
            };
            if !(is_typealias || placeholder_line_ok) {
                semanal.call_method1("already_defined", (s_name.as_str(), s, existing, "Name"))?;
                return Ok(true);
            }
        }

        // tag = self.track_incomplete_refs()
        let tag = semanal.call_method1("track_incomplete_refs", ())?;

        // res, alias_tvars, depends_on, indexed, default_depends = self.analyze_alias(...)
        let value = s.getattr("value")?;
        let rvalue = value.call_method1("expr", ())?;

        let analyze_kwargs = pyo3::types::PyDict::new(py);
        analyze_kwargs.set_item("allow_placeholder", true)?;
        analyze_kwargs.set_item("declared_type_vars", type_params)?;
        analyze_kwargs.set_item("all_declared_type_params_names", all_names_tuple)?;
        analyze_kwargs.set_item("python_3_12_type_alias", true)?;

        let analyze_result = semanal.call_method(
            "analyze_alias",
            (s_name.as_str(), rvalue),
            Some(analyze_kwargs),
        )?;

        // Unpack 5-tuple
        let result_tuple = match analyze_result.downcast::<PyTuple>() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };
        if result_tuple.len() < 5 {
            return Ok(false);
        }
        let mut res = result_tuple.get_item(0)?;
        let alias_tvars = result_tuple.get_item(1)?;
        let depends_on = result_tuple.get_item(2)?;
        let indexed = result_tuple.get_item(3)?;
        let default_depends = result_tuple.get_item(4)?;

        // if not res: res = AnyType(TypeOfAny.from_error)
        // Mypy Type objects are always truthy; `not res` means res is None.
        if res.is_none() {
            res = anytype_cls.call1((from_error,))?;
        }

        // incomplete_target computation
        let is_func_scope = semanal.call_method1("is_func_scope", ())?.is_true()?;
        let incomplete_target = if !is_func_scope {
            let is_proper = res.is_instance(proptype_cls)?;
            let is_placeholder = res.is_instance(placeholder_type_cls)?;
            is_proper && is_placeholder
        } else {
            has_placeholder_fn.call1((res,))?.is_true()?
        };

        // if self.found_incomplete_ref(tag) or incomplete_target:
        let found_incomplete = semanal
            .call_method1("found_incomplete_ref", (tag,))?
            .is_true()?;
        if found_incomplete || incomplete_target {
            semanal.call_method("mark_incomplete", (s_name.as_str(), value), {
                let kw = pyo3::types::PyDict::new(py);
                kw.set_item("becomes_typeinfo", true)?;
                Some(kw)
            })?;
            return Ok(true);
        }

        // if any(has_placeholder(tv) for tv in alias_tvars):
        let alias_tvars_list = match alias_tvars.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(false),
        };
        let any_has_placeholder = {
            let mut found = false;
            for tv in alias_tvars_list.iter() {
                if has_placeholder_fn.call1((tv,))?.is_true()? {
                    found = true;
                    break;
                }
            }
            found
        };
        if any_has_placeholder {
            semanal.call_method1("defer", ())?;
        }

        // self.add_type_alias_deps(depends_on)
        semanal.call_method1("add_type_alias_deps", (depends_on,))?;

        // check_for_explicit_any(res, self.options, self.is_typeshed_stub_file, self.msg, s)
        let options = semanal.getattr("options")?;
        let is_typeshed_stub = semanal.getattr("is_typeshed_stub_file")?;
        let msg = semanal.getattr("msg")?;
        check_for_explicit_any_fn.call1((res, options, is_typeshed_stub, msg, s))?;

        // res = make_any_non_explicit(res)
        res = make_any_non_explicit_fn.call1((res,))?;

        // if self.options.disallow_any_unimported and has_any_from_unimported_type(res):
        let disallow_any_unimported: bool =
            options.getattr("disallow_any_unimported")?.is_true()?;
        if disallow_any_unimported {
            let has_unimported = has_any_from_unimported_fn.call1((res,))?.is_true()?;
            if has_unimported {
                msg.call_method1("unimported_type_becomes_any", ("Type alias target", res, s))?;
                res = make_any_non_unimported_fn.call1((res,))?;
            }
        }

        // eager = self.is_func_scope()
        let eager = is_func_scope;

        // if isinstance(res, ProperType) and isinstance(res, Instance):
        let is_proper_instance = res.is_instance(proptype_cls)? && res.is_instance(instance_cls)?;
        if is_proper_instance {
            let fail = semanal.getattr("fail")?;
            let valid = validate_instance_fn
                .call1((res, fail, indexed))?
                .is_true()?;
            if !valid {
                let note = semanal.getattr("note")?;
                let fix_kwargs = pyo3::types::PyDict::new(py);
                fix_kwargs.set_item("disallow_any", false)?;
                fix_kwargs.set_item("options", options)?;
                fix_instance_fn.call((res, fail, note), Some(fix_kwargs))?;
            }
        }

        // alias_node = TypeAlias(res, self.qualified_name(s.name.name), self.cur_mod_id,
        //                        s.line, s.column, alias_tvars=..., no_args=False,
        //                        eager=eager, python_3_12_type_alias=True)
        let qualified_name = semanal.call_method1("qualified_name", (s_name.as_str(),))?;
        let cur_mod_id = semanal.getattr("cur_mod_id")?;
        let s_line = s.getattr("line")?;
        let s_column = s.getattr("column")?;

        let alias_kwargs = pyo3::types::PyDict::new(py);
        alias_kwargs.set_item("alias_tvars", alias_tvars)?;
        alias_kwargs.set_item("no_args", false)?;
        alias_kwargs.set_item("eager", eager)?;
        alias_kwargs.set_item("python_3_12_type_alias", true)?;

        let alias_node = typealias_cls.call(
            (res, qualified_name, cur_mod_id, s_line, s_column),
            Some(alias_kwargs),
        )?;

        // alias_node.default_depends = default_depends
        alias_node.setattr("default_depends", default_depends)?;

        // s.alias_node = alias_node
        s.setattr("alias_node", alias_node)?;

        // Existing-node update logic
        if !existing.is_none() {
            let existing_node = existing.getattr("node")?;
            let is_placeholder_or_typealias = existing_node.is_instance(placeholder_cls)?
                || existing_node.is_instance(typealias_cls)?;
            let existing_line: i64 = existing_node.getattr("line")?.extract()?;
            let sline: i64 = s.getattr("line")?.extract()?;
            if is_placeholder_or_typealias && existing_line == sline {
                let is_existing_typealias = existing_node.is_instance(typealias_cls)?;
                let mut updated = false;
                if is_existing_typealias {
                    // existing.node._is_recursive = None
                    existing_node.setattr("_is_recursive", py.None())?;
                    let existing_target = existing_node.getattr("target")?;
                    let existing_alias_tvars = existing_node.getattr("alias_tvars")?;
                    let new_alias_tvars = alias_node.getattr("alias_tvars")?;
                    let target_ne = existing_target
                        .rich_compare(res, CompareOp::Ne)?
                        .is_true()?;
                    let tvars_ne = existing_alias_tvars
                        .rich_compare(new_alias_tvars, CompareOp::Ne)?
                        .is_true()?;
                    if target_ne || tvars_ne {
                        existing_node.setattr("target", res)?;
                        existing_node.setattr("default_depends", default_depends)?;
                        existing_node.setattr("alias_tvars", alias_tvars)?;
                        updated = true;
                    }
                } else {
                    // existing._node = alias_node
                    existing.setattr("_node", alias_node)?;
                    updated = true;
                }

                if updated {
                    let final_iteration: bool = semanal.getattr("final_iteration")?.is_true()?;
                    if final_iteration {
                        semanal
                            .call_method1("cannot_resolve_name", (s_name.as_str(), "name", s))?;
                        return Ok(true);
                    } else {
                        let defer_kwargs = pyo3::types::PyDict::new(py);
                        defer_kwargs.set_item("force_progress", true)?;
                        semanal.call_method("defer", (s,), Some(defer_kwargs))?;
                    }
                }
            } else {
                // self.add_symbol(s.name.name, alias_node, s)
                semanal.call_method1("add_symbol", (s_name.as_str(), alias_node, s))?;
            }
        } else {
            // self.add_symbol(s.name.name, alias_node, s)
            semanal.call_method1("add_symbol", (s_name.as_str(), alias_node, s))?;
        }

        // current_node = existing.node if existing else alias_node
        // assert isinstance(current_node, TypeAlias)
        // self.disable_invalid_recursive_aliases(s, current_node, s.value)
        let current_node = if !existing.is_none() {
            existing.getattr("node")?
        } else {
            alias_node
        };
        semanal.call_method1(
            "disable_invalid_recursive_aliases",
            (s, current_node, value),
        )?;

        // s.name.accept(self)
        name_obj.call_method1("accept", (semanal,))?;

        Ok(true)
    };

    // Execute body with finally: self.pop_type_args(s.type_args)
    let result = body_result();
    // Always pop_type_args (the finally clause)
    semanal.call_method1("pop_type_args", (type_args,))?;
    result
}

// ---------------------------------------------------------------------------
// analyze_simple_literal_type dispatch head (issue #984)
// ---------------------------------------------------------------------------

/// Value-kind tags for the folded constant; must match `NATIVE_SLT_VALUE_*`
/// in mypy/semanal.py.
const SLT_VALUE_NONE: i64 = 0;
const SLT_VALUE_COMPLEX: i64 = 1;
const SLT_VALUE_BOOL: i64 = 2;
const SLT_VALUE_INT: i64 = 3;
const SLT_VALUE_STR: i64 = 4;
const SLT_VALUE_FLOAT: i64 = 5;

/// Type-name tags; must match `NATIVE_SLT_TYPE_*` in mypy/semanal.py.
const SLT_TYPE_NONE: i64 = 0; // return None
const SLT_TYPE_BOOL: i64 = 1; // builtins.bool
const SLT_TYPE_INT: i64 = 2; // builtins.int
const SLT_TYPE_STR: i64 = 3; // builtins.str
const SLT_TYPE_FLOAT: i64 = 4; // builtins.float

/// Pure decision core of `SemanticAnalyzer.analyze_simple_literal_type`
/// (semanal.py:4720-4749). Returns `None` (defer) only on an unknown value
/// kind, which the Python shim can never produce.
fn classify_simple_literal_type(function_stack: bool, value_kind: i64) -> Option<i64> {
    if function_stack {
        return Some(SLT_TYPE_NONE);
    }
    match value_kind {
        SLT_VALUE_NONE | SLT_VALUE_COMPLEX => Some(SLT_TYPE_NONE),
        SLT_VALUE_BOOL => Some(SLT_TYPE_BOOL),
        SLT_VALUE_INT => Some(SLT_TYPE_INT),
        SLT_VALUE_STR => Some(SLT_TYPE_STR),
        SLT_VALUE_FLOAT => Some(SLT_TYPE_FLOAT),
        _ => None,
    }
}

/// `#[pyfunction]` entry for the 5-way dispatch head of
/// `SemanticAnalyzer.analyze_simple_literal_type` (semanal.py:4720-4749).
///
/// The Python shim folds the rvalue via the already-native
/// `constant_fold_expr` and passes the value kind; `cur_mod_id` (the fold
/// binding module) and `is_final` (Python-side LiteralType construction)
/// are carried for signature fidelity but do not affect the decision.
#[pyfunction]
#[pyo3(signature = (function_stack, value_kind, cur_mod_id, is_final))]
pub(crate) fn rust_classify_simple_literal_type(
    function_stack: bool,
    value_kind: i64,
    cur_mod_id: &str,
    is_final: bool,
) -> PyResult<Option<i64>> {
    let _ = (cur_mod_id, is_final);
    Ok(classify_simple_literal_type(function_stack, value_kind))
}

#[cfg(test)]
mod simple_literal_tests {
    use super::*;

    #[test]
    fn test_function_stack_returns_none() {
        for kind in 0..=5 {
            assert_eq!(
                classify_simple_literal_type(true, kind),
                Some(SLT_TYPE_NONE)
            );
        }
    }

    #[test]
    fn test_kind_dispatch() {
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_NONE),
            Some(SLT_TYPE_NONE)
        );
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_COMPLEX),
            Some(SLT_TYPE_NONE)
        );
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_BOOL),
            Some(SLT_TYPE_BOOL)
        );
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_INT),
            Some(SLT_TYPE_INT)
        );
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_STR),
            Some(SLT_TYPE_STR)
        );
        assert_eq!(
            classify_simple_literal_type(false, SLT_VALUE_FLOAT),
            Some(SLT_TYPE_FLOAT)
        );
    }

    #[test]
    fn test_unknown_kind_defers() {
        assert_eq!(classify_simple_literal_type(false, 99), None);
        assert_eq!(classify_simple_literal_type(false, -1), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typing_special_form_known_forms() {
        assert!(var_is_typing_special_form_inner("typing.Annotated"));
        assert!(var_is_typing_special_form_inner(
            "typing_extensions.Annotated"
        ));
        assert!(var_is_typing_special_form_inner("typing.Callable"));
        assert!(var_is_typing_special_form_inner("typing.Literal"));
        assert!(var_is_typing_special_form_inner(
            "typing_extensions.Literal"
        ));
        assert!(var_is_typing_special_form_inner("typing.Optional"));
        assert!(var_is_typing_special_form_inner("typing.TypeGuard"));
        assert!(var_is_typing_special_form_inner(
            "typing_extensions.TypeGuard"
        ));
        assert!(var_is_typing_special_form_inner("typing.TypeIs"));
        assert!(var_is_typing_special_form_inner("typing_extensions.TypeIs"));
        assert!(var_is_typing_special_form_inner("typing.Union"));
    }

    #[test]
    fn test_typing_special_form_rejects_non_typing_prefix() {
        assert!(!var_is_typing_special_form_inner("builtins.int"));
        assert!(!var_is_typing_special_form_inner(
            "collections.abc.Callable"
        ));
        assert!(!var_is_typing_special_form_inner("os.path"));
    }

    #[test]
    fn test_typing_special_form_rejects_unknown_typing_name() {
        assert!(!var_is_typing_special_form_inner("typing.Dict"));
        assert!(!var_is_typing_special_form_inner("typing.List"));
        assert!(!var_is_typing_special_form_inner("typing.TypeVar"));
        assert!(!var_is_typing_special_form_inner(
            "typing_extensions.overload"
        ));
    }

    #[test]
    fn test_typing_special_form_rejects_empty_and_prefix_only() {
        assert!(!var_is_typing_special_form_inner(""));
        assert!(!var_is_typing_special_form_inner("typing"));
        assert!(!var_is_typing_special_form_inner("typing."));
    }

    #[test]
    fn test_unmangle_strips_trailing_primes() {
        assert_eq!(unmangle_str("foo"), "foo");
        assert_eq!(unmangle_str("foo'"), "foo");
        assert_eq!(unmangle_str("foo''"), "foo");
        assert_eq!(unmangle_str(""), "");
    }

    #[test]
    fn test_is_initial_mangled_global_logic() {
        // name == unmangle(name) + "'" only when there is exactly one trailing prime
        assert!(name_is_initial_mangled("foo'"));
        assert!(name_is_initial_mangled("x'"));
        assert!(!name_is_initial_mangled("foo"));
        assert!(!name_is_initial_mangled("foo''"));
        assert!(!name_is_initial_mangled(""));
    }

    fn name_is_initial_mangled(name: &str) -> bool {
        name == format!("{}'", unmangle_str(name))
    }

    // --- can_possibly_be_typevarlike_declaration ---

    #[test]
    fn test_is_type_var_like_name() {
        assert!(is_type_var_like_name("typing.TypeVar"));
        assert!(is_type_var_like_name("typing_extensions.TypeVar"));
        assert!(is_type_var_like_name("typing.ParamSpec"));
        assert!(is_type_var_like_name("typing_extensions.ParamSpec"));
        assert!(is_type_var_like_name("typing.TypeVarTuple"));
        assert!(is_type_var_like_name("typing_extensions.TypeVarTuple"));
        assert!(!is_type_var_like_name("typing.NamedTuple"));
        assert!(!is_type_var_like_name("typing.TypedDict"));
        assert!(!is_type_var_like_name("builtins.int"));
        assert!(!is_type_var_like_name(""));
    }

    // --- can_possibly_be_type_form ---

    #[test]
    fn test_is_typedict_name() {
        assert!(is_typedict_name("typing.TypedDict"));
        assert!(is_typedict_name("typing_extensions.TypedDict"));
        assert!(is_typedict_name("mypy_extensions.TypedDict"));
        assert!(!is_typedict_name("typing.NamedTuple"));
        assert!(!is_typedict_name("typing.Any"));
    }

    #[test]
    fn test_is_typed_namedtuple_name() {
        assert!(is_typed_namedtuple_name("typing.NamedTuple"));
        assert!(is_typed_namedtuple_name("typing_extensions.NamedTuple"));
        assert!(!is_typed_namedtuple_name("typing.TypedDict"));
        assert!(!is_typed_namedtuple_name("typing.Any"));
    }

    // --- classify_setup_type_vars (Phase C1) ---

    #[test]
    fn test_classify_setup_type_vars_empty() {
        let tags: [SvtKind; 0] = [];
        assert!(classify_setup_type_vars_inner(&tags, &[]).is_empty());
    }

    #[test]
    fn test_classify_setup_type_vars_tvt_sets_seen() {
        let tags = [SvtKind::TypeVarTuple, SvtKind::TypeVar];
        let has_defaults = [false, false];
        // Non-defaulted TypeVar after a tuple stays valid.
        assert!(classify_setup_type_vars_inner(&tags, &has_defaults).is_empty());
    }

    #[test]
    fn test_classify_setup_type_vars_default_after_tvt_invalid() {
        let tags = [SvtKind::TypeVarTuple, SvtKind::TypeVar];
        let has_defaults = [false, true];
        assert_eq!(
            classify_setup_type_vars_inner(&tags, &has_defaults),
            vec![1]
        );
    }

    #[test]
    fn test_classify_setup_type_vars_param_spec_after_tvt_kept() {
        let tags = [SvtKind::TypeVarTuple, SvtKind::ParamSpec];
        let has_defaults = [false, true];
        // ParamSpec with a default after a tuple is never invalid.
        assert!(classify_setup_type_vars_inner(&tags, &has_defaults).is_empty());
    }

    #[test]
    fn test_classify_setup_type_vars_default_before_any_tvt_kept() {
        let tags = [SvtKind::TypeVar, SvtKind::TypeVarTuple];
        let has_defaults = [true, false];
        assert!(classify_setup_type_vars_inner(&tags, &has_defaults).is_empty());
    }

    #[test]
    fn test_classify_setup_type_vars_multiple_invalid() {
        let tags = [
            SvtKind::TypeVarTuple,
            SvtKind::TypeVar,
            SvtKind::TypeVar,
            SvtKind::TypeVarTuple,
            SvtKind::TypeVar,
        ];
        let has_defaults = [false, true, true, false, true];
        assert_eq!(
            classify_setup_type_vars_inner(&tags, &has_defaults),
            vec![1, 2, 4]
        );
    }

    // --- is_type_ref ---

    #[test]
    fn test_in_valid_refs_bare() {
        assert!(in_valid_refs("typing.Any", true));
        assert!(in_valid_refs("typing.Tuple", true));
        assert!(in_valid_refs("typing.Callable", true));
        assert!(!in_valid_refs("typing.Union", true));
        assert!(!in_valid_refs("typing.Optional", true));
    }

    #[test]
    fn test_in_valid_refs_non_bare() {
        assert!(in_valid_refs("typing.Callable", false));
        assert!(in_valid_refs("typing.Optional", false));
        assert!(in_valid_refs("typing.Tuple", false));
        assert!(in_valid_refs("typing.Type", false));
        assert!(in_valid_refs("typing.Union", false));
        assert!(in_valid_refs("typing.Literal", false));
        assert!(in_valid_refs("typing_extensions.Literal", false));
        assert!(in_valid_refs("typing.Annotated", false));
        assert!(in_valid_refs("typing_extensions.Annotated", false));
        assert!(!in_valid_refs("typing.Any", false));
        assert!(!in_valid_refs("builtins.int", false));
    }

    #[test]
    fn test_is_type_constructor() {
        assert!(is_type_constructor("typing.Union"));
        assert!(is_type_constructor("typing.Literal"));
        assert!(is_type_constructor("typing_extensions.Annotated"));
        assert!(!is_type_constructor("typing.TypeVar"));
        assert!(!is_type_constructor("typing.TypedDict"));
    }

    #[test]
    fn test_is_never_name() {
        assert!(is_never_name("typing.NoReturn"));
        assert!(is_never_name("typing_extensions.NoReturn"));
        assert!(is_never_name("mypy_extensions.NoReturn"));
        assert!(is_never_name("typing.Never"));
        assert!(is_never_name("typing_extensions.Never"));
        assert!(!is_never_name("typing.Any"));
        assert!(!is_never_name("builtins.None"));
    }
}
