//! Stubgen annotation renderer.
//!
//! Walks live Python mypy AST expression nodes (`NameExpr`, `MemberExpr`,
//! `IndexExpr`, `UnaryExpr`, `TupleExpr`, `StrExpr`, etc.) and produces the
//! stub text representation — the same logic as `AliasPrinter` in
//! `mypy.stubgen`.
//!
//! Entry point: `rust_stubgen_render(expr: Node) -> Option[str]`.
//!
//! Target: PyO3 0.20.x (uses `&PyAny`, not `Bound<'_, PyAny>`).

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Walk a mypy AST expression node and return the rendered stub text,
/// or `None` if Rust does not recognise this node type.
fn render_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let type_name: String = node.get_type().name()?.into();

    match type_name.as_str() {
        "NameExpr" => render_name_expr(py, node),
        "MemberExpr" => render_member_expr(py, node),
        "IndexExpr" => render_index_expr(py, node),
        "UnaryExpr" => render_unary_expr(py, node),
        "TupleExpr" => render_tuple_expr(py, node),
        "StrExpr" => render_str_expr(py, node),
        "IntExpr" => render_int_expr(py, node),
        "FloatExpr" => render_float_expr(py, node),
        "ComplexExpr" => render_complex_expr(py, node),
        "BytesExpr" => render_bytes_expr(py, node),
        "ListExpr" => render_list_expr(py, node),
        "SetExpr" => render_set_expr(py, node),
        "DictExpr" => render_dict_expr(py, node),
        "CallExpr" => render_call_expr(py, node),
        "OpExpr" => render_op_expr(py, node),
        "SliceExpr" => render_slice_expr(py, node),
        "StarExpr" => render_star_expr(py, node),
        "EllipsisExpr" => Ok(Some("...".to_string())),
        "TemplateStrExpr" => Ok(Some("Incomplete".to_string())),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Leaf nodes
// ---------------------------------------------------------------------------

fn render_name_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    let name: String = node.getattr("name")?.extract()?;
    Ok(Some(name))
}

fn render_member_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    // Prefer `.fullname` (set by semantic analysis); fall back to building
    // the dotted chain from `.expr`.
    if let Ok(fullname) = node.getattr("fullname").and_then(|v| v.extract::<String>()) {
        if !fullname.is_empty() {
            return Ok(Some(fullname));
        }
    }
    // Build dotted chain: e.g. MemberExpr{name="bar", expr=NameExpr{name="foo"}}
    // → "foo.bar"
    let name: String = node.getattr("name")?.extract()?;
    let expr = node.getattr("expr")?;
    let expr_type_name: String = expr.get_type().name()?.into();
    if expr_type_name == "NameExpr" {
        let inner: String = expr.getattr("name")?.extract()?;
        return Ok(Some(format!("{}.{}", inner, name)));
    }
    // Nested MemberExpr — recurse.
    let inner_str = render_expr(py, expr)?;
    match inner_str {
        Some(s) => Ok(Some(format!("{}.{}", s, name))),
        None => Ok(None),
    }
}

fn render_str_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    // Delegate to Python's repr() for exact match on quote style,
    // escaping, etc.
    let value = node.getattr("value")?;
    let repr_fn = py.import("builtins")?.getattr("repr")?;
    Ok(Some(repr_fn.call1((value,))?.extract()?))
}

fn render_int_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    // Python repr(int) is exact and matches AliasPrinter._visit_literal_node.
    let value = node.getattr("value")?;
    let repr_fn = py.import("builtins")?.getattr("repr")?;
    Ok(Some(repr_fn.call1((value,))?.extract()?))
}

fn render_float_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    // Delegate to Python repr() so whole floats stay "1.0" and exponents
    // match repr formatting exactly (e.g. 1e+16 not 1e16).
    let value = node.getattr("value")?;
    let repr_fn = py.import("builtins")?.getattr("repr")?;
    Ok(Some(repr_fn.call1((value,))?.extract()?))
}

fn render_complex_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    // value is a Python complex; build "1+2j" exactly like AliasPrinter's
    // repr(node.value). Delegate to Python repr().
    let value = node.getattr("value")?;
    let repr_fn = py.import("builtins")?.getattr("repr")?;
    Ok(Some(repr_fn.call1((value,))?.extract()?))
}

fn render_bytes_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let _py = py;
    let value = node.getattr("value")?;
    // bytes value is a Python bytes; AliasPrinter returns "b" + repr(value).
    let py_repr = py.import("builtins")?.getattr("repr")?;
    Ok(Some(format!(
        "b{}",
        py_repr.call1((value,))?.extract::<String>()?
    )))
}

// ---------------------------------------------------------------------------
// Composite nodes
// ---------------------------------------------------------------------------

