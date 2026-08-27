//! M28: fine-grained server update — pure trigger/target computation.
//!
//! This module ports the pure (stateless) components of mypy's fine-grained
//! incremental update pipeline to Rust. Two categories of functions live here:
//!
//! 1. **Type-trigging** (M28): `rust_get_type_triggers` walks live Python
//!    `mypy.types.Type` objects and produces trigger strings. Mirrors
//!    `mypy.server.deps.TypeTriggersVisitor`.
//!
//! 2. **Server trigger/target computation** (M354): pure computations that take
//!    dependency records (triggers → target sets) and a set of changed/triggered
//!    fully-qualified names, and return the set of module IDs that need
//!    reprocessing. These are the separable pure logic pieces from
//!    `mypy.server.update`:
//!      - `compute_target_modules` — mirrors the pure BFS of
//!        `find_targets_recursive` (given triggers + deps + up_to_date → modules)
//!      - `compute_wildcard_triggers` — mirrors the wildcard expansion in
//!        `calculate_active_triggers` (given changed name prefixes + nesting
//!        levels → trigger strings)
//!
//! Everything that touches live AST objects (reprocess_nodes, lookup_target,
//! strip_target, merge_asts) stays in Python. Rust functions that cannot
//! handle all cases return `None` / an empty result, so the Python caller
//! falls back to the pure-Python implementation — the strangler-fig per-call
//! gate.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple, PyType};

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

// ====================================================================
// Issue #1007: attribute_triggers port
// ====================================================================

/// Shared state threaded through the attribute-trigger recursion.
struct AttrTriggerCtx<'a> {
    name: &'a str,
    refs: &'a TypeRefs<'a>,
    make_trigger: &'a PyAny,
    get_proper_type: &'a PyAny,
}

impl AttrTriggerCtx<'_> {
    fn make_trigger_str(&self, member: &str) -> Result<String, DeferError> {
        let result = self.make_trigger.call1((member,)).map_err(|_| DeferError)?;
        let s: &PyString = result.downcast().map_err(|_| DeferError)?;
        s.to_str().map(|s| s.to_string()).map_err(|_| DeferError)
    }

    /// Mirrors `mypy.types.get_proper_type` by calling the Python helper, so
    /// alias-expansion edge cases (no_args aliases, TypeGuardedType) behave
    /// identically.
    fn proper_type(&self, obj: &PyAny) -> Result<&PyAny, DeferError> {
        let expanded = self.get_proper_type.call1((obj,)).map_err(|_| DeferError)?;
        Ok(expanded)
    }
}

/// Native `attribute_triggers(typ, name) -> list[str] | None`.
///
/// Mirrors `mypy.server.deps.DependencyVisitor.attribute_triggers`: returns
/// the member trigger strings for an attribute access on `typ`. Unreadable
/// facts defer (`None`) so the Python caller falls back to the pure-Python
/// method; the AST walk (DependencyVisitor) stays Python-side.
#[pyfunction]
pub(crate) fn rust_attribute_triggers(
    py: Python<'_>,
    typ: &PyAny,
    name: &PyString,
) -> PyResult<Option<PyObject>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let name = match name.to_str() {
        Ok(n) => n,
        Err(_) => return Ok(None),
    };
    let trigger_mod = py.import("mypy.server.trigger")?;
    let make_trigger = trigger_mod.getattr("make_trigger")?;
    let get_proper_type = py.import("mypy.types")?.getattr("get_proper_type")?;
    let ctx = AttrTriggerCtx {
        name,
        refs: &refs,
        make_trigger,
        get_proper_type,
    };
    match attribute_triggers_walk(py, typ, &ctx) {
        Ok(triggers) => Ok(Some(PyList::new(py, &triggers).into())),
        Err(DeferError) => Ok(None),
    }
}

/// The decision body of `DependencyVisitor.attribute_triggers`, recursion
/// included: type-kind dispatch producing member trigger strings.
fn attribute_triggers_walk(
    py: Python<'_>,
    typ: &PyAny,
    ctx: &AttrTriggerCtx<'_>,
) -> Result<Vec<String>, DeferError> {
    let refs = ctx.refs;
    let name = ctx.name;

    // Entry unwraps mirror the Python method exactly: get_proper_type, then
    // a single TypeVarType upper-bound unwrap, then a TupleType
    // partial-fallback unwrap (no re-dispatch on the rebound kinds).
    let mut typ = ctx.proper_type(typ)?;
    if is_instance(typ, refs.type_var_type) {
        let ub = get_attr_or_defer(typ, "upper_bound")?;
        typ = ctx.proper_type(ub)?;
    }
    if is_instance(typ, refs.tuple_type) {
        typ = get_attr_or_defer(typ, "partial_fallback")?;
    }

    if is_instance(typ, refs.instance) {
        let info = get_attr_or_defer(typ, "type")?;
        let fullname = get_str_attr_or_defer(py, info, "fullname")?;
        let member = format!("{fullname}.{name}");
        return Ok(vec![ctx.make_trigger_str(&member)?]);
    }

    if is_instance(typ, refs.function_like) {
        // Python: `elif isinstance(typ, FunctionLike) and typ.is_type_obj():`.
        // is_type_obj()/type_object() are Python method calls on the live
        // object; a raise defers and the Python fallback re-raises it.
        let is_tobj = typ.call_method0("is_type_obj").map_err(|_| DeferError)?;
        if is_tobj.is_true().map_err(|_| DeferError)? {
            let tinfo = typ.call_method0("type_object").map_err(|_| DeferError)?;
            let fullname = get_str_attr_or_defer(py, tinfo, "fullname")?;
            let member = format!("{fullname}.{name}");
            let mut triggers = vec![ctx.make_trigger_str(&member)?];
            let fb = get_attr_or_defer(typ, "fallback")?;
            triggers.extend(attribute_triggers_walk(py, fb, ctx)?);
            return Ok(triggers);
        }
        // FunctionLike but not a type object falls through the Python
        // elif-chain to the empty tail.
        return Ok(Vec::new());
    }

    if is_instance(typ, refs.union_type) {
        let items = get_attr_or_defer(typ, "items")?;
        let mut out: Vec<String> = Vec::new();
        for item in iter_seq(items)? {
            out.extend(attribute_triggers_walk(py, item, ctx)?);
        }
        return Ok(out);
    }

    if is_instance(typ, refs.type_type) {
        let item = get_attr_or_defer(typ, "item")?;
        let mut triggers = attribute_triggers_walk(py, item, ctx)?;
        if is_instance(item, refs.instance) {
            let itype = get_attr_or_defer(item, "type")?;
            let mt = itype.getattr("metaclass_type").map_err(|_| DeferError)?;
            if !mt.is_none() {
                let mt_type = get_attr_or_defer(mt, "type")?;
                let mt_fullname = get_str_attr_or_defer(py, mt_type, "fullname")?;
                let member = format!("{mt_fullname}.{name}");
                triggers.push(ctx.make_trigger_str(&member)?);
            }
        }
        return Ok(triggers);
    }

    Ok(Vec::new())
}

