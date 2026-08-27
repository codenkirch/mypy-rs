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
use pyo3::types::{PyAny, PyList, PyType};
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
