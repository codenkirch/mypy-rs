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
use pyo3::types::{PyAny, PyDict, PyList, PyType};
use std::collections::HashSet;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::Type;

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

/// `mypy.util.is_dunder` (util.py:125) with the default exclude_special.
fn is_dunder(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__")
}

/// `mypy.util.is_sunder` (util.py:139) mirrors the pure-Python predicate.
fn is_sunder(name: &str) -> bool {
    !is_dunder(name) && name.starts_with('_') && name.ends_with('_') && name != "_"
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

/// Decision tags for `check_compatibility_classvar_super`; must match
/// `NATIVE_CLASSVAR_SUPER_*` in mypy/checker.py.
const KIND_CLASSVAR_SUPER_NOT_VAR: i64 = 0;
const KIND_CLASSVAR_SUPER_OK: i64 = 1;
const KIND_CLASSVAR_SUPER_INSTANCE_VAR: i64 = 2;
const KIND_CLASSVAR_SUPER_CLASS_VAR: i64 = 3;

/// The pure 2x2 decision of `TypeChecker.check_compatibility_classvar_super`
/// (checker.py:4796-4807). `is_var` means `base_node` is a `Var`;
/// `node_is_classvar` / `base_is_classvar` are the two `is_classvar` flags.
/// Branch order mirrors the Python body: not-a-Var passes first, then the
/// instance-variable violation, then the class-variable violation, then the
/// implicit trailing pass.
fn classify_classvar_super(is_var: bool, node_is_classvar: bool, base_is_classvar: bool) -> i64 {
    if !is_var {
        return KIND_CLASSVAR_SUPER_NOT_VAR;
    }
    if node_is_classvar && !base_is_classvar {
        return KIND_CLASSVAR_SUPER_INSTANCE_VAR;
    }
    if !node_is_classvar && base_is_classvar {
        return KIND_CLASSVAR_SUPER_CLASS_VAR;
    }
    KIND_CLASSVAR_SUPER_OK
}

/// `#[pyfunction]` entry for `TypeChecker.check_compatibility_classvar_super`
/// (mypy/checker.py:4796-4807). Reads the live `base_node` shape (Var or not),
/// the `node.is_classvar` flag computed by the shim, and the live
/// `base_node.is_classvar` flag via PyO3. Returns `Some(tag)` for every
/// reachable branch, or `None` to defer (an unreadable `base_node.is_classvar`).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_classvar_super(
    py: Python<'_>,
    base_node: &PyAny,
    node_is_classvar: bool,
) -> PyResult<Option<i64>> {
    let var_cls = nodes_class(py, "Var")?;
    let is_var = base_node.is_instance(var_cls)?;

    // checker.py:4801 reads `base_node.is_classvar` only after the branch-0
    // isinstance gate, so a None base_node never reaches this read.
    let base_is_classvar = if is_var {
        match read_bool_attr(base_node, "is_classvar")? {
            Some(b) => b,
            None => return Ok(None),
        }
    } else {
        false
    };

    Ok(Some(classify_classvar_super(
        is_var,
        node_is_classvar,
        base_is_classvar,
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

// ---------------------------------------------------------------------------
// check_enum_new base-fold decision port (issue #923)
// ---------------------------------------------------------------------------

/// Per-base decision tags for `check_enum_new`; values must match
/// `NATIVE_ENUM_NEW_*` in mypy/checker.py.
const KIND_ENUM_NEW_SKIP: i64 = 0;
const KIND_ENUM_NEW_ADVANCE: i64 = 1;
const KIND_ENUM_NEW_CONFLICT: i64 = 2;

/// Resolved facts for a single base in the `check_enum_new` fold
/// (checker.py:3748-3765).
struct BaseFacts {
    is_enum: bool,
    /// `has_new_method(base.type)`; only read when `is_enum` is false.
    base_has_new: bool,
    /// `(b.is_enum, has_new_method(b))` for each `b` in
    /// `base.type.mro[1:-1]`; only read when `is_enum` is true.
    mro_middle: Vec<(bool, bool)>,
}

/// Pure fold mirroring `check_enum_new` (checker.py:3748-3765) over the
/// resolved per-base facts. Returns one tag per base.
fn classify_enum_new(bases: &[BaseFacts]) -> Vec<i64> {
    let mut tags = Vec::with_capacity(bases.len());
    let mut has_new = false;
    for f in bases {
        let candidate = if f.is_enum {
            f.mro_middle
                .iter()
                .any(|&(b_is_enum, b_has_new)| !b_is_enum && b_has_new)
        } else {
            f.base_has_new
        };
        if candidate && has_new {
            tags.push(KIND_ENUM_NEW_CONFLICT);
        } else if candidate {
            has_new = true;
            tags.push(KIND_ENUM_NEW_ADVANCE);
        } else {
            tags.push(KIND_ENUM_NEW_SKIP);
        }
    }
    tags
}

/// `has_new_method` (checker.py:3740-3746) on a live `TypeInfo`: the
/// `__new__` lookup returns a symbol whose node fullname is not
/// `builtins.object.__new__`.
fn has_new_method_live(info: &PyAny) -> PyResult<bool> {
    let new_method = info.call_method1("get", ("__new__",))?;
    if new_method.is_none() {
        return Ok(false);
    }
    let node = new_method.getattr("node")?;
    if node.is_none() {
        return Ok(false);
    }
    let fullname: String = node.getattr("fullname")?.extract()?;
    Ok(fullname != "builtins.object.__new__")
}

/// `#[pyfunction]` entry for `TypeChecker.check_enum_new`
/// (mypy/checker.py:3739-3766). Reads the live `defn.info.bases` (a
/// list of `Instance`) via PyO3, resolves each base's facts, and runs
/// the fold natively. Returns one tag per base; the Python shim applies
/// the `self.fail` side effect and keeps its own `has_new` bookkeeping.
/// Returns `None` when `bases` is not a list (deferral).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_enum_new(bases: &PyAny) -> PyResult<Option<Vec<i64>>> {
    let list = match bases.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let mut facts = Vec::with_capacity(list.len());
    for base in list.iter() {
        let base_type = base.getattr("type")?;
        let is_enum: bool = base_type.getattr("is_enum")?.extract()?;
        let base_has_new = if is_enum {
            false
        } else {
            has_new_method_live(base_type)?
        };
        let mut mro_middle = Vec::new();
        if is_enum {
            let mro = base_type.getattr("mro")?.downcast::<PyList>()?;
            let n = mro.len().saturating_sub(2);
            for b in mro.iter().skip(1).take(n) {
                let b_is_enum: bool = b.getattr("is_enum")?.extract()?;
                let b_has_new = has_new_method_live(b)?;
                mro_middle.push((b_is_enum, b_has_new));
            }
        }
        facts.push(BaseFacts {
            is_enum,
            base_has_new,
            mro_middle,
        });
    }
    Ok(Some(classify_enum_new(&facts)))
}

// ---------------------------------------------------------------------------
// check_enum_bases fold port (issue #937)
// ---------------------------------------------------------------------------

/// Pure fold mirroring `check_enum_bases` (checker.py:3850-3876) over the
/// resolved per-base `is_enum` facts. Returns `(enum_base_idx,
/// violating_idx)`: `enum_base_idx` is the index of the first enum base
/// (-1 if none); `violating_idx` is the index of the first non-enum base
/// after an enum base (-1 if none).
fn classify_enum_bases(is_enums: &[bool]) -> (i64, i64) {
    let mut enum_base_idx: i64 = -1;
    for (i, &is_enum) in is_enums.iter().enumerate() {
        if enum_base_idx < 0 && is_enum {
            enum_base_idx = i as i64;
        } else if enum_base_idx >= 0 && !is_enum {
            return (enum_base_idx, i as i64);
        }
    }
    (enum_base_idx, -1)
}

/// `#[pyfunction]` entry for `TypeChecker.check_enum_bases`
/// (mypy/checker.py:3850-3876). Reads `defn.info.bases` (a list of
/// `Instance`) via PyO3, extracts each `base.type.is_enum` bool, and runs
/// the fold natively. Returns `Some((enum_base_idx, violating_idx))` or
/// `None` when `bases` is not a list (deferral). The Python shim applies
/// `self.fail` with the offending enum base's `str_with_options`.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_enum_bases(bases: &PyAny) -> PyResult<Option<(i64, i64)>> {
    let list = match bases.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let mut is_enums = Vec::with_capacity(list.len());
    for base in list.iter() {
        let base_type = base.getattr("type")?;
        let is_enum: bool = base_type.getattr("is_enum")?.extract()?;
        is_enums.push(is_enum);
    }
    Ok(Some(classify_enum_bases(&is_enums)))
}

// ---------------------------------------------------------------------------
// check_enum multi-arm classifier port (issue #971)
// ---------------------------------------------------------------------------

/// Bit-flag tags for `check_enum` arms; must match
/// `NATIVE_ENUM_CHECK_*` in mypy/checker.py.
const ENUM_CHECK_MEMBERS_OVERRIDE: i64 = 1;
const ENUM_CHECK_STUB_EMPTY: i64 = 2;

/// Pure decision mirroring `TypeChecker.check_enum`
/// (checker.py:3843-3870). `members_override` is arm (a);
/// `stub_empty` is arm (b). Arm (c) passes through the list of
/// offending base fullnames unchanged. Returns `(tag, base_names)`.
fn classify_enum(
    members_override: bool,
    stub_empty: bool,
    final_enum_bases: Vec<String>,
) -> (i64, Vec<String>) {
    let mut tag = 0i64;
    if members_override {
        tag |= ENUM_CHECK_MEMBERS_OVERRIDE;
    }
    if stub_empty {
        tag |= ENUM_CHECK_STUB_EMPTY;
    }
    (tag, final_enum_bases)
}

/// `#[pyfunction]` entry for `TypeChecker.check_enum`
/// (checker.py:3843-3870). Reads the live `defn.info` (TypeInfo),
/// `is_stub`, `tree_fullname`, and `ENUM_BASES` via PyO3. Classifies
/// three arms: (a) `__members__` override, (c) final-enum base
/// loop, (b) stub-empty-enum. Returns `Some((tag, base_names))`
/// where tag is a bit flag and base_names are the arm-(c) offending
/// base fullnames. Returns `None` to defer (non-dict `names` or
/// non-list `mro`). The Python shim applies `self.fail` / `self.note`
/// and calls `check_final_enum` / `check_enum_bases` /
/// `check_enum_new`.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_enum(
    py: Python<'_>,
    info: &PyAny,
    is_stub: bool,
    tree_fullname: &str,
    enum_bases: Vec<String>,
) -> PyResult<Option<(i64, Vec<String>)>> {
    let names = match info.getattr("names")?.downcast::<PyDict>() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    let fullname: String = info.getattr("fullname")?.extract()?;
    let enum_bases_set: HashSet<String> = enum_bases.into_iter().collect();

    // Arm (a): __members__ override.
    let members_override =
        if !enum_bases_set.contains(&fullname) && names.contains("__members__")? {
            let sym = match names.get_item("__members__")? {
                Some(s) => s,
                None => return Ok(None),
            };
            let node = sym.getattr("node")?;
            if node.is_none() {
                false
            } else {
                let var_cls = nodes_class(py, "Var")?;
                if !node.is_instance(var_cls)? {
                    false
                } else {
                    match read_bool_attr(node, "has_explicit_value")? {
                        Some(b) => b,
                        None => return Ok(None),
                    }
                }
            }
        } else {
            false
        };

    // Arm (c): final-enum base loop over mro[1:-1].
    let mro = match info.getattr("mro")?.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let mut final_enum_bases = Vec::new();
    let n = mro.len();
    if n > 2 {
        for base in mro.iter().skip(1).take(n - 2) {
            let is_enum: bool = base.getattr("is_enum")?.extract()?;
            let base_fullname: String = base.getattr("fullname")?.extract()?;
            if is_enum && !enum_bases_set.contains(&base_fullname) {
                final_enum_bases.push(base_fullname);
            }
        }
    }

    // Arm (b): stub-empty-enum.
    let stub_empty = if is_stub && tree_fullname != "enum" && tree_fullname != "_typeshed" {
        let members = info.getattr("enum_members")?;
        match members.is_true() {
            Ok(has) => !has,
            Err(_) => return Ok(None),
        }
    } else {
        false
    };

    Ok(Some(classify_enum(
        members_override,
        stub_empty,
        final_enum_bases,
    )))
}

// ---------------------------------------------------------------------------
// check_getattr_method 4-way dispatch-head port (issue #985)
// ---------------------------------------------------------------------------

/// Decision tags; values must match `NATIVE_GETATTR_METHOD_*` in
/// mypy/checker.py.
const KIND_GETATTR_MODULE_GETATTRIBUTE: i64 = 0;
const KIND_GETATTR_MODULE: i64 = 1;
const KIND_GETATTR_CLASS: i64 = 2;
const KIND_GETATTR_PASS: i64 = 3;

/// The pure 4-way dispatch for `TypeChecker.check_getattr_method`
/// (checker.py:3066-3093). Kept separate from the PyO3 entry so the branch
/// algebra is unit-testable without a Python runtime.
fn classify_getattr_method(module_scope: bool, is_getattribute: bool, active_class: bool) -> i64 {
    if module_scope {
        if is_getattribute {
            return KIND_GETATTR_MODULE_GETATTRIBUTE;
        }
        return KIND_GETATTR_MODULE;
    }
    if active_class {
        return KIND_GETATTR_CLASS;
    }
    KIND_GETATTR_PASS
}

/// `#[pyfunction]` entry for `TypeChecker.check_getattr_method`
/// (mypy/checker.py:3066-3093). Reads the live `Scope` via PyO3
/// (`len(scope.stack) == 1` and `scope.active_class()`); `name` arrives as a
/// scalar string. Returns `Some(tag)` for every reachable branch, or `None`
/// to defer (an unreadable `scope.stack` / `active_class()` result). The
/// Python shim builds the fixed `CallableType` via `named_type`, runs the
/// (already native) `is_subtype`, and applies the
/// MODULE_LEVEL_GETATTRIBUTE / invalid_signature_for_special_method
/// emission.
#[pyfunction]
pub(crate) fn rust_classify_getattr_method(scope: &PyAny, name: &str) -> PyResult<Option<i64>> {
    let stack = match scope.getattr("stack") {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let module_scope = match stack.len() {
        Ok(len) => len == 1,
        Err(_) => return Ok(None),
    };
    let active_class = match scope.call_method0("active_class") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    Ok(Some(classify_getattr_method(
        module_scope,
        name == "__getattribute__",
        active_class,
    )))
}

#[cfg(test)]
mod classify_getattr_method_tests {
    use super::*;

    #[test]
    fn test_module_getattribute() {
        assert_eq!(classify_getattr_method(true, true, false), 0);
    }

    #[test]
    fn test_module_other_name() {
        assert_eq!(classify_getattr_method(true, false, false), 1);
    }

    #[test]
    fn test_class_scope() {
        assert_eq!(classify_getattr_method(false, false, true), 2);
        assert_eq!(classify_getattr_method(false, true, true), 2);
    }

    #[test]
    fn test_pass() {
        assert_eq!(classify_getattr_method(false, false, false), 3);
        assert_eq!(classify_getattr_method(false, true, false), 3);
    }
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
    // --- check_enum_new fold tests (issue #923) ---

    #[test]
    fn test_classify_enum_new_non_enum_advance() {
        // Non-enum base with __new__: advance (set has_new).
        let facts = [BaseFacts {
            is_enum: false,
            base_has_new: true,
            mro_middle: vec![],
        }];
        assert_eq!(classify_enum_new(&facts), vec![KIND_ENUM_NEW_ADVANCE]);
    }

    #[test]
    fn test_classify_enum_new_non_enum_skip() {
        // Non-enum base without __new__: skip.
        let facts = [BaseFacts {
            is_enum: false,
            base_has_new: false,
            mro_middle: vec![],
        }];
        assert_eq!(classify_enum_new(&facts), vec![KIND_ENUM_NEW_SKIP]);
    }

    #[test]
    fn test_classify_enum_new_enum_mro_fold() {
        // Enum base whose mro[1:-1] has a non-enum mixin with __new__.
        let facts = [BaseFacts {
            is_enum: true,
            base_has_new: false,
            mro_middle: vec![(true, false), (false, true)],
        }];
        assert_eq!(classify_enum_new(&facts), vec![KIND_ENUM_NEW_ADVANCE]);
    }

    #[test]
    fn test_classify_enum_new_enum_mro_all_enum_skip() {
        // Enum base whose mro[1:-1] items are all enums or lack __new__.
        let facts = [BaseFacts {
            is_enum: true,
            base_has_new: false,
            mro_middle: vec![(true, true), (false, false)],
        }];
        assert_eq!(classify_enum_new(&facts), vec![KIND_ENUM_NEW_SKIP]);
    }

    #[test]
    fn test_classify_enum_new_conflict_vs_advance() {
        // Two mixin candidates: second is a conflict, third is skip,
        // fourth is a conflict (has_new still set from first).
        let facts = [
            BaseFacts {
                is_enum: false,
                base_has_new: true,
                mro_middle: vec![],
            },
            BaseFacts {
                is_enum: false,
                base_has_new: true,
                mro_middle: vec![],
            },
            BaseFacts {
                is_enum: false,
                base_has_new: false,
                mro_middle: vec![],
            },
            BaseFacts {
                is_enum: false,
                base_has_new: true,
                mro_middle: vec![],
            },
        ];
        assert_eq!(
            classify_enum_new(&facts),
            vec![
                KIND_ENUM_NEW_ADVANCE,
                KIND_ENUM_NEW_CONFLICT,
                KIND_ENUM_NEW_SKIP,
                KIND_ENUM_NEW_CONFLICT,
            ]
        );
    }

    // --- check_enum_bases fold tests (issue #937) ---

    #[test]
    fn test_classify_enum_bases_no_enum() {
        // No enum base: no violation, enum_base_idx = -1.
        assert_eq!(classify_enum_bases(&[false, false]), (-1, -1));
    }

    #[test]
    fn test_classify_enum_bases_enum_only() {
        // Single enum base, no non-enum after it.
        assert_eq!(classify_enum_bases(&[true]), (0, -1));
    }

    #[test]
    fn test_classify_enum_bases_enum_then_enum() {
        // Multiple enum bases: all fine.
        assert_eq!(classify_enum_bases(&[true, true]), (0, -1));
    }

    #[test]
    fn test_classify_enum_bases_nonenum_then_enum() {
        // Non-enum before enum: fine, no violation.
        assert_eq!(classify_enum_bases(&[false, true]), (1, -1));
    }

    #[test]
    fn test_classify_enum_bases_enum_then_nonenum() {
        // Enum then non-enum: violation at index 1.
        assert_eq!(classify_enum_bases(&[true, false]), (0, 1));
    }

    #[test]
    fn test_classify_enum_bases_nonenum_enum_nonenum() {
        // Non-enum, enum, non-enum: violation at index 2.
        assert_eq!(classify_enum_bases(&[false, true, false]), (1, 2));
    }

    #[test]
    fn test_classify_enum_bases_empty() {
        // Empty bases: no violation.
        assert_eq!(classify_enum_bases(&[]), (-1, -1));
    }
}

// `TypeChecker.check_metaclass_compatibility` decision-head port
// (checker.py:3918-3941); fail + note side effects stay in Python.

/// Decision tags; values must match `NATIVE_METACLASS_COMPAT_*` in
/// mypy/checker.py.
const KIND_METACLASS_PASS: i64 = 0;
const KIND_METACLASS_CONFLICT: i64 = 1;

/// The pure decision over resolved facts. Kept separate from the PyO3
/// entry so the branch algebra is unit-testable without a Python runtime.
///
/// `typeddict_type_is_none` is the inverted `typeddict_type is not None`
/// test from Python (True means the attr is None, i.e. not a TypedDict).
/// `metaclass_type_is_none` is `typ.metaclass_type is None`. The last
/// argument is `any(base.type.metaclass_type is not None for base in bases)`.
fn classify_metaclass_compat(
    is_metaclass: bool,
    is_protocol: bool,
    is_named_tuple: bool,
    is_enum: bool,
    typeddict_type_is_none: bool,
    metaclass_type_is_none: bool,
    any_base_has_metaclass: bool,
) -> i64 {
    // checker.py:3920-3927: exempt metaclasses, protocols, named tuples,
    // enums, and TypedDicts from the check.
    if is_metaclass || is_protocol || is_named_tuple || is_enum || !typeddict_type_is_none {
        return KIND_METACLASS_PASS;
    }
    // checker.py:3929-3931: conflict iff the class has no metaclass but a
    // base does.
    if metaclass_type_is_none && any_base_has_metaclass {
        return KIND_METACLASS_CONFLICT;
    }
    KIND_METACLASS_PASS
}

/// Read a bool flag attribute off a live Python object; return `None` to
/// defer on any read/truthiness failure (strangler-fig fallback).
fn read_bool_attr(obj: &PyAny, name: &str) -> PyResult<Option<bool>> {
    match obj.getattr(name) {
        Ok(v) => match v.is_true() {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// Read an attribute and report whether it is Python `None`; return `None`
/// to defer on any read failure.
fn read_attr_is_none(obj: &PyAny, name: &str) -> PyResult<Option<bool>> {
    match obj.getattr(name) {
        Ok(v) => Ok(Some(v.is_none())),
        Err(_) => Ok(None),
    }
}

/// `#[pyfunction]` entry for `TypeChecker.check_metaclass_compatibility`
/// (mypy/checker.py:3918-3941).
///
/// `info` is the live `TypeInfo` under check. Rust reads the exempt flags
/// (`is_metaclass` computed via `rust_typeinfo_is_metaclass`, the stored
/// `is_protocol` / `is_named_tuple` / `is_enum` flags, and the
/// `typeddict_type`/`metaclass_type` None tests) and walks `info.bases` to
/// test whether any base carries a metaclass. Returns `Some(tag)` for the
/// two reachable branches, or `None` to defer (an unreadable attribute).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_metaclass_compat(
    py: Python<'_>,
    info: &PyAny,
) -> PyResult<Option<i64>> {
    // `is_metaclass` is a computed method (not a stored flag), so mirror it
    // via the existing native helper (has_base + fullname + fallback_to_any)
    // with precise=False, matching the Python default at the call site.
    let is_metaclass = match crate::checker_visitor::rust_typeinfo_is_metaclass(py, info, false) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let is_protocol = match read_bool_attr(info, "is_protocol")? {
        Some(b) => b,
        None => return Ok(None),
    };
    let is_named_tuple = match read_bool_attr(info, "is_named_tuple")? {
        Some(b) => b,
        None => return Ok(None),
    };
    let is_enum = match read_bool_attr(info, "is_enum")? {
        Some(b) => b,
        None => return Ok(None),
    };
    let typeddict_type_is_none = match read_attr_is_none(info, "typeddict_type")? {
        Some(b) => b,
        None => return Ok(None),
    };
    let metaclass_type_is_none = match read_attr_is_none(info, "metaclass_type")? {
        Some(b) => b,
        None => return Ok(None),
    };

    // Walk `info.bases` (a list of Instance); for each base read
    // `base.type.metaclass_type` and test `is not None`. Defer on any read
    // failure so a malformed live object falls back to Python.
    let any_base_has_metaclass = {
        let bases = match info.getattr("bases") {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        let bases_list = match bases.downcast::<pyo3::types::PyList>() {
            Ok(l) => l,
            Err(_) => return Ok(None),
        };
        let mut found = false;
        for base in bases_list.iter() {
            let base_type = match base.getattr("type") {
                Ok(t) => t,
                Err(_) => return Ok(None),
            };
            let mc = match base_type.getattr("metaclass_type") {
                Ok(t) => t,
                Err(_) => return Ok(None),
            };
            if !mc.is_none() {
                found = true;
                break;
            }
        }
        found
    };

    Ok(Some(classify_metaclass_compat(
        is_metaclass,
        is_protocol,
        is_named_tuple,
        is_enum,
        typeddict_type_is_none,
        metaclass_type_is_none,
        any_base_has_metaclass,
    )))
}

/// `TypeChecker.is_final_enum_value` (checker.py:3825-3848): a pure bool
/// predicate over a `SymbolTableNode`. FuncBase/Decorator -> False (a
/// method is fine); non-Var -> True (class or anything else); for a Var,
/// private/dunder/sunder names or a `FunctionLike` proper type -> False,
/// else `is_stub or has_explicit_value`. Reads live objects via PyO3,
/// mirroring `rust_is_magic_base`; never defers (always returns bool).
#[pyfunction]
pub(crate) fn rust_is_final_enum_value(
    py: Python<'_>,
    sym: &PyAny,
    is_stub: bool,
) -> PyResult<bool> {
    let node = sym.getattr("node")?;
    let var_cls = nodes_class(py, "Var")?;
    let func_base_cls = nodes_class(py, "FuncBase")?;
    let decorator_cls = nodes_class(py, "Decorator")?;
    if node.is_instance(func_base_cls)? || node.is_instance(decorator_cls)? {
        return Ok(false);
    }
    if !node.is_instance(var_cls)? {
        return Ok(true);
    }
    let name: String = node.getattr("name")?.extract()?;
    if is_private(&name) || is_dunder(&name) || is_sunder(&name) {
        return Ok(false);
    }
    let typ = node.getattr("type")?;
    let types_mod = py.import("mypy.types")?;
    let proper = types_mod.getattr("get_proper_type")?.call1((typ,))?;
    let function_like_cls: &PyType = types_mod.getattr("FunctionLike")?.downcast()?;
    if proper.is_instance(function_like_cls)? {
        return Ok(false);
    }
    let has_explicit_value: bool = node.getattr("has_explicit_value")?.extract()?;
    Ok(is_stub || has_explicit_value)
}

/// `TypeChecker.check_for_untyped_decorator` (checker.py:6955-6964): the
/// bool conjunction `disallow_untyped_decorators and is_typed_callable(func.type)
/// and is_untyped_decorator(dec_type) and not current_node_deferred`, reduced to
/// a pure decision over resolved facts. `func_is_typed` / `dec_is_untyped` are
/// `Option<bool>` where `None` means the sub-predicate deferred (e.g. an
/// instance decorator whose `__call__` needs live TypeInfo, or an alias).
/// Short-circuit order mirrors the Python body, so a `false` disallow flag or
/// an untruthy sub-predicate answers `Some(false)` without touching the rest.
fn check_untyped_decorator(
    disallow: bool,
    func_is_typed: Option<bool>,
    dec_is_untyped: Option<bool>,
    deferred: bool,
) -> Option<bool> {
    if !disallow {
        return Some(false);
    }
    let func_typed = func_is_typed?;
    if !func_typed {
        return Some(false);
    }
    let dec_untyped = dec_is_untyped?;
    if !dec_untyped {
        return Some(false);
    }
    Some(!deferred)
}

/// `#[pyfunction]` entry for `TypeChecker.check_for_untyped_decorator`
/// (mypy/checker.py:6955-6964). The two type sub-predicates are computed on
/// the wire format via the existing `is_typed_callable` /
/// `is_untyped_decorator` ports; the scalar flags (`disallow_untyped_decorators`
/// and `current_node_deferred`) arrive as plain bools. Returns `Some(bool)` for
/// every decidable conjunction, or `None` to defer (an undecodable type blob or
/// a deferred sub-predicate, which mirrors the Python `try/except` shim).
#[pyfunction]
#[pyo3(signature = (disallow_untyped_decorators, func_type_bytes, dec_type_bytes, current_node_deferred))]
pub(crate) fn rust_check_for_untyped_decorator(
    disallow_untyped_decorators: bool,
    func_type_bytes: Option<&[u8]>,
    dec_type_bytes: Option<&[u8]>,
    current_node_deferred: bool,
) -> PyResult<Option<bool>> {
    let func_is_typed = match func_type_bytes {
        // `is_typed_callable(None)` is False (get_proper_type(None) is falsy).
        None => Some(false),
        Some(bytes) => match crate::checkmember::decode_type(bytes) {
            Some(t) => crate::checkexpr_functions::is_typed_callable_inner(&t),
            None => return Ok(None),
        },
    };
    let dec_is_untyped = match dec_type_bytes {
        // `is_untyped_decorator(None)` is True (get_proper_type(None) is falsy).
        None => Some(true),
        Some(bytes) => match crate::checkmember::decode_type(bytes) {
            Some(t) => crate::checkexpr_functions::is_untyped_decorator_inner(&t),
            None => return Ok(None),
        },
    };
    Ok(check_untyped_decorator(
        disallow_untyped_decorators,
        func_is_typed,
        dec_is_untyped,
        current_node_deferred,
    ))
}

/// `TypeChecker.check_match_args` (checker.py:3128-3141): the type
/// predicate head. Python keeps the `scope.active_class()` gate and the
/// LITERAL_REQ note emission; Rust reads one wire Type and returns
/// `isinstance(typ, TupleType) and all(is_string_literal(item))` as a
/// plain bool, mirroring `rust_is_final_enum_value` (pure bool, no
/// checker callbacks). Defers (`None`) on an undecodable blob, an alias
/// type (`get_proper_type` would unwrap it on the Python side), or any
/// tuple item whose string-literal check defers.
#[pyfunction]
pub(crate) fn rust_check_match_args(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match crate::checkmember::decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(check_match_args_inner(&typ))
}

pub(crate) fn check_match_args_inner(typ: &Type) -> Option<bool> {
    let proper = crate::checker_helpers::get_proper_or_none(typ)?;
    let items = match proper {
        Type::TupleType { items, .. } => items,
        _ => return Some(false),
    };
    let mut all_str = true;
    for item in items.iter() {
        match crate::checkexpr_functions::is_string_literal_inner(item) {
            Some(true) => {}
            Some(false) => return Some(false),
            None => {
                // An undecidable item means Python's `is_string_literal`
                // fallback (try_getting_str_literals_from_type) may still
                // answer; defer.
                all_str = false;
                break;
            }
        }
    }
    if all_str {
        return Some(true);
    }
    None
}

#[cfg(test)]
mod match_args_tests {
    use super::*;
    use crate::wire::LiteralValue;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_str_literal(s: &str) -> Type {
        Type::LiteralType {
            fallback: Box::new(make_instance("builtins.str", vec![])),
            value: LiteralValue::Str(s.to_string()),
        }
    }

    fn make_int_literal(v: i64) -> Type {
        Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(v),
        }
    }

    fn make_tuple(items: Vec<Type>) -> Type {
        Type::TupleType {
            items,
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            implicit: false,
        }
    }

    #[test]
    fn test_non_tuple_defers_to_false() {
        // Not a tuple -> note is emitted -> Some(false).
        assert_eq!(
            check_match_args_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_empty_tuple_is_valid_match_args() {
        assert_eq!(check_match_args_inner(&make_tuple(vec![])), Some(true));
    }

    #[test]
    fn test_all_string_literals() {
        let t = make_tuple(vec![make_str_literal("a"), make_str_literal("b")]);
        assert_eq!(check_match_args_inner(&t), Some(true));
    }

    #[test]
    fn test_int_literal_item_fails() {
        let t = make_tuple(vec![make_str_literal("a"), make_int_literal(1)]);
        assert_eq!(check_match_args_inner(&t), Some(false));
    }

    #[test]
    fn test_alias_item_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "A".to_string(),
        };
        let t = make_tuple(vec![alias]);
        assert_eq!(check_match_args_inner(&t), None);
    }

    #[test]
    fn test_alias_type_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "T".to_string(),
        };
        assert_eq!(check_match_args_inner(&alias), None);
    }
}

#[cfg(test)]
mod metaclass_compat_tests {
    use super::*;

    fn classify(
        is_metaclass: bool,
        is_protocol: bool,
        is_named_tuple: bool,
        is_enum: bool,
        typeddict_type_is_none: bool,
        metaclass_type_is_none: bool,
        any_base_has_metaclass: bool,
    ) -> i64 {
        classify_metaclass_compat(
            is_metaclass,
            is_protocol,
            is_named_tuple,
            is_enum,
            typeddict_type_is_none,
            metaclass_type_is_none,
            any_base_has_metaclass,
        )
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_metaclass() {
        assert_eq!(
            classify(true, false, false, false, true, true, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_protocol() {
        assert_eq!(
            classify(false, true, false, false, true, true, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_named_tuple() {
        assert_eq!(
            classify(false, false, true, false, true, true, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_enum() {
        assert_eq!(
            classify(false, false, false, true, true, true, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_typeddict() {
        // typeddict_type is not None -> typeddict_type_is_none == False.
        assert_eq!(
            classify(false, false, false, false, false, true, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_conflict() {
        // No exempt flag, class has no metaclass, a base has one.
        assert_eq!(
            classify(false, false, false, false, true, true, true),
            KIND_METACLASS_CONFLICT
        );
    }

    #[test]
    fn test_classify_metaclass_compat_no_conflict_class_has_metaclass() {
        // Class already has a metaclass: no conflict even if a base has one.
        assert_eq!(
            classify(false, false, false, false, true, false, true),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_no_conflict_no_base_metaclass() {
        // No base carries a metaclass: no conflict.
        assert_eq!(
            classify(false, false, false, false, true, true, false),
            KIND_METACLASS_PASS
        );
    }

    #[test]
    fn test_classify_metaclass_compat_exempt_wins_over_conflict() {
        // Exemption short-circuits before the conflict arm.
        assert_eq!(
            classify(true, false, false, false, true, true, true),
            KIND_METACLASS_PASS
        );
        assert_eq!(
            classify(false, false, false, false, false, true, true),
            KIND_METACLASS_PASS
        );
    }
}

#[cfg(test)]
mod classvar_super_tests {
    use super::*;

    fn classify(is_var: bool, node_is_classvar: bool, base_is_classvar: bool) -> i64 {
        classify_classvar_super(is_var, node_is_classvar, base_is_classvar)
    }

    #[test]
    fn test_not_var_passes() {
        assert_eq!(classify(false, true, true), KIND_CLASSVAR_SUPER_NOT_VAR);
        assert_eq!(classify(false, false, false), KIND_CLASSVAR_SUPER_NOT_VAR);
    }

    #[test]
    fn test_both_classvar_ok() {
        assert_eq!(classify(true, true, true), KIND_CLASSVAR_SUPER_OK);
    }

    #[test]
    fn test_both_not_classvar_ok() {
        assert_eq!(classify(true, false, false), KIND_CLASSVAR_SUPER_OK);
    }

    #[test]
    fn test_node_classvar_base_not_instance_var_violation() {
        assert_eq!(
            classify(true, true, false),
            KIND_CLASSVAR_SUPER_INSTANCE_VAR
        );
    }

    #[test]
    fn test_node_not_classvar_base_class_var_violation() {
        assert_eq!(classify(true, false, true), KIND_CLASSVAR_SUPER_CLASS_VAR);
    }
}

#[cfg(test)]
mod final_enum_value_tests {
    use super::*;

    #[test]
    fn test_is_dunder() {
        assert!(is_dunder("__init__"));
        assert!(is_dunder("__hash__"));
        assert!(!is_dunder("_order_"));
        assert!(!is_dunder("__private"));
        assert!(!is_dunder("plain"));
        assert!(!is_dunder("_"));
    }

    #[test]
    fn test_is_sunder() {
        assert!(is_sunder("_order_"));
        assert!(is_sunder("_value_"));
        assert!(!is_sunder("__init__"));
        assert!(!is_sunder("__private"));
        assert!(!is_sunder("plain"));
        assert!(!is_sunder("_"));
    }

    #[test]
    fn test_is_private_overlap() {
        // A name like `__value_` is both private and sunder.
        assert!(is_private("__value_"));
        assert!(is_sunder("__value_"));
        // `__prop` is private but not sunder (no trailing underscore).
        assert!(is_private("__prop"));
        assert!(!is_sunder("__prop"));
    }
}

// ---------------------------------------------------------------------------
// check_lvalue dispatch port (issue #955)
// ---------------------------------------------------------------------------

/// Decision tags; values must match `NATIVE_LVALUE_*` in mypy/checker.py.
const KIND_LVALUE_NAME_DEF: i64 = 0;
const KIND_LVALUE_MEMBER_DEF: i64 = 1;
const KIND_LVALUE_INDEX: i64 = 2;
const KIND_LVALUE_MEMBER: i64 = 3;
const KIND_LVALUE_NAME: i64 = 4;
const KIND_LVALUE_TUPLE_LIST: i64 = 5;
const KIND_LVALUE_STAR: i64 = 6;
const KIND_LVALUE_ELSE: i64 = 7;

/// The pure decision over resolved facts. Kept separate from the PyO3
/// entry so the branch algebra is unit-testable without a Python runtime.
///
/// `is_definition` is `self.is_definition(lvalue)` (passed from Python);
/// `is_name` / `is_member` / `is_index` / `is_tuple` / `is_list` / `is_star`
/// are the lvalue node-kind tags; `node_is_var` is `isinstance(lvalue.node,
/// Var)` (meaningful only when `is_name`); `skip_definition` mirrors the
/// Python conjunction. Branch order matches the Python `if/elif` chain.
#[allow(clippy::too_many_arguments)]
fn classify_check_lvalue(
    is_definition: bool,
    is_name: bool,
    is_member: bool,
    is_index: bool,
    is_tuple: bool,
    is_list: bool,
    is_star: bool,
    node_is_var: bool,
    skip_definition: bool,
) -> i64 {
    if is_definition && (!is_name || node_is_var) && !skip_definition {
        if is_name {
            KIND_LVALUE_NAME_DEF
        } else {
            KIND_LVALUE_MEMBER_DEF
        }
    } else if is_index {
        KIND_LVALUE_INDEX
    } else if is_member {
        KIND_LVALUE_MEMBER
    } else if is_name {
        KIND_LVALUE_NAME
    } else if is_tuple || is_list {
        KIND_LVALUE_TUPLE_LIST
    } else if is_star {
        KIND_LVALUE_STAR
    } else {
        KIND_LVALUE_ELSE
    }
}

/// `#[pyfunction]` entry for `TypeChecker.check_lvalue`
/// (mypy/checker.py:5568-5632). Reads the live `lvalue` node-kind tags and
/// the `Var` node facts needed for `skip_definition` via PyO3;
/// `is_definition` (`self.is_definition(lvalue)`) and `allow_redefinition`
/// (`self.options.allow_redefinition`) arrive as plain bools. Returns
/// `Some(tag)` for every reachable branch, or `None` to defer (an unreadable
/// node fact, mirroring the Python `try/except` shim).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_check_lvalue(
    py: Python<'_>,
    lvalue: &PyAny,
    allow_redefinition: bool,
    is_definition: bool,
) -> PyResult<Option<i64>> {
    let name_cls = nodes_class(py, "NameExpr")?;
    let member_cls = nodes_class(py, "MemberExpr")?;
    let index_cls = nodes_class(py, "IndexExpr")?;
    let tuple_cls = nodes_class(py, "TupleExpr")?;
    let list_cls = nodes_class(py, "ListExpr")?;
    let star_cls = nodes_class(py, "StarExpr")?;
    let var_cls = nodes_class(py, "Var")?;
    let partial_type_cls = py
        .import("mypy.types")?
        .getattr("PartialType")?
        .downcast::<PyType>()?;

    let is_name = lvalue.is_instance(name_cls)?;
    let is_member = lvalue.is_instance(member_cls)?;
    let is_index = lvalue.is_instance(index_cls)?;
    let is_tuple = lvalue.is_instance(tuple_cls)?;
    let is_list = lvalue.is_instance(list_cls)?;
    let is_star = lvalue.is_instance(star_cls)?;

    // `node_is_var` and `skip_definition` only matter when `is_name`; for
    // other node kinds the Python top-condition short-circuits and
    // `skip_definition` is False.
    let (node_is_var, skip_definition) = if is_name {
        let node = match lvalue.getattr("node") {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        let node_is_var = node.is_instance(var_cls)?;
        let skip_definition = if allow_redefinition && node_is_var {
            let is_inferred = match read_bool_attr(node, "is_inferred")? {
                Some(b) => b,
                None => return Ok(None),
            };
            if !is_inferred {
                false
            } else {
                let node_type = match node.getattr("type") {
                    Ok(t) => t,
                    Err(_) => return Ok(None),
                };
                if node_type.is_none() {
                    false
                } else {
                    let type_is_partial = node_type.is_instance(partial_type_cls)?;
                    if type_is_partial {
                        false
                    } else {
                        match read_bool_attr(node, "is_index_var")? {
                            Some(b) => !b,
                            None => return Ok(None),
                        }
                    }
                }
            }
        } else {
            false
        };
        (node_is_var, skip_definition)
    } else {
        (false, false)
    };

    Ok(Some(classify_check_lvalue(
        is_definition,
        is_name,
        is_member,
        is_index,
        is_tuple,
        is_list,
        is_star,
        node_is_var,
        skip_definition,
    )))
}

#[cfg(test)]
mod check_lvalue_tests {
    use super::*;

    fn classify(args: [bool; 9]) -> i64 {
        classify_check_lvalue(
            args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
        )
    }

    #[test]
    fn test_name_def() {
        // is_definition, NameExpr, node is Var, not skip -> NAME_DEF.
        assert_eq!(
            classify([true, true, false, false, false, false, false, true, false]),
            KIND_LVALUE_NAME_DEF
        );
    }

    #[test]
    fn test_member_def() {
        // is_definition, not NameExpr (MemberExpr) -> MEMBER_DEF.
        assert_eq!(
            classify([true, false, true, false, false, false, false, false, false]),
            KIND_LVALUE_MEMBER_DEF
        );
    }

    #[test]
    fn test_name_def_skipped_falls_to_name() {
        // is_definition True but skip True -> falls to the NameExpr branch.
        assert_eq!(
            classify([true, true, false, false, false, false, false, true, true]),
            KIND_LVALUE_NAME
        );
    }

    #[test]
    fn test_name_def_node_not_var_falls_to_name() {
        // is_definition True, NameExpr, node not Var -> NAME branch.
        assert_eq!(
            classify([true, true, false, false, false, false, false, false, false]),
            KIND_LVALUE_NAME
        );
    }

    #[test]
    fn test_index() {
        assert_eq!(
            classify([false, false, false, true, false, false, false, false, false]),
            KIND_LVALUE_INDEX
        );
    }

    #[test]
    fn test_member() {
        assert_eq!(
            classify([false, false, true, false, false, false, false, false, false]),
            KIND_LVALUE_MEMBER
        );
    }

    #[test]
    fn test_name() {
        assert_eq!(
            classify([false, true, false, false, false, false, false, false, false]),
            KIND_LVALUE_NAME
        );
    }

    #[test]
    fn test_tuple() {
        assert_eq!(
            classify([false, false, false, false, true, false, false, false, false]),
            KIND_LVALUE_TUPLE_LIST
        );
    }

    #[test]
    fn test_list() {
        assert_eq!(
            classify([false, false, false, false, false, true, false, false, false]),
            KIND_LVALUE_TUPLE_LIST
        );
    }

    #[test]
    fn test_star() {
        assert_eq!(
            classify([false, false, false, false, false, false, true, false, false]),
            KIND_LVALUE_STAR
        );
    }

    #[test]
    fn test_else() {
        assert_eq!(
            classify([false, false, false, false, false, false, false, false, false]),
            KIND_LVALUE_ELSE
        );
    }
}

#[cfg(test)]
mod check_untyped_decorator_tests {
    use super::*;

    fn decide(
        disallow: bool,
        func_is_typed: Option<bool>,
        dec_is_untyped: Option<bool>,
        deferred: bool,
    ) -> Option<bool> {
        check_untyped_decorator(disallow, func_is_typed, dec_is_untyped, deferred)
    }

    #[test]
    fn test_disallow_false_short_circuits() {
        // disallow off: False regardless of the sub-predicates.
        assert_eq!(decide(false, None, None, true), Some(false));
        assert_eq!(decide(false, Some(true), Some(true), false), Some(false));
    }

    #[test]
    fn test_func_not_typed_short_circuits() {
        assert_eq!(decide(true, Some(false), None, true), Some(false));
        assert_eq!(decide(true, Some(false), Some(true), false), Some(false));
    }

    #[test]
    fn test_func_typed_defer_propagates() {
        assert_eq!(decide(true, None, Some(true), false), None);
    }

    #[test]
    fn test_dec_not_untyped_short_circuits() {
        assert_eq!(decide(true, Some(true), Some(false), true), Some(false));
        assert_eq!(decide(true, Some(true), Some(false), false), Some(false));
    }

    #[test]
    fn test_dec_untyped_defer_propagates() {
        assert_eq!(decide(true, Some(true), None, false), None);
    }

    #[test]
    fn test_all_true_not_deferred() {
        assert_eq!(decide(true, Some(true), Some(true), false), Some(true));
    }

    #[test]
    fn test_all_true_but_deferred() {
        assert_eq!(decide(true, Some(true), Some(true), true), Some(false));
    }
}

// `TypeChecker.check_explicit_override_decorator` conjunction port
// (checker.py:3137-3158); the message emission stays in Python.

/// The pure 5-flag conjunction of `check_explicit_override_decorator`
/// (checker.py:3149-3155). Kept separate from the PyO3 entry so the
/// predicate is unit-testable without a Python runtime.
fn check_explicit_override_decorator(
    plugin_generated: bool,
    found_method_base_classes: bool,
    is_explicit_override: bool,
    is_init_or_new: bool,
    is_private_name: bool,
) -> bool {
    !plugin_generated
        && found_method_base_classes
        && !is_explicit_override
        && !is_init_or_new
        && !is_private_name
}

/// `#[pyfunction]` entry for `TypeChecker.check_explicit_override_decorator`
/// (mypy/checker.py:3137-3158).
///
/// `defn` is the live `FuncDef`/`OverloadedFuncDef`; `found_method_base_classes`
/// is the live `list[TypeInfo] | None`. Rust reads the 5 scalar flags via PyO3:
/// `plugin_generated` (from `defn.info.get(defn.name).plugin_generated`),
/// `found_method_base_classes` non-empty, `defn.is_explicit_override`,
/// `defn.name` membership in {"__init__", "__new__"}, and `is_private(name)`.
/// Returns `true` when the full conjunction holds (the shim emits the missing
/// decorator message); returns `false` to defer to the pure-Python body when a
/// flag is None or unreadable, mirroring the Python default for `plugin_generated`.
#[pyfunction]
pub(crate) fn rust_check_explicit_override_decorator(
    defn: &PyAny,
    found_method_base_classes: &PyAny,
) -> PyResult<bool> {
    // `defn.name`: needed for the dunder check, `is_private`, and the symbol
    // lookup into `defn.info`.
    let name: String = match defn.getattr("name") {
        Ok(v) => match v.extract() {
            Ok(s) => s,
            Err(_) => return Ok(false),
        },
        Err(_) => return Ok(false),
    };

    // `plugin_generated` (checker.py:3143-3147): true only when `defn.info`,
    // `defn.info.get(defn.name)`, and the symbol node's `plugin_generated`
    // attr are all truthy; a None lookup or unreadable attr defers to Python.
    let info = match defn.getattr("info") {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    if info.is_none() {
        return Ok(false);
    }
    let node = match info.call_method1("get", (&name,)) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    if node.is_none() {
        return Ok(false);
    }
    let plugin_generated: bool = match node.getattr("plugin_generated") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(false),
        },
        Err(_) => return Ok(false),
    };

    // `found_method_base_classes` truthiness: a non-empty list.
    let found = match found_method_base_classes.is_true() {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };

    // `defn.is_explicit_override`.
    let is_explicit_override: bool = match defn.getattr("is_explicit_override") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(false),
        },
        Err(_) => return Ok(false),
    };

    let is_init_or_new = name == "__init__" || name == "__new__";
    let is_private_name = is_private(&name);

    Ok(check_explicit_override_decorator(
        plugin_generated,
        found,
        is_explicit_override,
        is_init_or_new,
        is_private_name,
    ))
}

#[cfg(test)]
mod explicit_override_decorator_tests {
    use super::check_explicit_override_decorator;

    fn classify(
        plugin_generated: bool,
        found: bool,
        is_explicit_override: bool,
        name: &str,
    ) -> bool {
        check_explicit_override_decorator(
            plugin_generated,
            found,
            is_explicit_override,
            name == "__init__" || name == "__new__",
            name.starts_with("__") && !name.ends_with("__"),
        )
    }

    #[test]
    fn test_emit_plain_override_public_name() {
        // All flags false except found: emit.
        assert!(classify(false, true, false, "override"));
    }

    #[test]
    fn test_plugin_generated_suppresses() {
        // plugin-generated methods are exempt even with a base class.
        assert!(!classify(true, true, false, "override"));
    }

    #[test]
    fn test_no_base_class_does_not_emit() {
        // found is falsy: no method base classes.
        assert!(!classify(false, false, false, "override"));
        assert!(!classify(false, false, false, "__init__"));
    }

    #[test]
    fn test_explicit_override_suppresses() {
        assert!(!classify(false, true, true, "override"));
    }

    #[test]
    fn test_init_and_new_suppress() {
        assert!(!classify(false, true, false, "__init__"));
        assert!(!classify(false, true, false, "__new__"));
    }

    #[test]
    fn test_private_name_suppresses() {
        // `is_private` requires starts_with("__") and not ends_with("__").
        assert!(!classify(false, true, false, "__private"));
        // Single underscore is NOT private by mypy's definition.
        assert!(classify(false, true, false, "_single"));
        // Dunder names are not private either.
        assert!(classify(false, true, false, "__dunder__"));
    }
}

#[cfg(test)]
mod classify_enum_tests {
    use super::*;

    fn classify(
        members_override: bool,
        stub_empty: bool,
        final_enum_bases: Vec<&str>,
    ) -> (i64, Vec<String>) {
        classify_enum(
            members_override,
            stub_empty,
            final_enum_bases.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn test_no_arms() {
        let (tag, bases) = classify(false, false, vec![]);
        assert_eq!(tag, 0);
        assert!(bases.is_empty());
    }

    #[test]
    fn test_members_override_only() {
        let (tag, _) = classify(true, false, vec![]);
        assert_eq!(tag, ENUM_CHECK_MEMBERS_OVERRIDE);
    }

    #[test]
    fn test_stub_empty_only() {
        let (tag, _) = classify(false, true, vec![]);
        assert_eq!(tag, ENUM_CHECK_STUB_EMPTY);
    }

    #[test]
    fn test_members_and_stub() {
        let (tag, _) = classify(true, true, vec![]);
        assert_eq!(tag, ENUM_CHECK_MEMBERS_OVERRIDE | ENUM_CHECK_STUB_EMPTY);
    }

    #[test]
    fn test_final_enum_bases() {
        let (tag, bases) = classify(false, false, vec!["mod.A", "mod.B"]);
        assert_eq!(tag, 0);
        assert_eq!(bases, vec!["mod.A", "mod.B"]);
    }

    #[test]
    fn test_all_three() {
        let (tag, bases) = classify(true, true, vec!["mod.A"]);
        assert_eq!(tag, ENUM_CHECK_MEMBERS_OVERRIDE | ENUM_CHECK_STUB_EMPTY);
        assert_eq!(bases, vec!["mod.A"]);
    }
}

// ---------------------------------------------------------------------------
// check_rvalue_count_in_assignment dispatch port (issue #1003)
// ---------------------------------------------------------------------------

/// Decision tags for `check_rvalue_count_in_assignment`; must match
/// `NATIVE_RVALUE_COUNT_*` in mypy/checker.py.
const RVALUE_COUNT_PASS: i64 = 0;
const RVALUE_COUNT_FAIL_STAR_REQUIRED: i64 = 1;
const RVALUE_COUNT_FAIL_TOO_MANY: i64 = 2;
const RVALUE_COUNT_WARN_TOO_MANY: i64 = 3;
const RVALUE_COUNT_FAIL_WRONG_STAR: i64 = 4;
const RVALUE_COUNT_FAIL_WRONG: i64 = 5;

/// Pure decision mirroring `TypeChecker.check_rvalue_count_in_assignment`
/// (checker.py:5319-5354). The variadic arm (`rvalue_unpack` set) requires
/// a star target, rejects too many targets, and flags asymmetric
/// prefix/suffix unpack while still succeeding. The plain arms check the
/// star-lvalue count (`len - 1`) or the exact count.
fn classify_rvalue_count(
    has_star: bool,
    star_index: i64,
    lvalues_len: i64,
    rvalue_count: i64,
    rvalue_unpack: Option<i64>,
) -> i64 {
    if let Some(rv_unpack) = rvalue_unpack {
        if !has_star {
            return RVALUE_COUNT_FAIL_STAR_REQUIRED;
        }
        if lvalues_len > rvalue_count {
            return RVALUE_COUNT_FAIL_TOO_MANY;
        }
        let left_prefix = star_index;
        let left_suffix = lvalues_len - star_index - 1;
        let right_prefix = rv_unpack;
        let right_suffix = rvalue_count - rv_unpack - 1;
        if left_suffix > right_suffix || left_prefix > right_prefix {
            return RVALUE_COUNT_WARN_TOO_MANY;
        }
        return RVALUE_COUNT_PASS;
    }
    if has_star {
        if lvalues_len - 1 > rvalue_count {
            return RVALUE_COUNT_FAIL_WRONG_STAR;
        }
    } else if rvalue_count != lvalues_len {
        return RVALUE_COUNT_FAIL_WRONG;
    }
    RVALUE_COUNT_PASS
}

/// `#[pyfunction]` entry for `TypeChecker.check_rvalue_count_in_assignment`
/// (mypy/checker.py:5319-5354). Reads the live `lvalues` list via PyO3
/// (StarExpr isinstance scan) plus the scalar `rvalue_count` and optional
/// `rvalue_unpack` arity. Returns `Some(tag)` for every reachable branch,
/// or `None` when `lvalues` is not a list (deferral). The Python shim
/// applies the fail / wrong-number side effects.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_rvalue_count(
    py: Python<'_>,
    lvalues: &PyAny,
    rvalue_count: i64,
    rvalue_unpack: Option<i64>,
) -> PyResult<Option<i64>> {
    let list = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let star_cls = nodes_class(py, "StarExpr")?;
    let mut has_star = false;
    let mut star_index = 0i64;
    for (i, lv) in list.iter().enumerate() {
        if !has_star && lv.is_instance(star_cls)? {
            has_star = true;
            star_index = i as i64;
        }
    }
    Ok(Some(classify_rvalue_count(
        has_star,
        star_index,
        list.len() as i64,
        rvalue_count,
        rvalue_unpack,
    )))
}

/// Decision tags; values must match `NATIVE_MISSING_ANN_*` in
/// mypy/checker.py.
const KIND_MISSING_ANN_NONE: i64 = 0;
const KIND_MISSING_ANN_RETURN_UNTYPED: i64 = 1;
const KIND_MISSING_ANN_FUNC_TYPE_EXPECTED: i64 = 2;
const KIND_MISSING_ANN_RETURN_EXPECTED: i64 = 3;

/// The shim's `fdef.type` isinstance classification; values must match
/// `NATIVE_MISSING_ANN_TYPE_*` in mypy/checker.py. `TYPE_TAG_OTHER` is
/// produced by the Python shim only.
#[allow(dead_code)]
const TYPE_TAG_NONE: i64 = 0;
#[allow(dead_code)]
const TYPE_TAG_CALLABLE: i64 = 1;
#[allow(dead_code)]
const TYPE_TAG_OTHER: i64 = 2;

/// The pure decision behind `TypeChecker.check_for_missing_annotations`
/// (checker.py:2722-2771). Kept separate from the PyO3 entry so the branch
/// algebra is unit-testable without a Python runtime.
///
/// Mirrors the Python order exactly: the `show_untyped` gate, the
/// `has_explicit_annotation` scan (raw ret/arg types; aliases are never
/// unannotated-any, matching `is_unannotated_any` on a non-ProperType),
/// the `disallow_untyped_defs or check_incomplete_defs` gate, the
/// self/cls-only special case for an untyped def, and the per-site
/// return/param Any-ness with generator/coroutine ret unwrapping (reusing
/// the `get_generator_return_type` / `get_coroutine_return_type` ports).
/// Returns `(tag, param_fail)`: `tag` selects the untyped-def /
/// return-site failure, `param_fail` is the independent
/// PARAM_TYPE_EXPECTED site. `None` defers to the pure-Python body
/// (alias ret type, decode failure, or an undecided generator unwrap).
#[allow(clippy::too_many_arguments)]
fn classify_missing_annotations(
    is_typeshed_stub: bool,
    warn_incomplete_stub: bool,
    disallow_untyped_defs: bool,
    disallow_incomplete_defs: bool,
    type_tag: i64,
    arguments_len: usize,
    arg_names: &[Option<String>],
    is_generator: bool,
    is_coroutine: bool,
    ret_type: Option<&Type>,
    arg_types: &[Type],
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<(i64, bool)> {
    // show_untyped = not is_typeshed_stub or warn_incomplete_stub.
    if is_typeshed_stub && !warn_incomplete_stub {
        return Some((KIND_MISSING_ANN_NONE, false));
    }
    let mut has_explicit_annotation = false;
    if type_tag == TYPE_TAG_CALLABLE {
        for t in arg_types {
            if !crate::visitor::is_unannotated_any_inner(t) {
                has_explicit_annotation = true;
                break;
            }
        }
        if !has_explicit_annotation {
            if let Some(ret) = ret_type {
                if !crate::visitor::is_unannotated_any_inner(ret) {
                    has_explicit_annotation = true;
                }
            }
        }
    }
    let check_incomplete_defs = disallow_incomplete_defs && has_explicit_annotation;
    if !(disallow_untyped_defs || check_incomplete_defs) {
        return Some((KIND_MISSING_ANN_NONE, false));
    }
    if type_tag == TYPE_TAG_NONE && disallow_untyped_defs {
        let self_cls_only = arguments_len == 0
            || (arguments_len == 1
                && matches!(arg_names.first(), Some(Some(name)) if name == "self" || name == "cls"));
        return Some((
            if self_cls_only {
                KIND_MISSING_ANN_RETURN_UNTYPED
            } else {
                KIND_MISSING_ANN_FUNC_TYPE_EXPECTED
            },
            false,
        ));
    }
    if type_tag == TYPE_TAG_CALLABLE {
        let ret = ret_type?;
        // get_proper_type(fdef.type.ret_type): an alias expands on the
        // Python side (the wire carries no alias target), so defer.
        if matches!(ret, Type::TypeAliasType { .. }) {
            return None;
        }
        let ret_fail = if crate::visitor::is_unannotated_any_inner(ret) {
            true
        } else if is_generator {
            let g = crate::generators::get_generator_return_type_inner(
                ret,
                is_coroutine,
                strict_optional,
                res,
            )?;
            crate::visitor::is_unannotated_any_inner(&g)
        } else if is_coroutine && matches!(ret, Type::Instance { .. }) {
            let c = crate::generators::get_coroutine_return_type_inner(ret)?;
            crate::visitor::is_unannotated_any_inner(&c)
        } else {
            false
        };
        let param_fail = arg_types
            .iter()
            .any(crate::visitor::is_unannotated_any_inner);
        return Some((
            if ret_fail {
                KIND_MISSING_ANN_RETURN_EXPECTED
            } else {
                KIND_MISSING_ANN_NONE
            },
            param_fail,
        ));
    }
    Some((KIND_MISSING_ANN_NONE, false))
}

/// `TypeChecker.check_for_missing_annotations` (checker.py:2722-2771): the
/// annotation-completeness decision head, ported. Rust reads the option
/// bools, the shim's `fdef.type` isinstance tag, `len(fdef.arguments)` /
/// `arg_names` (for the self/cls-only special case), the generator /
/// coroutine flags, and the raw ret/arg types as wire bytes
/// (`is_unannotated_any` is already native; generator/coroutine ret
/// unwrapping reuses the existing ports). Returns `(tag, param_fail)`;
/// the Python shim applies the fail/note side effects (the RETURN_UNTYPED
/// note decision routes through the existing `rust_has_return_statement`
/// seam) and keeps the pure-Python body as the fallback. Defers (`None`)
/// on an undecodable blob, a `TypeAliasType` ret type, or an undecided
/// generator unwrap.
#[pyfunction]
#[pyo3(signature = (is_typeshed_stub, warn_incomplete_stub, disallow_untyped_defs, disallow_incomplete_defs, type_tag, arguments_len, arg_names, is_generator, is_coroutine, ret_type_bytes, arg_type_blobs, strict_optional, resolver))]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_missing_annotations(
    is_typeshed_stub: bool,
    warn_incomplete_stub: bool,
    disallow_untyped_defs: bool,
    disallow_incomplete_defs: bool,
    type_tag: i64,
    arguments_len: usize,
    arg_names: Vec<Option<String>>,
    is_generator: bool,
    is_coroutine: bool,
    ret_type_bytes: Option<&[u8]>,
    arg_type_blobs: Vec<Vec<u8>>,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<(i64, bool)>> {
    let ret_type = match ret_type_bytes {
        None => None,
        Some(bytes) => match crate::checkmember::decode_type(bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
    };
    let mut arg_types = Vec::with_capacity(arg_type_blobs.len());
    for blob in &arg_type_blobs {
        match crate::checkmember::decode_type(blob) {
            Some(t) => arg_types.push(t),
            None => return Ok(None),
        }
    }
    Ok(classify_missing_annotations(
        is_typeshed_stub,
        warn_incomplete_stub,
        disallow_untyped_defs,
        disallow_incomplete_defs,
        type_tag,
        arguments_len,
        &arg_names,
        is_generator,
        is_coroutine,
        ret_type.as_ref(),
        &arg_types,
        strict_optional,
        resolver.resolver(),
    ))
}

#[cfg(test)]
mod rvalue_count_tests {
    use super::classify_rvalue_count;
    use super::{
        RVALUE_COUNT_FAIL_STAR_REQUIRED, RVALUE_COUNT_FAIL_TOO_MANY, RVALUE_COUNT_FAIL_WRONG,
        RVALUE_COUNT_FAIL_WRONG_STAR, RVALUE_COUNT_PASS, RVALUE_COUNT_WARN_TOO_MANY,
    };

    #[test]
    fn test_variadic_no_star_fails() {
        assert_eq!(
            classify_rvalue_count(false, 0, 3, 4, Some(2)),
            RVALUE_COUNT_FAIL_STAR_REQUIRED
        );
    }

    #[test]
    fn test_variadic_too_many_targets() {
        assert_eq!(
            classify_rvalue_count(true, 1, 5, 4, Some(2)),
            RVALUE_COUNT_FAIL_TOO_MANY
        );
    }

    #[test]
    fn test_variadic_asymmetric_suffix() {
        // left_suffix 2 > right_suffix 1: warns but succeeds.
        assert_eq!(
            classify_rvalue_count(true, 0, 4, 5, Some(3)),
            RVALUE_COUNT_WARN_TOO_MANY
        );
    }

    #[test]
    fn test_variadic_asymmetric_prefix() {
        // left_prefix 2 > right_prefix 1: warns but succeeds.
        assert_eq!(
            classify_rvalue_count(true, 2, 4, 5, Some(1)),
            RVALUE_COUNT_WARN_TOO_MANY
        );
    }

    #[test]
    fn test_variadic_symmetric_passes() {
        assert_eq!(
            classify_rvalue_count(true, 1, 4, 4, Some(1)),
            RVALUE_COUNT_PASS
        );
        // Right suffix longer than left is fine.
        assert_eq!(
            classify_rvalue_count(true, 0, 2, 4, Some(2)),
            RVALUE_COUNT_PASS
        );
    }

    #[test]
    fn test_star_lvalue_count_fail() {
        // len - 1 = 3 > rvalue_count 2.
        assert_eq!(
            classify_rvalue_count(true, 0, 4, 2, None),
            RVALUE_COUNT_FAIL_WRONG_STAR
        );
    }

    #[test]
    fn test_star_lvalue_count_pass() {
        assert_eq!(
            classify_rvalue_count(true, 0, 3, 2, None),
            RVALUE_COUNT_PASS
        );
    }

    #[test]
    fn test_exact_count_fail() {
        assert_eq!(
            classify_rvalue_count(false, 0, 3, 2, None),
            RVALUE_COUNT_FAIL_WRONG
        );
    }

    #[test]
    fn test_exact_count_pass() {
        assert_eq!(
            classify_rvalue_count(false, 0, 3, 3, None),
            RVALUE_COUNT_PASS
        );
    }
}

#[cfg(test)]
mod missing_annotations_tests {
    use super::classify_missing_annotations;
    use super::{
        KIND_MISSING_ANN_FUNC_TYPE_EXPECTED, KIND_MISSING_ANN_NONE,
        KIND_MISSING_ANN_RETURN_EXPECTED, KIND_MISSING_ANN_RETURN_UNTYPED, TYPE_TAG_CALLABLE,
        TYPE_TAG_NONE, TYPE_TAG_OTHER,
    };
    use crate::subtypes::COVARIANT;
    use crate::typeinfo::{TypeInfoSnapshot, TypeResolver};
    use crate::wire::Type;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    /// A snapshot with its own fullname in mro/has_base and `n` covariant
    /// type vars `(Ti, COVARIANT, kind=0)`, so an exactly-matching Instance
    /// with `n` args can be judged by the nominal path.
    fn generic_snap(fullname: &str, name: &str, n_tvars: usize) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s.type_vars_with_variance = (0..n_tvars)
            .map(|i| (format!("T{i}"), COVARIANT, 0))
            .collect();
        s
    }

    fn generator_snap() -> TypeInfoSnapshot {
        generic_snap("typing.Generator", "Generator", 3)
    }

    fn instance(ref_name: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: ref_name.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn unannotated_any() -> Type {
        Type::AnyType {
            type_of_any: 1, // TypeOfAny.unannotated
            source_any: None,
            missing_import_name: None,
        }
    }

    fn explicit_any() -> Type {
        Type::AnyType {
            type_of_any: 2, // TypeOfAny.explicit
            source_any: None,
            missing_import_name: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn decide(
        is_typeshed_stub: bool,
        warn_incomplete_stub: bool,
        disallow_untyped_defs: bool,
        disallow_incomplete_defs: bool,
        type_tag: i64,
        arguments_len: usize,
        arg_names: Vec<Option<String>>,
        is_generator: bool,
        is_coroutine: bool,
        ret: Option<Type>,
        args: Vec<Type>,
    ) -> Option<(i64, bool)> {
        let res = make_resolver(vec![generator_snap(), instance_snap()]);
        classify_missing_annotations(
            is_typeshed_stub,
            warn_incomplete_stub,
            disallow_untyped_defs,
            disallow_incomplete_defs,
            type_tag,
            arguments_len,
            &arg_names,
            is_generator,
            is_coroutine,
            ret.as_ref(),
            &args,
            true,
            &res,
        )
    }

    fn instance_snap() -> TypeInfoSnapshot {
        generic_snap("builtins.int", "int", 0)
    }

    fn int_() -> Type {
        instance("builtins.int", vec![])
    }

    #[test]
    fn test_typeshed_stub_no_warn_noop() {
        // show_untyped is False: is_typeshed_stub and not warn_incomplete_stub.
        assert_eq!(
            decide(
                true,
                false,
                true,
                true,
                TYPE_TAG_NONE,
                0,
                vec![],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
        assert_eq!(
            decide(
                true,
                false,
                true,
                true,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(unannotated_any()),
                vec![unannotated_any()]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_typeshed_stub_with_warn_proceeds() {
        // warn_incomplete_stub flips show_untyped back on.
        assert_eq!(
            decide(
                true,
                true,
                true,
                false,
                TYPE_TAG_NONE,
                0,
                vec![],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_UNTYPED, false))
        );
    }

    #[test]
    fn test_no_gates_noop() {
        // Neither disallow flag: the gate is off even with unannotated args.
        assert_eq!(
            decide(
                false,
                false,
                false,
                false,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(explicit_any()),
                vec![unannotated_any()]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
        // Untyped def with only disallow_incomplete_defs: has_explicit is
        // False (non-callable fdef.type), so check_incomplete_defs is off.
        assert_eq!(
            decide(
                false,
                false,
                false,
                true,
                TYPE_TAG_NONE,
                0,
                vec![],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_untyped_def_self_cls_only() {
        // No arguments at all.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                0,
                vec![],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_UNTYPED, false))
        );
        // Single self / cls argument.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                1,
                vec![Some("self".into())],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_UNTYPED, false))
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                1,
                vec![Some("cls".into())],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_UNTYPED, false))
        );
    }

    #[test]
    fn test_untyped_def_other_args() {
        // A single non-self/cls arg, or two args: FUNCTION_TYPE_EXPECTED.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                1,
                vec![Some("x".into())],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_FUNC_TYPE_EXPECTED, false))
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                2,
                vec![Some("self".into()), Some("x".into())],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_FUNC_TYPE_EXPECTED, false))
        );
        // A positional-only first arg (None name) is not self/cls.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_NONE,
                1,
                vec![None],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_FUNC_TYPE_EXPECTED, false))
        );
    }

    #[test]
    fn test_non_callable_other_tag_noop() {
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_OTHER,
                1,
                vec![Some("x".into())],
                false,
                false,
                None,
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_callable_fully_annotated_noop() {
        // Fully annotated: no fail on either site, both under
        // disallow_untyped_defs and under check_incomplete_defs.
        let fully = (
            TYPE_TAG_CALLABLE,
            vec![Some("x".into())],
            Some(int_()),
            vec![int_()],
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                fully.0,
                1,
                fully.1.clone(),
                false,
                false,
                fully.2,
                fully.3
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
        // check_incomplete_defs alone turns the gate on (ret is explicit),
        // but an annotated body emits nothing.
        assert_eq!(
            decide(
                false,
                false,
                false,
                true,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(int_()),
                vec![int_()]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_callable_unannotated_return() {
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(unannotated_any()),
                vec![int_()]
            ),
            Some((KIND_MISSING_ANN_RETURN_EXPECTED, false))
        );
    }

    #[test]
    fn test_callable_unannotated_param() {
        // Annotated ret, unannotated param: only the param site fires.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(int_()),
                vec![unannotated_any()]
            ),
            Some((KIND_MISSING_ANN_NONE, true))
        );
        // Both sites fire together.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                1,
                vec![Some("x".into())],
                false,
                false,
                Some(unannotated_any()),
                vec![unannotated_any()]
            ),
            Some((KIND_MISSING_ANN_RETURN_EXPECTED, true))
        );
    }

    #[test]
    fn test_generator_unannotated_tr() {
        // Generator[int, Any, Any(unannotated)]: the unwrapped return type
        // is unannotated Any, so the return site fires.
        let gen = instance(
            "typing.Generator",
            vec![int_(), explicit_any(), unannotated_any()],
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                true,
                false,
                Some(gen),
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_EXPECTED, false))
        );
        // Generator[int, Any, str]: tr is annotated, no fail.
        let gen = instance("typing.Generator", vec![int_(), explicit_any(), int_()]);
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                true,
                false,
                Some(gen),
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_coroutine_unannotated_tr() {
        // Coroutine[Any, Any, Any(unannotated)]: fires.
        let coro = instance(
            "typing.Coroutine",
            vec![explicit_any(), explicit_any(), unannotated_any()],
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                false,
                true,
                Some(coro),
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_EXPECTED, false))
        );
        // Coroutine[Any, Any, str]: no fail.
        let coro = instance(
            "typing.Coroutine",
            vec![explicit_any(), explicit_any(), int_()],
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                false,
                true,
                Some(coro),
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
        // Coroutine ret on a non-coroutine function: neither generator nor
        // coroutine arm runs; an unannotated-any ret fires directly.
        let coro = instance(
            "typing.Coroutine",
            vec![explicit_any(), explicit_any(), int_()],
        );
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                false,
                false,
                Some(coro),
                vec![]
            ),
            Some((KIND_MISSING_ANN_NONE, false))
        );
    }

    #[test]
    fn test_generator_decided_by_unannotated_ret_before_unwrap() {
        // ret is Any(unannotated) directly: fires before the generator arm.
        assert_eq!(
            decide(
                false,
                false,
                true,
                false,
                TYPE_TAG_CALLABLE,
                0,
                vec![],
                true,
                false,
                Some(unannotated_any()),
                vec![]
            ),
            Some((KIND_MISSING_ANN_RETURN_EXPECTED, false))
        );
    }
}

/// Decision tags for `check_for_truthy_type`; must match `NATIVE_TRUTHY_*`
/// in mypy/checker.py.
const TRUTHY_SKIP: i64 = 0;
const TRUTHY_FUNCTION: i64 = 1;
const TRUTHY_UNION: i64 = 2;
const TRUTHY_ITERABLE: i64 = 3;
const TRUTHY_OTHER: i64 = 4;

/// The Instance arm of `TypeChecker._is_truthy_type` (checker.py:7882-7896)
/// as a pure decision over its scalar facts: `type_present` is
/// `bool(t.type)`, `has_bool` / `has_len` are the `has_readable_member`
/// results, and `fullname_is_object` is the `builtins.object` guard.
fn instance_is_truthy(
    type_present: bool,
    has_bool: bool,
    has_len: bool,
    fullname_is_object: bool,
) -> bool {
    type_present && !has_bool && !has_len && !fullname_is_object
}

/// The `check_for_truthy_type` dispatch (checker.py:7928-7950) for an
/// already-truthy type: FunctionLike first, then UnionType, then the
/// `typing.Iterable` Instance special case, else the generic message.
fn dispatch_truthy_tag(is_function_like: bool, is_union: bool, is_iterable_instance: bool) -> i64 {
    if is_function_like {
        TRUTHY_FUNCTION
    } else if is_union {
        TRUTHY_UNION
    } else if is_iterable_instance {
        TRUTHY_ITERABLE
    } else {
        TRUTHY_OTHER
    }
}

/// The recursive truthiness predicate `_is_truthy_type`
/// (checker.py:7882-7896) over live Python type objects. The shim passes
/// the already-proper type; union items are resolved per-item through
/// `mypy.types.get_proper_type` exactly like `get_proper_types`.
/// Returns `None` to defer on any unreadable fact (the Python shim then
/// falls back to the pure-Python body, which raises identically).
fn is_truthy_type(
    t: &PyAny,
    instance_cls: &PyType,
    fl_cls: &PyType,
    union_cls: &PyType,
    get_proper: &PyAny,
) -> PyResult<Option<bool>> {
    if t.is_instance(instance_cls)? {
        let info = match t.getattr("type") {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let type_present = match info.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if !type_present {
            return Ok(Some(false));
        }
        let mut ok = true;
        for member in ["__bool__", "__len__"] {
            let has = match info.call_method1("has_readable_member", (member,)) {
                Ok(v) => match v.is_true() {
                    Ok(b) => b,
                    Err(_) => return Ok(None),
                },
                Err(_) => return Ok(None),
            };
            if has {
                ok = false;
                break;
            }
        }
        if !ok {
            return Ok(Some(false));
        }
        let fullname: String = match info.getattr("fullname") {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        Ok(Some(instance_is_truthy(
            true,
            false,
            false,
            fullname == "builtins.object",
        )))
    } else if t.is_instance(fl_cls)? {
        Ok(Some(true))
    } else if t.is_instance(union_cls)? {
        let items = match t.getattr("items") {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        for item in items.iter()? {
            let item = item?;
            let proper = get_proper.call1((item,))?;
            match is_truthy_type(proper, instance_cls, fl_cls, union_cls, get_proper)? {
                Some(true) => {}
                Some(false) => return Ok(Some(false)),
                None => return Ok(None),
            }
        }
        Ok(Some(true))
    } else {
        Ok(Some(false))
    }
}

/// `#[pyfunction]` entry for `TypeChecker.check_for_truthy_type`
/// (mypy/checker.py:7898-7956) plus its `_is_truthy_type` helper
/// (checker.py:7882-7896). Rust classifies the strict-optional
/// truthiness arbitration into a branch tag (SKIP / FUNCTION / UNION /
/// ITERABLE / OTHER); the `format_type` message formatting and the
/// `self.fail` emission stay in Python. The shim passes the
/// already-proper type. Defers (`None`) only on an unreadable fact,
/// mirroring the shim `try/except`.
#[pyfunction]
pub(crate) fn rust_classify_truthy_type(py: Python<'_>, t: &PyAny) -> PyResult<Option<i64>> {
    let types_mod = py.import("mypy.types")?;
    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;
    let fl_cls: &PyType = types_mod.getattr("FunctionLike")?.downcast()?;
    let union_cls: &PyType = types_mod.getattr("UnionType")?.downcast()?;
    let get_proper = types_mod.getattr("get_proper_type")?;

    let truthy = match is_truthy_type(t, instance_cls, fl_cls, union_cls, get_proper)? {
        Some(b) => b,
        None => return Ok(None),
    };
    if !truthy {
        return Ok(Some(TRUTHY_SKIP));
    }
    let is_function_like = t.is_instance(fl_cls)?;
    let is_union = t.is_instance(union_cls)?;
    let is_iterable_instance = if t.is_instance(instance_cls)? {
        t.getattr("type")
            .and_then(|info| info.getattr("fullname"))
            .and_then(|f| f.extract::<String>())
            .map(|s| s == "typing.Iterable")
            .unwrap_or(false)
    } else {
        false
    };
    Ok(Some(dispatch_truthy_tag(
        is_function_like,
        is_union,
        is_iterable_instance,
    )))
}

#[cfg(test)]
mod truthy_type_tests {
    use super::*;

    #[test]
    fn test_instance_plain_truthy() {
        assert!(instance_is_truthy(true, false, false, false));
    }

    #[test]
    fn test_instance_no_type_falsy() {
        // bool(t.type) is False (a None TypeInfo).
        assert!(!instance_is_truthy(false, false, false, false));
    }

    #[test]
    fn test_instance_bool_member_falsy() {
        assert!(!instance_is_truthy(true, true, false, false));
    }

    #[test]
    fn test_instance_len_member_falsy() {
        assert!(!instance_is_truthy(true, false, true, false));
    }

    #[test]
    fn test_instance_object_falsy() {
        assert!(!instance_is_truthy(true, false, false, true));
    }

    #[test]
    fn test_dispatch_function_like_first() {
        // A FunctionLike wins even though the union / iterable tags are
        // unreachable for it; order must mirror checker.py.
        assert_eq!(dispatch_truthy_tag(true, true, true), TRUTHY_FUNCTION);
    }

    #[test]
    fn test_dispatch_union() {
        assert_eq!(dispatch_truthy_tag(false, true, true), TRUTHY_UNION);
    }

    #[test]
    fn test_dispatch_iterable() {
        assert_eq!(dispatch_truthy_tag(false, false, true), TRUTHY_ITERABLE);
    }

    #[test]
    fn test_dispatch_other() {
        assert_eq!(dispatch_truthy_tag(false, false, false), TRUTHY_OTHER);
    }
}

// `TypeChecker.check_final` decision-head port (mypy.checker:5095).
// ---------------------------------------------------------------------------

/// The pure `final_without_value` conjunction: a final declaration at class
/// scope without a value emits the extra message only when the initializer
/// was unset in the class body, never set in `__init__`, we are not in a
/// stub file, the statement is an `AssignmentStmt` with a declared type,
/// and the active class is not a NamedTuple.
fn final_without_value(
    final_unset_in_class: bool,
    final_set_in_init: bool,
    is_stub: bool,
    is_assignment_stmt: bool,
    s_type_is_none: bool,
    is_named_tuple: bool,
) -> bool {
    final_unset_in_class
        && !final_set_in_init
        && !is_stub
        && is_assignment_stmt
        && !s_type_is_none
        && !is_named_tuple
}

/// `#[pyfunction]` entry for `TypeChecker.check_final`
/// (mypy/checker.py:5095-5196). Everything after the shim's
/// `flatten_lvalues` / `is_final_decl` computation is a pure sequence of
/// message-emission decisions, so Rust owns the whole front:
///
/// - the `final_without_value` gate over scalar Var/statement/class facts;
/// - the per-lvalue arbitration: RefExpr -> Var gate, the MRO walk over
///   `cls.mro[1:]` looking up `base.names[name]` for a final base Var
///   (emit + break), and the own `lv.node.is_final` check.
///
/// `lvalues` is the flattened lvalue list, `cls` the live active class
/// (`TypeInfo` or None), and the remaining scalars mirror the Python
/// guards. Returns `Some((without_value, msgs))` where `msgs` is the
/// ordered `cant_assign_to_final` emission list of `(name, info_is_none)`;
/// the Python shim applies the emissions and keeps the pure-Python body
/// as the fallback. `None` defers on any unreadable fact, or when the
/// `is_final_decl` preconditions would trip a Python `assert` (a non-
/// RefExpr first lvalue or a non-Var node) so the shim re-runs the
/// original body and surfaces the same assertion.
#[pyfunction]
#[pyo3(signature = (lvalues, is_final_decl, cls, is_stub, s_type_is_none, is_assignment_stmt))]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::type_complexity)]
pub(crate) fn rust_classify_check_final(
    lvalues: &PyAny,
    is_final_decl: bool,
    cls: Option<&PyAny>,
    is_stub: bool,
    s_type_is_none: bool,
    is_assignment_stmt: bool,
) -> PyResult<Option<(bool, Vec<(String, bool)>)>> {
    let py = lvalues.py();
    let lvs = match lvalues.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let var_cls = nodes_class(py, "Var")?;
    let refexpr_cls = nodes_class(py, "RefExpr")?;

    // The `final_without_value` gate (only reachable with an active class).
    let mut without_value = false;
    if is_final_decl {
        if let Some(active_class) = cls {
            // Python asserts the first lvalue is a RefExpr; defer so the shim
            // re-runs the original body and raises the same AssertionError.
            let lv0 = match lvs.get_item(0) {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            if !lv0.is_instance(refexpr_cls)? {
                return Ok(None);
            }
            let node = lv0.getattr("node")?;
            if !node.is_none() {
                if !node.is_instance(var_cls)? {
                    return Ok(None);
                }
                let final_unset_in_class = match read_bool_attr(node, "final_unset_in_class")? {
                    Some(b) => b,
                    None => return Ok(None),
                };
                let final_set_in_init = match read_bool_attr(node, "final_set_in_init")? {
                    Some(b) => b,
                    None => return Ok(None),
                };
                let is_named_tuple = match read_bool_attr(active_class, "is_named_tuple")? {
                    Some(b) => b,
                    None => return Ok(None),
                };
                without_value = final_without_value(
                    final_unset_in_class,
                    final_set_in_init,
                    is_stub,
                    is_assignment_stmt,
                    s_type_is_none,
                    is_named_tuple,
                );
            }
        }
    }

    // The per-lvalue final-assignment arbitration.
    let mut msgs: Vec<(String, bool)> = Vec::new();
    for lv in lvs.iter() {
        if !lv.is_instance(refexpr_cls)? {
            continue;
        }
        let node = lv.getattr("node")?;
        if node.is_none() || !node.is_instance(var_cls)? {
            continue;
        }
        let name: String = match node.getattr("name") {
            Ok(n) => match n.extract() {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        if let Some(active) = cls {
            // These additional checks exist to give more error messages
            // even if the final attribute was overridden with a new symbol
            // (which is itself an error); overriding a final method is
            // caught in `check_compatibility_final_super()` instead.
            let mro = match active.getattr("mro") {
                Ok(m) => m,
                Err(_) => return Ok(None),
            };
            let mro_list = match mro.downcast::<PyList>() {
                Ok(l) => l,
                Err(_) => return Ok(None),
            };
            for base in mro_list.iter().skip(1) {
                let names = match base.getattr("names") {
                    Ok(n) => n,
                    Err(_) => return Ok(None),
                };
                let sym = match names.call_method1("get", (name.as_str(),)) {
                    Ok(s) => s,
                    Err(_) => return Ok(None),
                };
                if sym.is_none() {
                    continue;
                }
                let sym_node = match sym.getattr("node") {
                    Ok(n) => n,
                    Err(_) => return Ok(None),
                };
                if !sym_node.is_instance(var_cls)? {
                    continue;
                }
                let base_is_final = match read_bool_attr(sym_node, "is_final")? {
                    Some(b) => b,
                    None => return Ok(None),
                };
                if base_is_final && !is_final_decl {
                    let info_is_none = match read_attr_is_none(sym_node, "info")? {
                        Some(b) => b,
                        None => return Ok(None),
                    };
                    msgs.push((name.clone(), info_is_none));
                    // ...but only once
                    break;
                }
            }
        }
        let own_is_final = match read_bool_attr(node, "is_final")? {
            Some(b) => b,
            None => return Ok(None),
        };
        if own_is_final && !is_final_decl {
            let info_is_none = match read_attr_is_none(node, "info")? {
                Some(b) => b,
                None => return Ok(None),
            };
            msgs.push((name, info_is_none));
        }
    }
    Ok(Some((without_value, msgs)))
}

#[cfg(test)]
mod check_final_tests {
    use super::*;

    #[test]
    fn test_without_value_all_facts_true() {
        assert!(final_without_value(true, false, false, true, false, false));
    }

    #[test]
    fn test_without_value_set_in_init() {
        assert!(!final_without_value(true, true, false, true, false, false));
    }

    #[test]
    fn test_without_value_unset_in_class() {
        assert!(!final_without_value(
            false, false, false, true, false, false
        ));
    }

    #[test]
    fn test_without_value_stub_file() {
        assert!(!final_without_value(true, false, true, true, false, false));
    }

    #[test]
    fn test_without_value_not_assignment_stmt() {
        assert!(!final_without_value(
            true, false, false, false, false, false
        ));
    }

    #[test]
    fn test_without_value_no_type_annotation() {
        assert!(!final_without_value(true, false, false, true, true, false));
    }

    #[test]
    fn test_without_value_named_tuple() {
        assert!(!final_without_value(true, false, false, true, false, true));
    }
}