// ====================================================================
// M354: Pure server trigger/target computation
// ====================================================================

// These functions take dependency records (triggers -> target sets) and sets of
// fully-qualified names as pure inputs and return the computed results. They do
// NOT touch live AST objects, symbol tables, or any mutable daemon state.

/// WILDCARD_TAG must match `mypy.server.trigger.WILDCARD_TAG`.
const WILDCARD_TAG: &str = "[wildcard]";

/// Compute the set of trigger strings that a symbol-diff produces.
///
/// This mirrors the core loop of `mypy.server.update.calculate_active_triggers`
/// (lines 794-815): given a set of changed name prefixes and the package
/// nesting level, compute the trigger strings (including wildcard expansions)
/// that would be fired.
///
/// - If a changed item has `count(".") <= nesting_level + 1`, add
///   `{item}{WILDCARD_TAG}` (module-level catch-all for `from m import *`).
/// - If a changed item has `count(".") > nesting_level + 1`, add
///   `{item's parent}{WILDCARD_TAG}` (class-level trigger for protocols).
///
/// Returns `None` if the input lists are empty (Python should handle it).
#[pyfunction]
#[pyo3(name = "rust_compute_wildcard_triggers")]
pub(crate) fn rust_compute_wildcard_triggers(
    changed_names: Vec<String>,
    package_nesting_level: usize,
) -> Option<Vec<String>> {
    if changed_names.is_empty() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for item in &changed_names {
        let dot_count = item.chars().filter(|&c| c == '.').count();
        // Module-level wildcard for "from m import *"
        if dot_count <= package_nesting_level + 1 {
            out.push(format!("{item}{WILDCARD_TAG}"));
        }
        // Class-level wildcard for protocol attribute changes
        if dot_count > package_nesting_level + 1 {
            let parent = item.rsplit_once('.').map(|(p, _)| p).unwrap_or(item);
            out.push(format!("{parent}{WILDCARD_TAG}"));
        }
    }
    Some(out)
}

/// Compute which module IDs need reprocessing given a set of triggered
/// triggers and a dependency map.
///
/// This mirrors the pure BFS core of
/// `mypy.server.update.find_targets_recursive` (lines 920-967). Given:
///   - `triggers`: the set of fired trigger strings (e.g. "<mod.foo>")
///   - `deps`: the dependency map from trigger → set of affected locations
///   - `up_to_date_modules`: modules already processed in this cycle
///   - `module_ids`: the set of all known module IDs in the graph
///
/// Returns a list of module IDs that need reprocessing.
///
/// The algorithm:
/// 1. For each trigger starting with `<`, look up its dependents in `deps`
///    and add them to the worklist (BFS).
/// 2. For each non-trigger target (a bare module or target name):
///    - Skip if it's in `up_to_date_modules`.
///    - Otherwise, resolve it to a module prefix using `module_ids`.
/// 3. Return the set of unique module IDs that need reprocessing.
///
/// Note: `ensure_deps_loaded` and `lookup_target` are NOT ported — they need
/// live AST objects and are handled by the Python caller.
#[pyfunction]
#[pyo3(name = "rust_compute_target_modules")]
pub(crate) fn rust_compute_target_modules(
    triggers: Vec<String>,
    deps: Vec<(String, Vec<String>)>,
    up_to_date_modules: Vec<String>,
    module_ids: Vec<String>,
) -> Vec<String> {
    let _trigger_set: std::collections::HashSet<&str> =
        triggers.iter().map(|s| s.as_str()).collect();
    let up_to_date: std::collections::HashSet<&str> =
        up_to_date_modules.iter().map(|s| s.as_str()).collect();
    let modules: std::collections::HashSet<&str> = module_ids.iter().map(|s| s.as_str()).collect();

    // Build deps map: trigger → set of target strings
    let deps_map: std::collections::HashMap<&str, std::collections::HashSet<&str>> = {
        let mut map = std::collections::HashMap::new();
        for (trigger, targets) in &deps {
            map.insert(
                trigger.as_str(),
                targets.iter().map(|s| s.as_str()).collect(),
            );
        }
        map
    };

    let mut result: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut worklist: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut processed: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Initialize worklist with all triggers
    for trigger in &triggers {
        if processed.insert(trigger.clone()) {
            worklist.push_back(trigger.clone());
        }
    }

    // BFS over triggers → targets → more triggers
    while let Some(target) = worklist.pop_front() {
        if target.starts_with('<') {
            // It's a trigger — look up dependents
            if let Some(targets) = deps_map.get(target.as_str()) {
                for dep in targets {
                    let dep_str = dep.to_string();
                    if processed.insert(dep_str.clone()) {
                        worklist.push_back(dep_str);
                    }
                }
            }
        } else {
            // It's a target name — resolve to module prefix
            let module_id = compute_module_prefix(&modules, &target);
            if let Some(mid) = module_id {
                if !up_to_date.contains(mid.as_str()) {
                    result.insert(mid);
                }
            }
        }
    }

    // Collect into sorted list for determinism
    let mut result_vec: Vec<String> = result.into_iter().collect();
    result_vec.sort();
    result_vec
}

/// Resolve a target name to a module prefix given the set of known module IDs.
///
/// Mirrors `mypy.util.module_prefix` / `split_target`. Given a target like
/// "mod.foo.Bar", returns "mod" if "mod" is a known module. Returns None if
/// the target cannot be resolved to a known module.
fn compute_module_prefix(
    modules: &std::collections::HashSet<&str>,
    target: &str,
) -> Option<String> {
    let mut current = target;
    while !current.is_empty() {
        if modules.contains(current) {
            return Some(current.to_string());
        }
        match current.rsplit_once('.') {
            Some((parent, _)) => current = parent,
            None => break,
        }
    }
    None
}

