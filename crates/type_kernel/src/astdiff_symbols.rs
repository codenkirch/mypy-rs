//! B7 slice 2 (issue #1500): native symbol-table snapshot builder for
//! `mypy.server.astdiff.snapshot_symbol_table`.
//!
//! Mirrors `snapshot_symbol_table` / `snapshot_definition` /
//! `snapshot_untyped_signature` (mypy/server/astdiff.py:195-583): a
//! recursive snapshot of a live `SymbolTable` as a Python dict of nested
//! tuples, preserving `table.items()` insertion order. Type leaves reuse
//! the slice-1 walk (`astdiff_snapshot`); when that walk defers
//! (currently generic `CallableType`), the deferred node is handed to the
//! Python `astdiff.snapshot_type` callback so the enclosing symbol
//! snapshot still completes natively. That per-node fallback was chosen
//! over whole-call deferral: fine-grained corpora carry generic callables
//! in ~2% of types, so whole-call deferral would lose the module snapshot
//! for a large fraction of symbol tables (see the wave-58 AGENTS.md note).
//!
//! `find_dataclass_transform_spec` is called through the live Python
//! `mypy.semanal_shared` function (itself Rust-backed behind its own
//! gate), so dataclass-transform semantics and gate state stay
//! Python-owned; `node.dataclass_transform_spec` short-circuits for
//! `FuncDef` and is the checked first step for `TypeInfo`, exactly like
//! the Python body.
//!
//! Strangler-fig contract: `None` defers to the pure-Python body.
//! Deferrals: unknown or `None` node kinds (Python asserts), an
//! `UNBOUND_IMPORTED` symbol kind (Python asserts), a non-str
//! `fullname`, and any unreadable attribute. Per the #1466/#1468 contract
//! only `PyAttributeError` maps to a defer; other PyErrs propagate so
//! kernel bugs stay visible.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyString, PyTuple, PyType};

use crate::astdiff_snapshot::{snapshot_value, sorted_tuple};
use crate::astdiff_snapshot::{tuple_from, SnapshotRefs};
use crate::refs::is_instance;

/// `mypy.nodes` class objects plus the semanal/astdiff callbacks, resolved
/// once per seam entry.
struct SymbolRefs<'py> {
    mypy_file: &'py PyType,
    type_var_expr: &'py PyType,
    type_alias: &'py PyType,
    param_spec_expr: &'py PyType,
    type_var_tuple_expr: &'py PyType,
    overloaded_func_def: &'py PyType,
    func_def: &'py PyType,
    decorator: &'py PyType,
    var: &'py PyType,
    type_info: &'py PyType,
    unbound_imported: i64,
    find_dataclass_transform_spec: &'py PyAny,
    py_snapshot_type: &'py PyAny,
    py_snapshot_symbol_table: &'py PyAny,
    types: SnapshotRefs<'py>,
}

impl<'py> SymbolRefs<'py> {
    fn try_new(py: Python<'py>) -> PyResult<Self> {
        let nodes_mod = py.import("mypy.nodes")?;
        macro_rules! class {
            ($name:literal) => {{
                nodes_mod.getattr($name)?.downcast::<PyType>()?
            }};
        }
        Ok(SymbolRefs {
            mypy_file: class!("MypyFile"),
            type_var_expr: class!("TypeVarExpr"),
            type_alias: class!("TypeAlias"),
            param_spec_expr: class!("ParamSpecExpr"),
            type_var_tuple_expr: class!("TypeVarTupleExpr"),
            overloaded_func_def: class!("OverloadedFuncDef"),
            func_def: class!("FuncDef"),
            decorator: class!("Decorator"),
            var: class!("Var"),
            type_info: class!("TypeInfo"),
            unbound_imported: nodes_mod.getattr("UNBOUND_IMPORTED")?.extract()?,
            find_dataclass_transform_spec: py
                .import("mypy.semanal_shared")?
                .getattr("find_dataclass_transform_spec")?,
            py_snapshot_type: py.import("mypy.server.astdiff")?.getattr("snapshot_type")?,
            py_snapshot_symbol_table: py
                .import("mypy.server.astdiff")?
                .getattr("snapshot_symbol_table")?,
            types: SnapshotRefs::try_new(py)?,
        })
    }
}