/// IndexExpr → `base[index]` with special handling for typing.Union / typing.Optional.
fn render_index_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let base = node.getattr("base")?;
    let index = node.getattr("index")?;

    // Resolve the base's fully-qualified name for special-case detection.
    let base_fullname = get_expr_qualified_name(base)?;

    // Check for Union / Optional special handling.
    // The Python AliasPrinter checks `base_fullname == "typing.Union"` etc.
    // Only the fully-qualified names trigger the special syntax; bare
    // "Union" / "Optional" are passed through as-is.
    // AliasPrinter checks exact fullnames only; bare "Union"/"Optional" pass
    // through as regular names.
    let is_typing_union = matches!(base_fullname.as_str(), "typing.Union");
    let is_typing_optional = matches!(base_fullname.as_str(), "typing.Optional");

    if is_typing_union {
        let type_name: String = index.get_type().name()?.into();
        if type_name == "TupleExpr" {
            // `Union[A, B, C]` → `A | B | C`
            let tuple_items = index.downcast::<PyList>()?;
            let mut parts = Vec::new();
            for item in tuple_items.iter() {
                match render_expr(py, item)? {
                    Some(s) => parts.push(s),
                    None => return Ok(None),
                }
            }
            return Ok(Some(parts.join(" | ")));
        }
        // Union without tuple index → just render the index.
        return render_expr(py, index);
    }

    if is_typing_optional {
        let type_name: String = index.get_type().name()?.into();
        if type_name == "TupleExpr" {
            // Optional with tuple → Incomplete (not expected in practice,
            // mirrors Python stubgen's `add_name("_typeshed.Incomplete")`,
            // which returns the short name "Incomplete").
            return Ok(Some("Incomplete".to_string()));
        }
        let index_str = render_expr(py, index)?;
        return match index_str {
            Some(s) => Ok(Some(format!("{} | None", s))),
            None => Ok(None),
        };
    }

    // Normal generic indexing: `base[index]`.
    let base_str = render_expr(py, base)?;
    let index_str = render_expr(py, index)?;
    match (base_str, index_str) {
        (Some(b), Some(i)) => {
            // Strip leading '(' and trailing ')' from index if it wraps a tuple
            let formatted = if i.len() > 2 && i.starts_with('(') && i.ends_with(')') {
                let inner = &i[1..i.len() - 1].trim_end_matches(',');
                format!("{}[{}]", b, inner)
            } else {
                format!("{}[{}]", b, i)
            };
            Ok(Some(formatted))
        }
        _ => Ok(None),
    }
}

fn render_unary_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let op: String = node.getattr("op")?.extract()?;
    let expr = node.getattr("expr")?;
    let inner = render_expr(py, expr)?;
    match inner {
        Some(s) => Ok(Some(format!("{}{}", op, s))),
        None => Ok(None),
    }
}

fn render_tuple_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let items = node.getattr("items")?.downcast::<PyList>()?;
    let mut rendered = Vec::with_capacity(items.len());
    for item in items.iter() {
        match render_expr(py, item)? {
            Some(s) => rendered.push(s),
            None => return Ok(None),
        }
    }
    let suffix = if rendered.len() == 1 { "," } else { "" };
    Ok(Some(format!("({}{})", rendered.join(", "), suffix)))
}

fn render_list_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let items = node.getattr("items")?.downcast::<PyList>()?;
    let mut rendered = Vec::with_capacity(items.len());
    for item in items.iter() {
        match render_expr(py, item)? {
            Some(s) => rendered.push(s),
            None => return Ok(None),
        }
    }
    Ok(Some(format!("[{}]", rendered.join(", "))))
}

fn render_set_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let items = node.getattr("items")?.downcast::<PyList>()?;
    let mut rendered = Vec::with_capacity(items.len());
    for item in items.iter() {
        match render_expr(py, item)? {
            Some(s) => rendered.push(s),
            None => return Ok(None),
        }
    }
    Ok(Some(format!("{{{}}}", rendered.join(", "))))
}

fn render_dict_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let items = node.getattr("items")?.downcast::<PyList>()?;
    let mut entries = Vec::with_capacity(items.len());
    for item in items.iter() {
        let tuple = item.downcast::<pyo3::types::PyTuple>()?;
        let key = render_expr(py, tuple.get_item(0)?)?;
        let value = render_expr(py, tuple.get_item(1)?)?;
        match (key, value) {
            (Some(k), Some(v)) => entries.push(format!("{}: {}", k, v)),
            _ => return Ok(None),
        }
    }
    Ok(Some(format!("{{{}}}", entries.join(", "))))
}

