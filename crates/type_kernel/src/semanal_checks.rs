//! Native ports of `SemanticAnalyzer` check/arbitration functions.
//!
//! Ports semanal decision heads that read scalar facts and return a
//! branch tag; Python applies the side effects:
//! - `check_function_signature` (semanal.py:2072) count arbitration
//! - `check_decorated_function_is_method` (semanal.py:2256) predicate
//! - `check_fixed_args` (semanal.py:6962) arg-count + arg-kinds arbitration
//! - `should_wait_rhs` (semanal.py:4179) assignment-rvalue wait predicate
//! - `prepare_method_signature` (semanal.py:1543) method-signature dispatch

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString, PyType};

// ---------------------------------------------------------------------------
// check_function_signature count arbitration (issue #940)
// ---------------------------------------------------------------------------

/// Decision tags; must match `NATIVE_FUNC_SIG_*` in mypy/semanal.py.
pub(crate) const FUNC_SIG_OK: i64 = 0;
pub(crate) const FUNC_SIG_TOO_FEW: i64 = 1;
pub(crate) const FUNC_SIG_TOO_MANY: i64 = 2;

/// Pure decision core: compare the signature argument count against the
/// declared argument count. Kept separate from the PyO3 entry so the
/// decision table is unit-testable without a Python runtime.
fn classify_function_signature(sig_arg_types_len: usize, arguments_len: usize) -> i64 {
    if sig_arg_types_len < arguments_len {
        FUNC_SIG_TOO_FEW
    } else if sig_arg_types_len > arguments_len {
        FUNC_SIG_TOO_MANY
    } else {
        FUNC_SIG_OK
    }
}

/// `#[pyfunction]` entry for
/// `SemanticAnalyzer.check_function_signature` (semanal.py:2072).
///
/// Reads the length of the signature's `arg_types` and the length of the
/// function's `arguments`, returning a branch tag. Always decidable;
/// never returns `None`.
#[pyfunction]
#[pyo3(signature = (sig_arg_types_len, arguments_len))]
pub(crate) fn rust_classify_function_signature(
    sig_arg_types_len: usize,
    arguments_len: usize,
) -> PyResult<i64> {
    Ok(classify_function_signature(
        sig_arg_types_len,
        arguments_len,
    ))
}

// ---------------------------------------------------------------------------
// check_decorated_function_is_method predicate (issue #941)
// ---------------------------------------------------------------------------

/// The pure decision over resolved facts, kept separate from the PyO3
/// entry so the algebra is unit-testable without a Python runtime.
///
/// `self_type_is_none` mirrors `not self.type`; `is_func_scope` mirrors
/// `self.is_func_scope()`. Returns `true` when the function is a method
/// (inside a class body, not nested in a function scope) and `false`
/// when the decorator is used in a non-method context.
fn classify_decorated_function_is_method(self_type_is_none: bool, is_func_scope: bool) -> bool {
    // method iff self.type is not None and not in a function scope.
    !self_type_is_none && !is_func_scope
}