/// Native `snapshot_symbol_table(name_prefix, table) -> dict | None`.
#[pyfunction]
pub(crate) fn rust_snapshot_symbol_table(
    py: Python<'_>,
    name_prefix: &str,
    table: &PyAny,
) -> PyResult<Option<PyObject>> {
    let refs = SymbolRefs::try_new(py)?;
    match table_value(py, name_prefix, table, &refs) {
        Ok(value) => Ok(value),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Walk `for name, symbol in table.items()`, building the result dict in
/// insertion order. `Ok(None)` defers the whole table to Python.
fn table_value(
    py: Python<'_>,
    name_prefix: &str,
    table: &PyAny,
    refs: &SymbolRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let result = PyDict::new(py);
    let items = table.call_method0("items")?;
    for pair in items.iter()? {
        let pair = pair?.downcast::<PyTuple>()?;
        let name = pair.get_item(0)?;
        let symbol = pair.get_item(1)?;
        let node = symbol.getattr("node")?;
        let has_node = node.is_true()?;
        let fullname: PyObject = if has_node {
            node.getattr("fullname")?.into()
        } else {
            py.None()
        };
        let common = tuple_from(
            py,
            vec![
                fullname.clone_ref(py),
                symbol.getattr("kind")?.into(),
                symbol.getattr("module_public")?.into(),
            ],
        );
        if is_instance(node, refs.mypy_file) {
            let value = tuple_from(py, vec![PyString::new(py, "Moduleref").into(), common]);
            result.set_item(name, value)?;
        } else if is_instance(node, refs.type_var_expr) {
            result.set_item(name, snapshot_type_var_expr(py, node, refs)?)?;
        } else if is_instance(node, refs.type_alias) {
            result.set_item(name, snapshot_type_alias(py, node, refs)?)?;
        } else if is_instance(node, refs.param_spec_expr) {
            result.set_item(name, snapshot_param_spec_expr(py, node, refs)?)?;
        } else if is_instance(node, refs.type_var_tuple_expr) {
            result.set_item(name, snapshot_type_var_tuple_expr(py, node, refs)?)?;
        } else {
            // `assert symbol.kind != UNBOUND_IMPORTED`; defer so the
            // Python fallback raises the identical assert.
            let kind = symbol.getattr("kind")?.extract::<i64>().unwrap_or(-1);
            if kind == refs.unbound_imported {
                return Ok(None);
            }
            if !has_node {
                // `snapshot_definition(None, common)` asserts in Python.
                return Ok(None);
            }
            // `if node and get_prefix(node.fullname) != name_prefix` ->
            // CrossRef; the node is defined in this module otherwise.
            let value = match prefix_matches(fullname.as_ref(py), name_prefix)? {
                Some(false) => tuple_from(
                    py,
                    vec![
                        PyString::new(py, "CrossRef").into(),
                        common,
                        node.get_type().getattr("__name__")?.into(),
                    ],
                ),
                Some(true) => match definition_value(py, node, common, refs)? {
                    Some(value) => value,
                    None => return Ok(None),
                },
                // Non-str fullname: Python raises AttributeError.
                None => return Ok(None),
            };
            result.set_item(name, value)?;
        }
    }
    Ok(Some(result.into()))
}

/// `get_prefix(fullname) == name_prefix`; `None` when `fullname` is not a
/// str (the Python body then raises AttributeError).
fn prefix_matches(fullname: &PyAny, name_prefix: &str) -> PyResult<Option<bool>> {
    match fullname.downcast::<PyString>() {
        Ok(s) => Ok(Some(prefix_of(s.to_str()?) == name_prefix)),
        Err(_) => Ok(None),
    }
}

/// `fullname.rsplit(".", 1)[0]` (mypy.util.get_prefix).
fn prefix_of(fullname: &str) -> &str {
    match fullname.rsplit_once('.') {
        Some((prefix, _)) => prefix,
        None => fullname,
    }
}

/// `snapshot_definition` for a node defined in the current module.
///
/// `common` is moved into the returned tuple; `Ok(None)` defers (unknown
/// kinds and `None` assert in Python).
fn definition_value(
    py: Python<'_>,
    node: &PyAny,
    common: PyObject,
    refs: &SymbolRefs<'_>,
) -> PyResult<Option<PyObject>> {
    if is_instance(node, refs.func_def) || is_instance(node, refs.overloaded_func_def) {
        return func_definition(py, node, common, refs).map(Some);
    }
    if is_instance(node, refs.var) {
        return Ok(Some(tuple_from(
            py,
            vec![
                PyString::new(py, "Var").into(),
                common,
                optional_or_python(py, node.getattr("type")?, refs)?,
                node.getattr("is_final")?.into(),
            ],
        )));
    }
    if is_instance(node, refs.decorator) {
        let func = match definition_value(py, node.getattr("func")?, common, refs)? {
            Some(value) => value,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![
                PyString::new(py, "Decorator").into(),
                node.getattr("is_overload")?.into(),
                optional_or_python(py, node.getattr("var")?.getattr("type")?, refs)?,
                func,
            ],
        )));
    }
    if is_instance(node, refs.type_info) {
        return type_info_definition(py, node, common, refs).map(Some);
    }
    Ok(None)
}

