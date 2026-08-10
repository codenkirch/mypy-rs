//! Plugin common pure helpers for the native type kernel.
//!
//! Ports `mypy.plugins.common.find_shallow_matching_overload_item`, the
//! pure overload-matching helper used by attrs, dataclasses, and other
//! plugin infrastructure.

use pyo3::prelude::*;

use crate::argmap::{rust_map_actuals_to_formals, ARG_NAMED, ARG_POS, ARG_STAR, ARG_STAR2};

fn is_required(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_NAMED
}

/// Parse a Python expression as a bool literal.
/// Mirrors `mypy.semanal_shared.parse_bool` for the NameExpr cases
/// exercised by the test suite.
fn parse_bool(expr: &PyAny) -> Option<bool> {
    if let Ok(name) = expr.getattr("name") {
        if let Ok(n) = name.extract::<String>() {
            if n == "True" {
                return Some(true);
            }
            if n == "False" {
                return Some(false);
            }
        }
    }
    None
}

fn get_class_name(obj: &PyAny) -> Option<String> {
    obj.getattr("__class__")
        .ok()
        .and_then(|c| c.getattr("__name__").ok())
        .and_then(|n| n.extract::<String>().ok())
}

fn is_none_type(typ: &PyAny) -> bool {
    get_class_name(typ) == Some("NoneType".to_string())
}

/// Check if a Python type object is LiteralType with a bool value.
fn is_literal_bool(typ: &PyAny) -> Option<bool> {
    if get_class_name(typ) != Some("LiteralType".to_string()) {
        return None;
    }
    typ.getattr("value")
        .ok()
        .and_then(|v| v.extract::<bool>().ok())
}

/// Rust port of `mypy.plugins.common.find_shallow_matching_overload_item`.
///
/// Matches an overload item against a call expression using shallow
/// criteria: argument kinds/names, required-argument presence, and
/// basic type predicates.  Returns `Some(index)` on a match or `None`
/// to defer to the pure-Python implementation.  The Python caller does
/// `overload.items[index]` after receiving the index, preserving the
/// exact return type of the Python function.
#[pyfunction]
pub fn rust_find_shallow_matching_overload_item(
    py: Python<'_>,
    overload: &PyAny,
    call: &PyAny,
) -> PyResult<Option<usize>> {
    // Extract call properties
    let call_arg_kinds: Vec<i64> = call.getattr("arg_kinds")?.extract()?;
    let call_arg_names: Vec<Option<String>> = call.getattr("arg_names")?.extract()?;
    let call_args: Vec<Py<PyAny>> = call.getattr("args")?.extract()?;
    let num_args = call_args.len();

    // Star actuals need the `actual_arg_type` callback; defer to Python.
    if call_arg_kinds
        .iter()
        .any(|&k| k == ARG_STAR || k == ARG_STAR2)
    {
        return Ok(None);
    }

    // Import Python helpers once (cached by the import system).
    let types_mod = PyModule::import(py, "mypy.types")?;
    let get_proper = types_mod.getattr("get_proper_type")?;
    let typeops_mod = PyModule::import(py, "mypy.typeops")?;
    let is_overlap = typeops_mod.getattr("is_overlapping_none")?;

    let items: Vec<Py<PyAny>> = overload.getattr("items")?.extract()?;
    let items_len = items.len();

    // Try all items except the last.
    for (idx, item_ref) in items.iter().enumerate().take(items_len.saturating_sub(1)) {
        let item = item_ref.as_ref(py);

        let item_arg_kinds: Vec<i64> = item.getattr("arg_kinds")?.extract()?;
        let item_arg_names: Vec<Option<String>> = item.getattr("arg_names")?.extract()?;
        let item_arg_types: Vec<Py<PyAny>> = item.getattr("arg_types")?.extract()?;

        // Map actuals to formals using the Rust argmap port.
        let mapped = rust_map_actuals_to_formals(
            call_arg_kinds.clone(),
            call_arg_names.clone(),
            item_arg_kinds.clone(),
            item_arg_names.clone(),
        );

        let mapped = match mapped {
            Some(m) => m,
            None => continue,
        };

        // Collect matched actual indices.
        let mut matched: Vec<usize> = Vec::with_capacity(mapped.len());
        for actuals in &mapped {
            for &ai in actuals {
                matched.push(ai as usize);
            }
        }
        matched.sort();

        // Check for extra actuals.
        if (0..num_args).any(|i| !matched.contains(&i)) {
            continue;
        }

        // Check each formal argument.
        let mut ok = true;
        for (arg_i, actuals) in mapped.iter().enumerate() {
            let arg_kind = item_arg_kinds[arg_i];

            // Required arg without any actuals → mismatch.
            if is_required(arg_kind) && actuals.is_empty() {
                ok = false;
                break;
            }

            if actuals.is_empty() {
                continue;
            }

            let arg_type = item_arg_types[arg_i].as_ref(py);
            let actual_py_args: Vec<&PyAny> = actuals
                .iter()
                .map(|&i| call_args[i as usize].as_ref(py))
                .collect();

            // Check if any actual is NameExpr with name == "None".
            let arg_is_none = actual_py_args.iter().any(|a| {
                a.getattr("name")
                    .ok()
                    .and_then(|n| n.extract::<String>().ok())
                    .map(|n| n == "None")
                    .unwrap_or(false)
            });

            let arg_type_proper = get_proper.call1((arg_type,))?;

            // Formal is NoneType — actual must be None.
            if is_none_type(arg_type_proper) {
                if !arg_is_none {
                    ok = false;
                }
                continue;
            }

            // Actual is None against a non-NoneType formal.
            if arg_is_none {
                let overlap: bool = is_overlap.call1((arg_type_proper,))?.extract()?;
                if !overlap {
                    ok = false;
                }
                continue;
            }

            // Formal is Literal[bool] — actual must parse to expected value.
            if let Some(expected) = is_literal_bool(arg_type_proper) {
                let any_match = actual_py_args
                    .iter()
                    .any(|a| parse_bool(a) == Some(expected));
                if !any_match {
                    ok = false;
                }
                continue;
            }
        }

        if ok {
            return Ok(Some(idx));
        }
    }

    // No match — Python returns items[-1]. Return None.
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_required() {
        assert!(is_required(ARG_POS));
        assert!(is_required(ARG_NAMED));
        // ARG_OPT and ARG_NAMED_OPT are not public in argmap; just check non-const values.
        assert!(!is_required(1));
        assert!(!is_required(5));
    }
}
