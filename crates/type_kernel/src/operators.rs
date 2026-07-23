//! Native port of `mypy/operators.py` operator method/symbol data tables.
//!
//! These are pure static mappings from Python operator symbols to dunder
//! method names (and reverse). They have zero runtime dependencies and
//! are used throughout the type checker for operator method lookup.
//!
//! Parity: the data is exposed as `#[pyfunction]`s returning Python dicts,
//! mirroring the module-level constants in `mypy/operators.py`.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PySet};

/// Build the `op_methods` dict: binary operator symbol -> dunder method name.
/// Mirrors `mypy.operators.op_methods`.
fn op_methods(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    let entries: &[(&str, &str)] = &[
        ("+", "__add__"),
        ("-", "__sub__"),
        ("*", "__mul__"),
        ("/", "__truediv__"),
        ("%", "__mod__"),
        ("divmod", "__divmod__"),
        ("//", "__floordiv__"),
        ("**", "__pow__"),
        ("@", "__matmul__"),
        ("&", "__and__"),
        ("|", "__or__"),
        ("^", "__xor__"),
        ("<<", "__lshift__"),
        (">>", "__rshift__"),
        ("==", "__eq__"),
        ("!=", "__ne__"),
        ("<", "__lt__"),
        (">=", "__ge__"),
        (">", "__gt__"),
        ("<=", "__le__"),
        ("in", "__contains__"),
    ];
    for (k, v) in entries {
        dict.set_item(*k, *v)?;
    }
    Ok(dict.into())
}

/// Build the `reverse_op_methods` dict: method name -> reverse method name.
/// Mirrors `mypy.operators.reverse_op_methods`.
fn reverse_op_methods(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    let entries: &[(&str, &str)] = &[
        ("__add__", "__radd__"),
        ("__sub__", "__rsub__"),
        ("__mul__", "__rmul__"),
        ("__truediv__", "__rtruediv__"),
        ("__mod__", "__rmod__"),
        ("__divmod__", "__rdivmod__"),
        ("__floordiv__", "__rfloordiv__"),
        ("__pow__", "__rpow__"),
        ("__matmul__", "__rmatmul__"),
        ("__and__", "__rand__"),
        ("__or__", "__ror__"),
        ("__xor__", "__rxor__"),
        ("__lshift__", "__rlshift__"),
        ("__rshift__", "__rrshift__"),
        ("__eq__", "__eq__"),
        ("__ne__", "__ne__"),
        ("__lt__", "__gt__"),
        ("__ge__", "__le__"),
        ("__gt__", "__lt__"),
        ("__le__", "__ge__"),
    ];
    for (k, v) in entries {
        dict.set_item(*k, *v)?;
    }
    Ok(dict.into())
}

/// Build the `unary_op_methods` dict: unary operator symbol -> method name.
/// Mirrors `mypy.operators.unary_op_methods`.
fn unary_op_methods(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("-", "__neg__")?;
    dict.set_item("+", "__pos__")?;
    dict.set_item("~", "__invert__")?;
    Ok(dict.into())
}

/// Build the `flip_ops` dict: comparison operator -> flipped operator.
/// Mirrors `mypy.operators.flip_ops`.
fn flip_ops(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    let entries: &[(&str, &str)] = &[("<", ">"), ("<=", ">="), (">", "<"), (">=", "<=")];
    for (k, v) in entries {
        dict.set_item(*k, *v)?;
    }
    Ok(dict.into())
}

/// Build the `neg_ops` dict: comparison operator -> negated operator.
/// Mirrors `mypy.operators.neg_ops`.
fn neg_ops(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    let entries: &[(&str, &str)] = &[
        ("==", "!="),
        ("!=", "=="),
        ("is", "is not"),
        ("is not", "is"),
        ("<", ">="),
        ("<=", ">"),
        (">", "<="),
        (">=", "<"),
    ];
    for (k, v) in entries {
        dict.set_item(*k, *v)?;
    }
    Ok(dict.into())
}

/// Build a set of operator method names. Mirrors `mypy.operators.op_methods_that_shortcut`.
fn op_methods_that_shortcut(py: Python<'_>) -> PyResult<PyObject> {
    let set = PySet::empty(py)?;
    let items: &[&str] = &[
        "__add__",
        "__sub__",
        "__mul__",
        "__truediv__",
        "__mod__",
        "__divmod__",
        "__floordiv__",
        "__pow__",
        "__matmul__",
        "__and__",
        "__or__",
        "__xor__",
        "__lshift__",
        "__rshift__",
    ];
    for item in items {
        set.add(*item)?;
    }
    Ok(set.into())
}

/// Build the `ops_with_inplace_method` set. Mirrors
/// `mypy.operators.ops_with_inplace_method`.
fn ops_with_inplace_method(py: Python<'_>) -> PyResult<PyObject> {
    let set = PySet::empty(py)?;
    let items: &[&str] = &[
        "+", "-", "*", "/", "%", "//", "**", "@", "&", "|", "^", "<<", ">>",
    ];
    for item in items {
        set.add(*item)?;
    }
    Ok(set.into())
}

