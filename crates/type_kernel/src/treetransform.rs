//! Native `TransformVisitor` port — deep-copy of live mypy AST nodes.
//!
//! Mirrors `mypy.treetransform.TransformVisitor` as a PyO3 extension that
//! walks live Python AST nodes and creates identical copies. Dispatch is by
//! class name (`node.get_type().name()`), the same pattern as `stubgen.rs`.
//!
//! Entry point: `rust_transform_copy(node) -> PyObject`.
//!
//! Returns `None` (deferred) for node types Rust does not handle, so the
//! Python caller can fall back to the pure-Python `TransformVisitor`.
//!
//! Target: PyO3 0.20.x (uses `&PyAny`, not `Bound<'_, PyAny>`).

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// State: var_map and func_placeholder_map, held as Python dicts.
// ---------------------------------------------------------------------------

/// Create a fresh Python dict for var_map / func_placeholder_map.
fn new_dict(py: Python<'_>) -> PyResult<Py<PyDict>> {
    Ok(PyDict::new(py).into())
}

/// Look up a key in a Python dict; returns true if the key is present.
fn dict_contains(dict: &PyDict, key: &PyAny) -> PyResult<bool> {
    dict.contains(key)
}

/// Get a value from a Python dict, or None.
fn dict_get<'py>(dict: &'py PyDict, key: &PyAny) -> PyResult<Option<&'py PyAny>> {
    Ok(dict.get_item(key).unwrap_or(None))
}

/// Set a key/value in a Python dict.
fn dict_set(dict: &PyDict, key: &PyAny, value: &PyAny) -> PyResult<()> {
    dict.set_item(key, value)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers mirroring TransformVisitor's convenience methods.
// ---------------------------------------------------------------------------

/// `expr(e)` → transform + set_line.
fn transform_expr(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let new = transform_node(py, node, var_map, fpm)?;
    let new_ref = new.as_ref(py);
    new_ref.call_method1("set_line", (node,))?;
    Ok(new)
}

/// `optional_expr(e)` → transform or None.
fn transform_optional_expr(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Option<PyObject>> {
    if node.is_none() {
        return Ok(None);
    }
    Ok(Some(transform_expr(py, node, var_map, fpm)?))
}

/// `stmt(s)` → transform + set_line.
fn transform_stmt(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let new = transform_node(py, node, var_map, fpm)?;
    let new_ref = new.as_ref(py);
    new_ref.call_method1("set_line", (node,))?;
    Ok(new)
}

/// `block(b)` → visit_block + set_line.
fn transform_block(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let new = visit_block(py, node, var_map, fpm)?;
    let new_ref = new.as_ref(py);
    new_ref.call_method1("set_line", (node,))?;
    Ok(new)
}

/// `optional_block(b)` → transform or None.
fn transform_optional_block(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Option<PyObject>> {
    if node.is_none() {
        return Ok(None);
    }
    Ok(Some(transform_block(py, node, var_map, fpm)?))
}

/// `statements(list)` → list of transformed stmts.
fn transform_statements(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<PyObject>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(transform_stmt(py, item, var_map, fpm)?);
    }
    Ok(out)
}

/// `expressions(list)` → list of transformed exprs.
fn transform_expressions(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<PyObject>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(transform_expr(py, item, var_map, fpm)?);
    }
    Ok(out)
}

/// `optional_expressions(list)` → list of optional exprs.
fn transform_optional_expressions(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<Option<PyObject>>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(transform_optional_expr(py, item, var_map, fpm)?);
    }
    Ok(out)
}

/// `blocks(list)` → list of transformed blocks.
fn transform_blocks(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<PyObject>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(transform_block(py, item, var_map, fpm)?);
    }
    Ok(out)
}

/// `optional_names(list)` → list of optional NameExpr copies.
fn transform_optional_names(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<Option<PyObject>>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        if item.is_none() {
            out.push(None);
        } else {
            out.push(Some(duplicate_name(py, item, var_map, fpm)?));
        }
    }
    Ok(out)
}

