//! Native port of `mypy.fixup` — `NodeFixer` and `TypeFixer`.
//!
//! Walks live Python `mypy.nodes` AST objects and `mypy.types.Type`
//! objects, resolving serialized cross-references (`type_ref`,
//! `cross_ref`, `_mro_refs`) into live pointers after cache
//! deserialization.
//!
//! Design: the Python `NodeFixer`/`TypeFixer` classes remain the
//! entry points. The Rust `#[pyfunction]`s replace the visitor
//! method bodies and are called through the `native_type_kernel`
//! gate. The Python `accept` dispatch and the `SymbolTableNode.node`
//! property lazy-fixup stay in Python. Only the resolution logic
//! is ported.
//!
//! Entry points:
//!   * `rust_fixup_type` — TypeFixer body. Walks a live `Type`,
//!     resolves `type_ref` to `type`/`alias`, recurses children.
//!   * `rust_fixup_type_info` — NodeFixer `visit_type_info` body.
//!   * `rust_resolve_cross_ref` — NodeFixer `resolve_cross_ref`.
//!   * `rust_fixup_symbol_table` — NodeFixer `visit_symbol_table`.
//!   * `rust_fixup_overloaded_func_def` — NodeFixer visit method.
//!   * `rust_fixup_decorator` — NodeFixer visit method.
//!
//! Target: PyO3 0.20.x (uses `&PyAny`, not `Bound<'_, PyAny>`).

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// Helpers: delegate to Python mypy.fixup / mypy.lookup
// ---------------------------------------------------------------------------

/// Call `mypy.fixup.lookup_fully_qualified_typeinfo(modules, name,
/// allow_missing=...)`. This triggers `stnode.node` which fires the
/// Python NodeFixer lazy-fixup, so the target node is fixed up
/// before we receive it.
fn lookup_typeinfo(
    py: Python<'_>,
    name: &str,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<PyObject> {
    let fixup_mod = py.import("mypy.fixup")?;
    let func = fixup_mod.getattr("lookup_fully_qualified_typeinfo")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("allow_missing", allow_missing)?;
    Ok(func.call((modules, name), Some(kwargs))?.into())
}

/// Call `mypy.fixup.lookup_fully_qualified_alias(modules, name,
/// allow_missing=...)`.
fn lookup_alias(
    py: Python<'_>,
    name: &str,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<PyObject> {
    let fixup_mod = py.import("mypy.fixup")?;
    let func = fixup_mod.getattr("lookup_fully_qualified_alias")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("allow_missing", allow_missing)?;
    Ok(func.call((modules, name), Some(kwargs))?.into())
}

/// Call `mypy.lookup.lookup_fully_qualified(name, modules,
/// raise_on_missing=...)`. Returns `PyObject` (may be None).
fn lookup_fq(
    py: Python<'_>,
    name: &str,
    modules: &PyDict,
    raise_on_missing: bool,
) -> PyResult<PyObject> {
    let lookup_mod = py.import("mypy.lookup")?;
    let func = lookup_mod.getattr("lookup_fully_qualified")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("raise_on_missing", raise_on_missing)?;
    Ok(func.call((name, modules), Some(kwargs))?.into())
}

/// Get the type name of a Python object (e.g. "Instance", "FuncDef").
fn type_name(obj: &PyAny) -> PyResult<String> {
    Ok(obj.get_type().name()?.to_string())
}

/// Check if a `PyObject` is Python `None`.
fn is_py_none(py: Python<'_>, obj: &PyObject) -> bool {
    obj.is_none(py)
}

/// Cache of mypy.types and mypy.nodes class objects for isinstance checks.
struct ClassCache<'py> {
    types_mod: &'py PyModule,
    nodes_mod: &'py PyModule,
}

impl<'py> ClassCache<'py> {
    fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(ClassCache {
            types_mod: py.import("mypy.types")?,
            nodes_mod: py.import("mypy.nodes")?,
        })
    }

    fn is_type(&self, obj: &PyAny, class_name: &str) -> PyResult<bool> {
        let cls = self.types_mod.getattr(class_name)?;
        obj.is_instance(cls)
    }

    fn is_node(&self, obj: &PyAny, class_name: &str) -> PyResult<bool> {
        let cls = self.nodes_mod.getattr(class_name)?;
        obj.is_instance(cls)
    }
}