/// Build the `inplace_operator_methods` set. Mirrors
/// `mypy.operators.inplace_operator_methods`.
fn inplace_operator_methods(py: Python<'_>) -> PyResult<PyObject> {
    let set = PySet::empty(py)?;
    let items: &[&str] = &[
        "__iadd__",
        "__isub__",
        "__imul__",
        "__itruediv__",
        "__imod__",
        "__ifloordiv__",
        "__ipow__",
        "__imatmul__",
        "__iand__",
        "__ior__",
        "__ixor__",
        "__ilshift__",
        "__irshift__",
    ];
    for item in items {
        set.add(*item)?;
    }
    Ok(set.into())
}

/// Build the `ops_falling_back_to_cmp` set. Mirrors
/// `mypy.operators.ops_falling_back_to_cmp`.
fn ops_falling_back_to_cmp(py: Python<'_>) -> PyResult<PyObject> {
    let set = PySet::empty(py)?;
    let items: &[&str] = &["__ne__", "__eq__", "__lt__", "__le__", "__gt__", "__ge__"];
    for item in items {
        set.add(*item)?;
    }
    Ok(set.into())
}

/// Build the `reverse_op_method_names` set. Mirrors
/// `mypy.operators.reverse_op_method_names` (values of `reverse_op_methods`).
fn reverse_op_method_names(py: Python<'_>) -> PyResult<PyObject> {
    let set = PySet::empty(py)?;
    let items: &[&str] = &[
        "__radd__",
        "__rsub__",
        "__rmul__",
        "__rtruediv__",
        "__rmod__",
        "__rdivmod__",
        "__rfloordiv__",
        "__rpow__",
        "__rmatmul__",
        "__rand__",
        "__ror__",
        "__rxor__",
        "__rlshift__",
        "__rrshift__",
        "__eq__",
        "__ne__",
        "__gt__",
        "__le__",
        "__lt__",
        "__ge__",
    ];
    for item in items {
        set.add(*item)?;
    }
    Ok(set.into())
}

/// Build the `normal_from_reverse_op` dict: reverse method -> normal method.
/// Mirrors `mypy.operators.normal_from_reverse_op` (inverse of
/// `reverse_op_methods`).
fn normal_from_reverse_op(py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    let entries: &[(&str, &str)] = &[
        ("__radd__", "__add__"),
        ("__rsub__", "__sub__"),
        ("__rmul__", "__mul__"),
        ("__rtruediv__", "__truediv__"),
        ("__rmod__", "__mod__"),
        ("__rdivmod__", "__divmod__"),
        ("__rfloordiv__", "__floordiv__"),
        ("__rpow__", "__pow__"),
        ("__rmatmul__", "__matmul__"),
        ("__rand__", "__and__"),
        ("__ror__", "__or__"),
        ("__rxor__", "__xor__"),
        ("__rlshift__", "__lshift__"),
        ("__rrshift__", "__rshift__"),
        ("__eq__", "__eq__"),
        ("__ne__", "__ne__"),
        ("__gt__", "__lt__"),
        ("__le__", "__ge__"),
        ("__lt__", "__gt__"),
        ("__ge__", "__le__"),
    ];
    for (k, v) in entries {
        dict.set_item(*k, *v)?;
    }
    Ok(dict.into())
}

/// `#[pyfunction]`: return all operator data tables as a single dict.
/// Each key maps to the corresponding Python constant from `mypy/operators.py`.
/// This avoids multiple FFI calls: one call gets everything.
#[pyfunction]
pub(crate) fn rust_operator_tables(py: Python<'_>) -> PyResult<PyObject> {
    let result = PyDict::new(py);
    result.set_item("op_methods", op_methods(py)?)?;
    result.set_item("op_methods_to_symbols", {
        let dict = PyDict::new(py);
        let entries: &[(&str, &str)] = &[
            ("__add__", "+"),
            ("__sub__", "-"),
            ("__mul__", "*"),
            ("__truediv__", "/"),
            ("__mod__", "%"),
            ("__divmod__", "divmod"),
            ("__floordiv__", "//"),
            ("__pow__", "**"),
            ("__matmul__", "@"),
            ("__and__", "&"),
            ("__or__", "|"),
            ("__xor__", "^"),
            ("__lshift__", "<<"),
            ("__rshift__", ">>"),
            ("__eq__", "=="),
            ("__ne__", "!="),
            ("__lt__", "<"),
            ("__ge__", ">="),
            ("__gt__", ">"),
            ("__le__", "<="),
            ("__contains__", "in"),
        ];
        for (k, v) in entries {
            dict.set_item(*k, *v)?;
        }
        dict
    })?;
    result.set_item("reverse_op_methods", reverse_op_methods(py)?)?;
    result.set_item("unary_op_methods", unary_op_methods(py)?)?;
    result.set_item("flip_ops", flip_ops(py)?)?;
    result.set_item("neg_ops", neg_ops(py)?)?;
    result.set_item("op_methods_that_shortcut", op_methods_that_shortcut(py)?)?;
    result.set_item("ops_with_inplace_method", ops_with_inplace_method(py)?)?;
    result.set_item("inplace_operator_methods", inplace_operator_methods(py)?)?;
    result.set_item("ops_falling_back_to_cmp", ops_falling_back_to_cmp(py)?)?;
    result.set_item("reverse_op_method_names", reverse_op_method_names(py)?)?;
    result.set_item("normal_from_reverse_op", normal_from_reverse_op(py)?)?;
    Ok(result.into())
}