/// `types(list)` → identity (types are not transformed by default).
fn identity_types(py: Python<'_>, nodes: &PyAny) -> PyResult<Vec<PyObject>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(item.into_py(py));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// copy_ref: copy ref attributes from original to new RefExpr.
// ---------------------------------------------------------------------------

fn copy_ref(
    py: Python<'_>,
    new: &PyAny,
    original: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<()> {
    // new.kind = original.kind
    let kind = original.getattr("kind")?;
    new.setattr("kind", kind)?;
    // new.fullname = original.fullname
    let fullname = original.getattr("fullname")?;
    new.setattr("fullname", fullname)?;
    // target = original.node
    let target = original.getattr("node")?;
    // Resolve target: Var → visit_var (unless GDEF), Decorator → visit_var,
    // FuncDef → func_placeholder_map.get.
    let target_type: String = target.get_type().name()?.into();
    let resolved = match target_type.as_str() {
        "Var" => {
            let kind_val = original.getattr("kind")?;
            let kind_int: Option<i64> = kind_val.extract().ok();
            if kind_int == Some(1) {
                // GDEF: don't transform global vars
                target.into_py(py)
            } else {
                visit_var(py, target, var_map, fpm)?
            }
        }
        "Decorator" => {
            let inner_var = target.getattr("var")?;
            visit_var(py, inner_var, var_map, fpm)?
        }
        "FuncDef" => {
            if dict_contains(fpm, target)? {
                let placeholder = dict_get(fpm, target)?.unwrap();
                placeholder.into_py(py)
            } else {
                target.into_py(py)
            }
        }
        _ => target.into_py(py),
    };
    new.setattr("node", resolved.as_ref(py))?;
    let is_new_def = original.getattr("is_new_def")?;
    new.setattr("is_new_def", is_new_def)?;
    let is_inferred_def = original.getattr("is_inferred_def")?;
    new.setattr("is_inferred_def", is_inferred_def)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// duplicate_name: NameExpr copy with copy_ref.
// ---------------------------------------------------------------------------

fn duplicate_name(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let name: String = node.getattr("name")?.extract()?;
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod.getattr("NameExpr")?.call1((name,))?;
    copy_ref(py, new, node, var_map, fpm)?;
    let is_special_form = node.getattr("is_special_form")?;
    new.setattr("is_special_form", is_special_form)?;
    Ok(new.into_py(py))
}

// ---------------------------------------------------------------------------
// visit_var: Var copy with var_map caching.
// ---------------------------------------------------------------------------

fn visit_var(py: Python<'_>, node: &PyAny, var_map: &PyDict, _fpm: &PyDict) -> PyResult<PyObject> {
    if dict_contains(var_map, node)? {
        let existing = dict_get(var_map, node)?.unwrap();
        return Ok(existing.into_py(py));
    }
    let name: String = node.getattr("name")?.extract()?;
    let typ = node.getattr("type")?;
    let type_arg = if typ.is_none() {
        py.None()
    } else {
        typ.into_py(py)
    };
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod.getattr("Var")?.call1((name, type_arg))?;
    // Copy attributes
    let line = node.getattr("line")?;
    new.setattr("line", line)?;
    let fullname = node.getattr("_fullname")?;
    new.setattr("_fullname", fullname)?;
    let info = node.getattr("info")?;
    new.setattr("info", info)?;
    let is_self = node.getattr("is_self")?;
    new.setattr("is_self", is_self)?;
    let is_ready = node.getattr("is_ready")?;
    new.setattr("is_ready", is_ready)?;
    let is_init = node.getattr("is_initialized_in_class")?;
    new.setattr("is_initialized_in_class", is_init)?;
    let is_sm = node.getattr("is_staticmethod")?;
    new.setattr("is_staticmethod", is_sm)?;
    let is_cm = node.getattr("is_classmethod")?;
    new.setattr("is_classmethod", is_cm)?;
    let is_prop = node.getattr("is_property")?;
    new.setattr("is_property", is_prop)?;
    let is_final = node.getattr("is_final")?;
    new.setattr("is_final", is_final)?;
    let final_value = node.getattr("final_value")?;
    new.setattr("final_value", final_value)?;
    let final_unset = node.getattr("final_unset_in_class")?;
    new.setattr("final_unset_in_class", final_unset)?;
    let final_set = node.getattr("final_set_in_init")?;
    new.setattr("final_set_in_init", final_set)?;
    // set_line(node)
    new.call_method1("set_line", (node,))?;
    dict_set(var_map, node, new)?;
    Ok(new.into_py(py))
}

// ---------------------------------------------------------------------------
// copy_argument: Argument copy.
// ---------------------------------------------------------------------------

fn copy_argument(
    py: Python<'_>,
    argument: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let variable = argument.getattr("variable")?;
    let new_var = visit_var(py, variable, var_map, fpm)?;
    let type_annotation = argument.getattr("type_annotation")?;
    let type_arg = if type_annotation.is_none() {
        py.None()
    } else {
        type_annotation.into_py(py)
    };
    let initializer = argument.getattr("initializer")?;
    let init_arg = if initializer.is_none() {
        py.None()
    } else {
        initializer.into_py(py)
    };
    let kind = argument.getattr("kind")?;
    let nodes_mod = py.import("mypy.nodes")?;
    let arg = nodes_mod
        .getattr("Argument")?
        .call1((new_var, type_arg, init_arg, kind))?;
    // set_line(argument)
    arg.call_method1("set_line", (argument,))?;
    Ok(arg.into_py(py))
}

// ---------------------------------------------------------------------------
// copy_function_attributes: shared between FuncDef and LambdaExpr.
// ---------------------------------------------------------------------------

fn copy_function_attributes(py: Python<'_>, new: &PyAny, original: &PyAny) -> PyResult<()> {
    let _ = py;
    let info = original.getattr("info")?;
    new.setattr("info", info)?;
    let min_args = original.getattr("min_args")?;
    new.setattr("min_args", min_args)?;
    let max_pos = original.getattr("max_pos")?;
    new.setattr("max_pos", max_pos)?;
    let is_overload = original.getattr("is_overload")?;
    new.setattr("is_overload", is_overload)?;
    let is_generator = original.getattr("is_generator")?;
    new.setattr("is_generator", is_generator)?;
    let is_coroutine = original.getattr("is_coroutine")?;
    new.setattr("is_coroutine", is_coroutine)?;
    let is_async_gen = original.getattr("is_async_generator")?;
    new.setattr("is_async_generator", is_async_gen)?;
    let is_awaitable = original.getattr("is_awaitable_coroutine")?;
    new.setattr("is_awaitable_coroutine", is_awaitable)?;
    let line = original.getattr("line")?;
    new.setattr("line", line)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// visit_block: Block(body, is_unreachable=...)
// ---------------------------------------------------------------------------

fn visit_block(py: Python<'_>, node: &PyAny, var_map: &PyDict, fpm: &PyDict) -> PyResult<PyObject> {
    let body = node.getattr("body")?;
    let stmts = transform_statements(py, body, var_map, fpm)?;
    let is_unreachable = node.getattr("is_unreachable")?;
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod.getattr("Block")?.call1((stmts,))?;
    new.setattr("is_unreachable", is_unreachable)?;
    Ok(new.into_py(py))
}

// ---------------------------------------------------------------------------
// FuncMapInitializer: scan body for nested FuncDefs, create placeholders.
// ---------------------------------------------------------------------------

fn init_func_placeholders(py: Python<'_>, body_stmts: &PyAny, fpm: &PyDict) -> PyResult<()> {
    let list = body_stmts.downcast::<PyList>()?;
    for stmt in list.iter() {
        let type_name: String = stmt.get_type().name()?.into();
        if type_name == "FuncDef" && !dict_contains(fpm, stmt)? {
            let name: String = stmt.getattr("name")?.extract()?;
            let arguments = stmt.getattr("arguments")?;
            let body = stmt.getattr("body")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let placeholder =
                nodes_mod
                    .getattr("FuncDef")?
                    .call1((name, arguments, body, py.None()))?;
            dict_set(fpm, stmt, placeholder)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Main dispatch: transform_node.
// ---------------------------------------------------------------------------

fn transform_node(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let type_name: String = node.get_type().name()?.into();
    match type_name.as_str() {
        // --- Statements ---
        "ExpressionStmt" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ExpressionStmt")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "AssignmentStmt" => {
            let lvalues = node.getattr("lvalues")?;
            let new_lvalues = transform_expressions(py, lvalues, var_map, fpm)?;
            let rvalue = node.getattr("rvalue")?;
            let new_rvalue = transform_expr(py, rvalue, var_map, fpm)?;
            let unanalyzed_type = node.getattr("unanalyzed_type")?;
            let type_arg = if unanalyzed_type.is_none() {
                py.None()
            } else {
                unanalyzed_type.into_py(py)
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new =
                nodes_mod
                    .getattr("AssignmentStmt")?
                    .call1((new_lvalues, new_rvalue, type_arg))?;
            let line = node.getattr("line")?;
            new.setattr("line", line)?;
            let is_final_def = node.getattr("is_final_def")?;
            new.setattr("is_final_def", is_final_def)?;
            let typ = node.getattr("type")?;
            new.setattr("type", typ)?;
            Ok(new.into_py(py))
        }
        "OperatorAssignmentStmt" => {
            let op: String = node.getattr("op")?.extract()?;
            let lvalue = node.getattr("lvalue")?;
            let rvalue = node.getattr("rvalue")?;
            let new_lvalue = transform_expr(py, lvalue, var_map, fpm)?;
            let new_rvalue = transform_expr(py, rvalue, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("OperatorAssignmentStmt")?
                .call1((op, new_lvalue, new_rvalue))?;
            Ok(new.into_py(py))
        }
        "WhileStmt" => {
            let expr = node.getattr("expr")?;
            let body = node.getattr("body")?;
            let else_body = node.getattr("else_body")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let new_body = transform_block(py, body, var_map, fpm)?;
            let new_else = if let Some(eb) = transform_optional_block(py, else_body, var_map, fpm)?
            {
                eb
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("WhileStmt")?
                .call1((new_expr, new_body, new_else))?;
            Ok(new.into_py(py))
        }
        "ForStmt" => {
            let index = node.getattr("index")?;
            let expr = node.getattr("expr")?;
            let body = node.getattr("body")?;
            let else_body = node.getattr("else_body")?;
            let unanalyzed_index_type = node.getattr("unanalyzed_index_type")?;
            let new_index = transform_expr(py, index, var_map, fpm)?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let new_body = transform_block(py, body, var_map, fpm)?;
            let new_else = if let Some(eb) = transform_optional_block(py, else_body, var_map, fpm)?
            {
                eb
            } else {
                py.None()
            };
            let uit = if unanalyzed_index_type.is_none() {
                py.None()
            } else {
                unanalyzed_index_type.into_py(py)
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("ForStmt")?
                .call1((new_index, new_expr, new_body, new_else, uit))?;
            let is_async = node.getattr("is_async")?;
            new.setattr("is_async", is_async)?;
            let index_type = node.getattr("index_type")?;
            new.setattr("index_type", index_type)?;
            Ok(new.into_py(py))
        }
        "ReturnStmt" => {
            let expr = node.getattr("expr")?;
            let new_expr = if let Some(e) = transform_optional_expr(py, expr, var_map, fpm)? {
                e
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ReturnStmt")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "AssertStmt" => {
            let expr = node.getattr("expr")?;
            let msg = node.getattr("msg")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let new_msg = if let Some(m) = transform_optional_expr(py, msg, var_map, fpm)? {
                m
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("AssertStmt")?
                .call1((new_expr, new_msg))?;
            Ok(new.into_py(py))
        }
        "DelStmt" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("DelStmt")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "IfStmt" => {
            let expr = node.getattr("expr")?;
            let body = node.getattr("body")?;
            let else_body = node.getattr("else_body")?;
            let new_exprs = transform_expressions(py, expr, var_map, fpm)?;
            let new_bodies = transform_blocks(py, body, var_map, fpm)?;
            let new_else = if let Some(eb) = transform_optional_block(py, else_body, var_map, fpm)?
            {
                eb
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("IfStmt")?
                .call1((new_exprs, new_bodies, new_else))?;
            Ok(new.into_py(py))
        }
        "BreakStmt" => {
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("BreakStmt")?.call0()?;
            Ok(new.into_py(py))
        }
        "ContinueStmt" => {
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ContinueStmt")?.call0()?;
            Ok(new.into_py(py))
        }
        "PassStmt" => {
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("PassStmt")?.call0()?;
            Ok(new.into_py(py))
        }
        "RaiseStmt" => {
            let expr = node.getattr("expr")?;
            let from_expr = node.getattr("from_expr")?;
            let new_expr = if let Some(e) = transform_optional_expr(py, expr, var_map, fpm)? {
                e
            } else {
                py.None()
            };
            let new_from = if let Some(f) = transform_optional_expr(py, from_expr, var_map, fpm)? {
                f
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("RaiseStmt")?
                .call1((new_expr, new_from))?;
            Ok(new.into_py(py))
        }
        "TryStmt" => {
            let body = node.getattr("body")?;
            let vars = node.getattr("vars")?;
            let types = node.getattr("types")?;
            let handlers = node.getattr("handlers")?;
            let else_body = node.getattr("else_body")?;
            let finally_body = node.getattr("finally_body")?;
            let new_body = transform_block(py, body, var_map, fpm)?;
            let new_vars = transform_optional_names(py, vars, var_map, fpm)?;
            // types: list[Expression | None]
            let types_list = types.downcast::<PyList>()?;
            let mut new_types = Vec::with_capacity(types_list.len());
            for t in types_list.iter() {
                if t.is_none() {
                    new_types.push(py.None());
                } else {
                    new_types.push(transform_expr(py, t, var_map, fpm)?);
                }
            }
            let new_handlers = transform_blocks(py, handlers, var_map, fpm)?;
            let new_else = if let Some(eb) = transform_optional_block(py, else_body, var_map, fpm)?
            {
                eb
            } else {
                py.None()
            };
            let new_finally =
                if let Some(fb) = transform_optional_block(py, finally_body, var_map, fpm)? {
                    fb
                } else {
                    py.None()
                };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TryStmt")?.call1((
                new_body,
                new_vars,
                new_types,
                new_handlers,
                new_else,
                new_finally,
            ))?;
            let is_star = node.getattr("is_star")?;
            new.setattr("is_star", is_star)?;
            Ok(new.into_py(py))
        }
        "WithStmt" => {
            let expr = node.getattr("expr")?;
            let target = node.getattr("target")?;
            let body = node.getattr("body")?;
            let unanalyzed_type = node.getattr("unanalyzed_type")?;
            let new_exprs = transform_expressions(py, expr, var_map, fpm)?;
            // target: list[Lvalue | None]
            let target_list = target.downcast::<PyList>()?;
            let mut new_targets = Vec::with_capacity(target_list.len());
            for t in target_list.iter() {
                if t.is_none() {
                    new_targets.push(py.None());
                } else {
                    new_targets.push(transform_expr(py, t, var_map, fpm)?);
                }
            }
            let new_body = transform_block(py, body, var_map, fpm)?;
            let ut = if unanalyzed_type.is_none() {
                py.None()
            } else {
                unanalyzed_type.into_py(py)
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new =
                nodes_mod
                    .getattr("WithStmt")?
                    .call1((new_exprs, new_targets, new_body, ut))?;
            let is_async = node.getattr("is_async")?;
            new.setattr("is_async", is_async)?;
            // analyzed_types: identity copy
            let analyzed_types = node.getattr("analyzed_types")?;
            new.setattr("analyzed_types", analyzed_types)?;
            Ok(new.into_py(py))
        }
        "GlobalDecl" => {
            let names = node.getattr("names")?;
            let names_list = names.downcast::<PyList>()?;
            let mut new_names = Vec::with_capacity(names_list.len());
            for n in names_list.iter() {
                new_names.push(n.into_py(py));
            }
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("GlobalDecl")?.call1((new_names,))?;
            Ok(new.into_py(py))
        }
        "NonlocalDecl" => {
            let names = node.getattr("names")?;
            let names_list = names.downcast::<PyList>()?;
            let mut new_names = Vec::with_capacity(names_list.len());
            for n in names_list.iter() {
                new_names.push(n.into_py(py));
            }
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("NonlocalDecl")?.call1((new_names,))?;
            Ok(new.into_py(py))
        }
        "Import" => {
            let ids = node.getattr("ids")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("Import")?.call1((ids,))?;
            Ok(new.into_py(py))
        }
        "ImportFrom" => {
            let id: String = node.getattr("id")?.extract()?;
            let relative: i64 = node.getattr("relative")?.extract()?;
            let names = node.getattr("names")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("ImportFrom")?
                .call1((id, relative, names))?;
            Ok(new.into_py(py))
        }
        "ImportAll" => {
            let id: String = node.getattr("id")?.extract()?;
            let relative: i64 = node.getattr("relative")?.extract()?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ImportAll")?.call1((id, relative))?;
            Ok(new.into_py(py))
        }
        "Decorator" => visit_decorator(py, node, var_map, fpm),
        "FuncDef" => visit_func_def(py, node, var_map, fpm),
        "OverloadedFuncDef" => visit_overloaded_func_def(py, node, var_map, fpm),
        "ClassDef" => visit_class_def(py, node, var_map, fpm),
        // --- Expressions ---
        "NameExpr" => duplicate_name(py, node, var_map, fpm),
        "MemberExpr" => {
            let expr = node.getattr("expr")?;
            let name: String = node.getattr("name")?.extract()?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let member = nodes_mod.getattr("MemberExpr")?.call1((new_expr, name))?;
            let def_var = node.getattr("def_var")?;
            if !def_var.is_none() {
                member.setattr("def_var", def_var)?;
            }
            copy_ref(py, member, node, var_map, fpm)?;
            Ok(member.into_py(py))
        }
        "CallExpr" => {
            let callee = node.getattr("callee")?;
            let args = node.getattr("args")?;
            let arg_kinds = node.getattr("arg_kinds")?;
            let arg_names = node.getattr("arg_names")?;
            let analyzed = node.getattr("analyzed")?;
            let new_callee = transform_expr(py, callee, var_map, fpm)?;
            let new_args = transform_expressions(py, args, var_map, fpm)?;
            let new_analyzed = if let Some(a) = transform_optional_expr(py, analyzed, var_map, fpm)?
            {
                a
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("CallExpr")?.call1((
                new_callee,
                new_args,
                arg_kinds,
                arg_names,
                new_analyzed,
            ))?;
            Ok(new.into_py(py))
        }
        "OpExpr" => {
            let op: String = node.getattr("op")?.extract()?;
            let left = node.getattr("left")?;
            let right = node.getattr("right")?;
            let analyzed = node.getattr("analyzed")?;
            let new_left = transform_expr(py, left, var_map, fpm)?;
            let new_right = transform_expr(py, right, var_map, fpm)?;
            let new_analyzed = if let Some(a) = transform_optional_expr(py, analyzed, var_map, fpm)?
            {
                a
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new =
                nodes_mod
                    .getattr("OpExpr")?
                    .call1((op, new_left, new_right, new_analyzed))?;
            let method_type = node.getattr("method_type")?;
            new.setattr("method_type", method_type)?;
            Ok(new.into_py(py))
        }
        "UnaryExpr" => {
            let op: String = node.getattr("op")?.extract()?;
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("UnaryExpr")?.call1((op, new_expr))?;
            let method_type = node.getattr("method_type")?;
            new.setattr("method_type", method_type)?;
            Ok(new.into_py(py))
        }
        "ComparisonExpr" => {
            let operators = node.getattr("operators")?;
            let operands = node.getattr("operands")?;
            let new_operands = transform_expressions(py, operands, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("ComparisonExpr")?
                .call1((operators, new_operands))?;
            let method_types = node.getattr("method_types")?;
            new.setattr("method_types", method_types)?;
            Ok(new.into_py(py))
        }
        "IntExpr" => {
            let value = node.getattr("value")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("IntExpr")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "StrExpr" => {
            let value: String = node.getattr("value")?.extract()?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("StrExpr")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "BytesExpr" => {
            let value = node.getattr("value")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("BytesExpr")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "FloatExpr" => {
            let value = node.getattr("value")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("FloatExpr")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "ComplexExpr" => {
            let value = node.getattr("value")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ComplexExpr")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "EllipsisExpr" => {
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("EllipsisExpr")?.call0()?;
            Ok(new.into_py(py))
        }
        "StarExpr" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("StarExpr")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "ListExpr" => {
            let items = node.getattr("items")?;
            let new_items = transform_expressions(py, items, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ListExpr")?.call1((new_items,))?;
            Ok(new.into_py(py))
        }
        "SetExpr" => {
            let items = node.getattr("items")?;
            let new_items = transform_expressions(py, items, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("SetExpr")?.call1((new_items,))?;
            Ok(new.into_py(py))
        }
        "TupleExpr" => {
            let items = node.getattr("items")?;
            let new_items = transform_expressions(py, items, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TupleExpr")?.call1((new_items,))?;
            Ok(new.into_py(py))
        }
        "DictExpr" => {
            let items = node.getattr("items")?;
            let list = items.downcast::<PyList>()?;
            let mut new_items: Vec<PyObject> = Vec::with_capacity(list.len());
            for entry in list.iter() {
                let tuple = entry.downcast::<pyo3::types::PyTuple>()?;
                let key = tuple.get_item(0)?;
                let value = tuple.get_item(1)?;
                let new_key = if key.is_none() {
                    py.None()
                } else {
                    transform_expr(py, key, var_map, fpm)?
                };
                let new_value = transform_expr(py, value, var_map, fpm)?;
                let pair = (new_key, new_value);
                new_items.push(pair.into_py(py));
            }
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("DictExpr")?.call1((new_items,))?;
            Ok(new.into_py(py))
        }
        "SliceExpr" => {
            let begin = node.getattr("begin_index")?;
            let end = node.getattr("end_index")?;
            let stride = node.getattr("stride")?;
            let new_begin = if let Some(b) = transform_optional_expr(py, begin, var_map, fpm)? {
                b
            } else {
                py.None()
            };
            let new_end = if let Some(e) = transform_optional_expr(py, end, var_map, fpm)? {
                e
            } else {
                py.None()
            };
            let new_stride = if let Some(s) = transform_optional_expr(py, stride, var_map, fpm)? {
                s
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("SliceExpr")?
                .call1((new_begin, new_end, new_stride))?;
            Ok(new.into_py(py))
        }
        "ConditionalExpr" => {
            let cond = node.getattr("cond")?;
            let if_expr = node.getattr("if_expr")?;
            let else_expr = node.getattr("else_expr")?;
            let new_cond = transform_expr(py, cond, var_map, fpm)?;
            let new_if = transform_expr(py, if_expr, var_map, fpm)?;
            let new_else = transform_expr(py, else_expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("ConditionalExpr")?
                .call1((new_cond, new_if, new_else))?;
            Ok(new.into_py(py))
        }
        "YieldExpr" => {
            let expr = node.getattr("expr")?;
            let new_expr = if let Some(e) = transform_optional_expr(py, expr, var_map, fpm)? {
                e
            } else {
                py.None()
            };
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("YieldExpr")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "YieldFromExpr" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("YieldFromExpr")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "AwaitExpr" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("AwaitExpr")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "AssignmentExpr" => {
            let target = node.getattr("target")?;
            let value = node.getattr("value")?;
            let new_target = duplicate_name(py, target, var_map, fpm)?;
            let new_value = transform_expr(py, value, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("AssignmentExpr")?
                .call1((new_target, new_value))?;
            Ok(new.into_py(py))
        }
        "SuperExpr" => {
            let name: String = node.getattr("name")?.extract()?;
            let call = node.getattr("call")?;
            let new_call = transform_expr(py, call, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("SuperExpr")?.call1((name, new_call))?;
            let info = node.getattr("info")?;
            new.setattr("info", info)?;
            Ok(new.into_py(py))
        }
        "CastExpr" => {
            let expr = node.getattr("expr")?;
            let typ = node.getattr("type")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("CastExpr")?.call1((new_expr, typ))?;
            Ok(new.into_py(py))
        }
        "TypeFormExpr" => {
            let typ = node.getattr("type")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TypeFormExpr")?.call1((typ,))?;
            Ok(new.into_py(py))
        }
        "AssertTypeExpr" => {
            let expr = node.getattr("expr")?;
            let typ = node.getattr("type")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("AssertTypeExpr")?
                .call1((new_expr, typ))?;
            Ok(new.into_py(py))
        }
        "RevealExpr" => {
            let kind: i64 = node.getattr("kind")?.extract()?;
            if kind == 0 {
                // REVEAL_TYPE
                let expr = node.getattr("expr")?;
                let new_expr = transform_expr(py, expr, var_map, fpm)?;
                let nodes_mod = py.import("mypy.nodes")?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("kind", kind)?;
                kwargs.set_item("expr", new_expr)?;
                let new = nodes_mod.getattr("RevealExpr")?.call((), Some(kwargs))?;
                Ok(new.into_py(py))
            } else {
                // Reveal locals: return as-is
                Ok(node.into_py(py))
            }
        }
        "IndexExpr" => {
            let base = node.getattr("base")?;
            let index = node.getattr("index")?;
            let new_base = transform_expr(py, base, var_map, fpm)?;
            let new_index = transform_expr(py, index, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("IndexExpr")?
                .call1((new_base, new_index))?;
            let method_type = node.getattr("method_type")?;
            if !method_type.is_none() {
                new.setattr("method_type", method_type)?;
            }
            let analyzed = node.getattr("analyzed")?;
            if !analyzed.is_none() {
                let analyzed_type: String = analyzed.get_type().name()?.into();
                let new_analyzed: PyObject = if analyzed_type == "TypeApplication" {
                    let inner_expr = analyzed.getattr("expr")?;
                    let inner_types = analyzed.getattr("types")?;
                    let new_inner = transform_expr(py, inner_expr, var_map, fpm)?;
                    let new_types = identity_types(py, inner_types)?;
                    let ta = nodes_mod
                        .getattr("TypeApplication")?
                        .call1((new_inner, new_types))?;
                    ta.into_py(py)
                } else {
                    // TypeAliasExpr: copy node as-is
                    analyzed.into_py(py)
                };
                new.setattr("analyzed", new_analyzed.as_ref(py))?;
                let analyzed_ref = new_analyzed.as_ref(py);
                analyzed_ref.call_method1("set_line", (analyzed,))?;
            }
            Ok(new.into_py(py))
        }
        "LambdaExpr" => visit_lambda_expr(py, node, var_map, fpm),
        "GeneratorExpr" => duplicate_generator(py, node, var_map, fpm),
        "ListComprehension" => {
            let generator = node.getattr("generator")?;
            let new_gen = duplicate_generator(py, generator, var_map, fpm)?;
            let new_gen_ref = new_gen.as_ref(py);
            new_gen_ref.call_method1("set_line", (generator,))?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ListComprehension")?.call1((new_gen,))?;
            Ok(new.into_py(py))
        }
        "SetComprehension" => {
            let generator = node.getattr("generator")?;
            let new_gen = duplicate_generator(py, generator, var_map, fpm)?;
            let new_gen_ref = new_gen.as_ref(py);
            new_gen_ref.call_method1("set_line", (generator,))?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("SetComprehension")?.call1((new_gen,))?;
            Ok(new.into_py(py))
        }
        "DictionaryComprehension" => {
            let key = node.getattr("key")?;
            let value = node.getattr("value")?;
            let indices = node.getattr("indices")?;
            let sequences = node.getattr("sequences")?;
            let condlists = node.getattr("condlists")?;
            let is_async = node.getattr("is_async")?;
            let new_key = transform_expr(py, key, var_map, fpm)?;
            let new_value = transform_expr(py, value, var_map, fpm)?;
            let new_indices = transform_expressions(py, indices, var_map, fpm)?;
            let new_sequences = transform_expressions(py, sequences, var_map, fpm)?;
            let cl_list = condlists.downcast::<PyList>()?;
            let mut new_condlists = Vec::with_capacity(cl_list.len());
            for cl in cl_list.iter() {
                new_condlists.push(transform_expressions(py, cl, var_map, fpm)?);
            }
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("DictionaryComprehension")?.call1((
                new_key,
                new_value,
                new_indices,
                new_sequences,
                new_condlists,
                is_async,
            ))?;
            Ok(new.into_py(py))
        }
        "TypeVarExpr" => {
            let name: String = node.getattr("_name")?.extract()?;
            let fullname: String = node.getattr("fullname")?.extract()?;
            let values = node.getattr("values")?;
            let upper_bound = node.getattr("upper_bound")?;
            let default = node.getattr("default")?;
            let variance = node.getattr("variance")?;
            let new_values = identity_types(py, values)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TypeVarExpr")?.call1((
                name,
                fullname,
                new_values,
                upper_bound,
                default,
            ))?;
            new.setattr("variance", variance)?;
            Ok(new.into_py(py))
        }
        "ParamSpecExpr" => {
            let name: String = node.getattr("_name")?.extract()?;
            let fullname: String = node.getattr("fullname")?.extract()?;
            let upper_bound = node.getattr("upper_bound")?;
            let default = node.getattr("default")?;
            let variance = node.getattr("variance")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("ParamSpecExpr")?.call1((
                name,
                fullname,
                upper_bound,
                default,
            ))?;
            new.setattr("variance", variance)?;
            Ok(new.into_py(py))
        }
        "TypeVarTupleExpr" => {
            let name: String = node.getattr("_name")?.extract()?;
            let fullname: String = node.getattr("fullname")?.extract()?;
            let upper_bound = node.getattr("upper_bound")?;
            let tuple_fallback = node.getattr("tuple_fallback")?;
            let default = node.getattr("default")?;
            let variance = node.getattr("variance")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TypeVarTupleExpr")?.call1((
                name,
                fullname,
                upper_bound,
                tuple_fallback,
                default,
            ))?;
            new.setattr("variance", variance)?;
            Ok(new.into_py(py))
        }
        "TypeAliasExpr" => {
            let inner_node = node.getattr("node")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TypeAliasExpr")?.call1((inner_node,))?;
            Ok(new.into_py(py))
        }
        "NewTypeExpr" => {
            let name: String = node.getattr("name")?.extract()?;
            let old_type = node.getattr("old_type")?;
            let line = node.getattr("line")?;
            let column = node.getattr("column")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("line", line)?;
            kwargs.set_item("column", column)?;
            let new = nodes_mod
                .getattr("NewTypeExpr")?
                .call((name, old_type), Some(kwargs))?;
            let info = node.getattr("info")?;
            new.setattr("info", info)?;
            Ok(new.into_py(py))
        }
        "NamedTupleExpr" => {
            let info = node.getattr("info")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("NamedTupleExpr")?.call1((info,))?;
            Ok(new.into_py(py))
        }
        "EnumCallExpr" => {
            let info = node.getattr("info")?;
            let items = node.getattr("items")?;
            let values = node.getattr("values")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("EnumCallExpr")?
                .call1((info, items, values))?;
            Ok(new.into_py(py))
        }
        "TypedDictExpr" => {
            let info = node.getattr("info")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TypedDictExpr")?.call1((info,))?;
            Ok(new.into_py(py))
        }
        "PromoteExpr" => {
            let typ = node.getattr("type")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("PromoteExpr")?.call1((typ,))?;
            Ok(new.into_py(py))
        }
        "TempNode" => {
            let typ = node.getattr("type")?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TempNode")?.call1((typ,))?;
            Ok(new.into_py(py))
        }
        "TypeApplication" => {
            let expr = node.getattr("expr")?;
            let types = node.getattr("types")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let new_types = identity_types(py, types)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod
                .getattr("TypeApplication")?
                .call1((new_expr, new_types))?;
            Ok(new.into_py(py))
        }
        "TemplateStrExpr" => {
            let items = node.getattr("items")?;
            let list = items.downcast::<PyList>()?;
            let mut new_items: Vec<PyObject> = Vec::with_capacity(list.len());
            for item in list.iter() {
                let item_type: String = item.get_type().name()?.into();
                if item_type == "tuple" {
                    let tuple = item.downcast::<pyo3::types::PyTuple>()?;
                    let value = tuple.get_item(0)?;
                    let source: String = tuple.get_item(1)?.extract()?;
                    let conversion = tuple.get_item(2)?;
                    let format_spec = tuple.get_item(3)?;
                    let new_value = transform_expr(py, value, var_map, fpm)?;
                    let new_fs =
                        if let Some(f) = transform_optional_expr(py, format_spec, var_map, fpm)? {
                            f
                        } else {
                            py.None()
                        };
                    let new_tuple = (new_value, source, conversion, new_fs).into_py(py);
                    new_items.push(new_tuple);
                } else {
                    new_items.push(transform_expr(py, item, var_map, fpm)?);
                }
            }
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("TemplateStrExpr")?.call1((new_items,))?;
            Ok(new.into_py(py))
        }
        // --- Patterns ---
        "AsPattern" => {
            let pattern = node.getattr("pattern")?;
            let name = node.getattr("name")?;
            let new_pattern =
                if let Some(p) = transform_optional_pattern(py, pattern, var_map, fpm)? {
                    p
                } else {
                    py.None()
                };
            let new_name = if let Some(n) = transform_optional_expr(py, name, var_map, fpm)? {
                n
            } else {
                py.None()
            };
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod
                .getattr("AsPattern")?
                .call1((new_pattern, new_name))?;
            Ok(new.into_py(py))
        }
        "OrPattern" => {
            let patterns = node.getattr("patterns")?;
            let new_patterns = transform_patterns(py, patterns, var_map, fpm)?;
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod.getattr("OrPattern")?.call1((new_patterns,))?;
            Ok(new.into_py(py))
        }
        "ValuePattern" => {
            let expr = node.getattr("expr")?;
            let new_expr = transform_expr(py, expr, var_map, fpm)?;
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod.getattr("ValuePattern")?.call1((new_expr,))?;
            Ok(new.into_py(py))
        }
        "SingletonPattern" => {
            let value = node.getattr("value")?;
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod.getattr("SingletonPattern")?.call1((value,))?;
            Ok(new.into_py(py))
        }
        "SequencePattern" => {
            let patterns = node.getattr("patterns")?;
            let new_patterns = transform_patterns(py, patterns, var_map, fpm)?;
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod
                .getattr("SequencePattern")?
                .call1((new_patterns,))?;
            Ok(new.into_py(py))
        }
        "StarredPattern" => {
            let capture = node.getattr("capture")?;
            let new_capture = if let Some(c) = transform_optional_expr(py, capture, var_map, fpm)? {
                c
            } else {
                py.None()
            };
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod
                .getattr("StarredPattern")?
                .call1((new_capture,))?;
            Ok(new.into_py(py))
        }
        "MappingPattern" => {
            let keys = node.getattr("keys")?;
            let values = node.getattr("values")?;
            let rest = node.getattr("rest")?;
            let new_keys = transform_expressions(py, keys, var_map, fpm)?;
            let new_values = transform_patterns(py, values, var_map, fpm)?;
            let new_rest = if let Some(r) = transform_optional_expr(py, rest, var_map, fpm)? {
                r
            } else {
                py.None()
            };
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod
                .getattr("MappingPattern")?
                .call1((new_keys, new_values, new_rest))?;
            Ok(new.into_py(py))
        }
        "ClassPattern" => {
            let class_ref = node.getattr("class_ref")?;
            let positionals = node.getattr("positionals")?;
            let keyword_keys = node.getattr("keyword_keys")?;
            let keyword_values = node.getattr("keyword_values")?;
            let new_class_ref = transform_node(py, class_ref, var_map, fpm)?;
            let new_positionals = transform_patterns(py, positionals, var_map, fpm)?;
            let new_keyword_values = transform_patterns(py, keyword_values, var_map, fpm)?;
            let patterns_mod = py.import("mypy.patterns")?;
            let new = patterns_mod.getattr("ClassPattern")?.call1((
                new_class_ref,
                new_positionals,
                keyword_keys,
                new_keyword_values,
            ))?;
            Ok(new.into_py(py))
        }
        "MatchStmt" => {
            let subject = node.getattr("subject")?;
            let patterns = node.getattr("patterns")?;
            let guards = node.getattr("guards")?;
            let bodies = node.getattr("bodies")?;
            let new_subject = transform_expr(py, subject, var_map, fpm)?;
            let new_patterns = transform_patterns(py, patterns, var_map, fpm)?;
            let new_guards = transform_optional_expressions(py, guards, var_map, fpm)?;
            let new_bodies = transform_blocks(py, bodies, var_map, fpm)?;
            let nodes_mod = py.import("mypy.nodes")?;
            let new = nodes_mod.getattr("MatchStmt")?.call1((
                new_subject,
                new_patterns,
                new_guards,
                new_bodies,
            ))?;
            Ok(new.into_py(py))
        }
        // --- Block ---
        "Block" => visit_block(py, node, var_map, fpm),
        // Var can appear at top level (e.g. class-level attrs); mirror the
        // Python visit_var instead of falling through to None.
        "Var" => Ok(visit_var(py, node, var_map, fpm)?.into_py(py)),
        _ => {
            // Unhandled: return None to defer to Python.
            Ok(py.None())
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern helpers
// ---------------------------------------------------------------------------

fn transform_pattern(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let new = transform_node(py, node, var_map, fpm)?;
    if new.is_none(py) {
        return Ok(new);
    }
    let new_ref = new.as_ref(py);
    new_ref.call_method1("set_line", (node,))?;
    Ok(new)
}

fn transform_optional_pattern(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Option<PyObject>> {
    if node.is_none() {
        return Ok(None);
    }
    Ok(Some(transform_pattern(py, node, var_map, fpm)?))
}

fn transform_patterns(
    py: Python<'_>,
    nodes: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<Vec<PyObject>> {
    let list = nodes.downcast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(transform_pattern(py, item, var_map, fpm)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Compound node visitors
// ---------------------------------------------------------------------------

fn visit_func_def(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    // Init func placeholders for nested FuncDefs in body
    let body = node.getattr("body")?;
    let body_body = body.getattr("body")?;
    init_func_placeholders(py, body_body, fpm)?;

    let name: String = node.getattr("name")?.extract()?;
    let arguments = node.getattr("arguments")?;
    let arg_list = arguments.downcast::<PyList>()?;
    let mut new_args = Vec::with_capacity(arg_list.len());
    for arg in arg_list.iter() {
        new_args.push(copy_argument(py, arg, var_map, fpm)?);
    }
    let new_body = transform_block(py, body, var_map, fpm)?;
    let typ = node.getattr("type")?;
    let type_arg = if typ.is_none() {
        py.None()
    } else {
        typ.into_py(py)
    };

    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod
        .getattr("FuncDef")?
        .call1((name, new_args, new_body, type_arg))?;

    copy_function_attributes(py, new, node)?;

    let fullname = node.getattr("_fullname")?;
    new.setattr("_fullname", fullname)?;
    let is_decorated = node.getattr("is_decorated")?;
    new.setattr("is_decorated", is_decorated)?;
    let is_conditional = node.getattr("is_conditional")?;
    new.setattr("is_conditional", is_conditional)?;
    let abstract_status = node.getattr("abstract_status")?;
    new.setattr("abstract_status", abstract_status)?;
    let is_static = node.getattr("is_static")?;
    new.setattr("is_static", is_static)?;
    let is_class = node.getattr("is_class")?;
    new.setattr("is_class", is_class)?;
    let is_property = node.getattr("is_property")?;
    new.setattr("is_property", is_property)?;
    let is_final = node.getattr("is_final")?;
    new.setattr("is_final", is_final)?;
    let original_def = node.getattr("original_def")?;
    new.setattr("original_def", original_def)?;

    // If there's a placeholder, replace its state with this new node
    if dict_contains(fpm, node)? {
        let placeholder = dict_get(fpm, node)?.unwrap();
        // replace_object_state(placeholder, new)
        let util_mod = py.import("mypy.util")?;
        util_mod
            .getattr("replace_object_state")?
            .call1((placeholder, new))?;
        Ok(placeholder.into_py(py))
    } else {
        Ok(new.into_py(py))
    }
}

fn visit_lambda_expr(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let arguments = node.getattr("arguments")?;
    let body = node.getattr("body")?;
    let typ = node.getattr("type")?;
    let arg_list = arguments.downcast::<PyList>()?;
    let mut new_args = Vec::with_capacity(arg_list.len());
    for arg in arg_list.iter() {
        new_args.push(copy_argument(py, arg, var_map, fpm)?);
    }
    let new_body = transform_block(py, body, var_map, fpm)?;
    let type_arg = if typ.is_none() {
        py.None()
    } else {
        typ.into_py(py)
    };
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod
        .getattr("LambdaExpr")?
        .call1((new_args, new_body, type_arg))?;
    copy_function_attributes(py, new, node)?;
    Ok(new.into_py(py))
}

fn visit_decorator(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let func = node.getattr("func")?;
    let new_func = visit_func_def(py, func, var_map, fpm)?;
    let new_func_ref = new_func.as_ref(py);
    let func_line = func.getattr("line")?;
    new_func_ref.setattr("line", func_line)?;
    let decorators = node.getattr("decorators")?;
    let new_decorators = transform_expressions(py, decorators, var_map, fpm)?;
    let var = node.getattr("var")?;
    let new_var = visit_var(py, var, var_map, fpm)?;
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod
        .getattr("Decorator")?
        .call1((new_func, new_decorators, new_var))?;
    let is_overload = node.getattr("is_overload")?;
    new.setattr("is_overload", is_overload)?;
    Ok(new.into_py(py))
}

fn visit_overloaded_func_def(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let items = node.getattr("items")?;
    let item_list = items.downcast::<PyList>()?;
    let mut new_items: Vec<PyObject> = Vec::with_capacity(item_list.len());
    for (i, item) in item_list.iter().enumerate() {
        let new_item = transform_node(py, item, var_map, fpm)?;
        new_items.push(new_item);
        // set line from old item
        let new_ref = new_items[i].as_ref(py);
        let old_line = item.getattr("line")?;
        new_ref.setattr("line", old_line)?;
    }
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod
        .getattr("OverloadedFuncDef")?
        .call1((new_items,))?;
    let fullname = node.getattr("_fullname")?;
    new.setattr("_fullname", fullname)?;
    let typ = node.getattr("type")?;
    new.setattr("type", typ)?;
    let info = node.getattr("info")?;
    new.setattr("info", info)?;
    let is_static = node.getattr("is_static")?;
    new.setattr("is_static", is_static)?;
    let is_class = node.getattr("is_class")?;
    new.setattr("is_class", is_class)?;
    let is_property = node.getattr("is_property")?;
    new.setattr("is_property", is_property)?;
    let is_final = node.getattr("is_final")?;
    new.setattr("is_final", is_final)?;
    let impl_attr = node.getattr("impl")?;
    if !impl_attr.is_none() {
        let new_impl = transform_node(py, impl_attr, var_map, fpm)?;
        new.setattr("impl", new_impl.as_ref(py))?;
    }
    Ok(new.into_py(py))
}

fn visit_class_def(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let name: String = node.getattr("name")?.extract()?;
    let defs = node.getattr("defs")?;
    let type_vars = node.getattr("type_vars")?;
    let base_type_exprs = node.getattr("base_type_exprs")?;
    let metaclass = node.getattr("metaclass")?;
    let keywords = node.getattr("keywords")?;

    let new_defs = transform_block(py, defs, var_map, fpm)?;
    let new_base_type_exprs = transform_expressions(py, base_type_exprs, var_map, fpm)?;
    let new_metaclass = if let Some(m) = transform_optional_expr(py, metaclass, var_map, fpm)? {
        m
    } else {
        py.None()
    };
    // keywords: dict[str, Expression] → list of (key, value) tuples
    let kw_dict = keywords.downcast::<PyDict>()?;
    let mut new_keywords: Vec<PyObject> = Vec::new();
    for entry in kw_dict.items() {
        let (key, value) = entry.extract::<(&PyAny, &PyAny)>()?;
        let key_str: String = key.extract()?;
        let new_value = transform_expr(py, value, var_map, fpm)?;
        new_keywords.push((key_str, new_value).into_py(py));
    }

    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod.getattr("ClassDef")?.call1((
        name,
        new_defs,
        type_vars,
        new_base_type_exprs,
        new_metaclass,
        new_keywords,
    ))?;
    let fullname = node.getattr("fullname")?;
    new.setattr("fullname", fullname)?;
    let info = node.getattr("info")?;
    new.setattr("info", info)?;
    let decorators = node.getattr("decorators")?;
    let new_decorators = transform_expressions(py, decorators, var_map, fpm)?;
    new.setattr("decorators", new_decorators)?;
    Ok(new.into_py(py))
}

fn duplicate_generator(
    py: Python<'_>,
    node: &PyAny,
    var_map: &PyDict,
    fpm: &PyDict,
) -> PyResult<PyObject> {
    let left_expr = node.getattr("left_expr")?;
    let indices = node.getattr("indices")?;
    let sequences = node.getattr("sequences")?;
    let condlists = node.getattr("condlists")?;
    let is_async = node.getattr("is_async")?;
    let new_left = transform_expr(py, left_expr, var_map, fpm)?;
    let new_indices = transform_expressions(py, indices, var_map, fpm)?;
    let new_sequences = transform_expressions(py, sequences, var_map, fpm)?;
    let cl_list = condlists.downcast::<PyList>()?;
    let mut new_condlists = Vec::with_capacity(cl_list.len());
    for cl in cl_list.iter() {
        new_condlists.push(transform_expressions(py, cl, var_map, fpm)?);
    }
    let nodes_mod = py.import("mypy.nodes")?;
    let new = nodes_mod.getattr("GeneratorExpr")?.call1((
        new_left,
        new_indices,
        new_sequences,
        new_condlists,
        is_async,
    ))?;
    Ok(new.into_py(py))
}

// ---------------------------------------------------------------------------
// PyO3 entry point
// ---------------------------------------------------------------------------

/// Deep-copy a mypy AST node by walking it and creating new instances.
///
/// Returns a new node (identity transform) or `None` if the node type is
/// not handled, so the Python caller falls back to the pure-Python
/// `TransformVisitor`.
#[pyfunction]
pub fn rust_transform_copy(py: Python<'_>, node: &PyAny) -> PyResult<Option<PyObject>> {
    let var_map = new_dict(py)?;
    let fpm = new_dict(py)?;
    let result = transform_node(py, node, var_map.as_ref(py), fpm.as_ref(py))?;
    if result.is_none(py) {
        return Ok(None);
    }
    // set_line on the top-level result (mirrors TransformVisitor.node())
    let result_ref = result.as_ref(py);
    result_ref.call_method1("set_line", (node,))?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyDict;

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn transform_int_expr_creates_copy() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.nodes import IntExpr
node = IntExpr(42)
node.set_line(7)
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            let result = rust_transform_copy(py, node).unwrap().unwrap();
            let result_ref = result.as_ref(py);
            let value: i64 = result_ref.getattr("value").unwrap().extract().unwrap();
            assert_eq!(value, 42);
            let line: i64 = result_ref.getattr("line").unwrap().extract().unwrap();
            assert_eq!(line, 7);
            // Different object
            assert!(!result_ref.is(node));
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn transform_name_expr_copies_ref() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.nodes import NameExpr
node = NameExpr("foo")
node.fullname = "mod.foo"
node.kind = 1  # GDEF
node.set_line(3)
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            let result = rust_transform_copy(py, node).unwrap().unwrap();
            let result_ref = result.as_ref(py);
            let name: String = result_ref.getattr("name").unwrap().extract().unwrap();
            assert_eq!(name, "foo");
            let fullname: String = result_ref.getattr("fullname").unwrap().extract().unwrap();
            assert_eq!(fullname, "mod.foo");
            let kind: i64 = result_ref.getattr("kind").unwrap().extract().unwrap();
            assert_eq!(kind, 1);
        });
    }

    #[test]
    fn transform_unhandled_returns_none() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run("node = object()", None, Some(locals)).unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            let result = rust_transform_copy(py, node).unwrap();
            assert!(result.is_none());
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn transform_var_uses_var_map() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.nodes import Var
v = Var("x")
v._fullname = "mod.x"
v.set_line(5)
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let v = locals.get_item("v").unwrap().unwrap();
            let result = rust_transform_copy(py, v).unwrap().unwrap();
            let result_ref = result.as_ref(py);
            let name: String = result_ref.getattr("name").unwrap().extract().unwrap();
            assert_eq!(name, "x");
            let fullname: String = result_ref.getattr("_fullname").unwrap().extract().unwrap();
            assert_eq!(fullname, "mod.x");
            let line: i64 = result_ref.getattr("line").unwrap().extract().unwrap();
            assert_eq!(line, 5);
        });
    }
}
