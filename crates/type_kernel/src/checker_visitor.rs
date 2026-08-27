//! Native port of pure helper functions from `mypy/checker.py` used by the
//! complex statement visitors (Issue #208).
//!
//! Like `semanal_visitor` (Issue #209), these operate on live Python AST
//! nodes via PyO3 instead of the wire format: they are shallow structural
//! helpers on nodes, not type-walking visitors, so serializing the nodes
//! would cost more than it saves. Each mirrors the Python implementation
//! and returns a conservative `false` / `None` where structural invariants
//! do not hold, so Python falls back gracefully (the strangler-fig per-call
//! gate).
//!
//! Ported functions:
//! - literal checks: `is_true_literal`, `is_false_literal`,
//!   `is_literal_none`, `is_literal_not_implemented` — used by
//!   `visit_assert_stmt`, `visit_assignment_expr`, and the binder.
//! - decorator checks: `is_static`, `is_property`,
//!   `is_settable_property`, `is_custom_settable_property` — used by
//!   `visit_decorator` / `_visit_overloaded_func_def` /
//!   `visit_class_def`.
//! - `can_have_shared_disjoint_base` (from `mypy/typeops.py`) — used by
//!   `visit_class_def` and the TypedDict disjoint-base check.
//!
//! `is_private` and `is_async_def` are already ported elsewhere
//! (`checkexpr_functions`), so they are not duplicated here.

use pyo3::exceptions::PyAssertionError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PyString, PyType};

fn class_from_module<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<&'py PyType> {
    py.import(module)?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// Fetch a class from `mypy.nodes`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    class_from_module(py, "mypy.nodes", name)
}

/// Fetch a class from `mypy.types`.
fn types_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    class_from_module(py, "mypy.types", name)
}

// ---------------------------------------------------------------------------
// refers_to_fullname (single fullname, local helper)
// ---------------------------------------------------------------------------

/// `mypy.semanal.refers_to_fullname` with a single fullname — is `node` a
/// name or member expression with the given full name?
///
/// Mirrors semanal.py:8308-8319. Returns `false` when the node is not a
/// `RefExpr` (the Python function also returns `False` unconditionally).
/// For the `TypeAlias` indirection, returns `false` if any attribute access
/// fails (conservative: Python would never raise here).
fn refers_to_fullname(py: Python<'_>, node: &PyAny, fullname: &str) -> PyResult<bool> {
    let ref_expr_cls = nodes_class(py, "RefExpr")?;
    if !node.is_instance(ref_expr_cls)? {
        return Ok(false);
    }

    let node_name = node.getattr("fullname")?;
    if let Ok(s) = node_name.downcast::<PyString>() {
        if s.to_str()? == fullname {
            return Ok(true);
        }
    }

    // Check if node.node is a TypeAlias (and not python_3_12_type_alias).
    let node_attr = node.getattr("node")?;
    if node_attr.is_none() {
        return Ok(false);
    }
    let type_alias_cls = nodes_class(py, "TypeAlias")?;
    if !node_attr.is_instance(type_alias_cls)? {
        return Ok(false);
    }
    let python_3_12 = node_attr.getattr("python_3_12_type_alias")?;
    if python_3_12.is_true()? {
        return Ok(false);
    }

    // Recurse: is_named_instance(node.node.target, (fullname,)).
    let target = node_attr.getattr("target")?;
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let proper = get_proper_type.call1((target,))?;

    let instance_cls = types_class(py, "Instance")?;
    if !proper.is_instance(instance_cls)? {
        return Ok(false);
    }
    let typ = proper.getattr("type")?;
    let typ_fullname = typ.getattr("fullname")?;
    let typ_fullname_str: &str = typ_fullname.downcast::<PyString>()?.to_str()?;
    Ok(typ_fullname_str == fullname)
}

// ---------------------------------------------------------------------------
// literal checks
// ---------------------------------------------------------------------------

/// `mypy.checker.is_true_literal` — is this expression the `True`
/// literal/keyword?
///
/// Mirrors checker.py:9002-9004. A `NameExpr` referring to `builtins.True`
/// (possibly through a `TypeAlias` indirection) or any `IntExpr` with a
/// non-zero value.
#[pyfunction]
pub(crate) fn rust_is_true_literal(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    if refers_to_fullname(py, node, "builtins.True")? {
        return Ok(true);
    }
    let int_expr_cls = nodes_class(py, "IntExpr")?;
    if node.is_instance(int_expr_cls)? {
        let value: i64 = node.getattr("value")?.extract()?;
        return Ok(value != 0);
    }
    Ok(false)
}