/// Rust-side lookup helpers used by other kernel modules. These avoid
/// FFI round-trips when the type_kernel needs to check operator method
/// names internally (e.g. during `check_call` operator dispatch).
#[allow(dead_code)]
pub(crate) fn get_op_method(op: &str) -> Option<&'static str> {
    match op {
        "+" => Some("__add__"),
        "-" => Some("__sub__"),
        "*" => Some("__mul__"),
        "/" => Some("__truediv__"),
        "%" => Some("__mod__"),
        "divmod" => Some("__divmod__"),
        "//" => Some("__floordiv__"),
        "**" => Some("__pow__"),
        "@" => Some("__matmul__"),
        "&" => Some("__and__"),
        "|" => Some("__or__"),
        "^" => Some("__xor__"),
        "<<" => Some("__lshift__"),
        ">>" => Some("__rshift__"),
        "==" => Some("__eq__"),
        "!=" => Some("__ne__"),
        "<" => Some("__lt__"),
        ">=" => Some("__ge__"),
        ">" => Some("__gt__"),
        "<=" => Some("__le__"),
        "in" => Some("__contains__"),
        _ => None,
    }
}

/// Lookup: does this method name fall back to comparison?
/// Mirrors `ops_falling_back_to_cmp` membership.
#[allow(dead_code)]
pub(crate) fn falls_back_to_cmp(method: &str) -> bool {
    matches!(
        method,
        "__ne__" | "__eq__" | "__lt__" | "__le__" | "__gt__" | "__ge__"
    )
}

/// Lookup: reverse operator method name, if one exists.
/// Mirrors `reverse_op_methods` dict lookup.
#[allow(dead_code)]
pub(crate) fn get_reverse_op_method(method: &str) -> Option<&'static str> {
    match method {
        "__add__" => Some("__radd__"),
        "__sub__" => Some("__rsub__"),
        "__mul__" => Some("__rmul__"),
        "__truediv__" => Some("__rtruediv__"),
        "__mod__" => Some("__rmod__"),
        "__divmod__" => Some("__rdivmod__"),
        "__floordiv__" => Some("__rfloordiv__"),
        "__pow__" => Some("__rpow__"),
        "__matmul__" => Some("__rmatmul__"),
        "__and__" => Some("__rand__"),
        "__or__" => Some("__ror__"),
        "__xor__" => Some("__rxor__"),
        "__lshift__" => Some("__rlshift__"),
        "__rshift__" => Some("__rrshift__"),
        "__eq__" => Some("__eq__"),
        "__ne__" => Some("__ne__"),
        "__lt__" => Some("__gt__"),
        "__ge__" => Some("__le__"),
        "__gt__" => Some("__lt__"),
        "__le__" => Some("__ge__"),
        _ => None,
    }
}

/// Rust-side lookup: flip a comparison operator.
/// Mirrors `flip_ops` dict lookup.
#[allow(dead_code)]
pub(crate) fn flip_op(op: &str) -> Option<&'static str> {
    match op {
        "<" => Some(">"),
        "<=" => Some(">="),
        ">" => Some("<"),
        ">=" => Some("<="),
        _ => None,
    }
}

/// Rust-side lookup: negate a comparison operator.
/// Mirrors `neg_ops` dict lookup.
#[allow(dead_code)]
pub(crate) fn neg_op(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("!="),
        "!=" => Some("=="),
        "is" => Some("is not"),
        "is not" => Some("is"),
        "<" => Some(">="),
        "<=" => Some(">"),
        ">" => Some("<="),
        ">=" => Some("<"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_methods_lookup() {
        assert_eq!(get_op_method("+"), Some("__add__"));
        assert_eq!(get_op_method("in"), Some("__contains__"));
        assert_eq!(get_op_method("??"), None);
    }

    #[test]
    fn test_reverse_op_methods() {
        assert_eq!(get_reverse_op_method("__add__"), Some("__radd__"));
        assert_eq!(get_reverse_op_method("__lt__"), Some("__gt__"));
        assert_eq!(get_reverse_op_method("__contains__"), None);
    }

    #[test]
    fn test_falls_back_to_cmp() {
        assert!(falls_back_to_cmp("__eq__"));
        assert!(falls_back_to_cmp("__lt__"));
        assert!(!falls_back_to_cmp("__add__"));
    }

    #[test]
    fn test_flip_ops() {
        assert_eq!(flip_op("<"), Some(">"));
        assert_eq!(flip_op(">="), Some("<="));
        assert_eq!(flip_op("=="), None);
    }

    #[test]
    fn test_neg_ops() {
        assert_eq!(neg_op("=="), Some("!="));
        assert_eq!(neg_op("is"), Some("is not"));
        assert_eq!(neg_op("<"), Some(">="));
        assert_eq!(neg_op("+"), None);
    }
}