fn render_call_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let callee = node.getattr("callee")?;
    let args = node.getattr("args")?.downcast::<PyList>()?;
    let arg_names: Vec<Option<String>> = node
        .getattr("arg_names")
        .and_then(|a| a.extract())
        .unwrap_or_default();
    let arg_kinds: Vec<i64> = node
        .getattr("arg_kinds")
        .and_then(|k| k.extract())
        .unwrap_or_default();

    let callee_str = render_expr(py, callee)?;
    let callee_name = callee_str.unwrap_or_default();

    let mut formatted_args = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        let arg_str = render_expr(py, arg)?;
        match arg_str {
            Some(s) => {
                let kind = if i < arg_kinds.len() { arg_kinds[i] } else { 0 };
                let name = if i < arg_names.len() {
                    arg_names[i].clone()
                } else {
                    None
                };
                // ArgKind mirrors mypy.nodes: ARG_POS=0, ARG_OPT=1, ARG_STAR=2,
                // ARG_NAMED=3, ARG_STAR2=4, ARG_NAMED_OPT=5. AliasPrinter maps
                // 2→"*x", 4→"**x", 3→"name=x", everything else plain.
                match kind {
                    2 => formatted_args.push(format!("*{}", s)), // ARG_STAR
                    3 => formatted_args.push(format!("{}={}", name.unwrap_or_default(), s)), // ARG_NAMED
                    4 => formatted_args.push(format!("**{}", s)), // ARG_STAR2
                    _ => formatted_args.push(s), // ARG_POS / ARG_OPT / ARG_NAMED_OPT
                }
            }
            None => return Ok(None),
        }
    }
    Ok(Some(format!(
        "{}({})",
        callee_name,
        formatted_args.join(", ")
    )))
}

fn render_op_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let left = node.getattr("left")?;
    let op: String = node.getattr("op")?.extract()?;
    let right = node.getattr("right")?;
    let left_str = render_expr(py, left)?;
    let right_str = render_expr(py, right)?;
    match (left_str, right_str) {
        (Some(l), Some(r)) => Ok(Some(format!("{} {} {}", l, op, r))),
        _ => Ok(None),
    }
}

fn render_slice_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let begin = node.getattr("begin_index")?;
    let end = node.getattr("end_index")?;
    let stride = node.getattr("stride")?;

    let begin_str = if begin.is_none() {
        String::new()
    } else {
        render_expr(py, begin)?.unwrap_or_default()
    };
    let end_str = if end.is_none() {
        String::new()
    } else {
        render_expr(py, end)?.unwrap_or_default()
    };
    let stride_str = if stride.is_none() {
        String::new()
    } else {
        render_expr(py, stride)?.unwrap_or_default()
    };

    let mut parts = Vec::new();
    parts.push(begin_str);
    parts.push(end_str);
    if !stride_str.is_empty() {
        parts.push(stride_str);
    }
    Ok(Some(parts.join(":")))
}

fn render_star_expr(py: Python<'_>, node: &PyAny) -> PyResult<Option<String>> {
    let expr = node.getattr("expr")?;
    let inner = render_expr(py, expr)?;
    match inner {
        Some(s) => Ok(Some(format!("*{}", s))),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mirror `mypy.stubutil.get_qualified_name` for NameExpr and MemberExpr.
fn get_expr_qualified_name(node: &PyAny) -> PyResult<String> {
    let type_name: String = node.get_type().name()?.into();
    match type_name.as_str() {
        "NameExpr" => node
            .getattr("name")
            .and_then(|v| v.extract::<String>())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("NameExpr name: {}", e))),
        "MemberExpr" => {
            let name: String = node.getattr("name")?.extract()?;
            let expr = node.getattr("expr")?;
            let inner = get_expr_qualified_name(expr)?;
            Ok(format!("{}.{}", inner, name))
        }
        _ => Ok(String::new()),
    }
}

// ---------------------------------------------------------------------------
// PyO3 entry points
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Render a mypy AST expression node as stub text.
///
/// Returns `Some(string)` on success, `None` if the node type is not
/// handled (the Python caller should fall back to the pure-Python
/// `AliasPrinter`).
#[pyfunction]
pub fn rust_stubgen_render(py: Python<'_>, expr: &PyAny) -> PyResult<Option<String>> {
    render_expr(py, expr)
}

/// Render a list of mypy AST expression nodes as a comma-separated
/// string of type arguments (used for IndexExpr inner rendering).
#[pyfunction]
pub fn rust_stubgen_render_type_args(py: Python<'_>, items: &PyAny) -> PyResult<Option<String>> {
    let list = items.downcast::<PyList>()?;
    let mut rendered = Vec::with_capacity(list.len());
    for item in list.iter() {
        match render_expr(py, item)? {
            Some(s) => rendered.push(s),
            None => return Ok(None),
        }
    }
    Ok(Some(rendered.join(", ")))
}