// M388: Pure server update helpers (mypy/server/update.py).
// These are pure data transformations — set dedup, dict construction,
// regex string extraction — that have no dependency on live AST objects.

/// Deduplicate a list of (module_id, path) tuples by module_id.
///
/// Mirrors `mypy.server.update:dedupe_modules` (line 744).
#[pyfunction]
pub fn rust_dedupe_modules(modules: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut result: Vec<(String, String)> = Vec::new();
    for (id, path) in modules {
        if seen.insert(id.clone()) {
            result.push((id, path));
        }
    }
    result
}

/// Build a module-to-path mapping from a graph dict.
///
/// Mirrors `mypy.server.update:get_module_to_path_map` (line 754).
/// The Python `graph` is passed as a PyDict mapping module IDs to State objects.
#[pyfunction]
pub fn rust_get_module_to_path_map(
    _py: Python<'_>,
    graph: &PyDict,
) -> PyResult<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(graph.len());
    for (module, node) in graph.iter() {
        let module_id = module
            .downcast::<PyString>()?
            .to_str()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("graph key is not a string"))?;
        let xpath = node.getattr("xpath")?.extract::<String>().map_err(|_| {
            pyo3::exceptions::PyAttributeError::new_err("node has no xpath attribute")
        })?;
        pairs.push((module_id.to_string(), xpath));
    }
    Ok(pairs)
}

/// Build a list of BuildSource objects from already-filtered changed-module info.
///
/// Mirrors `mypy.server.update:get_sources` (line 758).
/// Takes a pre-filtered list of (module_id, path) pairs where the caller
/// has already done `fscache.isfile(path)`. Constructs
/// `BuildSource(path, id, None, followed=followed)` via FFI.
#[pyfunction]
pub fn rust_get_sources(
    py: Python<'_>,
    changed_modules: Vec<(String, String)>,
    followed: bool,
) -> PyResult<Vec<PyObject>> {
    let build_source = py.import("mypy.build")?.getattr("BuildSource")?;
    let none = py.None();
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("followed", followed)?;
    let mut sources: Vec<PyObject> = Vec::new();
    for (id, path) in changed_modules {
        let src = build_source.call((&path, &id, &none), Some(kwargs))?;
        sources.push(src.into());
    }
    Ok(sources)
}

/// Extract the file name prefix from a mypy message string.
///
/// Mirrors `mypy.server.update:extract_fnam_from_message` (line 1324).
/// Matches `"module.py:123: error: ..."` → `"module.py"`.
#[pyfunction]
pub fn rust_extract_fnam_from_message(message: String) -> Option<String> {
    // Manual regex match: prefix until `:`, then digits, then `: error|note: `
    let mut ch = message.chars().peekable();
    let mut prefix = String::new();
    while let Some(&c) = ch.peek() {
        if c == ':' {
            break;
        }
        prefix.push(c);
        ch.next();
    }
    if ch.next() != Some(':') {
        return None;
    }
    // Check for digits
    let mut has_digit = false;
    while let Some(&c) = ch.peek() {
        if c.is_ascii_digit() {
            has_digit = true;
            ch.next();
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }
    if ch.next() != Some(':') {
        return None;
    }
    let rest: String = ch.by_ref().collect();
    if rest.starts_with(" error: ") || rest.starts_with(" note: ") {
        Some(prefix)
    } else {
        None
    }
}

/// Extract possible file name prefix from a message.
///
/// Mirrors `mypy.server.update:extract_possible_fnam_from_message` (line 1331).
/// Returns everything before the first `:` (may include non-path content).
#[pyfunction]
pub fn rust_extract_possible_fnam_from_message(message: String) -> String {
    match message.split_once(':') {
        Some((prefix, _)) => prefix.to_string(),
        None => message,
    }
}

/// Sort messages so file order is preserved.
///
/// Mirrors `mypy.server.update:sort_messages_preserving_file_order` (line 1336).
/// Groups messages by file prefix and reorders groups to match prev_messages order.
#[pyfunction]
pub fn rust_sort_messages_preserving_file_order(
    messages: Vec<String>,
    prev_messages: Vec<String>,
) -> Vec<String> {
    // Phase 1: build file order from prev_messages
    let mut order: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut n: usize = 0;
    for msg in &prev_messages {
        let fnam = rust_extract_fnam_from_message(msg.clone());
        if let Some(f) = fnam {
            if let std::collections::hash_map::Entry::Vacant(e) = order.entry(f) {
                e.insert(n);
                n += 1;
            }
        }
    }

    // Phase 2: group messages
    let mut groups: Vec<(Option<usize>, Vec<String>)> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];
        let maybe_fnam = rust_extract_possible_fnam_from_message(msg.clone());
        let mut group = vec![msg.clone()];
        let mut group_key = None;

        if let Some(&key) = order.get(&maybe_fnam) {
            // This looks like a file name. Collect all lines related to this message.
            group_key = Some(key);
            while i + 1 < messages.len() {
                let next = &messages[i + 1];
                if !order.contains_key(&rust_extract_possible_fnam_from_message(next.clone()))
                    && rust_extract_fnam_from_message(next.clone()).is_none()
                    && !next.starts_with("mypy: ")
                {
                    i += 1;
                    group.push(next.clone());
                } else {
                    break;
                }
            }
        }

        groups.push((group_key, group));
        i += 1;
    }

    // Phase 3: sort groups by file order, then flatten
    groups.sort_by_key(|g| g.0.unwrap_or(n));
    groups.into_iter().flat_map(|(_, g)| g).collect()
}