/// `mypy.checker.is_false_literal` — is this expression the `False`
/// literal/keyword?
///
/// Mirrors checker.py:9007-9009.
#[pyfunction]
pub(crate) fn rust_is_false_literal(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    if refers_to_fullname(py, node, "builtins.False")? {
        return Ok(true);
    }
    let int_expr_cls = nodes_class(py, "IntExpr")?;
    if node.is_instance(int_expr_cls)? {
        let value: i64 = node.getattr("value")?.extract()?;
        return Ok(value == 0);
    }
    Ok(false)
}

/// `mypy.checker.is_literal_none` — is this expression the `None`
/// literal/keyword?
///
/// Mirrors checker.py:9012-9014. Only a `NameExpr` whose `fullname` is
/// exactly `builtins.None` counts; no alias indirection here.
#[pyfunction]
pub(crate) fn rust_is_literal_none(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    let name_expr_cls = nodes_class(py, "NameExpr")?;
    if !node.is_instance(name_expr_cls)? {
        return Ok(false);
    }
    let fullname = node.getattr("fullname")?;
    if fullname.is_none() {
        return Ok(false);
    }
    let s: &str = fullname.downcast::<PyString>()?.to_str()?;
    Ok(s == "builtins.None")
}

/// `mypy.checker.is_literal_not_implemented` — is this expression the
/// `NotImplemented` literal/keyword?
///
/// Mirrors checker.py:9017-9018. Accepts `None` (a `return` without a
/// value), which is not the literal, matching Python.
#[pyfunction]
pub(crate) fn rust_is_literal_not_implemented(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    if node.is_none() {
        return Ok(false);
    }
    let name_expr_cls = nodes_class(py, "NameExpr")?;
    if !node.is_instance(name_expr_cls)? {
        return Ok(false);
    }
    let fullname = node.getattr("fullname")?;
    if fullname.is_none() {
        return Ok(false);
    }
    let s: &str = fullname.downcast::<PyString>()?.to_str()?;
    Ok(s == "builtins.NotImplemented")
}

/// `mypy.checker._is_empty_generator_function` — is the body the two-statement
/// `return; yield` shape that promotes a plain function to a generator?
///
/// Mirrors checker.py:9419-9428. Walks live nodes: `func.body.body` must be
/// length 2, first a `ReturnStmt` whose expr is None or the `None` literal,
/// second an `ExpressionStmt` wrapping a `YieldExpr` whose expr is None or
/// the `None` literal. Any other shape returns False.
#[pyfunction]
pub(crate) fn rust_is_empty_generator_function(py: Python<'_>, func: &PyAny) -> PyResult<bool> {
    let block = func.getattr("body")?;
    let body_list = block.getattr("body")?.downcast::<PyList>()?;
    if body_list.len() != 2 {
        return Ok(false);
    }
    let ret_stmt = body_list.get_item(0)?;
    let return_cls = nodes_class(py, "ReturnStmt")?;
    if !ret_stmt.is_instance(return_cls)? {
        return Ok(false);
    }
    let ret_expr = ret_stmt.getattr("expr")?;
    if !ret_expr.is_none() && !rust_is_literal_none(py, ret_expr)? {
        return Ok(false);
    }
    let expr_stmt = body_list.get_item(1)?;
    let expr_stmt_cls = nodes_class(py, "ExpressionStmt")?;
    if !expr_stmt.is_instance(expr_stmt_cls)? {
        return Ok(false);
    }
    let yield_expr = expr_stmt.getattr("expr")?;
    let yield_cls = nodes_class(py, "YieldExpr")?;
    if !yield_expr.is_instance(yield_cls)? {
        return Ok(false);
    }
    let yield_inner = yield_expr.getattr("expr")?;
    Ok(yield_inner.is_none() || rust_is_literal_none(py, yield_inner)?)
}

// ---------------------------------------------------------------------------
// decorator checks
// ---------------------------------------------------------------------------