/// `snapshot_definition`'s `SYMBOL_FUNCBASE_TYPES` arm (FuncDef and
/// OverloadedFuncDef).
fn func_definition(
    py: Python<'_>,
    node: &PyAny,
    common: PyObject,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    let typ = node.getattr("type")?;
    let signature = if typ.is_true()? {
        type_or_python(py, typ, refs)?
    } else {
        untyped_signature_value(py, node, refs)?
    };
    // `impl = node` for FuncDef, else `node.impl` unwrapped through a
    // Decorator; only a FuncDef carries `is_trivial_body`.
    let mut is_trivial_body = false;
    if is_instance(node, refs.func_def) {
        is_trivial_body = node.getattr("is_trivial_body")?.is_true()?;
    } else {
        let raw_impl = node.getattr("impl")?;
        if raw_impl.is_true()? {
            let impl_node = if is_instance(raw_impl, refs.decorator) {
                raw_impl.getattr("func")?
            } else {
                raw_impl
            };
            is_trivial_body = impl_node.getattr("is_trivial_body")?.is_true()?;
        }
    }

    let spec = funcbase_spec(node, refs)?;

    // FuncDef.deprecated is a str|None; OverloadedFuncDef.deprecated is the
    // head of a list with every Decorator item's func.deprecated.
    let deprecated: PyObject = if is_instance(node, refs.func_def) {
        node.getattr("deprecated")?.into()
    } else {
        let items = node.getattr("items")?;
        let mut list: Vec<PyObject> = vec![node.getattr("deprecated")?.into()];
        for item in items.iter()? {
            let item = item?;
            if is_instance(item, refs.decorator) {
                list.push(item.getattr("func")?.getattr("deprecated")?.into());
            }
        }
        PyList::new(py, &list).into()
    };

    let mut setter_type: PyObject = py.None();
    if is_instance(node, refs.overloaded_func_def) {
        let items = node.getattr("items")?;
        if items.is_true()? {
            let first_item = items.get_item(0)?;
            if is_instance(first_item, refs.decorator)
                && first_item
                    .getattr("func")?
                    .getattr("is_property")?
                    .is_true()?
            {
                let setter = first_item.getattr("var")?.getattr("setter_type")?;
                setter_type = optional_or_python(py, setter, refs)?;
            }
        }
    }

    Ok(tuple_from(
        py,
        vec![
            PyString::new(py, "Func").into(),
            common,
            node.getattr("is_property")?.into(),
            node.getattr("is_final")?.into(),
            node.getattr("is_class")?.into(),
            node.getattr("is_static")?.into(),
            signature,
            PyBool::new(py, is_trivial_body).into(),
            spec,
            deprecated,
            setter_type,
        ],
    ))
}

/// `find_dataclass_transform_spec(node)` for a FuncBase node, serialized.
/// A FuncDef's spec is the plain attribute (the Python function's FuncDef
/// arm); every other FuncBase shape goes through the live Python function
/// (which owns the OverloadedFuncDef item/impl walk).
fn funcbase_spec(node: &PyAny, refs: &SymbolRefs<'_>) -> PyResult<PyObject> {
    let spec = if is_instance(node, refs.func_def) {
        node.getattr("dataclass_transform_spec")?
    } else {
        refs.find_dataclass_transform_spec.call1((node,))?
    };
    serialize_spec(spec)
}