/// `#[pyfunction]` entry for
/// `SemanticAnalyzer.check_decorated_function_is_method`
/// (mypy/semanal.py:2256-2258).
///
/// `semanal` is the live `SemanticAnalyzer` (`self`). Rust reads
/// `self.type` (None check) and calls `self.is_func_scope()` as a bound
/// method. Returns `Some(true)` when the function is a method (no-op),
/// `Some(false)` when it is a non-method context (Python emits the
/// fail), or `None` to defer when the live state cannot be read.
#[pyfunction]
pub(crate) fn rust_check_decorated_function_is_method(semanal: &PyAny) -> PyResult<Option<bool>> {
    let self_type = match semanal.getattr("type") {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let self_type_is_none = self_type.is_none();
    if self_type_is_none {
        // Outside a class body: not a method.
        return Ok(Some(false));
    }
    let is_func_scope = match semanal.call_method0("is_func_scope") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    Ok(Some(classify_decorated_function_is_method(
        self_type_is_none,
        is_func_scope,
    )))
}

// ---------------------------------------------------------------------------
// check_fixed_args arbitration (issue #935)
// ---------------------------------------------------------------------------

/// Decision tags for `check_fixed_args`; must match
/// `NATIVE_FIXED_ARGS_*` in mypy/semanal.py.
pub(crate) const FIXED_ARGS_OK: i64 = 0;
pub(crate) const FIXED_ARGS_WRONG_COUNT: i64 = 1;
pub(crate) const FIXED_ARGS_WRONG_KINDS: i64 = 2;

/// Pure decision core of `SemanticAnalyzer.check_fixed_args`
/// (semanal.py:6962-6976). Checks two gaps in order:
/// 1. `len(expr.args) != numargs` -> wrong count
/// 2. `expr.arg_kinds != [ARG_POS]*numargs` -> wrong kinds
///
/// `ARG_POS == 0` (mypy.nodes.ArgKind.ARG_POS).
fn classify_fixed_args(args_len: usize, arg_kinds: &[i64], numargs: usize) -> i64 {
    if args_len != numargs {
        return FIXED_ARGS_WRONG_COUNT;
    }
    if arg_kinds.len() != numargs || !arg_kinds.iter().all(|&k| k == 0) {
        return FIXED_ARGS_WRONG_KINDS;
    }
    FIXED_ARGS_OK
}

/// `#[pyfunction]` entry; the shim passes `len(expr.args)`, the integer
/// arg-kinds list, and `numargs`. Returns `Some(tag)` always (never
/// defers). Python applies the `self.fail` side effect per the tag.
#[pyfunction]
pub(crate) fn rust_classify_fixed_args(
    args_len: usize,
    arg_kinds: Vec<i64>,
    numargs: usize,
) -> PyResult<Option<i64>> {
    Ok(Some(classify_fixed_args(args_len, &arg_kinds, numargs)))
}

// ---------------------------------------------------------------------------
// should_wait_rhs predicate (issue #1008)
// ---------------------------------------------------------------------------

/// Step tags for the rvalue dispatch; must match `NATIVE_WAIT_RHS_*` in
/// mypy/semanal.py.
pub(crate) const WAIT_RHS_FALSE: u8 = 0;
pub(crate) const WAIT_RHS_LOOKUP_NAME: u8 = 1;
pub(crate) const WAIT_RHS_LOOKUP_QUALIFIED: u8 = 2;
pub(crate) const WAIT_RHS_DESCEND: u8 = 3;

/// Node-kind tags for the rvalue dispatch.
const KIND_NAME: u8 = 0;
const KIND_MEMBER: u8 = 1;
const KIND_INDEX: u8 = 2;
const KIND_CALL: u8 = 3;
const KIND_OTHER: u8 = 4;

/// Bound on the IndexExpr.base / CallExpr.callee descent. The Python
/// recursion is unbounded, but every descent step requires the child to be
/// a `RefExpr`, and the next iteration dispatches on the child's kind, so
/// real chains are a few links long. Past the bound the seam defers and
/// the pure-Python body runs unchanged.
const WAIT_RHS_MAX_DEPTH: usize = 512;

/// Pure decision core of the rvalue step dispatch, kept separate from the
/// PyO3 entry so the table is unit-testable without a Python runtime.
///
/// `kind` is one of the KIND_* tags; `has_fullname` mirrors
/// `get_member_expr_fullname(rv) is not None` for MemberExpr; `child_is_ref`
/// mirrors `isinstance(child, RefExpr)` for IndexExpr.base / CallExpr.callee.
fn classify_should_wait_rhs_step(kind: u8, has_fullname: bool, child_is_ref: bool) -> u8 {
    match kind {
        KIND_NAME => WAIT_RHS_LOOKUP_NAME,
        KIND_MEMBER => {
            if has_fullname {
                WAIT_RHS_LOOKUP_QUALIFIED
            } else {
                WAIT_RHS_FALSE
            }
        }
        KIND_INDEX | KIND_CALL => {
            if child_is_ref {
                WAIT_RHS_DESCEND
            } else {
                WAIT_RHS_FALSE
            }
        }
        _ => WAIT_RHS_FALSE,
    }
}

/// Pure decision core of the lookup-result classification
/// (`n and isinstance(n.node, PlaceholderNode) and not
/// n.node.becomes_typeinfo`).
fn placeholder_waits(is_placeholder: bool, becomes_typeinfo: bool) -> bool {
    is_placeholder && !becomes_typeinfo
}

/// Pure core of the `f"{initial}.{expr.name}"` tail of
/// `get_member_expr_fullname` (nodes.py:5475). `initial` is `None` when the
/// chain cannot be represented; the join then yields `None` too.
fn member_fullname_join(initial: Option<&str>, member: &str) -> Option<String> {
    initial.map(|i| format!("{}.{}", i, member))
}

/// Port of `get_member_expr_fullname` (nodes.py:5475-5486) as a bounded
/// recursive walk over `expr.expr` chains of NameExpr/MemberExpr.
///
/// Returns `Ok(None)` to defer when an attribute is unreadable (the Python
/// caller falls back to the pure-Python body), `Some(None)` when the chain
/// is not representable as `a.b.c` (Python returns `None`), and
/// `Some(Some(name))` with the dotted chain.
fn member_expr_fullname_rust(
    expr: &PyAny,
    name_cls: &PyType,
    member_cls: &PyType,
    depth: usize,
) -> PyResult<Option<Option<String>>> {
    if depth > WAIT_RHS_MAX_DEPTH {
        return Ok(None);
    }
    let inner = match expr.getattr("expr") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let initial: Option<String> = if inner.is_instance(name_cls)? {
        match inner.getattr("name") {
            Ok(v) => match v.downcast::<PyString>() {
                Ok(s) => Some(s.to_str()?.to_string()),
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        }
    } else if inner.is_instance(member_cls)? {
        match member_expr_fullname_rust(inner, name_cls, member_cls, depth + 1)? {
            Some(v) => v,
            None => return Ok(None),
        }
    } else {
        None
    };
    if initial.is_none() {
        return Ok(Some(None));
    }
    let name = match expr.getattr("name") {
        Ok(v) => match v.downcast::<PyString>() {
            Ok(s) => s.to_str()?.to_string(),
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    Ok(Some(member_fullname_join(initial.as_deref(), &name)))
}

/// Classify the lookup result `n` (a `SymbolTableNode | None`):
/// `Some(true)` when the symbol is a `PlaceholderNode` that does not
/// become typeinfo (wait), `Some(false)` otherwise, `None` to defer when
/// an attribute is unreadable. Truthiness of `n` is mirrored via `is_true`
/// so a falsy symbol node behaves exactly like Python's `if n and ...`.
fn should_wait_for_symbol(n: &PyAny, placeholder_cls: &PyType) -> PyResult<Option<bool>> {
    let truthy = match n.is_true() {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    if !truthy {
        return Ok(Some(false));
    }
    let node = match n.getattr("node") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !node.is_instance(placeholder_cls)? {
        return Ok(Some(false));
    }
    let becomes_typeinfo = match node.getattr("becomes_typeinfo") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    Ok(Some(placeholder_waits(true, becomes_typeinfo)))
}

/// `#[pyfunction]` entry for `SemanticAnalyzer.should_wait_rhs`
/// (semanal.py:4179-4206).
///
/// `semanal` is the live `SemanticAnalyzer` (`self`) and `rv` the rvalue
/// expression. Rust reads `self.final_iteration`, dispatches on the rvalue
/// node kind (NameExpr / MemberExpr / IndexExpr / CallExpr / other) with a
/// bounded descent through `IndexExpr.base` and `CallExpr.callee`, and
/// classifies the lookup results. The symbol lookups ride the real
/// `self.lookup` / `self.lookup_qualified` methods (called via PyO3), so
/// all their side effects (error emission, module_refs recording) stay in
/// Python. Returns `Some(bool)` for every decided case and `None` to defer
/// when a fact is unreadable or the descent bound is exceeded.
#[pyfunction]
pub(crate) fn rust_should_wait_rhs(semanal: &PyAny, rv: &PyAny) -> PyResult<Option<bool>> {
    let nodes_mod = semanal.py().import("mypy.nodes")?;
    let name_cls: &PyType = nodes_mod.getattr("NameExpr")?.downcast()?;
    let member_cls: &PyType = nodes_mod.getattr("MemberExpr")?.downcast()?;
    let index_cls: &PyType = nodes_mod.getattr("IndexExpr")?.downcast()?;
    let call_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let ref_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    let placeholder_cls: &PyType = nodes_mod.getattr("PlaceholderNode")?.downcast()?;

    let mut node = rv;
    for depth in 0..=WAIT_RHS_MAX_DEPTH {
        // The Python port re-enters should_wait_rhs per descent level,
        // which re-reads self.final_iteration; mirror that here.
        let final_iteration = match semanal.getattr("final_iteration") {
            Ok(v) => match v.is_true() {
                Ok(b) => b,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        if final_iteration {
            // No chance, nothing has changed.
            return Ok(Some(false));
        }

        let kind = if node.is_instance(name_cls)? {
            KIND_NAME
        } else if node.is_instance(member_cls)? {
            KIND_MEMBER
        } else if node.is_instance(index_cls)? {
            KIND_INDEX
        } else if node.is_instance(call_cls)? {
            KIND_CALL
        } else {
            KIND_OTHER
        };

        // Gather the per-kind scalar facts for the dispatch table.
        let mut has_fullname = false;
        let mut fname: Option<String> = None;
        let mut child_is_ref = false;
        let mut child: Option<&PyAny> = None;
        match kind {
            KIND_MEMBER => match member_expr_fullname_rust(node, name_cls, member_cls, depth)? {
                Some(v) => {
                    has_fullname = v.is_some();
                    fname = v;
                }
                None => return Ok(None),
            },
            KIND_INDEX => {
                let base = match node.getattr("base") {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                child_is_ref = base.is_instance(ref_cls)?;
                child = Some(base);
            }
            KIND_CALL => {
                let callee = match node.getattr("callee") {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                child_is_ref = callee.is_instance(ref_cls)?;
                child = Some(callee);
            }
            _ => {}
        }

        match classify_should_wait_rhs_step(kind, has_fullname, child_is_ref) {
            WAIT_RHS_FALSE => return Ok(Some(false)),
            WAIT_RHS_LOOKUP_NAME => {
                let name = match node.getattr("name") {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                let n = match semanal.call_method1("lookup", (name, node)) {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                return should_wait_for_symbol(n, placeholder_cls);
            }
            WAIT_RHS_LOOKUP_QUALIFIED => {
                let n = match semanal.call_method1(
                    "lookup_qualified",
                    (fname.as_deref().unwrap_or(""), node, true),
                ) {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                return should_wait_for_symbol(n, placeholder_cls);
            }
            WAIT_RHS_DESCEND => {
                node = match child {
                    Some(c) => c,
                    // Unreachable: DESCEND implies a gathered child.
                    None => return Ok(None),
                };
            }
            _ => return Ok(Some(false)),
        }
    }
    // Descent bound exceeded: defer to the pure-Python recursion.
    Ok(None)
}

// ---------------------------------------------------------------------------
// prepare_method_signature dispatch head (issue #1036)
// ---------------------------------------------------------------------------

/// Terminal branch tags for the method-signature dispatch; must match
/// `_NATIVE_METH_SIG_*` in mypy/semanal.py. NEW_STATIC and CLASS_SPECIAL
/// (the two unconditional write arms of the branch chain) are represented
/// as METH_SIG_OK combined with the `set_is_static` / `set_is_class` write
/// flags in the returned tuple.
pub(crate) const METH_SIG_ANY_SELF_REPLACE: i64 = 0;
pub(crate) const METH_SIG_ANY_SELF_TRIVIAL: i64 = 1;
pub(crate) const METH_SIG_REDUNDANT_SELF: i64 = 2;
pub(crate) const METH_SIG_EXPLICIT_SELF_CONFLICT: i64 = 3;
pub(crate) const METH_SIG_STATIC_SELF_FAIL: i64 = 4;
pub(crate) const METH_SIG_OK: i64 = 5;

/// Sentinel kinds for the unanalyzed first-argument fact (the inner
/// `elif has_self_type and isinstance(func.unanalyzed_type, CallableType)`
/// chain). `0` = the elif did not apply (unanalyzed not callable or not
/// gathered), `1` = unanalyzed arg0 IS AnyType (inner guard false), `2` =
/// unanalyzed arg0 is not AnyType (the expected-self checks run).
const UNANALYZED_NOT_CALLABLE: i64 = 0;
const UNANALYZED_ARG0_IS_ANY: i64 = 1;
const UNANALYZED_ARG0_NOT_ANY: i64 = 2;

/// Pure decision core of `SemanticAnalyzer.prepare_method_signature`
/// (semanal.py:1543-1582), kept separate from the PyO3 entry so the branch
/// chain is unit-testable without a Python runtime.
///
/// `self_type_is_any` mirrors `isinstance(get_proper_type(arg_types[0]),
/// AnyType)`; `None` means the wire blob was undecodable (defer).
/// `expected_self` is the shim-precomputed `is_expected_self_type` result
/// (it needs `lookup_qualified`, so Rust cannot compute it); `None` there
/// defers when the branch needs it.
///
/// Returns `(set_is_static, set_is_class, tag)`: the two write flags mirror
/// the unconditional arms (func.is_static at semanal.py:1548, func.is_class
/// at :1551), `tag` the terminal branch. The `func.is_class` read at :1561
/// is decidable without the write because Python's shim applies the
/// `set_is_class` write before its tag handler re-reads `func.is_class`.
#[allow(clippy::too_many_arguments)]
fn classify_method_signature(
    name: &str,
    has_self_or_cls: bool,
    has_arguments: bool,
    functype_is_callable: bool,
    self_type_is_any: Option<bool>,
    has_self_type: bool,
    unanalyzed_kind: i64,
    expected_self: Option<bool>,
) -> Option<(bool, bool, i64)> {
    let set_is_static = name == "__new__";
    let set_is_class =
        has_self_or_cls && (name == "__init_subclass__" || name == "__class_getitem__");
    let tag = if !has_self_or_cls {
        if has_self_type {
            METH_SIG_STATIC_SELF_FAIL
        } else {
            METH_SIG_OK
        }
    } else if has_arguments && functype_is_callable {
        let self_is_any = self_type_is_any?;
        if self_is_any {
            if has_self_type {
                METH_SIG_ANY_SELF_REPLACE
            } else {
                METH_SIG_ANY_SELF_TRIVIAL
            }
        } else if has_self_type {
            match unanalyzed_kind {
                UNANALYZED_ARG0_NOT_ANY => match expected_self {
                    Some(true) => METH_SIG_REDUNDANT_SELF,
                    Some(false) => METH_SIG_EXPLICIT_SELF_CONFLICT,
                    // Shim could not compute is_expected_self_type: defer.
                    None => return None,
                },
                UNANALYZED_NOT_CALLABLE | UNANALYZED_ARG0_IS_ANY => METH_SIG_OK,
                _ => METH_SIG_OK,
            }
        } else {
            METH_SIG_OK
        }
    } else if has_self_type {
        METH_SIG_STATIC_SELF_FAIL
    } else {
        METH_SIG_OK
    };
    Some((set_is_static, set_is_class, tag))
}

/// `#[pyfunction]` entry for `SemanticAnalyzer.prepare_method_signature`
/// (semanal.py:1543-1582).
///
/// `func` is the live `FuncDef`; Rust reads `name`,
/// `has_self_or_cls_argument`, `arguments` (non-empty), and the
/// `CallableType` isinstance of `func.type` via PyO3. The shim passes the
/// analyzed first-argument proper type serialized once to the wire format
/// (the AnyType check), the unanalyzed-arg kind, the precomputed
/// `is_expected_self_type` bool, and `has_self_type`. Returns
/// `Some((set_is_static, set_is_class, tag))` for every decided case and
/// `None` to defer when a fact is unreadable or undecodable; the Python
/// shim applies all writes and error emissions.
#[pyfunction]
#[pyo3(signature = (func, self_type_wire, unanalyzed_kind, expected_self, has_self_type))]
pub(crate) fn rust_classify_method_signature(
    func: &PyAny,
    self_type_wire: Option<&[u8]>,
    unanalyzed_kind: i64,
    expected_self: Option<bool>,
    has_self_type: bool,
) -> Option<(bool, bool, i64)> {
    let name = match func.getattr("name") {
        Ok(v) => match v.downcast::<PyString>() {
            Ok(s) => match s.to_str() {
                Ok(s) => s,
                Err(_) => return None,
            },
            Err(_) => return None,
        },
        Err(_) => return None,
    };
    let has_self_or_cls = match func.getattr("has_self_or_cls_argument") {
        Ok(v) => match v.is_true() {
            Ok(b) => b,
            Err(_) => return None,
        },
        Err(_) => return None,
    };
    let has_arguments = match func.getattr("arguments") {
        Ok(v) => match v.len() {
            Ok(n) => n > 0,
            Err(_) => return None,
        },
        Err(_) => return None,
    };
    let functype_is_callable = match func.getattr("type") {
        Ok(t) => {
            let types_mod = match func.py().import("mypy.types") {
                Ok(m) => m,
                Err(_) => return None,
            };
            let callable_cls: &PyType = match types_mod.getattr("CallableType") {
                Ok(c) => match c.downcast() {
                    Ok(c) => c,
                    Err(_) => return None,
                },
                Err(_) => return None,
            };
            match t.is_instance(callable_cls) {
                Ok(b) => b,
                Err(_) => return None,
            }
        }
        Err(_) => return None,
    };
    let self_type_is_any = match self_type_wire {
        Some(bytes) => {
            let t = crate::checkmember::decode_type(bytes)?;
            Some(matches!(t, crate::wire::Type::AnyType { .. }))
        }
        None => None,
    };
    classify_method_signature(
        name,
        has_self_or_cls,
        has_arguments,
        functype_is_callable,
        self_type_is_any,
        has_self_type,
        unanalyzed_kind,
        expected_self,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(sig_len: usize, args_len: usize) -> i64 {
        classify_function_signature(sig_len, args_len)
    }

    #[test]
    fn ok_when_equal() {
        assert_eq!(classify(0, 0), FUNC_SIG_OK);
        assert_eq!(classify(3, 3), FUNC_SIG_OK);
    }

    #[test]
    fn too_few_when_sig_shorter() {
        assert_eq!(classify(0, 1), FUNC_SIG_TOO_FEW);
        assert_eq!(classify(2, 5), FUNC_SIG_TOO_FEW);
    }

    #[test]
    fn too_many_when_sig_longer() {
        assert_eq!(classify(1, 0), FUNC_SIG_TOO_MANY);
        assert_eq!(classify(5, 2), FUNC_SIG_TOO_MANY);
    }

    fn classify_method(self_type_is_none: bool, is_func_scope: bool) -> bool {
        classify_decorated_function_is_method(self_type_is_none, is_func_scope)
    }

    #[test]
    fn test_method_in_class_body() {
        // self.type set, not in func scope: method.
        assert!(classify_method(false, false));
    }

    #[test]
    fn test_not_method_outside_class() {
        // self.type is None: not a method.
        assert!(!classify_method(true, false));
    }

    #[test]
    fn test_not_method_in_func_scope() {
        // Inside a function (nested): not a method even in a class.
        assert!(!classify_method(false, true));
    }

    #[test]
    fn test_not_method_outside_class_and_in_func() {
        assert!(!classify_method(true, true));
    }

    #[test]
    fn test_fixed_args_ok() {
        assert_eq!(classify_fixed_args(2, &[0, 0], 2), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_wrong_count() {
        assert_eq!(classify_fixed_args(1, &[0], 2), FIXED_ARGS_WRONG_COUNT);
        assert_eq!(
            classify_fixed_args(3, &[0, 0, 0], 2),
            FIXED_ARGS_WRONG_COUNT
        );
    }

    #[test]
    fn test_fixed_args_wrong_kinds() {
        assert_eq!(classify_fixed_args(2, &[0, 3], 2), FIXED_ARGS_WRONG_KINDS);
        assert_eq!(classify_fixed_args(2, &[3, 0], 2), FIXED_ARGS_WRONG_KINDS);
        assert_eq!(classify_fixed_args(2, &[3, 3], 2), FIXED_ARGS_WRONG_KINDS);
    }

    #[test]
    fn test_fixed_args_zero_args_ok() {
        assert_eq!(classify_fixed_args(0, &[], 0), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_one_arg_ok() {
        assert_eq!(classify_fixed_args(1, &[0], 1), FIXED_ARGS_OK);
    }

    #[test]
    fn test_fixed_args_arg_kinds_length_mismatch() {
        assert_eq!(classify_fixed_args(2, &[0], 2), FIXED_ARGS_WRONG_KINDS);
    }

    // should_wait_rhs (issue #1008)

    #[test]
    fn test_wait_rhs_step_name_looks_up() {
        assert_eq!(
            classify_should_wait_rhs_step(KIND_NAME, false, false),
            WAIT_RHS_LOOKUP_NAME
        );
    }

    #[test]
    fn test_wait_rhs_step_member_with_fullname_looks_up_qualified() {
        assert_eq!(
            classify_should_wait_rhs_step(KIND_MEMBER, true, false),
            WAIT_RHS_LOOKUP_QUALIFIED
        );
    }

    #[test]
    fn test_wait_rhs_step_member_without_fullname_is_false() {
        assert_eq!(
            classify_should_wait_rhs_step(KIND_MEMBER, false, false),
            WAIT_RHS_FALSE
        );
    }

    #[test]
    fn test_wait_rhs_step_index_and_call_descend_only_on_ref_child() {
        assert_eq!(
            classify_should_wait_rhs_step(KIND_INDEX, false, true),
            WAIT_RHS_DESCEND
        );
        assert_eq!(
            classify_should_wait_rhs_step(KIND_INDEX, false, false),
            WAIT_RHS_FALSE
        );
        assert_eq!(
            classify_should_wait_rhs_step(KIND_CALL, false, true),
            WAIT_RHS_DESCEND
        );
        assert_eq!(
            classify_should_wait_rhs_step(KIND_CALL, false, false),
            WAIT_RHS_FALSE
        );
    }

    #[test]
    fn test_wait_rhs_step_other_kind_is_false() {
        assert_eq!(
            classify_should_wait_rhs_step(KIND_OTHER, false, false),
            WAIT_RHS_FALSE
        );
        assert_eq!(
            classify_should_wait_rhs_step(9, false, false),
            WAIT_RHS_FALSE
        );
    }

    #[test]
    fn test_wait_rhs_placeholder_waits_unless_typeinfo() {
        // PlaceholderNode that never becomes typeinfo: wait.
        assert!(placeholder_waits(true, false));
        // PlaceholderNode that becomes typeinfo: no wait.
        assert!(!placeholder_waits(true, true));
        // Not a placeholder: no wait.
        assert!(!placeholder_waits(false, false));
        assert!(!placeholder_waits(false, true));
    }

    #[test]
    fn test_wait_rhs_member_fullname_join() {
        assert_eq!(
            member_fullname_join(Some("a"), "b"),
            Some("a.b".to_string())
        );
        assert_eq!(member_fullname_join(None, "b"), None);
        assert_eq!(member_fullname_join(None, ""), None);
    }

    // prepare_method_signature (issue #1036)

    #[test]
    fn test_method_sig_new_static_no_self() {
        // __new__ without a self-or-cls argument: is_static write, OK tail.
        assert_eq!(
            classify_method_signature("__new__", false, false, false, None, false, 0, None),
            Some((true, false, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_new_with_any_self_trivial() {
        // __new__ with an Any self and no Self type: is_static write +
        // trivial-self replace.
        assert_eq!(
            classify_method_signature("__new__", true, true, true, Some(true), false, 0, None),
            Some((true, false, METH_SIG_ANY_SELF_TRIVIAL))
        );
    }

    #[test]
    fn test_method_sig_class_special_write() {
        assert_eq!(
            classify_method_signature(
                "__init_subclass__",
                true,
                true,
                true,
                Some(false),
                false,
                0,
                None
            ),
            Some((false, true, METH_SIG_OK))
        );
        assert_eq!(
            classify_method_signature(
                "__class_getitem__",
                true,
                true,
                true,
                Some(false),
                false,
                0,
                None
            ),
            Some((false, true, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_class_special_no_self_arg() {
        // The is_class write is gated on has_self_or_cls_argument.
        assert_eq!(
            classify_method_signature(
                "__init_subclass__",
                false,
                false,
                false,
                None,
                false,
                0,
                None
            ),
            Some((false, false, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_static_self_fail() {
        // No self-or-cls argument but Self type used.
        assert_eq!(
            classify_method_signature("m", false, false, false, None, true, 0, None),
            Some((false, false, METH_SIG_STATIC_SELF_FAIL))
        );
        // Self-or-cls argument but no arguments / non-callable type.
        assert_eq!(
            classify_method_signature("m", true, false, false, None, true, 0, None),
            Some((false, false, METH_SIG_STATIC_SELF_FAIL))
        );
        assert_eq!(
            classify_method_signature("m", true, true, false, None, true, 0, None),
            Some((false, false, METH_SIG_STATIC_SELF_FAIL))
        );
    }

    #[test]
    fn test_method_sig_any_self_replace() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(true), true, 0, None),
            Some((false, false, METH_SIG_ANY_SELF_REPLACE))
        );
    }

    #[test]
    fn test_method_sig_any_self_trivial() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(true), false, 0, None),
            Some((false, false, METH_SIG_ANY_SELF_TRIVIAL))
        );
    }

    #[test]
    fn test_method_sig_redundant_self() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), true, 2, Some(true)),
            Some((false, false, METH_SIG_REDUNDANT_SELF))
        );
    }

    #[test]
    fn test_method_sig_explicit_self_conflict() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), true, 2, Some(false)),
            Some((false, false, METH_SIG_EXPLICIT_SELF_CONFLICT))
        );
    }

    #[test]
    fn test_method_sig_ok_unanalyzed_not_callable() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), true, 0, None),
            Some((false, false, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_ok_unanalyzed_arg0_any() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), true, 1, None),
            Some((false, false, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_ok_plain_method_no_self_type() {
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), false, 0, None),
            Some((false, false, METH_SIG_OK))
        );
    }

    #[test]
    fn test_method_sig_defers_on_expected_none() {
        // The shim could not compute is_expected_self_type: defer.
        assert_eq!(
            classify_method_signature("m", true, true, true, Some(false), true, 2, None),
            None
        );
    }

    #[test]
    fn test_method_sig_defers_on_undecodable_self_wire() {
        // Body applies but the self-type wire blob is missing: defer
        // rather than misclassify the AnyType check.
        assert_eq!(
            classify_method_signature("m", true, true, true, None, false, 0, None),
            None
        );
    }

    #[test]
    fn test_method_sig_new_with_self_type_replace() {
        // __new__ with Any self and a Self type: is_static write + replace.
        assert_eq!(
            classify_method_signature("__new__", true, true, true, Some(true), true, 0, None),
            Some((true, false, METH_SIG_ANY_SELF_REPLACE))
        );
    }
}