/// True when an IndexExpr is a `Literal[...]` reference, or a NameExpr
/// bound to a TypeAlias whose target resolves to a LiteralType.
///
/// Mirrors `mypy.checkexpr:is_expr_literal_type` (line 8335). Reads
/// live AST nodes via FFI. Returns None when the shape doesn't fit the
/// fast path so Python keeps its own branch behaviour.
#[pyfunction]
pub fn rust_is_expr_literal_type(py: Python<'_>, node: &PyAny) -> PyResult<Option<bool>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let index_expr_cls = nodes_mod.getattr("IndexExpr")?.downcast::<PyType>()?;
    let name_expr_cls = nodes_mod.getattr("NameExpr")?.downcast::<PyType>()?;
    let ref_expr_cls = nodes_mod.getattr("RefExpr")?.downcast::<PyType>()?;
    let type_alias_cls = nodes_mod.getattr("TypeAlias")?.downcast::<PyType>()?;
    let types_mod = py.import("mypy.types")?;
    let literal_type_cls = types_mod.getattr("LiteralType")?.downcast::<PyType>()?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;

    if node.is_instance(index_expr_cls)? {
        let base = node.getattr("base")?;
        if base.is_instance(ref_expr_cls)? {
            let fullname: String = base.getattr("fullname")?.extract()?;
            return Ok(Some(matches!(
                fullname.as_str(),
                "typing.Literal" | "typing_extensions.Literal" | "mypy_extensions.Literal"
            )));
        }
        return Ok(Some(false));
    }
    if node.is_instance(name_expr_cls)? {
        let underlying = node.getattr("node")?;
        if !underlying.is_none() && underlying.is_instance(type_alias_cls)? {
            let target = underlying.getattr("target")?;
            let pt = get_proper_type.call1((target,))?;
            return Ok(Some(pt.is_instance(literal_type_cls)?));
        }
        return Ok(Some(false));
    }
    Ok(Some(false))
}

/// Mirrors `mypy.checkexpr.get_partial_instance_type` (checkexpr.py:8411).
/// Returns the node itself when it is a `mypy.types.PartialType` with a
/// non-None `type` attribute; returns None otherwise. Live-object seam.
#[pyfunction]
pub fn rust_get_partial_instance_type<'py>(
    py: Python<'py>,
    node: &'py PyAny,
) -> PyResult<Option<&'py PyAny>> {
    let partial_cls = py
        .import("mypy.types")?
        .getattr("PartialType")?
        .downcast::<PyType>()?;
    if node.is_instance(partial_cls)? && !node.getattr("type")?.is_none() {
        return Ok(Some(node));
    }
    Ok(None)
}

/// Compare two symbol-table snapshots and return fully-qualified diff names.
///
/// Mirrors `mypy.server.astdiff:compare_symbol_table_snapshots` (line 123).
/// Pure over the plain snapshot representation (tuples, dicts, primitives):
/// for each name in symmetric difference, emit `prefix.name`. For names in
/// both, compare tuples; kind-mismatch emits the name; kind "TypeInfo"
/// compares everything except the trailing dict, then recurses into the
/// nested dict with the extended prefix.
#[pyfunction]
pub fn rust_compare_symbol_table_snapshots(
    name_prefix: &str,
    snapshot1: &PyDict,
    snapshot2: &PyDict,
) -> PyResult<HashSet<String>> {
    let mut triggers: HashSet<String> = HashSet::new();
    let prefix_dot = format!("{}.", name_prefix);

    let mut names1: std::collections::HashSet<String> = std::collections::HashSet::new();
    for k in snapshot1.keys().iter() {
        names1.insert(k.extract::<String>()?);
    }
    let mut names2: std::collections::HashSet<String> = std::collections::HashSet::new();
    for k in snapshot2.keys().iter() {
        names2.insert(k.extract::<String>()?);
    }

    for n in names1.symmetric_difference(&names2) {
        triggers.insert(format!("{}{}", prefix_dot, n));
    }

    for name in names1.intersection(&names2) {
        let item1 = snapshot1.get_item(name)?.unwrap();
        let item2 = snapshot2.get_item(name)?.unwrap();
        let item_name = format!("{}{}", prefix_dot, name);
        let tuple1 = item1.downcast::<PyTuple>()?;
        let tuple2 = item2.downcast::<PyTuple>()?;
        let len1 = tuple1.len();
        let len2 = tuple2.len();
        if len1 == 0 || len2 == 0 {
            triggers.insert(item_name);
            continue;
        }
        let kind1: String = tuple1.get_item(0)?.extract()?;
        let kind2: String = tuple2.get_item(0)?.extract()?;
        if kind1 != kind2 {
            triggers.insert(item_name);
            continue;
        }
        if kind1 == "TypeInfo" {
            let mut all_equal = true;
            // Mirrors item1[:-1] != item2[:-1]: compare the trailing-excluded
            // head — indices 1..len-1 (skip kind tag and nested-dict tail).
            let head_len = len1.min(len2).saturating_sub(2);
            for i in 0..head_len {
                if !tuple1.get_item(i + 1)?.eq(tuple2.get_item(i + 1)?)? {
                    all_equal = false;
                    break;
                }
            }
            if !all_equal || len1 != len2 {
                triggers.insert(item_name.clone());
            }
            let nested1 = tuple1.get_item(len1 - 1)?.downcast::<PyDict>()?;
            let nested2 = tuple2.get_item(len2 - 1)?.downcast::<PyDict>()?;
            let sub = rust_compare_symbol_table_snapshots(&item_name, nested1, nested2)?;
            triggers.extend(sub);
        } else {
            if !tuple1.eq(tuple2)? {
                triggers.insert(item_name);
            }
        }
    }

    Ok(triggers)
}

/// Bases of a class excluding builtins.object and the class itself.
///
/// Mirrors `mypy.server.deps:non_trivial_bases` (line 1222). Walks
/// `info.mro[1:]`, returns bases whose fullname is not "builtins.object".
/// Returns None if any MRO entry lacks fullname (partial init).
#[pyfunction]
pub fn rust_non_trivial_bases(py: Python<'_>, info: &PyAny) -> PyResult<Vec<PyObject>> {
    let mro = info.getattr("mro")?.downcast::<PyList>()?;
    let mut out: Vec<PyObject> = Vec::new();
    for (i, base) in mro.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let fullname: String = base.getattr("fullname")?.extract()?;
        if fullname != "builtins.object" {
            out.push(base.into());
        }
    }
    let _ = py;
    Ok(out)
}