/// `spec.serialize() if spec is not None else None`.
fn serialize_spec(spec: &PyAny) -> PyResult<PyObject> {
    if spec.is_none() {
        Ok(spec.into())
    } else {
        Ok(spec.call_method0("serialize")?.into())
    }
}

/// `snapshot_definition`'s TypeInfo arm (attrs tuple plus the recursively
/// snapshotted nested symbol table and its `(abstract)` entry).
fn type_info_definition(
    py: Python<'_>,
    node: &PyAny,
    common: PyObject,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    let mut spec = node.getattr("dataclass_transform_spec")?;
    if spec.is_none() {
        spec = refs.find_dataclass_transform_spec.call1((node,))?;
    }
    let spec_serialized = serialize_spec(spec)?;

    let mro = PyList::empty(py);
    for base in node.getattr("mro")?.iter()? {
        mro.append(base?.getattr("fullname")?)?;
    }
    let mut tvars: Vec<PyObject> = Vec::new();
    for tdef in node.getattr("defn")?.getattr("type_vars")?.iter()? {
        tvars.push(type_or_python(py, tdef?, refs)?);
    }
    let bases = PyList::empty(py);
    for base in node.getattr("bases")?.iter()? {
        bases.append(type_or_python(py, base?, refs)?)?;
    }
    let promote = PyList::empty(py);
    for item in node.getattr("_promote")?.iter()? {
        promote.append(type_or_python(py, item?, refs)?)?;
    }
    let attrs = tuple_from(
        py,
        vec![
            node.getattr("is_abstract")?.into(),
            node.getattr("is_enum")?.into(),
            node.getattr("is_protocol")?.into(),
            node.getattr("fallback_to_any")?.into(),
            node.getattr("meta_fallback_to_any")?.into(),
            node.getattr("is_named_tuple")?.into(),
            node.getattr("is_newtype")?.into(),
            optional_or_python(py, node.getattr("metaclass_type")?, refs)?,
            optional_or_python(py, node.getattr("tuple_type")?, refs)?,
            optional_or_python(py, node.getattr("typeddict_type")?, refs)?,
            mro.into(),
            PyTuple::new(py, &tvars).into(),
            bases.into(),
            promote.into(),
            spec_serialized,
            node.getattr("deprecated")?.into(),
        ],
    );

    let fullname = node.getattr("fullname")?;
    let prefix = match fullname.downcast::<PyString>() {
        Ok(s) => s.to_str()?.to_owned(),
        Err(_) => return Ok(py.None()),
    };
    let names = node.getattr("names")?;
    let nested = match table_value(py, &prefix, names, refs)? {
        Some(value) => value,
        // Deep defer: let the Python body build only the nested table.
        None => refs
            .py_snapshot_symbol_table
            .call1((prefix.as_str(), names))?
            .into(),
    };
    let nested_dict = nested.downcast::<PyDict>(py)?;
    let abstract_names = sorted_tuple(py, node.getattr("abstract_attributes")?, &refs.types)?;
    nested_dict.set_item(
        "(abstract)",
        tuple_from(
            py,
            vec![PyString::new(py, "Abstract").into(), abstract_names],
        ),
    )?;

    Ok(tuple_from(
        py,
        vec![PyString::new(py, "TypeInfo").into(), common, attrs, nested],
    ))
}

/// `snapshot_symbol_table`'s TypeVarExpr arm; `values` is a Python list.
fn snapshot_type_var_expr(
    py: Python<'_>,
    node: &PyAny,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    let values_seq = node.getattr("values")?;
    let values = PyList::empty(py);
    for item in values_seq.iter()? {
        values.append(type_or_python(py, item?, refs)?)?;
    }
    Ok(tuple_from(
        py,
        vec![
            PyString::new(py, "TypeVar").into(),
            node.getattr("variance")?.into(),
            values.into(),
            type_or_python(py, node.getattr("upper_bound")?, refs)?,
            type_or_python(py, node.getattr("default")?, refs)?,
        ],
    ))
}

