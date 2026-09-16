//! Native ports of `mypy.server.subexpr` (`get_subexpressions`) and
//! `mypy.server.aststrip` (`strip_ref_expr`).
//!
//! Both seams walk live Python AST objects via PyO3 (zero wire bytes),
//! mirroring the `depswalk.rs` pattern. `rust_get_subexpressions`
//! collects every `Expression` node in pre-order (the node is added
//! before its children are recursed into), exactly like
//! `SubexpressionFinder`. `rust_strip_ref_expr` resets the five
//! `RefExpr` fields that `strip_ref_expr` clears.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};

use crate::serverdeps::{class_name_is, get_attr_or_defer, DeferError};

// ---------------------------------------------------------------------------
// Helpers (mirror depswalk.rs private helpers)
// ---------------------------------------------------------------------------

fn class_name_owned(obj: &PyAny) -> Result<String, DeferError> {
    let cls = get_attr_or_defer(obj, "__class__")?;
    let name = get_attr_or_defer(cls, "__name__")?;
    name.downcast::<PyString>()
        .map_err(|_| DeferError)?
        .to_str()
        .map(|s| s.to_string())
        .map_err(|_| DeferError)
}

fn truthy_attr(obj: &PyAny, name: &str) -> Result<bool, DeferError> {
    get_attr_or_defer(obj, name)?
        .is_true()
        .map_err(|_| DeferError)
}

fn list_attr<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyList, DeferError> {
    get_attr_or_defer(obj, name)?
        .downcast::<PyList>()
        .map_err(|_| DeferError)
}

fn seq_get(obj: &PyAny, idx: usize) -> Result<&PyAny, DeferError> {
    if let Ok(t) = obj.downcast::<pyo3::types::PyTuple>() {
        return t.get_item(idx).map_err(|_| DeferError);
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        return l.get_item(idx).map_err(|_| DeferError);
    }
    Err(DeferError)
}

// ---------------------------------------------------------------------------
// SubexpressionFinder walk
// ---------------------------------------------------------------------------

struct SubexprWalker<'py> {
    py: Python<'py>,
    exprs: Vec<Py<PyAny>>,
}

impl<'py> SubexprWalker<'py> {
    fn new(py: Python<'py>) -> Self {
        SubexprWalker {
            py,
            exprs: Vec::new(),
        }
    }

    fn add(&mut self, o: &PyAny) {
        self.exprs.push(o.into());
    }