/// True if the class has any base outside builtins/typing/enum.
///
/// Mirrors `mypy.server.deps:has_user_bases` (line 1226).
#[pyfunction]
pub fn rust_has_user_bases(py: Python<'_>, info: &PyAny) -> PyResult<bool> {
    let mro = info.getattr("mro")?.downcast::<PyList>()?;
    for (i, base) in mro.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let module: String = base.getattr("module_name")?.extract()?;
        if module != "builtins" && module != "typing" && module != "enum" {
            return Ok(true);
        }
    }
    let _ = py;
    Ok(false)
}

/// Merge new dependency triggers into an existing deps map in place.
///
/// Mirrors `mypy.server.deps:merge_dependencies` (line 1208):
/// `deps.trigger |= new_deps.trigger` for every key in new_deps.
/// Mutates the passed dict directly.
#[pyfunction]
pub fn rust_merge_dependencies(new_deps: &PyDict, deps: &PyDict) -> PyResult<()> {
    for (trigger, targets) in new_deps.iter() {
        let trigger_str = trigger.downcast::<PyString>()?.to_str()?.to_string();
        let target_set: std::collections::HashSet<String> = targets.extract()?;
        if let Some(existing) = deps.get_item(&trigger_str)? {
            let existing_set: &PySet = existing.downcast::<PySet>()?;
            for t in target_set {
                existing_set.add(PyString::new(trigger.py(), &t))?;
            }
        } else {
            let py_set = PySet::new(trigger.py(), target_set.iter().collect::<Vec<_>>())?;
            deps.set_item(&trigger_str, py_set)?;
        }
    }
    Ok(())
}

/// Return the target name corresponding to a deferred node.
///
/// Mirrors `mypy.server.update:target_from_node` (line 1301). For a
/// MypyFile, returns the module iff node.fullname matches; else None.
/// For FuncDef/OverloadedFuncDef, returns info.fullname.name or
/// module.name. Returns None when the node is not a valid target.
#[pyfunction]
pub fn rust_target_from_node(
    py: Python<'_>,
    module: &str,
    node: &PyAny,
) -> PyResult<Option<String>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let mypy_file_cls = nodes_mod.getattr("MypyFile")?.downcast::<PyType>()?;
    let node_fullname = node.getattr("fullname")?.extract::<String>()?;

    if node.is_instance(mypy_file_cls)? {
        if module == node_fullname {
            return Ok(Some(module.to_string()));
        }
        return Ok(None);
    }

    // OverloadedFuncDef or FuncDef
    let info = node.getattr("info")?;
    let node_name = node.getattr("name")?.extract::<String>()?;
    if !info.is_none() {
        let info_fullname = info.getattr("fullname")?.extract::<String>()?;
        return Ok(Some(format!("{}.{}", info_fullname, node_name)));
    }
    Ok(Some(format!("{}.{}", module, node_name)))
}

/// Find all deps of initial modules that have not had their tree loaded.
///
/// Mirrors `mypy.server.update:find_unloaded_deps` (line 504). Walks a
/// LIFO worklist of module IDs; a module is unloaded iff it is in the
/// graph but not in `loaded`. Preserves the push/pop traversal order.
/// Returns None if any initial module is missing from the graph.
#[pyfunction]
pub fn rust_find_unloaded_deps(
    initial: Vec<String>,
    graph: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    loaded: std::collections::HashSet<String>,
) -> Option<Vec<String>> {
    let mut worklist: Vec<String> = initial;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unloaded: Vec<String> = Vec::new();
    while let Some(node) = worklist.pop() {
        if seen.contains(&node) {
            continue;
        }
        let entry = match graph.get(&node) {
            Some(e) => e,
            None => continue,
        };
        seen.insert(node.clone());
        if !loaded.contains(&node) {
            // worklist.extend(dependencies + ancestors)
            for dep in entry.0.iter().chain(entry.1.iter()) {
                worklist.push(dep.clone());
            }
            unloaded.push(node);
        }
    }
    Some(unloaded)
}

/// Find a module in a list that directly imports no other module in the list.
///
/// Mirrors `mypy.server.update:find_relative_leaf_module` (line 723).
/// Takes a list of (module, path) tuples and a mapping module -> dependency
/// module IDs. Returns the first (sorted) module with no intra-list deps,
/// or the lexicographically first module if none qualifies.
#[pyfunction]
pub fn rust_find_relative_leaf_module(
    modules: Vec<(String, String)>,
    deps: std::collections::HashMap<String, Vec<String>>,
) -> Option<(String, String)> {
    if modules.is_empty() {
        return None;
    }
    let mut sorted = modules;
    sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let module_set: std::collections::HashSet<&str> =
        sorted.iter().map(|(m, _)| m.as_str()).collect();
    for (module, path) in &sorted {
        let state_deps = deps.get(module.as_str());
        let has_intra = match state_deps {
            Some(dlist) => dlist.iter().any(|d| module_set.contains(d.as_str())),
            None => false,
        };
        if !has_intra {
            return Some((module.clone(), path.clone()));
        }
    }
    Some(sorted[0].clone())
}

// ====================================================================
// Issue #389: dmypy_server pure helpers — plain-record shuffling
// ====================================================================

//
// These functions port the pure (stateless) helpers from
// `mypy.dmypy_server`. They take plain Python objects (dicts, lists,

// strings, tuples) and return plain Python objects.  Each function
// returns `None` when it cannot handle the input so the Python caller
// falls back to the pure-Python implementation — the strangler-fig

// per-call gate.

