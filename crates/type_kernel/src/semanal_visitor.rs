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
//! - `get_deprecated` — extract deprecation string from a `CallExpr` decorator (Issue #391).
//! - `get_name_repr_of_expr` — simplified textual representation of an expression (Issue #391).

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