    fn walk(&mut self, obj: &PyAny) -> Result<(), DeferError> {
        let name = class_name_owned(obj)?;
        match name.as_str() {
            // --- Leaf expressions (add only, no recursion) ---
            "IntExpr" | "NameExpr" | "FloatExpr" | "StrExpr" | "BytesExpr" | "ComplexExpr"
            | "EllipsisExpr" | "SuperExpr" | "TypeVarExpr" | "TypeAliasExpr" | "NamedTupleExpr"
            | "TypedDictExpr" | "PromoteExpr" | "NewTypeExpr" | "EnumCallExpr" => {
                self.add(obj);
                Ok(())
            }

            // --- Composite expressions (add, then recurse) ---
            "MemberExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "YieldFromExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "YieldExpr" => {
                self.add(obj);
                if !get_attr_or_defer(obj, "expr")?.is_none() {
                    self.walk(get_attr_or_defer(obj, "expr")?)?;
                }
                Ok(())
            }
            "CallExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "callee")?)?;
                for a in list_attr(obj, "args")?.iter() {
                    self.walk(a)?;
                }
                if truthy_attr(obj, "analyzed")? {
                    self.walk(get_attr_or_defer(obj, "analyzed")?)?;
                }
                Ok(())
            }
            "OpExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "left")?)?;
                self.walk(get_attr_or_defer(obj, "right")?)?;
                let analyzed = get_attr_or_defer(obj, "analyzed")?;
                if !analyzed.is_none() {
                    self.walk(analyzed)?;
                }
                Ok(())
            }
            "ComparisonExpr" => {
                self.add(obj);
                for operand in list_attr(obj, "operands")?.iter() {
                    self.walk(operand)?;
                }
                Ok(())
            }
            "SliceExpr" => {
                self.add(obj);
                self.traverse_optional_child(obj, "begin_index")?;
                self.traverse_optional_child(obj, "end_index")?;
                self.traverse_optional_child(obj, "stride")?;
                Ok(())
            }
            "CastExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "TypeFormExpr" => {
                self.add(obj);
                // TraverserVisitor.visit_type_form_expr is a no-op.
                Ok(())
            }
            "AssertTypeExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "RevealExpr" => {
                self.add(obj);
                // TraverserVisitor walks `expr` only for REVEAL_TYPE (the only
                // kind with an inner expression). We check truthiness of `expr`
                // instead of comparing the kind int: simpler and parity-safe.
                let expr = get_attr_or_defer(obj, "expr")?;
                if !expr.is_none() {
                    self.walk(expr)?;
                }
                Ok(())
            }
            "AssignmentExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "target")?)?;
                self.walk(get_attr_or_defer(obj, "value")?)?;
                Ok(())
            }
            "UnaryExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "ListExpr" | "TupleExpr" | "SetExpr" => {
                self.add(obj);
                for item in list_attr(obj, "items")?.iter() {
                    self.walk(item)?;
                }
                Ok(())
            }
            "DictExpr" => {
                self.add(obj);
                for pair in list_attr(obj, "items")?.iter() {
                    let k = seq_get(pair, 0)?;
                    let v = seq_get(pair, 1)?;
                    if !k.is_none() {
                        self.walk(k)?;
                    }
                    self.walk(v)?;
                }
                Ok(())
            }
            "TemplateStrExpr" => {
                self.add(obj);
                for item in list_attr(obj, "items")?.iter() {
                    if let Ok(t) = item.downcast::<pyo3::types::PyTuple>() {
                        self.walk(t.get_item(0).map_err(|_| DeferError)?)?;
                        let fmt = t.get_item(3).map_err(|_| DeferError)?;
                        if !fmt.is_none() {
                            self.walk(fmt)?;
                        }
                    } else {
                        self.walk(item)?;
                    }
                }
                Ok(())
            }
            "IndexExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "base")?)?;
                self.walk(get_attr_or_defer(obj, "index")?)?;
                if truthy_attr(obj, "analyzed")? {
                    self.walk(get_attr_or_defer(obj, "analyzed")?)?;
                }
                Ok(())
            }
            "GeneratorExpr" => {
                self.add(obj);
                let indices = list_attr(obj, "indices")?;
                let sequences = list_attr(obj, "sequences")?;
                let condlists = list_attr(obj, "condlists")?;
                if indices.len() != sequences.len() || indices.len() != condlists.len() {
                    return Err(DeferError);
                }
                for i in 0..indices.len() {
                    self.walk(sequences.get_item(i).map_err(|_| DeferError)?)?;
                    self.walk(indices.get_item(i).map_err(|_| DeferError)?)?;
                    let conds = condlists
                        .get_item(i)
                        .map_err(|_| DeferError)?
                        .downcast::<PyList>()
                        .map_err(|_| DeferError)?;
                    for cond in conds.iter() {
                        self.walk(cond)?;
                    }
                }
                self.walk(get_attr_or_defer(obj, "left_expr")?)?;
                Ok(())
            }
            "DictionaryComprehension" => {
                self.add(obj);
                let indices = list_attr(obj, "indices")?;
                let sequences = list_attr(obj, "sequences")?;
                let condlists = list_attr(obj, "condlists")?;
                if indices.len() != sequences.len() || indices.len() != condlists.len() {
                    return Err(DeferError);
                }
                for i in 0..indices.len() {
                    self.walk(sequences.get_item(i).map_err(|_| DeferError)?)?;
                    self.walk(indices.get_item(i).map_err(|_| DeferError)?)?;
                    let conds = condlists
                        .get_item(i)
                        .map_err(|_| DeferError)?
                        .downcast::<PyList>()
                        .map_err(|_| DeferError)?;
                    for cond in conds.iter() {
                        self.walk(cond)?;
                    }
                }
                self.walk(get_attr_or_defer(obj, "key")?)?;
                self.walk(get_attr_or_defer(obj, "value")?)?;
                Ok(())
            }
            "ListComprehension" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "generator")?)
            }
            "SetComprehension" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "generator")?)
            }
            "ConditionalExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "cond")?)?;
                self.walk(get_attr_or_defer(obj, "if_expr")?)?;
                self.walk(get_attr_or_defer(obj, "else_expr")?)?;
                Ok(())
            }
            "TypeApplication" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "LambdaExpr" => {
                self.add(obj);
                self.visit_func(obj)
            }
            "StarExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }
            "AwaitExpr" => {
                self.add(obj);
                self.walk(get_attr_or_defer(obj, "expr")?)
            }

            // --- Statements and structural nodes (traverse, no add) ---
            "MypyFile" => {
                for d in list_attr(obj, "defs")?.iter() {
                    self.walk(d)?;
                }
                Ok(())
            }
            "Block" => {
                for s in list_attr(obj, "body")?.iter() {
                    self.walk(s)?;
                }
                Ok(())
            }
            "FuncDef" => self.visit_func(obj),
            "OverloadedFuncDef" => {
                for item in list_attr(obj, "items")?.iter() {
                    self.walk(item)?;
                }
                if truthy_attr(obj, "impl")? {
                    self.walk(get_attr_or_defer(obj, "impl")?)?;
                }
                Ok(())
            }
            "ClassDef" => {
                for d in list_attr(obj, "decorators")?.iter() {
                    self.walk(d)?;
                }
                for base in list_attr(obj, "base_type_exprs")?.iter() {
                    self.walk(base)?;
                }
                self.traverse_optional_child(obj, "metaclass")?;
                let kw = get_attr_or_defer(obj, "keywords")?;
                if !kw.is_none() {
                    let kw_dict = kw
                        .downcast::<pyo3::types::PyDict>()
                        .map_err(|_| DeferError)?;
                    for v in kw_dict.values() {
                        self.walk(v)?;
                    }
                }
                self.walk(get_attr_or_defer(obj, "defs")?)?;
                if truthy_attr(obj, "analyzed")? {
                    self.walk(get_attr_or_defer(obj, "analyzed")?)?;
                }
                Ok(())
            }
            "Decorator" => {
                self.walk(get_attr_or_defer(obj, "func")?)?;
                self.walk(get_attr_or_defer(obj, "var")?)?;
                for d in list_attr(obj, "decorators")?.iter() {
                    self.walk(d)?;
                }
                Ok(())
            }
            "ExpressionStmt" => self.walk(get_attr_or_defer(obj, "expr")?),
            "AssignmentStmt" => {
                self.walk(get_attr_or_defer(obj, "rvalue")?)?;
                for l in list_attr(obj, "lvalues")?.iter() {
                    self.walk(l)?;
                }
                Ok(())
            }
            "OperatorAssignmentStmt" => {
                self.walk(get_attr_or_defer(obj, "rvalue")?)?;
                self.walk(get_attr_or_defer(obj, "lvalue")?)
            }
            "WhileStmt" => {
                self.walk(get_attr_or_defer(obj, "expr")?)?;
                self.walk(get_attr_or_defer(obj, "body")?)?;
                self.traverse_optional_child(obj, "else_body")?;
                Ok(())
            }
            "ForStmt" => {
                self.walk(get_attr_or_defer(obj, "index")?)?;
                self.walk(get_attr_or_defer(obj, "expr")?)?;
                self.walk(get_attr_or_defer(obj, "body")?)?;
                self.traverse_optional_child(obj, "else_body")?;
                Ok(())
            }
            "ReturnStmt" => self.traverse_optional_child(obj, "expr"),
            "AssertStmt" => {
                self.traverse_optional_child(obj, "expr")?;
                self.traverse_optional_child(obj, "msg")?;
                Ok(())
            }
            "DelStmt" => self.traverse_optional_child(obj, "expr"),
            "IfStmt" => {
                for e in list_attr(obj, "expr")?.iter() {
                    self.walk(e)?;
                }
                for b in list_attr(obj, "body")?.iter() {
                    self.walk(b)?;
                }
                self.traverse_optional_child(obj, "else_body")?;
                Ok(())
            }
            "RaiseStmt" => {
                self.traverse_optional_child(obj, "expr")?;
                self.traverse_optional_child(obj, "from_expr")?;
                Ok(())
            }
            "TryStmt" => {
                self.walk(get_attr_or_defer(obj, "body")?)?;
                let types = list_attr(obj, "types")?;
                let handlers = list_attr(obj, "handlers")?;
                if types.len() != handlers.len() {
                    return Err(DeferError);
                }
                for i in 0..types.len() {
                    let tp = types.get_item(i).map_err(|_| DeferError)?;
                    if !tp.is_none() {
                        self.walk(tp)?;
                    }
                    self.walk(handlers.get_item(i).map_err(|_| DeferError)?)?;
                }
                let vars = list_attr(obj, "vars")?;
                for v in vars.iter() {
                    if !v.is_none() {
                        self.walk(v)?;
                    }
                }
                self.traverse_optional_child(obj, "else_body")?;
                self.traverse_optional_child(obj, "finally_body")?;
                Ok(())
            }
            "WithStmt" => {
                let exprs = list_attr(obj, "expr")?;
                let targets = list_attr(obj, "target")?;
                if exprs.len() != targets.len() {
                    return Err(DeferError);
                }
                for i in 0..exprs.len() {
                    self.walk(exprs.get_item(i).map_err(|_| DeferError)?)?;
                    let targ = targets.get_item(i).map_err(|_| DeferError)?;
                    if !targ.is_none() {
                        self.walk(targ)?;
                    }
                }
                self.walk(get_attr_or_defer(obj, "body")?)?;
                Ok(())
            }
            "MatchStmt" => {
                self.walk(get_attr_or_defer(obj, "subject")?)?;
                let patterns = list_attr(obj, "patterns")?;
                let guards = list_attr(obj, "guards")?;
                let bodies = list_attr(obj, "bodies")?;
                if patterns.len() != guards.len() || patterns.len() != bodies.len() {
                    return Err(DeferError);
                }
                for i in 0..patterns.len() {
                    self.walk(patterns.get_item(i).map_err(|_| DeferError)?)?;
                    let guard = guards.get_item(i).map_err(|_| DeferError)?;
                    if !guard.is_none() {
                        self.walk(guard)?;
                    }
                    self.walk(bodies.get_item(i).map_err(|_| DeferError)?)?;
                }
                Ok(())
            }
            "TypeAliasStmt" => {
                self.walk(get_attr_or_defer(obj, "name")?)?;
                self.walk(get_attr_or_defer(obj, "value")?)?;
                Ok(())
            }
            "Import" => {
                for a in list_attr(obj, "assignments")?.iter() {
                    self.walk(a)?;
                }
                Ok(())
            }
            "ImportFrom" => {
                for a in list_attr(obj, "assignments")?.iter() {
                    self.walk(a)?;
                }
                Ok(())
            }

            // --- Leaf non-expression nodes (TraverserVisitor no-ops) ---
            "Var" | "ContinueStmt" | "PassStmt" | "BreakStmt" | "TempNode" | "NonlocalDecl"
            | "GlobalDecl" | "ImportAll" | "ParamSpecExpr" | "TypeVarTupleExpr" | "TypeAlias"
            | "SingletonPattern" => Ok(()),

            // --- Patterns (TraverserVisitor traversal, no add) ---
            "AsPattern" => {
                self.traverse_optional_child(obj, "pattern")?;
                self.traverse_optional_child(obj, "name")?;
                Ok(())
            }
            "OrPattern" | "SequencePattern" => {
                for p in list_attr(obj, "patterns")?.iter() {
                    self.walk(p)?;
                }
                Ok(())
            }
            "ValuePattern" => self.walk(get_attr_or_defer(obj, "expr")?),
            "StarredPattern" => self.traverse_optional_child(obj, "capture"),
            "MappingPattern" => {
                for key in list_attr(obj, "keys")?.iter() {
                    self.walk(key)?;
                }
                for value in list_attr(obj, "values")?.iter() {
                    self.walk(value)?;
                }
                self.traverse_optional_child(obj, "rest")?;
                Ok(())
            }
            "ClassPattern" => {
                self.walk(get_attr_or_defer(obj, "class_ref")?)?;
                for p in list_attr(obj, "positionals")?.iter() {
                    self.walk(p)?;
                }
                for v in list_attr(obj, "keyword_values")?.iter() {
                    self.walk(v)?;
                }
                Ok(())
            }

            _ => Err(DeferError),
        }
    }

    fn visit_func(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let arguments = get_attr_or_defer(o, "arguments")?;
        if !arguments.is_none() {
            let args = arguments.downcast::<PyList>().map_err(|_| DeferError)?;
            for arg in args.iter() {
                let init = get_attr_or_defer(arg, "initializer")?;
                if !init.is_none() {
                    self.walk(init)?;
                }
            }
            // TraverserVisitor also visits arg.variable (visit_var), which is
            // a no-op for SubexpressionFinder (Var is a leaf non-expression).
        }
        self.walk(get_attr_or_defer(o, "body")?)?;
        Ok(())
    }

    fn traverse_optional_child(&mut self, o: &PyAny, field: &str) -> Result<(), DeferError> {
        let v = get_attr_or_defer(o, field)?;
        if !v.is_none() {
            self.walk(v)?;
        }
        Ok(())
    }

    fn into_list(self) -> PyObject {
        PyList::new(self.py, self.exprs).into()
    }
}

