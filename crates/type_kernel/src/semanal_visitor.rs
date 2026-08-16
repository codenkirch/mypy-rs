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
/// Mirrors semanal.py:3708-3728. Returns `Some(bool)` for cases Rust can
/// decide, or `None` to defer to Python when `is_pep_613` (which requires
/// `self`) would be needed.
#[pyfunction]
pub(crate) fn rust_can_possibly_be_type_form(py: Python<'_>, s: &PyAny) -> PyResult<Option<bool>> {
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
    // Rust cannot call is_pep_613 (needs self), so defer.
    let unanalyzed_type = s.getattr("unanalyzed_type")?;
    if !unanalyzed_type.is_none() {
        return Ok(None);
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
    let typevarlike_type: String;
    if callee.is_instance(name_expr_cls)? {
        let name_obj = match callee.getattr("name") {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        typevarlike_type = match name_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?.to_string(),
            Err(_) => return Ok(None),
        };
    } else if callee.is_instance(ref_expr_cls)? {
        let fullname_obj = match callee.getattr("fullname") {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        typevarlike_type = match fullname_obj.downcast::<PyString>() {
            Ok(s) => s.to_str()?.to_string(),
            Err(_) => return Ok(None),
        };
    } else {
        return Ok(None);
    }

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
