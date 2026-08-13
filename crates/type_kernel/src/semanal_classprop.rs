//! Native port of `mypy/semanal_classprop.py` (Issue #538).
//!
//! These functions calculate class-level properties after semantic analysis
//! and before type checking. Each mirrors the Python implementation and
//! operates on live Python AST/symbol objects via PyO3, following the same
//! strangler-fig pattern as `semanal_visitor`.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple, PyType};

// Hard-coded type promotions (shared between all Python versions).
const TYPE_PROMOTIONS: &[(&str, &str)] = &[
    ("builtins.int", "float"),
    ("builtins.float", "complex"),
    ("builtins.bytearray", "bytes"),
    ("builtins.memoryview", "bytes"),
];

// Abstract status constants from mypy.nodes.
const IS_ABSTRACT: i64 = 1;
const IMPLICITLY_ABSTRACT: i64 = 2;

// ---------------------------------------------------------------------------
// calculate_class_abstract_status
// ---------------------------------------------------------------------------

/// `mypy.semanal_classprop.calculate_class_abstract_status` — calculate
/// abstract status of a class.
///
/// Mirrors semanal_classprop.py:42-117. Sets `is_abstract` and
/// `abstract_attributes` on the TypeInfo. Reports errors for abstract
/// classes missing ABCMeta metaclass (in stubs) and for final classes
/// with abstract attributes.
#[pyfunction]
pub(crate) fn rust_calculate_class_abstract_status(
    py: Python<'_>,
    typ: &PyAny,
    is_stub_file: bool,
    errors: &PyAny,
) -> PyResult<()> {
    let nodes_mod = py.import("mypy.nodes")?;
    let overloaded_cls: &PyType = nodes_mod.getattr("OverloadedFuncDef")?.downcast()?;
    let decorator_cls: &PyType = nodes_mod.getattr("Decorator")?.downcast()?;
    let func_def_cls: &PyType = nodes_mod.getattr("FuncDef")?.downcast()?;
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;

    typ.setattr("is_abstract", false)?;
    typ.setattr("abstract_attributes", PyList::empty(py))?;

    // TypedDict can't be abstract.
    let typeddict_type = typ.getattr("typeddict_type")?;
    if !typeddict_type.is_none() {
        return Ok(());
    }

    // NewTypes are always non-abstract.
    let is_newtype = typ.getattr("is_newtype")?;
    if is_newtype.is_true()? {
        return Ok(());
    }

    let mut concrete: HashSet<String> = HashSet::new();
    let mut abstract_attrs: Vec<(String, i64)> = Vec::new();
    let mut abstract_in_this_class: Vec<String> = Vec::new();

    let mro = typ.getattr("mro")?;
    let mro_list = mro.downcast::<PyList>()?;

    for base in mro_list.iter() {
        let base_names = base.getattr("names")?;
        let base_names_dict = base_names.downcast::<PyDict>()?;
        for (key, symnode) in base_names_dict.iter() {
            let name_str: &str = key.downcast::<PyString>()?.to_str()?;
            let node = symnode.getattr("node")?;
            if node.is_none() {
                continue;
            }

            // Unwrap OverloadedFuncDef: check first item.
            let func_obj: &PyAny = if node.is_instance(overloaded_cls)? {
                let items = node.getattr("items")?;
                let items_list = items.downcast::<PyList>()?;
                if items_list.is_empty() {
                    continue;
                }
                items_list.get_item(0)?
            } else {
                node
            };

            // Unwrap Decorator -> FuncDef.
            let func_final: &PyAny = if func_obj.is_instance(decorator_cls)? {
                func_obj.getattr("func")?
            } else {
                func_obj
            };

            if func_final.is_instance(func_def_cls)? {
                let status_val = func_final.getattr("abstract_status")?;
                let status: i64 = status_val.extract()?;
                if (status == IS_ABSTRACT || status == IMPLICITLY_ABSTRACT)
                    && !concrete.contains(name_str)
                {
                    typ.setattr("is_abstract", true)?;
                    abstract_attrs.push((name_str.to_string(), status));
                    // `base is typ` identity check.
                    let same = base.is(typ);
                    if same {
                        abstract_in_this_class.push(name_str.to_string());
                    }
                }
            } else if node.is_instance(var_cls)? {
                let is_abstract_var = node.getattr("is_abstract_var")?;
                if is_abstract_var.is_true()? && !concrete.contains(name_str) {
                    typ.setattr("is_abstract", true)?;
                    abstract_attrs.push((name_str.to_string(), IS_ABSTRACT));
                    let same = base.is(typ);
                    if same {
                        abstract_in_this_class.push(name_str.to_string());
                    }
                }
            }
            concrete.insert(name_str.to_string());
        }
    }

    // typ.abstract_attributes = sorted(abstract)
    abstract_attrs.sort();
    let abs_list = PyList::empty(py);
    for (name, status) in &abstract_attrs {
        let tup = PyTuple::new(
            py,
            [
                PyString::new(py, name) as &PyAny,
                (*status).into_py(py).into_ref(py) as &PyAny,
            ],
        );
        abs_list.append(tup)?;
    }
    typ.setattr("abstract_attributes", abs_list)?;

    if is_stub_file {
        let declared_metaclass = typ.getattr("declared_metaclass")?;
        if !declared_metaclass.is_none() {
            let mc_type = declared_metaclass.getattr("type")?;
            let has_base = mc_type.call_method1("has_base", ("abc.ABCMeta",))?;
            if has_base.is_true()? {
                return Ok(());
            }
        }
        let is_protocol = typ.getattr("is_protocol")?;
        if is_protocol.is_true()? {
            return Ok(());
        }
        if !abstract_attrs.is_empty() && abstract_in_this_class.is_empty() {
            let typ_line = typ.getattr("line")?;
            let typ_column = typ.getattr("column")?;
            let typ_fullname = typ.getattr("fullname")?;
            let fullname_str: &str = typ_fullname.downcast::<PyString>()?.to_str()?;

            let mut sorted_attrs = abstract_attrs.clone();
            sorted_attrs.sort();
            let attrs_joined = sorted_attrs
                .iter()
                .map(|(n, _)| format!("\"{}\"", n))
                .collect::<Vec<_>>()
                .join(", ");

            let msg1 = format!(
                "Class {} has abstract attributes {}",
                fullname_str, attrs_joined
            );
            errors.call_method1("report", (typ_line, typ_column, msg1, py.None()))?;
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("severity", "note")?;
            let msg2 = "If it is meant to be abstract, \
                add 'abc.ABCMeta' as an explicit metaclass";
            errors.call_method(
                "report",
                (typ_line, typ_column, msg2, py.None()),
                Some(kwargs),
            )?;
        }
    }

    let is_final = typ.getattr("is_final")?;
    if is_final.is_true()? && !abstract_attrs.is_empty() {
        let typ_line = typ.getattr("line")?;
        let typ_column = typ.getattr("column")?;
        let typ_fullname = typ.getattr("fullname")?;
        let fullname_str: &str = typ_fullname.downcast::<PyString>()?.to_str()?;

        let mut sorted_attrs = abstract_attrs.clone();
        sorted_attrs.sort();
        let attrs_joined = sorted_attrs
            .iter()
            .map(|(n, _)| format!("\"{}\"", n))
            .collect::<Vec<_>>()
            .join(", ");

        let msg = format!(
            "Final class {} has abstract attributes {}",
            fullname_str, attrs_joined
        );
        errors.call_method1("report", (typ_line, typ_column, msg, py.None()))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// check_protocol_status
// ---------------------------------------------------------------------------

/// `mypy.semanal_classprop.check_protocol_status` — check that all classes
/// in MRO of a protocol are protocols.
///
/// Mirrors semanal_classprop.py:120-130.
#[pyfunction]
pub(crate) fn rust_check_protocol_status(
    py: Python<'_>,
    info: &PyAny,
    errors: &PyAny,
) -> PyResult<()> {
    let is_protocol = info.getattr("is_protocol")?;
    if !is_protocol.is_true()? {
        return Ok(());
    }

    let bases = info.getattr("bases")?;
    let bases_list = bases.downcast::<PyList>()?;
    let info_line = info.getattr("line")?;
    let info_column = info.getattr("column")?;

    for base_type in bases_list.iter() {
        let base_type_obj = base_type.getattr("type")?;
        let is_proto = base_type_obj.getattr("is_protocol")?;
        let fullname = base_type_obj.getattr("fullname")?;
        let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
        if !is_proto.is_true()? && fullname_str != "builtins.object" {
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("severity", "error")?;
            errors.call_method(
                "report",
                (
                    info_line,
                    info_column,
                    "All bases of a protocol must be protocols",
                    py.None(),
                ),
                Some(kwargs),
            )?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// calculate_class_vars
// ---------------------------------------------------------------------------

/// `mypy.semanal_classprop.calculate_class_vars` — infer additional class
/// variables.
///
/// Mirrors semanal_classprop.py:133-149. Subclass attribute assignments
/// with no type annotation are assumed to be classvar if overriding a
/// declared classvar from the base class.
#[pyfunction]
pub(crate) fn rust_calculate_class_vars(py: Python<'_>, info: &PyAny) -> PyResult<()> {
    let nodes_mod = py.import("mypy.nodes")?;
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;

    let names = info.getattr("names")?;
    let names_dict = names.downcast::<PyDict>()?;
    let mro = info.getattr("mro")?;
    let mro_list = mro.downcast::<PyList>()?;

    let entries: Vec<(PyObject, PyObject)> = names_dict
        .iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();

    for (key, sym) in &entries {
        let sym_ref: &PyAny = sym.as_ref(py);
        let node = sym_ref.getattr("node")?;
        if node.is_none() || !node.is_instance(var_cls)? {
            continue;
        }
        let node_info = node.getattr("info")?;
        if node_info.is_none() {
            continue;
        }
        let is_inferred = node.getattr("is_inferred")?;
        let is_classvar = node.getattr("is_classvar")?;
        if !is_inferred.is_true()? || is_classvar.is_true()? {
            continue;
        }

        let key_ref: &PyAny = key.as_ref(py);
        for base in mro_list.iter().skip(1) {
            let base_names = base.getattr("names")?;
            let base_names_dict = base_names.downcast::<PyDict>()?;
            let member = base_names_dict.get_item(key_ref)?;
            if member.is_none() {
                continue;
            }
            let member = member.unwrap();
            let member_node = member.getattr("node")?;
            if member_node.is_none() || !member_node.is_instance(var_cls)? {
                continue;
            }
            let member_is_classvar = member_node.getattr("is_classvar")?;
            if member_is_classvar.is_true()? {
                node.setattr("is_classvar", true)?;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// add_type_promotion
// ---------------------------------------------------------------------------

/// `mypy.semanal_classprop.add_type_promotion` — setup extra, ad-hoc
/// subtyping relationships between classes (promotion).
///
/// Mirrors semanal_classprop.py:152-188. Handles `_promote` class decorator
/// and the hardcoded TYPE_PROMOTIONS map, plus mypyc native int promotions.
#[pyfunction]
pub(crate) fn rust_add_type_promotion(
    py: Python<'_>,
    info: &PyAny,
    module_names: &PyAny,
    options: &PyAny,
    builtin_names: &PyAny,
) -> PyResult<()> {
    let nodes_mod = py.import("mypy.nodes")?;
    let types_mod = py.import("mypy.types")?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
    let promote_expr_cls: &PyType = nodes_mod.getattr("PromoteExpr")?.downcast()?;
    let instance_cls: &PyType = types_mod.getattr("Instance")?.downcast()?;
    let type_info_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;

    let defn = info.getattr("defn")?;
    let defn_fullname = defn.getattr("fullname")?;
    let defn_fullname_str: &str = defn_fullname.downcast::<PyString>()?.to_str()?;

    let mut promote_targets: Vec<PyObject> = Vec::new();

    // Collect _promote decorator targets.
    let decorators = defn.getattr("decorators")?;
    let decorators_list = decorators.downcast::<PyList>()?;
    for decorator in decorators_list.iter() {
        if decorator.is_instance(call_expr_cls)? {
            let analyzed = decorator.getattr("analyzed")?;
            if !analyzed.is_none() && analyzed.is_instance(promote_expr_cls)? {
                let promote_type = analyzed.getattr("type")?;
                promote_targets.push(promote_type.into_py(py));
            }
        }
    }

    // Hardcoded type promotions.
    if promote_targets.is_empty() {
        let target_name = TYPE_PROMOTIONS
            .iter()
            .find(|(k, _)| *k == defn_fullname_str)
            .map(|(_, v)| *v);

        if let Some(target_fullname) = target_name {
            let mut use_target = true;
            if defn_fullname_str == "builtins.bytearray" {
                let disable = options.getattr("disable_bytearray_promotion")?;
                if disable.is_true()? {
                    use_target = false;
                }
            } else if defn_fullname_str == "builtins.memoryview" {
                let disable = options.getattr("disable_memoryview_promotion")?;
                if disable.is_true()? {
                    use_target = false;
                }
            }
            if use_target {
                let target_sym = module_names.call_method1("get", (target_fullname,))?;
                if !target_sym.is_none() {
                    let target_info = target_sym.getattr("node")?;
                    if target_info.is_instance(type_info_cls)? {
                        let instance = instance_cls.call1((target_info, PyList::empty(py)))?;
                        promote_targets.push(instance.into_py(py));
                    }
                }
            }
        }
    }

    // Special case: promotions between 'int' and native integer types.
    let mypyc_native_int_names = types_mod.getattr("MYPYC_NATIVE_INT_NAMES")?;
    let mypyc_names_set = mypyc_native_int_names.downcast::<PySet>()?;
    let in_native_ints = mypyc_names_set.contains(defn_fullname_str)?;

    if in_native_ints {
        let int_sym = builtin_names.call_method1("get", ("int",))?;
        if !int_sym.is_none() {
            let int_node = int_sym.getattr("node")?;
            if int_node.is_instance(type_info_cls)? {
                let defn_info = defn.getattr("info")?;
                let defn_instance = instance_cls.call1((defn_info, PyList::empty(py)))?;
                let int_promote = int_node.getattr("_promote")?;
                let int_promote_list = int_promote.downcast::<PyList>()?;
                int_promote_list.append(defn_instance)?;
                let int_instance = instance_cls.call1((int_node, PyList::empty(py)))?;
                defn_info.setattr("alt_promote", int_instance)?;
            }
        }
    }

    // defn.info._promote.extend(promote_targets)
    if !promote_targets.is_empty() {
        let defn_info = defn.getattr("info")?;
        let promote = defn_info.getattr("_promote")?;
        let promote_list = promote.downcast::<PyList>()?;
        for target in &promote_targets {
            promote_list.append(target.as_ref(py))?;
        }
    }

    Ok(())
}