// ---------------------------------------------------------------------------
// TypeFixer: walk live Type objects, resolve type_ref -> type/alias
// ---------------------------------------------------------------------------

/// `TypeFixer.visit_instance` body: resolve `type_ref` to `type`,
/// fix bases, recurse args/last_known_value/extra_attrs.
fn tf_visit_instance(
    py: Python<'_>,
    inst: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let type_ref = inst.getattr("type_ref")?;
    if type_ref.is_none() {
        return Ok(());
    }
    let type_ref_str: String = type_ref.extract()?;
    // Resolve the type_ref BEFORE clearing it. If the lookup (or any
    // nested lazy-fixup it triggers) raises, the Python fallback in
    // TypeFixer.fixup retries via visit_instance — which checks

    // type_ref and bails if it is already None. Clearing type_ref only
    // after the lookup succeeds prevents the fallback from silently
    // skipping an unfixed Instance (leaving type=NOT_READY).
    let typ = lookup_typeinfo(py, &type_ref_str, modules, allow_missing)?;
    inst.setattr("type_ref", py.None())?;
    inst.setattr("type", typ)?;

    // Also fix up the bases, just in case (Python: if base.type
    // is NOT_READY: base.accept(self)).
    let inst_type = inst.getattr("type")?;
    let bases = inst_type.getattr("bases")?;
    if let Ok(bases_list) = bases.downcast::<PyList>() {
        let not_ready = py.import("mypy.types")?.getattr("NOT_READY")?;
        for base in bases_list.iter() {
            let base_type = base.getattr("type")?;
            if base_type.is(not_ready) {
                fixup_type(py, base, modules, allow_missing, cache)?;
            }
        }
    }

    // Recurse into args (tuple).
    recurse_children(py, inst.getattr("args")?, modules, allow_missing, cache)?;

    // last_known_value
    let lkv = inst.getattr("last_known_value")?;
    if !lkv.is_none() {
        fixup_type(py, lkv, modules, allow_missing, cache)?;
    }

    // extra_attrs
    let extra_attrs = inst.getattr("extra_attrs")?;
    if !extra_attrs.is_none() {
        let attrs = extra_attrs.getattr("attrs")?;
        if let Ok(attrs_dict) = attrs.downcast::<PyDict>() {
            for value in attrs_dict.values() {
                fixup_type(py, value, modules, allow_missing, cache)?;
            }
        }
    }
    Ok(())
}

/// `TypeFixer.visit_type_alias_type` body: resolve `type_ref` to
/// `alias`, recurse args.
fn tf_visit_type_alias_type(
    py: Python<'_>,
    t: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let type_ref = t.getattr("type_ref")?;
    if type_ref.is_none() {
        return Ok(());
    }
    let type_ref_str: String = type_ref.extract()?;
    // Same pattern as tf_visit_instance: resolve before clearing.
    let alias = lookup_alias(py, &type_ref_str, modules, allow_missing)?;
    t.setattr("type_ref", py.None())?;
    t.setattr("alias", alias)?;
    recurse_children(py, t.getattr("args")?, modules, allow_missing, cache)
}

