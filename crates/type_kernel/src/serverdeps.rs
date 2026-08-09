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

// ====================================================================
// M354: Pure server trigger/target computation
// ====================================================================
//
// These functions take dependency records (triggers → target sets) and sets of
// fully-qualified names as pure inputs and return the computed results. They do
// NOT touch live AST objects, symbol tables, or any mutable daemon state. The
// Python caller provides the deps graph and the list of module IDs; Rust returns
// the pure computation result, and Python handles the stateful parts
// (lookup_target, reprocess_nodes, strip_target, etc.).

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