/// `snapshot_symbol_table`'s TypeAlias arm.
fn snapshot_type_alias(py: Python<'_>, node: &PyAny, refs: &SymbolRefs<'_>) -> PyResult<PyObject> {
    Ok(tuple_from(
        py,
        vec![
            PyString::new(py, "TypeAlias").into(),
            types_or_python(py, node.getattr("alias_tvars")?, refs)?,
            node.getattr("normalized")?.into(),
            node.getattr("no_args")?.into(),
            optional_or_python(py, node.getattr("target")?, refs)?,
        ],
    ))
}

/// `snapshot_symbol_table`'s ParamSpecExpr arm.
fn snapshot_param_spec_expr(
    py: Python<'_>,
    node: &PyAny,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    Ok(tuple_from(
        py,
        vec![
            PyString::new(py, "ParamSpec").into(),
            node.getattr("variance")?.into(),
            type_or_python(py, node.getattr("upper_bound")?, refs)?,
            type_or_python(py, node.getattr("default")?, refs)?,
        ],
    ))
}

/// `snapshot_symbol_table`'s TypeVarTupleExpr arm.
fn snapshot_type_var_tuple_expr(
    py: Python<'_>,
    node: &PyAny,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    Ok(tuple_from(
        py,
        vec![
            PyString::new(py, "TypeVarTuple").into(),
            node.getattr("variance")?.into(),
            type_or_python(py, node.getattr("upper_bound")?, refs)?,
            type_or_python(py, node.getattr("default")?, refs)?,
        ],
    ))
}

/// `snapshot_untyped_signature` for a FuncItem or an OverloadedFuncDef.
fn untyped_signature_value(
    py: Python<'_>,
    func: &PyAny,
    refs: &SymbolRefs<'_>,
) -> PyResult<PyObject> {
    if is_instance(func, refs.func_def) {
        let names = collect_tuple(py, func.getattr("arg_names")?)?;
        let kinds = collect_tuple(py, func.getattr("arg_kinds")?)?;
        return Ok(tuple_from(py, vec![names, kinds]));
    }
    let items = func.getattr("items")?;
    let mut result: Vec<PyObject> = Vec::new();
    for item in items.iter()? {
        let item = item?;
        if is_instance(item, refs.decorator) {
            let var_type = item.getattr("var")?.getattr("type")?;
            if var_type.is_true()? {
                result.push(type_or_python(py, var_type, refs)?);
            } else {
                result.push(tuple_from(
                    py,
                    vec![PyString::new(py, "DecoratorWithoutType").into()],
                ));
            }
        } else {
            result.push(untyped_signature_value(py, item, refs)?);
        }
    }
    Ok(PyTuple::new(py, &result).into())
}

/// `snapshot_type`, with a per-node fallback to the Python builder when the
/// slice-1 walk defers (generic CallableType today).
fn type_or_python(py: Python<'_>, typ: &PyAny, refs: &SymbolRefs<'_>) -> PyResult<PyObject> {
    match snapshot_value(py, typ, &refs.types)? {
        Some(value) => Ok(value),
        None => Ok(refs.py_snapshot_type.call1((typ,))?.into()),
    }
}

/// `snapshot_types`: tuple of recursive snapshots.
fn types_or_python(py: Python<'_>, seq: &PyAny, refs: &SymbolRefs<'_>) -> PyResult<PyObject> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        out.push(type_or_python(py, item?, refs)?);
    }
    Ok(PyTuple::new(py, &out).into())
}

/// `snapshot_optional_type`: truthiness gate then snapshot, else
/// `("<not set>",)`.
fn optional_or_python(py: Python<'_>, typ: &PyAny, refs: &SymbolRefs<'_>) -> PyResult<PyObject> {
    if typ.is_true()? {
        type_or_python(py, typ, refs)
    } else {
        Ok(tuple_from(py, vec![PyString::new(py, "<not set>").into()]))
    }
}

/// `tuple(seq)` preserving element identity (arg names/kinds are raw
/// objects, unlike the type-snapshot encoding).
fn collect_tuple(py: Python<'_>, seq: &PyAny) -> PyResult<PyObject> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        out.push(item?.into());
    }
    Ok(PyTuple::new(py, &out).into())
}