/// `TypeFixer.visit_callable_type` body: recurse fallback,
/// arg_types, ret_type, variables, type_guard, type_is,
/// instance_type.
fn tf_visit_callable_type(
    py: Python<'_>,
    ct: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let fallback = ct.getattr("fallback")?;
    if !fallback.is_none() {
        fixup_type(py, fallback, modules, allow_missing, cache)?;
    }

    let arg_types = ct.getattr("arg_types")?;
    if let Ok(at_list) = arg_types.downcast::<PyList>() {
        for argt in at_list.iter() {
            if !argt.is_none() {
                fixup_type(py, argt, modules, allow_missing, cache)?;
            }
        }
    }

    let ret_type = ct.getattr("ret_type")?;
    if !ret_type.is_none() {
        fixup_type(py, ret_type, modules, allow_missing, cache)?;
    }

    let variables = ct.getattr("variables")?;
    // CallableType.variables is a tuple, not a list, so downcast::<PyList>
    // silently fails and skips TypeVarType fixup (issue #585).
    recurse_children(py, variables, modules, allow_missing, cache)?;

    let type_guard = ct.getattr("type_guard")?;
    if !type_guard.is_none() {
        fixup_type(py, type_guard, modules, allow_missing, cache)?;
    }

    let type_is = ct.getattr("type_is")?;
    if !type_is.is_none() {
        fixup_type(py, type_is, modules, allow_missing, cache)?;
    }

    let instance_type = ct.getattr("instance_type")?;
    if !instance_type.is_none() {
        fixup_type(py, instance_type, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// `TypeFixer.visit_typeddict_type` body: recurse items, fix
/// fallback type_ref if the lookup fails.
fn tf_visit_typeddict_type(
    py: Python<'_>,
    tdt: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let items = tdt.getattr("items")?;
    if let Ok(items_dict) = items.downcast::<PyDict>() {
        for value in items_dict.values() {
            fixup_type(py, value, modules, allow_missing, cache)?;
        }
    }

    let fallback = tdt.getattr("fallback")?;
    if !fallback.is_none() {
        let fb_type_ref = fallback.getattr("type_ref")?;
        if !fb_type_ref.is_none() {
            let fb_ref_str: String = fb_type_ref.extract()?;
            let stnode = lookup_fq(py, &fb_ref_str, modules, !allow_missing)?;
            if is_py_none(py, &stnode) {
                // Reject fake TypeInfos for TypedDict fallbacks.
                fallback.setattr("type_ref", "typing._TypedDict")?;
            }
        }
        fixup_type(py, fallback, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// Dispatch a live `Type` through the TypeFixer logic. Returns
/// `true` if handled, `false` to defer to Python.
fn fixup_type(
    py: Python<'_>,
    obj: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<bool> {
    if cache.is_type(obj, "Instance")? {
        tf_visit_instance(py, obj, modules, allow_missing, cache)?;
        return Ok(true);
    }
    if cache.is_type(obj, "CallableType")? {
        tf_visit_callable_type(py, obj, modules, allow_missing, cache)?;
        return Ok(true);
    }
    if cache.is_type(obj, "TypeAliasType")? {
        tf_visit_type_alias_type(py, obj, modules, allow_missing, cache)?;
        return Ok(true);
    }
    if cache.is_type(obj, "TypedDictType")? {
        tf_visit_typeddict_type(py, obj, modules, allow_missing, cache)?;
        return Ok(true);
    }
    // Types with children needing recursion but no resolution.
    if cache.is_type(obj, "Overloaded")? {
        let items = obj.getattr("items")?;
        if let Ok(items_list) = items.downcast::<PyList>() {
            for item in items_list.iter() {
                fixup_type(py, item, modules, allow_missing, cache)?;
            }
        }
        return Ok(true);
    }
    if cache.is_type(obj, "TupleType")? {
        let items = obj.getattr("items")?;
        if let Ok(items_list) = items.downcast::<PyList>() {
            for item in items_list.iter() {
                fixup_type(py, item, modules, allow_missing, cache)?;
            }
        }
        let pf = obj.getattr("partial_fallback")?;
        if !pf.is_none() {
            fixup_type(py, pf, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "UnionType")? {
        let items = obj.getattr("items")?;
        if let Ok(items_list) = items.downcast::<PyList>() {
            for item in items_list.iter() {
                fixup_type(py, item, modules, allow_missing, cache)?;
            }
        }
        return Ok(true);
    }
    if cache.is_type(obj, "TypeType")? {
        let item = obj.getattr("item")?;
        if !item.is_none() {
            fixup_type(py, item, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "LiteralType")? {
        let fallback = obj.getattr("fallback")?;
        if !fallback.is_none() {
            fixup_type(py, fallback, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "TypeVarType")? {
        let values = obj.getattr("values")?;
        if let Ok(v_list) = values.downcast::<PyList>() {
            for v in v_list.iter() {
                fixup_type(py, v, modules, allow_missing, cache)?;
            }
        }
        let ub = obj.getattr("upper_bound")?;
        if !ub.is_none() {
            fixup_type(py, ub, modules, allow_missing, cache)?;
        }
        let default = obj.getattr("default")?;
        if !default.is_none() {
            fixup_type(py, default, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "ParamSpecType")? {
        let ub = obj.getattr("upper_bound")?;
        if !ub.is_none() {
            fixup_type(py, ub, modules, allow_missing, cache)?;
        }
        let default = obj.getattr("default")?;
        if !default.is_none() {
            fixup_type(py, default, modules, allow_missing, cache)?;
        }
        let prefix = obj.getattr("prefix")?;
        if !prefix.is_none() {
            fixup_type(py, prefix, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "TypeVarTupleType")? {
        let tf = obj.getattr("tuple_fallback")?;
        if !tf.is_none() {
            fixup_type(py, tf, modules, allow_missing, cache)?;
        }
        let ub = obj.getattr("upper_bound")?;
        if !ub.is_none() {
            fixup_type(py, ub, modules, allow_missing, cache)?;
        }
        let default = obj.getattr("default")?;
        if !default.is_none() {
            fixup_type(py, default, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "UnpackType")? {
        let typ = obj.getattr("type")?;
        if !typ.is_none() {
            fixup_type(py, typ, modules, allow_missing, cache)?;
        }
        return Ok(true);
    }
    if cache.is_type(obj, "Parameters")? {
        let arg_types = obj.getattr("arg_types")?;
        if let Ok(at_list) = arg_types.downcast::<PyList>() {
            for argt in at_list.iter() {
                if !argt.is_none() {
                    fixup_type(py, argt, modules, allow_missing, cache)?;
                }
            }
        }
        let variables = obj.getattr("variables")?;
        if let Ok(var_list) = variables.downcast::<PyList>() {
            for var in var_list.iter() {
                fixup_type(py, var, modules, allow_missing, cache)?;
            }
        }
        return Ok(true);
    }
    if cache.is_type(obj, "UnboundType")? {
        recurse_children(py, obj.getattr("args")?, modules, allow_missing, cache)?;
        return Ok(true);
    }
    // Leaves: AnyType, NoneType, UninhabitedType, ErasedType,
    // DeletedType — nothing to do.
    if cache.is_type(obj, "AnyType")?
        || cache.is_type(obj, "NoneType")?
        || cache.is_type(obj, "UninhabitedType")?
        || cache.is_type(obj, "ErasedType")?
        || cache.is_type(obj, "DeletedType")?
    {
        return Ok(true);
    }
    // Unknown type — defer.
    Ok(false)
}

/// Recurse into children stored as a tuple or list.
fn recurse_children(
    py: Python<'_>,
    children: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    if let Ok(t) = children.downcast::<pyo3::types::PyTuple>() {
        for child in t.iter() {
            if !child.is_none() {
                fixup_type(py, child, modules, allow_missing, cache)?;
            }
        }
    } else if let Ok(l) = children.downcast::<PyList>() {
        for child in l.iter() {
            if !child.is_none() {
                fixup_type(py, child, modules, allow_missing, cache)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// NodeFixer: walk live AST nodes, resolve cross-references
// ---------------------------------------------------------------------------

/// `NodeFixer.resolve_cross_ref` body: replace cross_ref with the
/// actual referred node.
fn nf_resolve_cross_ref(
    py: Python<'_>,
    value: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    _cache: &ClassCache<'_>,
) -> PyResult<()> {
    let cross_ref = value.getattr("cross_ref")?;
    if cross_ref.is_none() {
        return Ok(());
    }
    let cross_ref_str: String = cross_ref.extract()?;
    value.setattr("cross_ref", py.None())?;
    value.setattr("unfixed", false)?;

    let stnode = lookup_fq(py, &cross_ref_str, modules, !allow_missing)?;
    if !is_py_none(py, &stnode) {
        // Check if stnode is value itself (self-reference).
        if stnode.is(value) {
            // Deleted submodule: replace with placeholder Var.
            let short_name = cross_ref_str.rsplit('.').next().unwrap_or("");
            let name = format!("{}@deleted", short_name);
            let var_cls = py.import("mypy.nodes")?.getattr("Var")?;
            let var = var_cls.call1((name,))?;
            value.setattr("_node", var)?;
        } else {
            let node = stnode.getattr(py, "node")?;
            if is_py_none(py, &node) {
                if !allow_missing {
                    return Err(pyo3::exceptions::PyAssertionError::new_err(format!(
                        "Could not find cross-ref {}",
                        cross_ref_str
                    )));
                }
                let fixup_mod = py.import("mypy.fixup")?;
                let info = fixup_mod.getattr("missing_info")?.call1((modules,))?;
                value.setattr("_node", info)?;
            } else {
                value.setattr("_node", node)?;
            }
        }
    } else if !allow_missing {
        return Err(pyo3::exceptions::PyAssertionError::new_err(format!(
            "Could not find cross-ref {}",
            cross_ref_str
        )));
    } else {
        let fixup_mod = py.import("mypy.fixup")?;
        let info = fixup_mod.getattr("missing_info")?.call1((modules,))?;
        value.setattr("_node", info)?;
    }
    Ok(())
}

/// `NodeFixer.visit_symbol_table` body: iterate keys, fix
/// cross_refs, recurse into TypeInfo nodes.
fn nf_visit_symbol_table(
    py: Python<'_>,
    symtab: &PyDict,
    modules: &PyDict,
    allow_missing: bool,
    current_info: Option<&PyAny>,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    // Collect keys first to avoid borrow issues during iteration.
    let keys: Vec<PyObject> = {
        let mut ks = Vec::new();
        for key in symtab.keys() {
            ks.push(key.into());
        }
        ks
    };
    for key in &keys {
        let value = symtab.get_item(key)?.expect("key exists in dict");
        let cross_ref = value.getattr("cross_ref")?;
        if !cross_ref.is_none() {
            let cr_str: String = cross_ref.extract()?;
            if modules.contains(cr_str.as_str())? {
                value.setattr("cross_ref", py.None())?;
                value.setattr("unfixed", false)?;
                let module = modules.get_item(cr_str.as_str())?;
                value.setattr("_node", module)?;
            } else if allow_missing {
                nf_resolve_cross_ref(py, value, modules, allow_missing, cache)?;
            }
        } else {
            // Look at _node (private) to avoid triggering fixup eagerly.
            let node = value.getattr("_node")?;
            if !node.is_none() && cache.is_node(node, "TypeInfo")? {
                nf_visit_type_info(py, node, modules, allow_missing, cache)?;
            } else if let Some(ci) = current_info {
                value.setattr("stored_info", ci)?;
            }
        }
    }
    Ok(())
}

/// `NodeFixer.visit_type_info` body: the core method. Fixes bases,
/// tuple_type, typeddict_type, metaclass, self_type, alt_promote,
/// mro_refs, and recurses into defn + symbol table.
fn nf_visit_type_info(
    py: Python<'_>,
    info: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    // defn — fix the class def's type_vars.
    let defn = info.getattr("defn")?;
    if !defn.is_none() {
        nf_visit_class_def(py, defn, modules, allow_missing, cache)?;
    }

    // names (symbol table) — visit_symbol_table with current_info.
    let names = info.getattr("names")?;
    if !names.is_none() {
        if let Ok(names_dict) = names.downcast::<PyDict>() {
            nf_visit_symbol_table(py, names_dict, modules, allow_missing, Some(info), cache)?;
        }
    }

    // bases — fix each base via type_fixer.
    let bases = info.getattr("bases")?;
    if !bases.is_none() {
        if let Ok(bases_list) = bases.downcast::<PyList>() {
            for base in bases_list.iter() {
                fixup_type(py, base, modules, allow_missing, cache)?;
            }
        }
    }

    // _promote — fix each via type_fixer.
    let promote = info.getattr("_promote")?;
    if !promote.is_none() {
        if let Ok(promote_list) = promote.downcast::<PyList>() {
            for p in promote_list.iter() {
                fixup_type(py, p, modules, allow_missing, cache)?;
            }
        }
    }

    // tuple_type — fix + update_tuple_type + special_alias.
    let tuple_type = info.getattr("tuple_type")?;
    if !tuple_type.is_none() {
        fixup_type(py, tuple_type, modules, allow_missing, cache)?;
        info.getattr("update_tuple_type")?.call1((tuple_type,))?;
        fixup_special_alias(py, info, cache)?;
    }

    // typeddict_type — fix + update_typeddict_type + special_alias.
    let typeddict_type = info.getattr("typeddict_type")?;
    if !typeddict_type.is_none() {
        fixup_type(py, typeddict_type, modules, allow_missing, cache)?;
        info.getattr("update_typeddict_type")?
            .call1((typeddict_type,))?;
        fixup_special_alias(py, info, cache)?;
    }

    // declared_metaclass
    let dm = info.getattr("declared_metaclass")?;
    if !dm.is_none() {
        fixup_type(py, dm, modules, allow_missing, cache)?;
    }

    // metaclass_type
    let mt = info.getattr("metaclass_type")?;
    if !mt.is_none() {
        fixup_type(py, mt, modules, allow_missing, cache)?;
    }

    // self_type
    let st = info.getattr("self_type")?;
    if !st.is_none() {
        fixup_type(py, st, modules, allow_missing, cache)?;
    }

    // alt_promote
    let alt_promote = info.getattr("alt_promote")?;
    if !alt_promote.is_none() {
        fixup_type(py, alt_promote, modules, allow_missing, cache)?;
        // Hack: add backwards promotion if not already present.
        let instance_cls = py.import("mypy.types")?.getattr("Instance")?;
        let instance = instance_cls.call1((info, PyList::empty(py)))?;
        let ap_type = alt_promote.getattr("type")?;
        let ap_promote = ap_type.getattr("_promote")?;
        if let Ok(ap_list) = ap_promote.downcast::<PyList>() {
            let found = ap_list.iter().any(|p| p.eq(instance).unwrap_or(false));
            if !found {
                ap_list.append(instance)?;
            }
        }
    }

    // _mro_refs — resolve to mro.
    let mro_refs = info.getattr("_mro_refs")?;
    if !mro_refs.is_none() {
        if let Ok(refs_list) = mro_refs.downcast::<PyList>() {
            let names: Vec<String> = refs_list
                .iter()
                .map(|r| r.extract::<String>())
                .collect::<PyResult<Vec<String>>>()?;
            let mut mro: Vec<PyObject> = Vec::with_capacity(names.len());
            for name in &names {
                let typ = lookup_typeinfo(py, name, modules, allow_missing)?;
                mro.push(typ);
            }
            let mro_list = PyList::new(py, &mro);
            info.setattr("mro", mro_list)?;
            info.setattr("_mro_refs", py.None())?;
        }
    }
    Ok(())
}

/// Fix special_alias alias_tvars and tvar_tuple_index for
/// tuple/typeddict types.
fn fixup_special_alias(_py: Python<'_>, info: &PyAny, cache: &ClassCache<'_>) -> PyResult<()> {
    let special_alias = info.getattr("special_alias")?;
    if special_alias.is_none() {
        return Ok(());
    }
    let defn = info.getattr("defn")?;
    let type_vars = defn.getattr("type_vars")?;
    if let Ok(tv_list) = type_vars.downcast::<PyList>() {
        // alias_tvars = list(defn.type_vars)
        special_alias.setattr("alias_tvars", tv_list)?;
        for (i, t) in tv_list.iter().enumerate() {
            if cache.is_type(t, "TypeVarTupleType")? {
                special_alias.setattr("tvar_tuple_index", i)?;
            }
        }
    }
    Ok(())
}

/// `NodeFixer.visit_class_def` body: fix type_vars.
fn nf_visit_class_def(
    py: Python<'_>,
    c: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let type_vars = c.getattr("type_vars")?;
    if type_vars.is_none() {
        return Ok(());
    }
    if let Ok(tv_list) = type_vars.downcast::<PyList>() {
        for v in tv_list.iter() {
            fixup_type(py, v, modules, allow_missing, cache)?;
        }
    }
    Ok(())
}

/// `NodeFixer.visit_func_def` body: fix type, set definition.
fn nf_visit_func_def(
    py: Python<'_>,
    func: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let typ = func.getattr("type")?;
    if !typ.is_none() {
        fixup_type(py, typ, modules, allow_missing, cache)?;
        if cache.is_type(typ, "CallableType")? {
            typ.setattr("definition", func)?;
        }
    }
    Ok(())
}

/// `NodeFixer.visit_overloaded_func_def` body.
fn nf_visit_overloaded_func_def(
    py: Python<'_>,
    o: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let typ = o.getattr("type")?;
    if !typ.is_none() {
        fixup_type(py, typ, modules, allow_missing, cache)?;
    }
    // items — recurse via node dispatch.
    let items = o.getattr("items")?;
    if let Ok(items_list) = items.downcast::<PyList>() {
        for item in items_list.iter() {
            nf_visit_node(py, item, modules, allow_missing, cache)?;
        }
    }
    // impl
    let impl_ = o.getattr("impl")?;
    if !impl_.is_none() {
        nf_visit_node(py, impl_, modules, allow_missing, cache)?;
    }
    // If isinstance(o.type, Overloaded): link typ.definition = item.
    let typ = o.getattr("type")?;
    if !typ.is_none() && cache.is_type(typ, "Overloaded")? {
        let typ_items = typ.getattr("items")?;
        let o_items = o.getattr("items")?;
        if let (Ok(typ_list), Ok(o_list)) =
            (typ_items.downcast::<PyList>(), o_items.downcast::<PyList>())
        {
            for (t, item) in typ_list.iter().zip(o_list.iter()) {
                t.setattr("definition", item)?;
            }
        }
    }
    Ok(())
}

/// `NodeFixer.visit_decorator` body.
fn nf_visit_decorator(
    py: Python<'_>,
    d: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let func = d.getattr("func")?;
    if !func.is_none() {
        nf_visit_func_def(py, func, modules, allow_missing, cache)?;
    }
    let var = d.getattr("var")?;
    if !var.is_none() {
        nf_visit_var(py, var, modules, allow_missing, cache)?;
    }
    // typ = d.var.type; if isinstance(typ, CallableType):
    //   typ.definition = d.func
    let typ = var.getattr("type")?;
    if !typ.is_none() && cache.is_type(typ, "CallableType")? {
        typ.setattr("definition", func)?;
    }
    Ok(())
}

/// `NodeFixer.visit_var` body.
fn nf_visit_var(
    py: Python<'_>,
    v: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let typ = v.getattr("type")?;
    if !typ.is_none() {
        fixup_type(py, typ, modules, allow_missing, cache)?;
    }
    let setter_type = v.getattr("setter_type")?;
    if !setter_type.is_none() {
        fixup_type(py, setter_type, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// `NodeFixer.visit_type_alias` body.
fn nf_visit_type_alias(
    py: Python<'_>,
    a: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let target = a.getattr("target")?;
    if !target.is_none() {
        fixup_type(py, target, modules, allow_missing, cache)?;
    }
    let alias_tvars = a.getattr("alias_tvars")?;
    if let Ok(tv_list) = alias_tvars.downcast::<PyList>() {
        for v in tv_list.iter() {
            fixup_type(py, v, modules, allow_missing, cache)?;
        }
    }
    Ok(())
}

/// `NodeFixer.visit_type_var_expr` body.
fn nf_visit_type_var_expr(
    py: Python<'_>,
    tv: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let values = tv.getattr("values")?;
    if let Ok(v_list) = values.downcast::<PyList>() {
        for v in v_list.iter() {
            fixup_type(py, v, modules, allow_missing, cache)?;
        }
    }
    let ub = tv.getattr("upper_bound")?;
    if !ub.is_none() {
        fixup_type(py, ub, modules, allow_missing, cache)?;
    }
    let default = tv.getattr("default")?;
    if !default.is_none() {
        fixup_type(py, default, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// `NodeFixer.visit_paramspec_expr` body.
fn nf_visit_paramspec_expr(
    py: Python<'_>,
    p: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let ub = p.getattr("upper_bound")?;
    if !ub.is_none() {
        fixup_type(py, ub, modules, allow_missing, cache)?;
    }
    let default = p.getattr("default")?;
    if !default.is_none() {
        fixup_type(py, default, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// `NodeFixer.visit_type_var_tuple_expr` body.
fn nf_visit_type_var_tuple_expr(
    py: Python<'_>,
    tv: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<()> {
    let ub = tv.getattr("upper_bound")?;
    if !ub.is_none() {
        fixup_type(py, ub, modules, allow_missing, cache)?;
    }
    let tf = tv.getattr("tuple_fallback")?;
    if !tf.is_none() {
        fixup_type(py, tf, modules, allow_missing, cache)?;
    }
    let default = tv.getattr("default")?;
    if !default.is_none() {
        fixup_type(py, default, modules, allow_missing, cache)?;
    }
    Ok(())
}

/// Dispatch an AST node through the NodeFixer logic by type name.
fn nf_visit_node(
    py: Python<'_>,
    node: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
    cache: &ClassCache<'_>,
) -> PyResult<bool> {
    let name = type_name(node)?;
    match name.as_str() {
        "FuncDef" => {
            nf_visit_func_def(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "OverloadedFuncDef" => {
            nf_visit_overloaded_func_def(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "Decorator" => {
            nf_visit_decorator(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "Var" => {
            nf_visit_var(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "TypeAlias" => {
            nf_visit_type_alias(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "ClassDef" => {
            nf_visit_class_def(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "TypeVarExpr" => {
            nf_visit_type_var_expr(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "ParamSpecExpr" => {
            nf_visit_paramspec_expr(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        "TypeVarTupleExpr" => {
            nf_visit_type_var_tuple_expr(py, node, modules, allow_missing, cache)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// PyO3 entry points
// ---------------------------------------------------------------------------

/// TypeFixer entry point. Walks a live `mypy.types.Type` and resolves
/// `type_ref` strings to live TypeInfo/TypeAlias pointers, recursing
/// into children. Returns `true` if handled, `false` to defer.
#[pyfunction]
pub fn rust_fixup_type(
    py: Python<'_>,
    typ: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    fixup_type(py, typ, modules, allow_missing, &cache)
}

/// NodeFixer `visit_type_info` entry point. Walks a `TypeInfo`,
/// resolves bases, tuple/typeddict types, metaclass, self_type,
/// alt_promote, mro_refs, and recurses into the symbol table.
#[pyfunction]
pub fn rust_fixup_type_info(
    py: Python<'_>,
    info: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    nf_visit_type_info(py, info, modules, allow_missing, &cache)?;
    Ok(true)
}

/// NodeFixer `resolve_cross_ref` entry point. Resolves a
/// `SymbolTableNode`'s cross_ref to the actual referred node.
#[pyfunction]
pub fn rust_resolve_cross_ref(
    py: Python<'_>,
    value: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    nf_resolve_cross_ref(py, value, modules, allow_missing, &cache)?;
    Ok(true)
}

/// NodeFixer `visit_symbol_table` entry point. Iterates a
/// `SymbolTable` (dict), fixing cross_refs and recursing into
/// TypeInfo nodes.
#[pyfunction]
pub fn rust_fixup_symbol_table(
    py: Python<'_>,
    symtab: &PyDict,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    nf_visit_symbol_table(py, symtab, modules, allow_missing, None, &cache)?;
    Ok(true)
}

/// NodeFixer `visit_overloaded_func_def` entry point.
#[pyfunction]
pub fn rust_fixup_overloaded_func_def(
    py: Python<'_>,
    o: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    nf_visit_overloaded_func_def(py, o, modules, allow_missing, &cache)?;
    Ok(true)
}

/// NodeFixer `visit_decorator` entry point.
#[pyfunction]
pub fn rust_fixup_decorator(
    py: Python<'_>,
    d: &PyAny,
    modules: &PyDict,
    allow_missing: bool,
) -> PyResult<bool> {
    let cache = ClassCache::new(py)?;
    nf_visit_decorator(py, d, modules, allow_missing, &cache)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        // The fixup functions operate on live mypy objects which require
        // the full mypy import chain. Parity is verified via the Python
        // test suite (testtypes.py, testcheck.py) with the gate on.
    }
}