/// Port of `mypy.dmypy_server.process_start_options`.
///
/// Parses CLI flags into `Options` via `mypy.main.process_options`,
/// then applies dmypy-specific validation and mutations.
/// Returns `None` if the options object is not recognized.
#[pyfunction]
pub(crate) fn rust_process_start_options(
    py: Python<'_>,
    flags: Vec<String>,
    allow_sources: bool,
) -> PyResult<Option<PyObject>> {
    let _ = allow_sources; // unused in the pure path
                           // Call mypy.main.process_options(["-i"] + flags, ...)
    let main_mod = py.import("mypy.main")?;

    // Build the args tuple: ("-i",) + tuple(flags)
    let py_i = PyString::new(py, "-i");
    let flags_strings: Vec<PyObject> = flags.iter().map(|s| PyString::new(py, s).into()).collect();
    let args_vec: Vec<PyObject> = std::iter::once(py_i.into()).chain(flags_strings).collect();
    let all_args_tuple = PyTuple::new(py, &args_vec);

    // Build kwargs dict and call
    let kwargs = PyDict::new(py);
    kwargs.set_item("require_targets", true)?;
    kwargs.set_item("server_options", true)?;

    let result = main_mod.call_method("process_options", (all_args_tuple,), Some(kwargs))?;

    // result is a tuple (_, options)
    let options = result.get_item(1)?;

    // Apply dmypy-specific mutations via Python side effects
    // if options.report_dirs: print(...)
    if let Ok(report_dirs) = options.getattr("report_dirs") {
        if !report_dirs.is_none() && report_dirs.is_true()? {
            let sys_mod = py.import("sys")?;
            let stdout = sys_mod.getattr("stdout")?;
            let msg = "dmypy: Ignoring report generation settings. Start/restart cannot generate reports.";
            stdout.call_method1("write", (msg,))?;
            stdout.call_method0("flush")?;
        }
    }

    // if options.junit_xml: print(...) then set to None
    if let Ok(junit_xml) = options.getattr("junit_xml") {
        if !junit_xml.is_none() && junit_xml.is_true()? {
            let sys_mod = py.import("sys")?;
            let stdout = sys_mod.getattr("stdout")?;
            let msg = "dmypy: Ignoring report generation settings. Start/restart does not support --junit-xml. Pass it to check/recheck instead";
            stdout.call_method1("write", (msg,))?;
            stdout.call_method0("flush")?;
            options.setattr("junit_xml", py.None())?;
        }
    }

    // if not options.incremental: sys.exit(...)
    if let Ok(incremental) = options.getattr("incremental") {
        if !incremental.is_true()? {
            let sys_mod = py.import("sys")?;
            sys_mod.call_method1(
                "exit",
                ("dmypy: start/restart should not disable incremental mode",),
            )?;
        }
    }

    // if options.follow_imports not in ("skip", "error", "normal"): sys.exit(...)
    if let Ok(follow_imports) = options.getattr("follow_imports") {
        if let Ok(fi_str) = follow_imports.extract::<String>() {
            if !["skip", "error", "normal"].contains(&fi_str.as_str()) {
                let sys_mod = py.import("sys")?;
                sys_mod.call_method1("exit", ("dmypy: follow-imports=silent not supported",))?;
            }
        }
    }

    // if not options.local_partial_types: sys.exit(...)
    if let Ok(local_partial) = options.getattr("local_partial_types") {
        if !local_partial.is_true()? {
            let sys_mod = py.import("sys")?;
            sys_mod.call_method1(
                "exit",
                ("dmypy: disabling local-partial-types not supported",),
            )?;
        }
    }

    Ok(Some(options.to_object(py)))
}

/// Port of `mypy.dmypy_server.ignore_suppressed_imports`.
///
/// Returns `Some(true)` if the module is an `encodings.*` submodule,
/// `Some(false)` otherwise. Always returns `Some` because this
/// function is trivially pure.
#[pyfunction]
pub(crate) fn rust_ignore_suppressed_imports(module: &PyString) -> Option<bool> {
    Some(module.to_str().unwrap_or("").starts_with("encodings."))
}

/// Port of `mypy.dmypy_server.get_meminfo`.
///
/// Collects memory usage info from `psutil` or `resource`.
/// Returns `None` if neither is importable (shouldn't happen).
#[pyfunction]
pub(crate) fn rust_get_meminfo(py: Python<'_>) -> PyResult<Option<PyObject>> {
    let res_dict = PyDict::new(py);

    // Try psutil first
    let psutil_result = py.import("psutil");
    match psutil_result {
        Ok(psutil_mod) => {
            let process_cls = psutil_mod.getattr("Process")?;
            let process = process_cls.call0()?;
            let meminfo = process.call_method0("memory_info")?;

            let rss: f64 = meminfo.getattr("rss")?.extract()?;
            let vms: f64 = meminfo.getattr("vms")?.extract()?;
            let mib = 2f64.powf(20.0);

            res_dict.set_item("memory_rss_mib", rss / mib)?;
            res_dict.set_item("memory_vms_mib", vms / mib)?;

            // Platform-specific peak RSS
            let platform: String = py.import("sys")?.getattr("platform")?.extract()?;
            if platform == "win32" {
                let peak_wset: f64 = meminfo.getattr("peak_wset")?.extract()?;
                res_dict.set_item("memory_maxrss_mib", peak_wset / mib)?;
            } else {
                let resource_mod = py.import("resource")?;
                // RUSAGE_SELF is an int (usually 0); fetch it, don't pass the
                // literal string. call_method1 unpacked ("RUSAGE_SELF",) as a
                // positional arg, so CPython got who="RUSAGE_SELF" -> TypeError.
                let rusage_self = resource_mod.getattr("RUSAGE_SELF")?;
                let rusage = resource_mod.call_method1("getrusage", (rusage_self,))?;
                let ru_maxrss: i64 = rusage.getattr("ru_maxrss")?.extract()?;
                let factor: i64 = if platform == "darwin" { 1 } else { 1024 };
                res_dict.set_item(
                    "memory_maxrss_mib",
                    (ru_maxrss as f64) * (factor as f64) / mib,
                )?;
            }
        }
        Err(_) => {
            res_dict.set_item(
                "memory_psutil_missing",
                "psutil not found, run pip install mypy[dmypy] to install the needed components for dmypy",
            )?;
        }
    }

    Ok(Some(res_dict.into()))
}

/// Port of `Server._response_metadata`.
///
/// Builds a small dict with platform and python_version from the
/// options object. Returns `None` if options lacks the expected
/// attributes (never happens in practice).
#[pyfunction]
pub(crate) fn rust_response_metadata(
    py: Python<'_>,
    options: &PyAny,
) -> PyResult<Option<PyObject>> {
    let py_ver = match options.getattr("python_version") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let platform = match options.getattr("platform") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let major = match py_ver.get_item(0) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let minor = match py_ver.get_item(1) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let py_ver_str = format!(
        "{}_{}",
        major.str()?.to_str().unwrap_or("?"),
        minor.str()?.to_str().unwrap_or("?")
    );

    let result = PyDict::new(py);
    result.set_item("platform", platform)?;
    result.set_item("python_version", py_ver_str)?;
    Ok(Some(result.into()))
}

