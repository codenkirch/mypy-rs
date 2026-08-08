//! M28: `get_type_triggers` — mirrors `mypy.server.deps.TypeTriggersVisitor`.
//!
//! Walks a live Python `mypy.types.Type` object and produces the list of
//! trigger strings that correspond to the type becoming stale. This is the
//! hot inner loop of `DependencyVisitor.add_type_dependencies`: every type
//! annotation, inferred type, and signature is run through this visitor to
//! collect fine-grained dependency triggers.
//!
//! Returns `None` for any type class Rust does not handle (or if any child
//! recurses into an unsupported case), so the Python caller falls back to
//! the pure-Python `TypeTriggersVisitor` — the strangler-fig per-call gate.
//!
//! Ported from `mypy/server/deps.py:get_type_triggers` +
//! `TypeTriggersVisitor` (lines 951-1104). The `DependencyVisitor` AST
//! traversal itself is not ported: it walks live Python AST nodes through
//! `TraverserVisitor` and is too deeply coupled to the Python object graph
//! to be wire-format-safe. The `get_type_triggers` function is the pure,
//! self-contained, leaf-level computation that the visitor calls repeatedly.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use crate::refs::{is_instance, TypeRefs};

/// Native `get_type_triggers(typ, use_logical_deps) -> list[str] | None`.
///
/// Returns `None` when the Rust path does not handle `typ` or one of its
/// sub-components; the Python caller falls back to the pure-Python
/// `TypeTriggersVisitor`. M28 of the type-kernel migration.
#[pyfunction]
pub(crate) fn rust_get_type_triggers(
    py: Python<'_>,
    typ: &PyAny,
    use_logical_deps: bool,
) -> PyResult<Option<PyObject>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let trigger_mod = py.import("mypy.server.trigger")?;
    let make_trigger = trigger_mod.getattr("make_trigger")?;
    let make_wildcard_trigger = trigger_mod.getattr("make_wildcard_trigger")?;
    let mut ctx = TriggerCtx {
        use_logical_deps,
        refs: &refs,
        seen: SeenAliases::default(),
        make_trigger,
        make_wildcard_trigger,
    };
    let mut out: Vec<String> = Vec::new();
    match collect_triggers(py, typ, &mut ctx, &mut out) {
        Ok(()) => {
            let list = PyList::new(py, &out);
            Ok(Some(list.into()))
        }
        Err(DeferError) => Ok(None),
    }
}

/// Error sentinel: a child type is not handled by Rust, so the whole
/// computation must fall back to Python.
struct DeferError;

/// Set of seen `TypeAliasType` Python objects, tracked by pointer identity
/// (mirrors the Python `set[TypeAliasType]` which uses `__hash__`/`__eq__`
/// default object identity).
#[derive(Default)]
struct SeenAliases {
    ptrs: HashSet<usize>,
}

impl SeenAliases {
    fn contains(&self, obj: &PyAny) -> bool {
        self.ptrs.contains(&(obj.as_ptr() as usize))
    }
    fn insert(&mut self, obj: &PyAny) {
        self.ptrs.insert(obj.as_ptr() as usize);
    }
}

/// Shared state threaded through the trigger-collection recursion.
struct TriggerCtx<'a> {
    use_logical_deps: bool,
    refs: &'a TypeRefs<'a>,
    seen: SeenAliases,
    make_trigger: &'a PyAny,
    make_wildcard_trigger: &'a PyAny,
}

impl<'a> TriggerCtx<'a> {
    fn make_trigger_str(&self, name: &str) -> Result<String, DeferError> {
        let result = self.make_trigger.call1((name,)).map_err(|_| DeferError)?;
        let s: &PyString = result.downcast().map_err(|_| DeferError)?;
        s.to_str().map(|s| s.to_string()).map_err(|_| DeferError)
    }