/// `mypy.checker.is_static` — is `func` a static method?
///
/// Mirrors checker.py:9858-9863. A `Decorator` unwraps to its `FuncDef`; a
/// `FuncBase` (e.g. `FuncDef`) exposes `is_static` directly. Any other node
/// type raises `AssertionError`, exactly like Python's `assert False`, and
/// the Python caller falls back to the pure-Python implementation.
#[pyfunction]
pub(crate) fn rust_is_static(py: Python<'_>, func: &PyAny) -> PyResult<bool> {
    let decorator_cls = nodes_class(py, "Decorator")?;
    if func.is_instance(decorator_cls)? {
        let inner = func.getattr("func")?;
        return rust_is_static(py, inner);
    }
    let func_base_cls = nodes_class(py, "FuncBase")?;
    if func.is_instance(func_base_cls)? {
        return func.getattr("is_static")?.is_true();
    }
    let typ = func.get_type();
    let name = typeof_name(typ)?;
    Err(PyAssertionError::new_err(format!(
        "Unexpected func type: {name}"
    )))
}

/// Render the Python type name of an object for an error message.
fn typeof_name(typ: &PyType) -> PyResult<String> {
    Ok(typ.name()?.to_string())
}

/// `mypy.checker.is_property` — does the node define a property?
///
/// Mirrors checker.py:9866-9874. `FuncDef` and `Decorator` expose
/// `is_property`; an `OverloadedFuncDef` is a property when its first item
/// is a `Decorator` whose function is a property. Any other node type is
/// not a property.
#[pyfunction]
pub(crate) fn rust_is_property(py: Python<'_>, defn: &PyAny) -> PyResult<bool> {
    let func_def_cls = nodes_class(py, "FuncDef")?;
    if defn.is_instance(func_def_cls)? {
        return defn.getattr("is_property")?.is_true();
    }
    let decorator_cls = nodes_class(py, "Decorator")?;
    if defn.is_instance(decorator_cls)? {
        return defn.getattr("func")?.getattr("is_property")?.is_true();
    }
    let overloaded_cls = nodes_class(py, "OverloadedFuncDef")?;
    if defn.is_instance(overloaded_cls)? {
        let items_list = defn.getattr("items")?.downcast::<PyList>()?;
        if !items_list.is_empty() {
            let first = items_list.get_item(0)?;
            if first.is_instance(decorator_cls)? {
                return first.getattr("func")?.getattr("is_property")?.is_true();
            }
        }
    }
    Ok(false)
}

/// `mypy.checker.is_method` — structural predicate over a SymbolNode.
///
/// Mirrors checker.py:10730-10735. An OverloadedFuncDef is a method when
/// it is not a property; a Decorator is a method when its var is not a
/// property; a FuncDef is always a method; anything else is not.
#[pyfunction]
pub(crate) fn rust_is_method(py: Python<'_>, node: &PyAny) -> PyResult<bool> {
    let overloaded_cls = nodes_class(py, "OverloadedFuncDef")?;
    if node.is_instance(overloaded_cls)? {
        return Ok(!node.getattr("is_property")?.is_true()?);
    }
    let decorator_cls = nodes_class(py, "Decorator")?;
    if node.is_instance(decorator_cls)? {
        return Ok(!node.getattr("var")?.getattr("is_property")?.is_true()?);
    }
    let func_def_cls = nodes_class(py, "FuncDef")?;
    node.is_instance(func_def_cls)
}

/// `mypy.checker.is_settable_property` — does an `OverloadedFuncDef` define
/// a property with a setter?
///
/// Mirrors checker.py:9877-9881. This is a `TypeGuard` in the Python
/// source; Rust just answers the structural question. `None` is not an
/// overload, matching Python's `isinstance(None, OverloadedFuncDef)`.
#[pyfunction]
pub(crate) fn rust_is_settable_property(py: Python<'_>, defn: &PyAny) -> PyResult<bool> {
    if defn.is_none() {
        return Ok(false);
    }
    let overloaded_cls = nodes_class(py, "OverloadedFuncDef")?;
    if !defn.is_instance(overloaded_cls)? {
        return Ok(false);
    }
    let items_list = defn.getattr("items")?.downcast::<PyList>()?;
    if items_list.is_empty() {
        return Ok(false);
    }
    let decorator_cls = nodes_class(py, "Decorator")?;
    let first = items_list.get_item(0)?;
    if !first.is_instance(decorator_cls)? {
        return Ok(false);
    }
    first.getattr("func")?.getattr("is_property")?.is_true()
}