/// Native `get_subexpressions`.
///
/// Walks the AST from `root` collecting every `Expression` node into a
/// list, mirroring `SubexpressionFinder`. Returns `None` on any
/// unhandled node so the Python visitor re-runs.
#[pyfunction]
pub(crate) fn rust_get_subexpressions(py: Python<'_>, root: &PyAny) -> PyResult<Option<PyObject>> {
    let mut walker = SubexprWalker::new(py);
    match walker.walk(root) {
        Ok(()) => Ok(Some(walker.into_list())),
        Err(_) => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// strip_ref_expr
// ---------------------------------------------------------------------------

/// Native `strip_ref_expr`.
///
/// Resets the five `RefExpr` fields (`kind`, `node`, `fullname`,
/// `is_new_def`, `is_inferred_def`) on the live node. Returns
/// `Some(true)` on success or `None` on a PyO3 failure so the Python
/// body re-runs.
#[pyfunction]
pub(crate) fn rust_strip_ref_expr(node: &PyAny) -> PyResult<Option<bool>> {
    // Verify the node is a RefExpr before stripping (parity with the
    // Python type check in the caller).
    if !class_name_is(node, "NameExpr") && !class_name_is(node, "MemberExpr") {
        return Ok(None);
    }
    let py = node.py();
    let result = (|| -> Result<(), DeferError> {
        node.setattr("kind", py.None()).map_err(|_| DeferError)?;
        node.setattr("node", py.None()).map_err(|_| DeferError)?;
        node.setattr("fullname", "").map_err(|_| DeferError)?;
        node.setattr("is_new_def", false).map_err(|_| DeferError)?;
        node.setattr("is_inferred_def", false)
            .map_err(|_| DeferError)?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Some(true)),
        Err(_) => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// G1.2 (#1674): shadow-served read flip for the aststrip lvalue arm
// ---------------------------------------------------------------------------

/// Native `NodeStripVisitor.process_lvalue_in_method`, MemberExpr arm.
///
/// The first expression-node **read flip**: `MemberExpr.is_new_def` and
/// `MemberExpr.name` are served from the G1 node shadow (`mypy/nodes_mirror`
/// + `crate::node_mirror`) instead of crossing to the live Python slots,
/// and the class-namespace delete goes through `crate::symtable_mirror`
/// exactly like the Python `delete_names_entry` accessor.
///
/// Reads only shadowed slots, never infers them: `is_new_def` comes from
/// the G1.0a record and `name` from the G1.2 text record, both written by
/// the same `__setattr__` hook that the live slots go through, so the
/// served values cannot drift from Python. `None` means "the store does
/// not cover this read" (no record, another lvalue shape, `type` None)
/// and the Python body stays the identical fallback, including its
/// `assert self.type is not None`.
///
/// Returns `Some(deleted)` when the shadow served the read: `deleted` is
/// whether the name was removed from the class namespace.
#[pyfunction]
#[pyo3(signature = (type_info, lvalue))]
pub(crate) fn rust_aststrip_process_lvalue<'py>(
    py: Python<'py>,
    type_info: Option<&'py PyAny>,
    lvalue: &'py PyAny,
) -> PyResult<Option<bool>> {
    match lvalue_value(py, type_info, lvalue) {
        Ok(served) => Ok(served),
        Err(_) => Ok(None),
    }
}

fn lvalue_value<'py>(
    py: Python<'py>,
    type_info: Option<&'py PyAny>,
    lvalue: &'py PyAny,
) -> Result<Option<bool>, DeferError> {
    // Only the MemberExpr arm of `process_lvalue_in_method` is flipped;
    // tuple/list/star lvalues stay on the Python recursion.
    if !class_name_is(lvalue, "MemberExpr") {
        return Ok(None);
    }
    let handle = match crate::identity::handle_of(lvalue) {
        Some(handle) => handle,
        None => return Ok(None),
    };
    let is_new_def = match crate::node_mirror::shadow_is_new_def(handle) {
        Some(value) => value,
        None => return Ok(None),
    };
    let name = match crate::node_mirror::shadow_field_text(handle, "name") {
        Some(value) => value,
        None => return Ok(None),
    };
    if !is_new_def {
        return Ok(Some(false));
    }
    // `assert self.type is not None` stays Python-side: defer so the
    // identical assert fires.
    let info = match type_info {
        Some(info) => {
            if info.is_none() {
                return Ok(None);
            }
            info
        }
        None => return Ok(None),
    };
    let names = get_attr_or_defer(info, "names")?;
    let names_dict = names
        .downcast::<pyo3::types::PyDict>()
        .map_err(|_| DeferError)?;
    let key = PyString::new(py, &name);
    if !names_dict.contains(key).map_err(|_| DeferError)? {
        return Ok(Some(false));
    }
    // Same order as `delete_names_entry`: live delete, then the shadow
    // drop, and only for a table with an identity handle (no minting).
    names_dict.del_item(key).map_err(|_| DeferError)?;
    if crate::identity::handle_of(names).is_some() {
        let _ = crate::symtable_mirror::delete(names, &name);
    }
    Ok(Some(true))
}

/// Register this module's Python-facing seam surface (#1677).
pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_get_subexpressions, m)?)?;

    m.add_function(wrap_pyfunction!(rust_strip_ref_expr, m)?)?;

    m.add_function(wrap_pyfunction!(rust_aststrip_process_lvalue, m)?)?;
    Ok(())
}