    fn make_wildcard_str(&self, module: &str) -> Result<String, DeferError> {
        let result = self
            .make_wildcard_trigger
            .call1((module,))
            .map_err(|_| DeferError)?;
        let s: &PyString = result.downcast().map_err(|_| DeferError)?;
        s.to_str().map(|s| s.to_string()).map_err(|_| DeferError)
    }
}

fn collect_triggers(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut TriggerCtx<'_>,
    out: &mut Vec<String>,
) -> Result<(), DeferError> {
    let refs = ctx.refs;

    // --- Instance ---
    if is_instance(obj, refs.instance) {
        let typ_type = get_attr_or_defer(obj, "type")?;
        let fullname = get_str_attr_or_defer(py, typ_type, "fullname")?;
        out.push(ctx.make_trigger_str(&fullname)?);
        let args = get_attr_or_defer(obj, "args")?;
        for arg in iter_seq(args)? {
            collect_triggers(py, arg, ctx, out)?;
        }
        let lkv = obj.getattr("last_known_value").map_err(|_| DeferError)?;
        if !lkv.is_none() {
            collect_triggers(py, lkv, ctx, out)?;
        }
        let extra_attrs = obj.getattr("extra_attrs").map_err(|_| DeferError)?;
        if !extra_attrs.is_none() {
            let mod_name = extra_attrs.getattr("mod_name").map_err(|_| DeferError)?;
            if !mod_name.is_none() {
                let mn = pystr_to_string(py, mod_name)?;
                out.push(ctx.make_wildcard_str(&mn)?);
            }
        }
        return Ok(());
    }

    // --- TypeAliasType ---
    if is_instance(obj, refs.type_alias_type) {
        if ctx.seen.contains(obj) {
            return Ok(());
        }
        ctx.seen.insert(obj);
        let alias = get_attr_or_defer(obj, "alias")?;
        if alias.is_none() {
            return Err(DeferError);
        }
        let fullname = get_str_attr_or_defer(py, alias, "fullname")?;
        out.push(ctx.make_trigger_str(&fullname)?);
        let args = get_attr_or_defer(obj, "args")?;
        for arg in iter_seq(args)? {
            collect_triggers(py, arg, ctx, out)?;
        }
        let target = get_attr_or_defer(alias, "target")?;
        collect_triggers(py, target, ctx, out)?;
        return Ok(());
    }

    // --- AnyType ---
    if is_instance(obj, refs.any_type) {
        let min = obj.getattr("missing_import_name").map_err(|_| DeferError)?;
        if !min.is_none() {
            let name = pystr_to_string(py, min)?;
            out.push(ctx.make_trigger_str(&name)?);
        }
        return Ok(());
    }

    // --- NoneType, UninhabitedType, UnboundType, DeletedType ---
    if is_instance(obj, refs.none_type)
        || is_instance(obj, refs.uninhabited_type)
        || is_instance(obj, refs.deleted_type)
    {
        return Ok(());
    }
    if class_name_is(obj, "UnboundType") {
        return Ok(());
    }

    // --- CallableType ---
    if is_instance(obj, refs.callable_type) {
        let arg_types = get_attr_or_defer(obj, "arg_types")?;
        for arg in iter_seq(arg_types)? {
            collect_triggers(py, arg, ctx, out)?;
        }
        let ret_type = get_attr_or_defer(obj, "ret_type")?;
        collect_triggers(py, ret_type, ctx, out)?;
        recurse_optional_attr(py, obj, "type_guard", ctx, out)?;
        recurse_optional_attr(py, obj, "type_is", ctx, out)?;
        recurse_optional_attr(py, obj, "instance_type", ctx, out)?;
        return Ok(());
    }

    // --- Overloaded ---
    if is_instance(obj, refs.overloaded) {
        let items = get_attr_or_defer(obj, "items")?;
        for item in iter_seq(items)? {
            collect_triggers(py, item, ctx, out)?;
        }
        return Ok(());
    }

    // --- TupleType ---
    if is_instance(obj, refs.tuple_type) {
        let items = get_attr_or_defer(obj, "items")?;
        for item in iter_seq(items)? {
            collect_triggers(py, item, ctx, out)?;
        }
        let pf = get_attr_or_defer(obj, "partial_fallback")?;
        collect_triggers(py, pf, ctx, out)?;
        return Ok(());
    }

    // --- TypedDictType ---
    if is_instance(obj, refs.typed_dict_type) {
        let items_dict = get_attr_or_defer(obj, "items")?;
        let dict: &PyDict = items_dict.downcast().map_err(|_| DeferError)?;
        for (_, value) in dict.iter() {
            collect_triggers(py, value, ctx, out)?;
        }
        let fb = get_attr_or_defer(obj, "fallback")?;
        collect_triggers(py, fb, ctx, out)?;
        return Ok(());
    }

    // --- LiteralType ---
    if is_instance(obj, refs.literal_type) {
        let fb = get_attr_or_defer(obj, "fallback")?;
        collect_triggers(py, fb, ctx, out)?;
        return Ok(());
    }

    // --- TypeType ---
    if is_instance(obj, refs.type_type) {
        let item = get_attr_or_defer(obj, "item")?;
        let mut child_triggers: Vec<String> = Vec::new();
        collect_triggers(py, item, ctx, &mut child_triggers)?;
        // Python appends __init__/__new__ AFTER the item triggers.
        if !ctx.use_logical_deps {
            out.extend(child_triggers.iter().cloned());
            for trigger in &child_triggers {
                let stripped = trigger.strip_suffix('>').unwrap_or(trigger);
                out.push(format!("{stripped}.__init__>"));
                out.push(format!("{stripped}.__new__>"));
            }
        } else {
            out.extend(child_triggers);
        }
        return Ok(());
    }

    // --- TypeVarType ---
    if is_instance(obj, refs.type_var_type) {
        add_fullname_trigger(py, obj, ctx, out)?;
        let ub = get_attr_or_defer(obj, "upper_bound")?;
        collect_triggers(py, ub, ctx, out)?;
        let default = get_attr_or_defer(obj, "default")?;
        collect_triggers(py, default, ctx, out)?;
        let values = get_attr_or_defer(obj, "values")?;
        for val in iter_seq(values)? {
            collect_triggers(py, val, ctx, out)?;
        }
        return Ok(());
    }

    // --- ParamSpecType ---
    if is_instance(obj, refs.param_spec_type) {
        add_fullname_trigger(py, obj, ctx, out)?;
        let ub = get_attr_or_defer(obj, "upper_bound")?;
        collect_triggers(py, ub, ctx, out)?;
        let default = get_attr_or_defer(obj, "default")?;
        collect_triggers(py, default, ctx, out)?;
        let prefix = get_attr_or_defer(obj, "prefix")?;
        collect_triggers(py, prefix, ctx, out)?;
        return Ok(());
    }

    // --- TypeVarTupleType ---
    if is_instance(obj, refs.type_var_tuple_type) {
        add_fullname_trigger(py, obj, ctx, out)?;
        let ub = get_attr_or_defer(obj, "upper_bound")?;
        collect_triggers(py, ub, ctx, out)?;
        let default = get_attr_or_defer(obj, "default")?;
        collect_triggers(py, default, ctx, out)?;
        return Ok(());
    }

    // --- UnpackType ---
    if is_instance(obj, refs.unpack_type) {
        let typ = get_attr_or_defer(obj, "type")?;
        collect_triggers(py, typ, ctx, out)?;
        return Ok(());
    }

    // --- UnionType ---
    if is_instance(obj, refs.union_type) {
        let items = get_attr_or_defer(obj, "items")?;
        for item in iter_seq(items)? {
            collect_triggers(py, item, ctx, out)?;
        }
        return Ok(());
    }

    // --- Parameters ---
    if class_name_is(obj, "Parameters") {
        let arg_types = get_attr_or_defer(obj, "arg_types")?;
        for arg in iter_seq(arg_types)? {
            collect_triggers(py, arg, ctx, out)?;
        }
        return Ok(());
    }

    // --- ErasedType, PartialType ---
    // Python asserts these should not be seen. Defer so Python raises.
    if class_name_is(obj, "ErasedType") || class_name_is(obj, "PartialType") {
        return Err(DeferError);
    }

    // Anything else — defer.
    Err(DeferError)
}