/// `mypy.checker.is_custom_settable_property` — is this a settable property
/// with a non-trivial setter type?
///
/// Mirrors checker.py:9884-9905. "Non-trivial" means the setter is known
/// (already type-checked), not `Any`, and different from the getter type.
/// The final comparison delegates to Python's `mypy.subtypes.is_same_type`
/// so the Rust mirror never diverges on subtype semantics; anything the
/// live-object walk cannot express raises `AssertionError`, deferring to
/// the pure-Python implementation.
#[pyfunction]
pub(crate) fn rust_is_custom_settable_property(py: Python<'_>, defn: &PyAny) -> PyResult<bool> {
    if defn.is_none() {
        return Ok(false);
    }
    if !rust_is_settable_property(py, defn)? {
        return Ok(false);
    }

    let items_list = defn.getattr("items")?.downcast::<PyList>()?;
    if items_list.is_empty() {
        return Err(PyAssertionError::new_err(
            "is_custom_settable_property: empty overload items",
        ));
    }
    let decorator_cls = nodes_class(py, "Decorator")?;
    let first_item = items_list.get_item(0)?;
    if !first_item.is_instance(decorator_cls)? {
        return Err(PyAssertionError::new_err(
            "is_custom_settable_property: first overload item is not a Decorator",
        ));
    }

    let var = first_item.getattr("var")?;
    if !var.getattr("is_settable_property")?.is_true()? {
        return Ok(false);
    }
    let var_type = var.getattr("type")?;
    let setter_type = var.getattr("setter_type")?;
    let partial_type_cls = types_class(py, "PartialType")?;
    if var_type.is_none() || setter_type.is_none() || var_type.is_instance(partial_type_cls)? {
        // The caller should defer in case of partial types or not ready variables.
        return Ok(false);
    }

    // setter_type = var.setter_type.arg_types[1]
    let arg_types_list = setter_type.getattr("arg_types")?.downcast::<PyList>()?;
    if arg_types_list.len() < 2 {
        return Err(PyAssertionError::new_err(
            "is_custom_settable_property: setter has fewer than 2 args",
        ));
    }
    let setter_arg = arg_types_list.get_item(1)?;

    // get_proper_type(setter_arg) is AnyType -> False.
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    let setter_proper = get_proper_type.call1((setter_arg,))?;
    let any_type_cls = types_class(py, "AnyType")?;
    if setter_proper.is_instance(any_type_cls)? {
        return Ok(false);
    }

    // get_property_type(get_proper_type(var.type))
    // get_property_type: CallableType -> proper ret_type; Overloaded ->
    // proper items[0].ret_type; otherwise the type itself

    // (checker.py:9908-9913).
    let var_proper = get_proper_type.call1((var_type,))?;
    let property_type = get_property_type(py, get_proper_type, var_proper)?;

    // return not is_same_type(property_type, setter_arg)
    let is_same_type = py.import("mypy.subtypes")?.getattr("is_same_type")?;
    let same = is_same_type.call1((property_type, setter_arg))?;
    Ok(!same.is_true()?)
}

/// Mirror of `mypy.checker.get_property_type` — the getter type of a
/// callable/overloaded node, falling back to the type itself.
///
/// `get_property_type` (checker.py:9908-9913): `CallableType` -> proper
/// `ret_type`; `Overloaded` -> proper `items[0].ret_type`; otherwise the
/// type unchanged.
fn get_property_type(py: Python<'_>, get_proper_type: &PyAny, t: &PyAny) -> PyResult<PyObject> {
    let callable_type_cls = types_class(py, "CallableType")?;
    if t.is_instance(callable_type_cls)? {
        let ret = t.getattr("ret_type")?;
        return get_proper_type.call1((ret,)).map(Into::into);
    }
    let overloaded_cls = types_class(py, "Overloaded")?;
    if t.is_instance(overloaded_cls)? {
        let items_list = t.getattr("items")?.downcast::<PyList>()?;
        if items_list.is_empty() {
            return Err(PyAssertionError::new_err(
                "get_property_type: empty overload items",
            ));
        }
        let ret = items_list.get_item(0)?.getattr("ret_type")?;
        return get_proper_type.call1((ret,)).map(Into::into);
    }
    Ok(t.into())
}

/// `mypy.checker.get_property_type` — the getter type of a callable or
/// overloaded node, falling back to the type itself.
///
/// This is the exposed `#[pyfunction]` entry point for the private
/// `get_property_type` helper above. It imports `mypy.types.get_proper_type`
/// and hands back the live `ProperType` object (no wire round-trip), exactly
/// like `rust_is_static` at the top of this file.
#[pyfunction]
pub(crate) fn rust_get_property_type(py: Python<'_>, t: &PyAny) -> PyResult<PyObject> {
    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;
    get_property_type(py, get_proper_type, t)
}

// ---------------------------------------------------------------------------
// can_have_shared_disjoint_base (from mypy/typeops.py)
// ---------------------------------------------------------------------------