/// Port of `mypy.dmypy_server.find_all_sources_in_build`.
///
/// Given a build graph (`dict[str] -> BuildState`) and an optional
/// sequence of extra `BuildSource`s, returns a list of all `BuildSource`
/// objects. Returns `None` if the graph structure is unexpected.
#[pyfunction]
pub(crate) fn rust_find_all_sources_in_build(
    py: Python<'_>,
    graph: &PyDict,
    extra: &PyAny,
) -> PyResult<Option<PyObject>> {
    let build_source_cls = match py.import("mypy.build")?.getattr("BuildSource") {
        Ok(cls) => cls,
        Err(_) => return Ok(None),
    };

    // Start with extra sources
    let mut result: Vec<PyObject> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for item in extra.iter()? {
        let item = item?;
        let module = match item.getattr("module") {
            Ok(v) => match v.extract::<String>() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        seen.insert(module);
        result.push(item.into());
    }

    // Walk graph
    for (key, value) in graph.iter() {
        let module: String = match key.extract() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if seen.contains(&module) {
            continue;
        }
        seen.insert(module.clone());

        let path = match value.getattr("path") {
            Ok(v) => match v.extract::<String>() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let bs = match build_source_cls.call1((&path, &module)) {
            Ok(bs) => bs,
            Err(_) => continue,
        };
        result.push(bs.into());
    }

    Ok(Some(PyList::new(py, &result).into()))
}

/// Port of `mypy.dmypy_server.add_all_sources_to_changed`.
///
/// Appends sources without paths or already-seen paths to the
/// `changed` list in place. Returns `None` to signal Python should
/// fall back (shouldn't happen for normal inputs).
#[pyfunction]
pub(crate) fn rust_add_all_sources_to_changed(
    py: Python<'_>,
    sources: &PyAny,
    changed: &PyAny,
) -> PyResult<Option<()>> {
    // Build changed_set
    let mut changed_set: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for item in changed.iter()? {
        let item = item?;
        let (mod_name, path): (String, String) = match item.extract() {
            Ok(t) => t,
            Err(_) => continue,
        };
        changed_set.insert((mod_name, path));
    }

    // Build new items
    let mut new_items: Vec<PyObject> = Vec::new();
    for source in sources.iter()? {
        let source = source?;
        let path = match source.getattr("path") {
            Ok(v) => match v.extract::<String>() {
                Ok(s) if !s.is_empty() => s,
                _ => continue,
            },
            Err(_) => continue,
        };
        let mod_name: String = match source.getattr("module") {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        if !changed_set.contains(&(mod_name.clone(), path.clone())) {
            changed_set.insert((mod_name.clone(), path.clone()));
            let py_pair: PyObject = (mod_name, path).into_py(py);
            new_items.push(py_pair);
        }
    }

    // Extend changed in place
    if let Ok(extend_fn) = changed.getattr("extend") {
        let new_list = PyList::new(py, &new_items);
        extend_fn.call1((new_list,))?;
    }

    Ok(Some(()))
}

/// Port of `mypy.dmypy_server.fix_module_deps`.
///
/// Re-assigns `state.dependencies`, `state.dependencies_set`,
/// `state.suppressed`, and `state.suppressed_set` based on whether
/// each dep exists in the graph. Returns `None` if the graph
/// structure is unexpected.
#[pyfunction]
pub(crate) fn rust_fix_module_deps(py: Python<'_>, graph: &PyDict) -> PyResult<Option<()>> {
    // Collect valid module keys
    let valid_modules: std::collections::HashSet<String> = graph
        .keys()
        .into_iter()
        .filter_map(|k| k.extract().ok())
        .collect();

    for (_key, value) in graph.iter() {
        // Get dependencies
        let dependencies: Vec<String> = match value.getattr("dependencies") {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        // Get suppressed
        let suppressed: Vec<String> = match value.getattr("suppressed") {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        // Classify
        let mut new_dependencies: Vec<String> = Vec::new();
        let mut new_suppressed: Vec<String> = Vec::new();

        for dep in dependencies.iter().chain(suppressed.iter()) {
            if valid_modules.contains(dep) {
                new_dependencies.push(dep.clone());
            } else {
                new_suppressed.push(dep.clone());
            }
        }

        // Set dependencies and dependencies_set
        value.setattr("dependencies", PyList::new(py, &new_dependencies))?;
        value.setattr("dependencies_set", PySet::new(py, &new_dependencies)?)?;

        // Set suppressed and suppressed_set
        value.setattr("suppressed", PyList::new(py, &new_suppressed))?;
        value.setattr("suppressed_set", PySet::new(py, &new_suppressed)?)?;
    }

    Ok(Some(()))
}

/// Port of `mypy.dmypy_server.filter_out_missing_top_level_packages`.
///
/// Given a set of package names and a `SearchPaths` / `FileSystemCache`,
/// returns the subset of packages that have entries on disk.
/// Returns `None` if the expected attributes are missing.
#[pyfunction]
pub(crate) fn rust_filter_out_missing_top_level_packages(
    py: Python<'_>,
    packages: &PyAny,
    search_paths: &PyAny,
    fscache: &PyAny,
) -> PyResult<Option<PyObject>> {
    // Extract paths from SearchPaths
    let path_attrs = ["python_path", "mypy_path", "package_path", "typeshed_path"];
    let mut all_paths: Vec<String> = Vec::new();

    for attr in &path_attrs {
        if let Ok(val) = search_paths.getattr(*attr) {
            if let Ok(paths) = val.extract::<Vec<String>>() {
                all_paths.extend(paths);
            }
        }
    }

    // Collect entries from each path
    let mut found: std::collections::HashSet<String> = std::collections::HashSet::new();
    let packages_set: std::collections::HashSet<String> = packages
        .iter()?
        .filter_map(|item| item.ok().and_then(|i| i.extract().ok()))
        .collect();

    for p in &all_paths {
        let entries: Vec<String> = match fscache.call_method1("listdir", (p,)) {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        for entry in &entries {
            let cleaned = if entry.ends_with(".py") {
                entry[..entry.len() - 3].to_string()
            } else if entry.ends_with(".pyi") {
                entry[..entry.len() - 4].to_string()
            } else if entry.ends_with("-stubs") {
                entry[..entry.len() - 6].to_string()
            } else {
                entry.clone()
            };

            if packages_set.contains(&cleaned) {
                found.insert(cleaned);
            }
        }
    }

    let pkg_vec: Vec<&str> = found.iter().map(|s| s.as_str()).collect();
    Ok(Some(PySet::new(py, &pkg_vec)?.into()))
}

// --- Helpers (for type-trigging; reused by serverdeps tests) ---

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

    // M354: pure computation tests
    #[test]
    fn wildcard_empty_input_returns_none() {
        assert!(crate::serverdeps::rust_compute_wildcard_triggers(vec![], 0).is_none());
    }

    #[test]
    fn wildcard_single_name_level_0() {
        // "foo" has 0 dots, level=0, so 0 <= 0+1 → wildcard
        let result = crate::serverdeps::rust_compute_wildcard_triggers(vec!["foo".to_string()], 0);
        assert!(result.is_some());
        let triggers = result.unwrap();
        assert!(triggers.contains(&"foo[wildcard]".to_string()));
    }

    #[test]
    fn wildcard_nested_name_class_level() {
        // "mod.foo.Bar" has 2 dots, level=0, 2 > 0+1 → parent wildcard
        let result =
            crate::serverdeps::rust_compute_wildcard_triggers(vec!["mod.foo.Bar".to_string()], 0);
        let triggers = result.unwrap();
        assert!(triggers.contains(&"mod.foo[wildcard]".to_string()));
    }

    #[test]
    fn wildcard_module_level_with_class() {
        // "mod.foo" has 1 dot, level=0, 1 <= 0+1 → module wildcard
        // "mod.foo.Bar" has 2 dots, level=0, 2 > 0+1 → parent wildcard
        let result = crate::serverdeps::rust_compute_wildcard_triggers(
            vec!["mod.foo".to_string(), "mod.foo.Bar".to_string()],
            0,
        );
        let triggers = result.unwrap();
        assert!(triggers.contains(&"mod.foo[wildcard]".to_string()));
        assert!(triggers.contains(&"mod.foo[wildcard]".to_string()));
        assert_eq!(triggers.len(), 2);
    }

    #[test]
    fn target_modules_basic_bfs() {
        // Simple trigger graph: <A> -> B, <B> -> <C>, <C> -> D
        let deps = vec![
            ("<A>".to_string(), vec!["B".to_string()]),
            ("<B>".to_string(), vec!["<C>".to_string()]),
            ("<C>".to_string(), vec!["D".to_string()]),
        ];
        let result = crate::serverdeps::rust_compute_target_modules(
            vec!["<A>".to_string()],
            deps,
            vec![],
            vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
            ],
        );
        // BFS: <A>→B. B is a non-trigger target (doesn't start with "<"),
        // so it resolves to module "B" and does NOT expand further.
        // D would only be reached if we followed <B> and <C> as triggers,

        // but B is a bare module name, not a trigger.
        assert_eq!(result, vec!["B"]);
    }

    #[test]
    fn target_modules_skip_up_to_date() {
        let deps = vec![("<A>".to_string(), vec!["B".to_string()])];
        let result = crate::serverdeps::rust_compute_target_modules(
            vec!["<A>".to_string()],
            deps,
            vec!["B".to_string()], // B is already up to date
            vec!["A".to_string(), "B".to_string()],
        );
        assert!(result.is_empty());
    }

    #[test]
    fn target_modules_unresolved_target() {
        let deps = vec![("<A>".to_string(), vec!["unknown.xyz".to_string()])];
        let result = crate::serverdeps::rust_compute_target_modules(
            vec!["<A>".to_string()],
            deps,
            vec![],
            vec!["A".to_string()], // "unknown" is NOT a known module
        );
        assert!(result.is_empty());
    }

    #[test]
    fn target_modules_dedup() {
        let deps = vec![("<A>".to_string(), vec!["B".to_string(), "B".to_string()])];
        let result = crate::serverdeps::rust_compute_target_modules(
            vec!["<A>".to_string()],
            deps,
            vec![],
            vec!["A".to_string(), "B".to_string()],
        );
        assert_eq!(result, vec!["B"]); // deduplicated
    }

    #[test]
    fn module_prefix_simple() {
        let modules: std::collections::HashSet<&str> =
            vec!["mod", "mod.sub", "other"].into_iter().collect();
        assert_eq!(
            crate::serverdeps::compute_module_prefix(&modules, "mod.foo"),
            Some("mod".to_string())
        );
        assert_eq!(
            crate::serverdeps::compute_module_prefix(&modules, "mod.sub.nested.x"),
            Some("mod.sub".to_string())
        );
        assert_eq!(
            crate::serverdeps::compute_module_prefix(&modules, "other"),
            Some("other".to_string())
        );
        assert_eq!(
            crate::serverdeps::compute_module_prefix(&modules, "missing.x.y"),
            None
        );
    }

    #[test]
    fn sort_messages_groups_continuation_header_with_its_file() {
        // A continuation header like `foo/y.py: In function "f":` has no line
        // number (`extract_fnam` returns None) but its prefix `foo/y.py` IS a
        // known file key, so it must be grouped with the `foo/y.py:123`

        // messages that follow it and keep the file's `prev_messages` order.
        let messages: Vec<String> = vec![
            "x.py:1: error: \"int\" not callable".into(),
            "and message continues (x: y)".into(),
            "   1()".into(),
            "   ^~~".into(),
            "foo/y.py: In function \"f\":".into(),
            "foo/y.py:123: note: \"X\" not defined".into(),
            "and again message continues".into(),
        ];
        let prev: Vec<String> = vec![
            "foo/y.py:12: note: \"Y\" not defined".into(),
            "x.py:8: error: \"str\" not callable".into(),
        ];
        let result = rust_sort_messages_preserving_file_order(messages, prev);
        let expected: Vec<String> = vec![
            "foo/y.py: In function \"f\":".into(),
            "foo/y.py:123: note: \"X\" not defined".into(),
            "and again message continues".into(),
            "x.py:1: error: \"int\" not callable".into(),
            "and message continues (x: y)".into(),
            "   1()".into(),
            "   ^~~".into(),
        ];
        assert_eq!(result, expected);
    }

    #[ignore = "requires Python mypy in test environment; see AGENTS.md"]
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
