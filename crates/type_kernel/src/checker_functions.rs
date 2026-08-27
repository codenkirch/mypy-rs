//! `TypeChecker.check_compatibility_final_super` decision-head port
//! (mypy.checker).
//!
//! The Python method (checker.py:4608-4636) is a pure decision over the
//! overriding attribute, the base attribute node, and two enum allowlists.
//! It returns either `True`, or `False` after emitting a
//! `cant_override_final` message, or `True` after a writability check.
//!
//! This module ports only the *decision*: it reads the live `base_node`
//! shape (Var / FuncBase / Decorator) and `is_final` flag via PyO3, plus
//! scalar facts the shim computes (the overriding node's `is_final` and
//! `name`, the base `fullname`, and the enum allowlists), and returns a
//! branch tag. The Python shim applies the side effects (message emission,
//! `check_if_final_var_override_writable`) and keeps the original
//! pure-Python body as the fallback.
//!
//! Strangler-fig contract: `None` defers to Python. The only deferral is
//! an unreadable `base_node.is_final` attribute, which mirrors the Python
//! `try/except` shim around the Rust call. Every reachable branch is
//! classified, including the implicit trailing `return True`.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyType};
use std::collections::HashSet;

/// Decision tags; values must match `NATIVE_FINAL_SUPER_*` in
/// mypy/checker.py.
const KIND_PASS_NOT_BASE: i64 = 0;
const KIND_PASS_PRIVATE: i64 = 1;
const KIND_CANT_OVERRIDE_FINAL: i64 = 2;
const KIND_PASS_ENUM: i64 = 3;
const KIND_CHECK_WRITABLE: i64 = 4;
const KIND_PASS_TAIL: i64 = 5;

/// Fetch a class from `mypy.nodes`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.nodes")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// `mypy.checker.is_private` (checker.py:9721-9723): name is private to a
/// class definition. Mirrors the pure-Python predicate.
fn is_private(node_name: &str) -> bool {
    node_name.starts_with("__") && !node_name.ends_with("__")
}

/// The pure decision over resolved facts. Kept separate from the PyO3
/// entry so the branch algebra is unit-testable without a Python runtime.
///
/// `is_known` means `base_node` is one of Var / FuncBase / Decorator;
/// `is_var` means it is specifically a Var (needed for the
/// `not isinstance(base_node, Var)` arm).
#[allow(clippy::too_many_arguments)]
fn classify_final_super(
    is_known: bool,
    is_var: bool,
    base_is_final: bool,
    node_is_final: bool,
    node_name: &str,
    base_fullname: &str,
    enum_bases: &HashSet<String>,
    enum_special_props: &HashSet<String>,
) -> i64 {
    if !is_known {
        return KIND_PASS_NOT_BASE;
    }
    if is_private(node_name) {
        return KIND_PASS_PRIVATE;
    }
    if base_is_final && (node_is_final || !is_var) {
        return KIND_CANT_OVERRIDE_FINAL;
    }
    if node_is_final {
        if enum_bases.contains(base_fullname) || enum_special_props.contains(node_name) {
            return KIND_PASS_ENUM;
        }
        return KIND_CHECK_WRITABLE;
    }
    KIND_PASS_TAIL
}