// --- Helpers ---

fn get_attr_or_defer<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyAny, DeferError> {
    obj.getattr(name).map_err(|_| DeferError)
}

fn get_str_attr_or_defer(py: Python<'_>, obj: &PyAny, name: &str) -> Result<String, DeferError> {
    let attr = obj.getattr(name).map_err(|_| DeferError)?;
    pystr_to_string(py, attr)
}

fn pystr_to_string(_py: Python<'_>, obj: &PyAny) -> Result<String, DeferError> {
    if let Ok(s) = obj.downcast::<PyString>() {
        s.to_str().map(|s| s.to_string()).map_err(|_| DeferError)
    } else {
        Err(DeferError)
    }
}

/// If `obj.attr` is not None, recurse into it.
fn recurse_optional_attr(
    py: Python<'_>,
    obj: &PyAny,
    attr: &str,
    ctx: &mut TriggerCtx<'_>,
    out: &mut Vec<String>,
) -> Result<(), DeferError> {
    let val = obj.getattr(attr).map_err(|_| DeferError)?;
    if !val.is_none() {
        collect_triggers(py, val, ctx, out)?;
    }
    Ok(())
}

/// If `obj.fullname` is not None, push `make_trigger(fullname)`.
fn add_fullname_trigger(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut TriggerCtx<'_>,
    out: &mut Vec<String>,
) -> Result<(), DeferError> {
    let fullname = obj.getattr("fullname").map_err(|_| DeferError)?;
    if !fullname.is_none() {
        let name = pystr_to_string(py, fullname)?;
        out.push(ctx.make_trigger_str(&name)?);
    }
    Ok(())
}