/// `mypy.typeops.can_have_shared_disjoint_base` — can the given instances
/// share a disjoint base?
///
/// Mirrors typeops.py:1688-1709: an instance's disjoint base is its type
/// when that type is disjoint, else the first disjoint base in its MRO, or
/// `object` (None). All-`object` bases can share; otherwise the candidate
/// bases must form a chain via `has_base`. The input may be any iterable
/// (callers pass lists and sets). Every instance in the input that is not
/// a `TypeInfo`-shaped object is skipped conservatively (Python assumes
/// the input is all `Instance`).
#[pyfunction]
pub(crate) fn rust_can_have_shared_disjoint_base(
    py: Python<'_>,
    instances: &PyAny,
) -> PyResult<bool> {
    let instances_iter = instances.iter()?;

    let mut disjoint_bases: Vec<PyObject> = Vec::new();
    for instance in instances_iter {
        let base = disjoint_base_of(instance?)?;
        if let Some(b) = base {
            disjoint_bases.push(b);
        }
    }

    if disjoint_bases.is_empty() {
        // All are `object`.
        return Ok(true);
    }

    let mut candidate = disjoint_bases[0].clone_ref(py);
    for base in &disjoint_bases[1..] {
        let base_any: &PyAny = base.as_ref(py);
        let cand_any: &PyAny = candidate.as_ref(py);
        let base_fullname = base_any.getattr("fullname")?;
        // candidate.has_base(base.fullname) -> chain continues.
        if cand_any
            .call_method1("has_base", (base_fullname,))?
            .is_true()?
        {
            continue;
        }
        let cand_fullname = cand_any.getattr("fullname")?;
        // base.has_base(candidate.fullname) -> candidate becomes base.
        if base_any
            .call_method1("has_base", (cand_fullname,))?
            .is_true()?
        {
            candidate = base.clone_ref(py);
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `mypy.typeops._get_disjoint_base_of` — the disjoint base of an instance,
/// if any.
///
/// Mirrors typeops.py:1678-1685: the instance's type when it is disjoint,
/// else the first disjoint base in its MRO, else `None`.
fn disjoint_base_of(instance: &PyAny) -> PyResult<Option<PyObject>> {
    let typ = instance.getattr("type")?;
    if is_disjoint_base(typ)? {
        return Ok(Some(typ.into()));
    }
    let mro_list = typ.getattr("mro")?.downcast::<PyList>()?;
    for base in mro_list.iter() {
        if is_disjoint_base(base)? {
            return Ok(Some(base.into()));
        }
    }
    Ok(None)
}

/// `mypy.typeops._is_disjoint_base` — delegates to the shared
/// `typeops::is_disjoint_base_inner` to avoid duplicating the slot
/// set-difference logic.
fn is_disjoint_base(info: &PyAny) -> PyResult<bool> {
    crate::typeops::is_disjoint_base_inner(info)
}

// ---------------------------------------------------------------------------
// Node object-model pure predicates (Issue #457)
// ---------------------------------------------------------------------------

/// `mypy.nodes.FuncBase.has_self_or_cls_argument` — does a method have a
/// `self`/`cls` argument for method binding?
///
/// Mirrors nodes.py:784-789: `not self.is_static or self.name == "__new__"`.
/// A `Decorator` unwraps to its `FuncDef`; an `OverloadedFuncDef` delegates
/// to its first item's name and `is_static`. Any node that is not a
/// `FuncBase` subclass raises `AssertionError`, matching Python's contract.
#[pyfunction]
pub(crate) fn rust_func_has_self_or_cls_argument(py: Python<'_>, func: &PyAny) -> PyResult<bool> {
    let decorator_cls = nodes_class(py, "Decorator")?;
    if func.is_instance(decorator_cls)? {
        let inner = func.getattr("func")?;
        return rust_func_has_self_or_cls_argument(py, inner);
    }
    let overloaded_cls = nodes_class(py, "OverloadedFuncDef")?;
    if func.is_instance(overloaded_cls)? {
        let is_static = func.getattr("is_static")?.is_true()?;
        let items = func.getattr("items")?.downcast::<PyList>()?;
        let name = if !items.is_empty() {
            let first = items.get_item(0)?;
            first.getattr("name")?.extract::<String>()?
        } else {
            func.getattr("name")?.extract::<String>()?
        };
        return Ok(!is_static || name == "__new__");
    }
    let func_base_cls = nodes_class(py, "FuncBase")?;
    if func.is_instance(func_base_cls)? {
        let is_static = func.getattr("is_static")?.is_true()?;
        let name: String = func.getattr("name")?.extract()?;
        return Ok(!is_static || name == "__new__");
    }
    let typ = func.get_type();
    let name = typeof_name(typ)?;
    Err(PyAssertionError::new_err(format!(
        "Unexpected func type: {name}"
    )))
}

/// `mypy.nodes.FuncItem.is_dynamic` — is the function untyped?
///
/// Mirrors nodes.py:1080-1084: `self.type is None or (isinstance(type,
/// CallableType) and type.implicit)`.
#[pyfunction]
pub(crate) fn rust_func_item_is_dynamic(py: Python<'_>, func: &PyAny) -> PyResult<bool> {
    let typ = func.getattr("type")?;
    if typ.is_none() {
        return Ok(true);
    }
    let callable_cls = types_class(py, "CallableType")?;
    if typ.is_instance(callable_cls)? {
        return typ.getattr("implicit")?.is_true();
    }
    Ok(false)
}

/// `mypy.nodes.Decorator.is_dynamic` — is the decorated function untyped?
///
/// Mirrors nodes.py:1401-1402: delegates to `self.func.is_dynamic()`.
/// The inner `func` is always a `FuncDef` (a `FuncItem`), so this recurses
/// into `rust_func_item_is_dynamic`.
#[pyfunction]
pub(crate) fn rust_decorator_is_dynamic(py: Python<'_>, dec: &PyAny) -> PyResult<bool> {
    let func = dec.getattr("func")?;
    rust_func_item_is_dynamic(py, func)
}

/// `mypy.nodes.OverloadedFuncDef.is_dynamic` — are all overload items
/// untyped?
///
/// Mirrors nodes.py:952: `all(item.is_dynamic() for item in self.items)`.
/// Each item is a `FuncDef` or `Decorator`; the dispatch matches the Python
/// `is_dynamic` method resolution.
#[pyfunction]
pub(crate) fn rust_overloaded_is_dynamic(py: Python<'_>, func: &PyAny) -> PyResult<bool> {
    let items = func.getattr("items")?.downcast::<PyList>()?;
    let decorator_cls = nodes_class(py, "Decorator")?;
    for item in items.iter() {
        if item.is_instance(decorator_cls)? {
            if !rust_decorator_is_dynamic(py, item)? {
                return Ok(false);
            }
        } else {
            if !rust_func_item_is_dynamic(py, item)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// `mypy.nodes.TypeInfo.is_generic` — does the type have type variables?
///
/// Mirrors nodes.py:3942-3944: `len(self.type_vars) > 0`.
#[pyfunction]
pub(crate) fn rust_typeinfo_is_generic(info: &PyAny) -> PyResult<bool> {
    let type_vars = info.getattr("type_vars")?.downcast::<PyList>()?;
    Ok(!type_vars.is_empty())
}

/// `mypy.nodes.TypeInfo.is_metaclass` — is this type a metaclass?
///
/// Mirrors nodes.py:4128-4133: `self.has_base("builtins.type") or
/// self.fullname == "abc.ABCMeta" or (self.fallback_to_any and not
/// precise)`.
#[pyfunction]
pub(crate) fn rust_typeinfo_is_metaclass(
    py: Python<'_>,
    info: &PyAny,
    precise: bool,
) -> PyResult<bool> {
    if rust_typeinfo_has_base(py, info, "builtins.type")? {
        return Ok(true);
    }
    let fullname: String = info.getattr("fullname")?.extract()?;
    if fullname == "abc.ABCMeta" {
        return Ok(true);
    }
    if !precise && info.getattr("fallback_to_any")?.is_true()? {
        return Ok(true);
    }
    Ok(false)
}

/// `mypy.nodes.TypeInfo.has_base` — does the MRO contain a base with the
/// given fullname?
///
/// Mirrors nodes.py:4135-4143: walks `self.mro` and compares each
/// `cls.fullname` to the target. Returns `false` if the MRO is not a list
/// (conservative; Python would raise).
#[pyfunction]
pub(crate) fn rust_typeinfo_has_base(
    _py: Python<'_>,
    info: &PyAny,
    fullname: &str,
) -> PyResult<bool> {
    let mro = info.getattr("mro")?;
    let mro_list = mro.downcast::<PyList>()?;
    for cls in mro_list.iter() {
        let cls_fullname: String = cls.getattr("fullname")?.extract()?;
        if cls_fullname == fullname {
            return Ok(true);
        }
    }
    Ok(false)
}