/// `#[pyfunction]` entry for `TypeChecker.check_compatibility_final_super`
/// (mypy/checker.py:4608-4636).
///
/// `base_node` is the live base attribute node (a Var / FuncBase /
/// Decorator, or None); the shim computes `node_is_final` / `node_name` /
/// `base_fullname` from the overriding node and base `TypeInfo`, and passes
/// the enum allowlists as plain string lists. Returns `Some(tag)` for every
/// reachable branch, or `None` to defer (an unreadable `base_node.is_final`).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_final_super(
    py: Python<'_>,
    base_node: &PyAny,
    node_is_final: bool,
    node_name: &str,
    base_fullname: &str,
    enum_bases: Vec<String>,
    enum_special_props: Vec<String>,
) -> PyResult<Option<i64>> {
    let var_cls = nodes_class(py, "Var")?;
    let func_base_cls = nodes_class(py, "FuncBase")?;
    let decorator_cls = nodes_class(py, "Decorator")?;
    let is_var = base_node.is_instance(var_cls)?;
    let is_func_base = base_node.is_instance(func_base_cls)?;
    let is_decorator = base_node.is_instance(decorator_cls)?;
    let is_known = is_var || is_func_base || is_decorator;

    // checker.py:4624 reads `base_node.is_final` only after the branch-0
    // isinstance gate, so a None base_node never reaches this read.
    let base_is_final = if is_known {
        match base_node.getattr("is_final") {
            Ok(v) => match v.is_true() {
                Ok(b) => b,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        }
    } else {
        false
    };

    let enum_bases_set: HashSet<String> = enum_bases.into_iter().collect();
    let enum_special_set: HashSet<String> = enum_special_props.into_iter().collect();

    Ok(Some(classify_final_super(
        is_known,
        is_var,
        base_is_final,
        node_is_final,
        node_name,
        base_fullname,
        &enum_bases_set,
        &enum_special_set,
    )))
}

/// Decision tags for `check___new___signature`; must match
/// `NATIVE_NEW_SIGNATURE_*` in mypy/checker.py.
const NEW_SIGNATURE_METACLASS: i64 = 0;
const NEW_SIGNATURE_NON_INSTANCE: i64 = 1;
const NEW_SIGNATURE_INSTANCE: i64 = 2;

/// The pure 3-way decision of `TypeChecker.check___new___signature`
/// (checker.py:2630-2664). `is_metaclass` is `fdef.info.is_metaclass()`;
/// `is_instance_ret` is whether `get_proper_type(bound_type.ret_type)` is one
/// of {AnyType, Instance, TupleType, UninhabitedType, LiteralType}. Every
/// branch is classified; the subtype checks and message emission stay Python.
fn classify_new_signature(is_metaclass: bool, is_instance_ret: bool) -> i64 {
    if is_metaclass {
        NEW_SIGNATURE_METACLASS
    } else if !is_instance_ret {
        NEW_SIGNATURE_NON_INSTANCE
    } else {
        NEW_SIGNATURE_INSTANCE
    }
}

/// `#[pyfunction]` entry; the shim computes the two scalar facts and keeps the
/// `check_subtype` calls + `INVALID_NEW_TYPE`/`NON_INSTANCE_NEW_TYPE`
/// emission. Returns `Some(tag)` always (never defers).
#[pyfunction]
pub(crate) fn rust_classify_new_signature(
    is_metaclass: bool,
    is_instance_ret: bool,
) -> PyResult<Option<i64>> {
    Ok(Some(classify_new_signature(is_metaclass, is_instance_ret)))
}

/// Decision tags; values must match `NATIVE_FUNC_DEF_OVERRIDE_*` in
/// mypy/checker.py.
const KIND_FUNC_OVER_FUNC: i64 = 0;
const KIND_ORIG_TYPE_NONE: i64 = 1;
const KIND_FILL_PARTIAL: i64 = 2;
const KIND_PARTIAL_INVALID: i64 = 3;
const KIND_BINDER_ASSIGN: i64 = 4;
const KIND_NO_OP: i64 = 5;

/// The pure branch-order match over scalar facts for
/// `TypeChecker.check_func_def_override` (checker.py:2095-2137). The
/// five dispatch arms plus the implicit no-op (invalid-redefinition tail).
/// Kept separate from the PyO3 entry so the branch algebra is unit-testable
/// without a Python runtime.
fn classify_func_def_override(
    is_funcdef: bool,
    orig_type_is_none: bool,
    is_partial: bool,
    partial_type_is_none: bool,
    is_invalid_redefinition: bool,
) -> i64 {
    if is_funcdef {
        return KIND_FUNC_OVER_FUNC;
    }
    if orig_type_is_none {
        return KIND_ORIG_TYPE_NONE;
    }
    if is_partial {
        if partial_type_is_none {
            return KIND_FILL_PARTIAL;
        }
        return KIND_PARTIAL_INVALID;
    }
    if !is_invalid_redefinition {
        return KIND_BINDER_ASSIGN;
    }
    KIND_NO_OP
}

/// `#[pyfunction]` entry for `TypeChecker.check_func_def_override`
/// (mypy/checker.py:2095-2137). The shim extracts five scalar facts from the
/// live `defn` and passes them in; Rust returns a tag for every input
/// combination (never defers). Branch bodies stay in Python, so the only
/// error mode is argument decoding, which the shim guards with a
/// `try/except` and falls back to the pure-Python body on failure.
#[pyfunction]
pub(crate) fn rust_classify_func_def_override(
    is_funcdef: bool,
    orig_type_is_none: bool,
    is_partial: bool,
    partial_type_is_none: bool,
    is_invalid_redefinition: bool,
) -> i64 {
    classify_func_def_override(
        is_funcdef,
        orig_type_is_none,
        is_partial,
        partial_type_is_none,
        is_invalid_redefinition,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn classify(
        is_known: bool,
        is_var: bool,
        base_is_final: bool,
        node_is_final: bool,
        node_name: &str,
        base_fullname: &str,
    ) -> i64 {
        let enum_bases = set(&["enum.Enum", "enum.IntEnum"]);
        let enum_special = set(&["name", "value"]);
        classify_final_super(
            is_known,
            is_var,
            base_is_final,
            node_is_final,
            node_name,
            base_fullname,
            &enum_bases,
            &enum_special,
        )
    }

    #[test]
    fn test_not_base_kind_defers_pass() {
        // base_node is not a Var/FuncBase/Decorator (or None): PASS.
        assert_eq!(
            classify(false, false, false, false, "attr", "mod.Base"),
            KIND_PASS_NOT_BASE
        );
        assert_eq!(
            classify(false, false, false, true, "attr", "mod.Base"),
            KIND_PASS_NOT_BASE
        );
    }

    #[test]
    fn test_private_name_pass() {
        // is_private wins over every later branch, including final overrides.
        assert_eq!(
            classify(true, true, true, true, "__priv", "mod.Base"),
            KIND_PASS_PRIVATE
        );
        assert_eq!(
            classify(true, true, false, false, "__priv", "mod.Base"),
            KIND_PASS_PRIVATE
        );
        // Dunder names are not private.
        assert_eq!(
            classify(true, true, false, false, "__init__", "mod.Base"),
            KIND_PASS_TAIL
        );
    }

    #[test]
    fn test_cant_override_final_var_and_method() {
        // base is final AND node is final: error.
        assert_eq!(
            classify(true, true, true, true, "attr", "mod.Base"),
            KIND_CANT_OVERRIDE_FINAL
        );
        // base is final AND base is a method (not a Var) but node is not final:
        // login via `not isinstance(base_node, Var)`.
        assert_eq!(
            classify(true, false, true, false, "attr", "mod.Base"),
            KIND_CANT_OVERRIDE_FINAL
        );
    }

    #[test]
    fn test_enum_pass() {
        // node is final and base.fullname is an enum base.
        assert_eq!(
            classify(true, true, false, true, "attr", "enum.Enum"),
            KIND_PASS_ENUM
        );
        // node is final and node.name is an enum special prop.
        assert_eq!(
            classify(true, true, false, true, "name", "mod.Base"),
            KIND_PASS_ENUM
        );
    }

    #[test]
    fn test_check_writable() {
        assert_eq!(
            classify(true, true, false, true, "attr", "mod.Base"),
            KIND_CHECK_WRITABLE
        );
    }

    #[test]
    fn test_tail_pass() {
        // base is a non-final Var, node is not final: trailing return True.
        assert_eq!(
            classify(true, true, false, false, "attr", "mod.Base"),
            KIND_PASS_TAIL
        );
    }

    #[test]
    fn test_classify_new_signature_metaclass() {
        // Metaclass wins regardless of the ret-type kind (branch order).
        assert_eq!(classify_new_signature(true, true), NEW_SIGNATURE_METACLASS);
        assert_eq!(classify_new_signature(true, false), NEW_SIGNATURE_METACLASS);
    }

    #[test]
    fn test_classify_new_signature_non_instance() {
        // Non-metaclass + a ret type that is not one of the five
        // instance-kinds (e.g. CallableType): NON_INSTANCE_NEW_TYPE.
        assert_eq!(
            classify_new_signature(false, false),
            NEW_SIGNATURE_NON_INSTANCE
        );
    }

    #[test]
    fn test_classify_new_signature_instance() {
        // Non-metaclass + Any/Instance/Tuple/Uninhabited/Literal: subtype
        // of the class.
        assert_eq!(classify_new_signature(false, true), NEW_SIGNATURE_INSTANCE);
    }

    // ---- classify_func_def_override unit tests ----

    #[test]
    fn test_classify_func_def_override_func_over_func() {
        // (a) original_def is a FuncDef: function-overrides-function arm.
        assert_eq!(
            classify_func_def_override(true, false, false, false, false),
            KIND_FUNC_OVER_FUNC
        );
        // orig_type facts are irrelevant when is_funcdef is True.
        assert_eq!(
            classify_func_def_override(true, true, true, true, true),
            KIND_FUNC_OVER_FUNC
        );
    }

    #[test]
    fn test_classify_func_def_override_orig_type_none() {
        // (b) original_def is not a FuncDef, orig_type is None: return.
        assert_eq!(
            classify_func_def_override(false, true, false, false, false),
            KIND_ORIG_TYPE_NONE
        );
    }

    #[test]
    fn test_classify_func_def_override_fill_partial() {
        // (c) orig_type is a PartialType with type None: fill it.
        assert_eq!(
            classify_func_def_override(false, false, true, true, false),
            KIND_FILL_PARTIAL
        );
    }

    #[test]
    fn test_classify_func_def_override_partial_invalid() {
        // (d) orig_type is a PartialType with type not None: invalid.
        assert_eq!(
            classify_func_def_override(false, false, true, false, false),
            KIND_PARTIAL_INVALID
        );
    }

    #[test]
    fn test_classify_func_def_override_binder_assign() {
        // (e) not partial, not invalid: binder.assign_type + check_subtype.
        assert_eq!(
            classify_func_def_override(false, false, false, false, false),
            KIND_BINDER_ASSIGN
        );
    }

    #[test]
    fn test_classify_func_def_override_no_op() {
        // implicit: not partial, is_invalid_redefinition: no-op.
        assert_eq!(
            classify_func_def_override(false, false, false, false, true),
            KIND_NO_OP
        );
    }
}