/// Iterate a sequence that is a list or tuple. Defer on anything else.
fn iter_seq(obj: &PyAny) -> Result<Vec<&PyAny>, DeferError> {
    if let Ok(list) = obj.downcast::<PyList>() {
        Ok(list.iter().collect())
    } else if let Ok(tuple) = obj.downcast::<PyTuple>() {
        Ok(tuple.iter().collect())
    } else {
        Err(DeferError)
    }
}

/// Check if obj's class name matches `expected`.
fn class_name_is(obj: &PyAny, expected: &str) -> bool {
    let class = match obj.getattr("__class__") {
        Ok(c) => c,
        Err(_) => return false,
    };
    let name = match class.getattr("__name__") {
        Ok(n) => n,
        Err(_) => return false,
    };
    match name.downcast::<PyString>() {
        Ok(s) => s.to_str().unwrap_or("") == expected,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn triggers_instance_basic() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
fx = TypeFixture(COVARIANT)
typ = fx.a
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let typ = locals.get_item("typ").unwrap().unwrap();
            let result = rust_get_type_triggers(py, typ, false).unwrap();
            assert!(result.is_some(), "Rust path should not fall back");
            let list = result.unwrap();
            let py_list = list.downcast::<PyList>(py).unwrap();
            assert_eq!(py_list.len(), 1);
        });
    }

    #[ignore = "requires librt built in venv; see AGENTS.md 'Type kernel build order'"]
    #[test]
    fn triggers_none_is_empty() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.types import NoneType
typ = NoneType()
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let typ = locals.get_item("typ").unwrap().unwrap();
            let result = rust_get_type_triggers(py, typ, false).unwrap();
            assert!(result.is_some());
            let binding = result.unwrap();
            let list = binding.downcast::<PyList>(py).unwrap();
            assert_eq!(list.len(), 0);
        });
    }
}
