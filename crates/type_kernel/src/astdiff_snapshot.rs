//! B7 slice 1 (issue #1497): native type-snapshot builder for
//! `mypy.server.astdiff.snapshot_type`.
//!
//! Mirrors `SnapshotTypeVisitor` (mypy/server/astdiff.py:389-557): a
//! recursive snapshot of a live `mypy.types.Type` as nested immutable
//! tuples of primitives. This port reads the live type via PyO3 and
//! recurses in Rust; the order-sensitive arms (union dedup+sort,
//! extra_attrs pairs, TypedDict required/readonly) call Python's own
//! `set`/`sorted` so the result is byte-for-byte the Python snapshot.
//!
//! Strangler-fig contract: `None` defers to the pure-Python visitor.
//! Deferrals: generic `CallableType` (`normalize_callable_variables`
//! needs `expand_type` + `state.strict_optional_set`), `PartialType`
//! (Python raises RuntimeError; the raise stays Python-side), a
//! `TypeAliasType` without an alias (Python asserts), an unhandled type
//! kind, and any unreadable attribute. Per the #1466/#1468 contract only
//! `PyAttributeError` maps to a defer; other PyErrs propagate so kernel
//! bugs stay visible.
//!
//! B7 slice 2 (#1500) reuses the recursive walk (`SnapshotRefs`,
//! `snapshot_value`, `snapshot_types`, `snapshot_optional`) from
//! `astdiff_symbols.rs` for the signature/type fields of a symbol
//! snapshot.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PySet, PyString, PyTuple, PyType};

use crate::refs::is_instance;

/// `mypy.types` class objects plus builtins.sorted, resolved once per
/// seam entry.
pub(crate) struct SnapshotRefs<'py> {
    unbound_type: &'py PyType,
    any_type: &'py PyType,
    none_type: &'py PyType,
    uninhabited_type: &'py PyType,
    erased_type: &'py PyType,
    deleted_type: &'py PyType,
    instance: &'py PyType,
    type_var_type: &'py PyType,
    param_spec_type: &'py PyType,
    type_var_tuple_type: &'py PyType,
    unpack_type: &'py PyType,
    parameters: &'py PyType,
    callable_type: &'py PyType,
    tuple_type: &'py PyType,
    typed_dict_type: &'py PyType,
    literal_type: &'py PyType,
    union_type: &'py PyType,
    overloaded: &'py PyType,
    partial_type: &'py PyType,
    type_type: &'py PyType,
    type_alias_type: &'py PyType,
    sorted: &'py PyAny,
}

impl<'py> SnapshotRefs<'py> {
    pub(crate) fn try_new(py: Python<'py>) -> PyResult<Self> {
        let types_mod = py.import("mypy.types")?;
        macro_rules! class {
            ($name:literal) => {{
                types_mod.getattr($name)?.downcast::<PyType>()?
            }};
        }
        Ok(SnapshotRefs {
            unbound_type: class!("UnboundType"),
            any_type: class!("AnyType"),
            none_type: class!("NoneType"),
            uninhabited_type: class!("UninhabitedType"),
            erased_type: class!("ErasedType"),
            deleted_type: class!("DeletedType"),
            instance: class!("Instance"),
            type_var_type: class!("TypeVarType"),
            param_spec_type: class!("ParamSpecType"),
            type_var_tuple_type: class!("TypeVarTupleType"),
            unpack_type: class!("UnpackType"),
            parameters: class!("Parameters"),
            callable_type: class!("CallableType"),
            tuple_type: class!("TupleType"),
            typed_dict_type: class!("TypedDictType"),
            literal_type: class!("LiteralType"),
            union_type: class!("UnionType"),
            overloaded: class!("Overloaded"),
            partial_type: class!("PartialType"),
            type_type: class!("TypeType"),
            type_alias_type: class!("TypeAliasType"),
            sorted: py.import("builtins")?.getattr("sorted")?,
        })
    }
}

/// Native `snapshot_type(typ) -> tuple | None`.
///
/// Returns the exact nested-tuple snapshot of `typ`, or `None` when the
/// shape is deferred and the Python caller must fall back to
/// `SnapshotTypeVisitor`.
#[pyfunction]
pub(crate) fn rust_snapshot_type(py: Python<'_>, typ: &PyAny) -> PyResult<Option<PyObject>> {
    let refs = SnapshotRefs::try_new(py)?;
    match snapshot_value(py, typ, &refs) {
        Ok(value) => Ok(value),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Recursive dispatch mirroring `Type.accept(SnapshotTypeVisitor())`.
pub(crate) fn snapshot_value(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    // Simple arms: snapshot_simple_type -> (type(typ).__name__,).
    if is_instance(typ, refs.any_type)
        || is_instance(typ, refs.none_type)
        || is_instance(typ, refs.uninhabited_type)
        || is_instance(typ, refs.erased_type)
        || is_instance(typ, refs.deleted_type)
    {
        return Ok(Some(snapshot_simple(py, typ)?));
    }
    // PartialType: Python raises RuntimeError; keep the raise Python-side.
    if is_instance(typ, refs.partial_type) {
        return Ok(None);
    }
    if is_instance(typ, refs.unbound_type) {
        return snapshot_unbound(py, typ, refs);
    }
    if is_instance(typ, refs.instance) {
        return snapshot_instance(py, typ, refs);
    }
    if is_instance(typ, refs.type_var_type) {
        return snapshot_type_var(py, typ, refs);
    }
    if is_instance(typ, refs.param_spec_type) {
        return snapshot_param_spec(py, typ, refs);
    }
    if is_instance(typ, refs.type_var_tuple_type) {
        return snapshot_type_var_tuple(py, typ, refs);
    }
    if is_instance(typ, refs.unpack_type) {
        let inner = match snapshot_value(py, typ.getattr("type")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![PyString::new(py, "UnpackType").into(), inner],
        )));
    }
    if is_instance(typ, refs.parameters) {
        return snapshot_parameters(py, typ, refs);
    }
    if is_instance(typ, refs.callable_type) {
        return snapshot_callable(py, typ, refs);
    }
    if is_instance(typ, refs.tuple_type) {
        let items = match snapshot_types(py, typ.getattr("items")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![PyString::new(py, "TupleType").into(), items],
        )));
    }
    if is_instance(typ, refs.typed_dict_type) {
        return snapshot_typed_dict(py, typ, refs);
    }
    if is_instance(typ, refs.literal_type) {
        let fallback = match snapshot_value(py, typ.getattr("fallback")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![
                PyString::new(py, "LiteralType").into(),
                fallback,
                typ.getattr("value")?.into(),
            ],
        )));
    }
    if is_instance(typ, refs.union_type) {
        return snapshot_union(py, typ, refs);
    }
    if is_instance(typ, refs.overloaded) {
        let items = match snapshot_types(py, typ.getattr("items")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![PyString::new(py, "Overloaded").into(), items],
        )));
    }
    if is_instance(typ, refs.type_type) {
        let item = match snapshot_value(py, typ.getattr("item")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![
                PyString::new(py, "TypeType").into(),
                item,
                typ.getattr("is_type_form")?.into(),
            ],
        )));
    }
    if is_instance(typ, refs.type_alias_type) {
        let alias = typ.getattr("alias")?;
        if alias.is_none() {
            // Python asserts `typ.alias is not None`; keep that raise Python-side.
            return Ok(None);
        }
        let fullname = alias.getattr("fullname")?;
        let args = match snapshot_types(py, typ.getattr("args")?, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        return Ok(Some(tuple_from(
            py,
            vec![
                PyString::new(py, "TypeAliasType").into(),
                fullname.into(),
                args,
            ],
        )));
    }
    // Unhandled kind (e.g. EllipsisType, a synthetic type): Python raises.
    Ok(None)
}

/// `snapshot_simple_type`: one-element tuple of the runtime class name.
fn snapshot_simple(py: Python<'_>, typ: &PyAny) -> PyResult<PyObject> {
    let name = typ.get_type().getattr("__name__")?;
    Ok(tuple_from(py, vec![name.into()]))
}

/// `snapshot_types`: tuple of recursive snapshots.
pub(crate) fn snapshot_types(
    py: Python<'_>,
    seq: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        match snapshot_value(py, item?, refs)? {
            Some(value) => out.push(value),
            None => return Ok(None),
        }
    }
    Ok(Some(PyTuple::new(py, &out).into()))
}

/// `snapshot_optional_type`: truthiness gate then snapshot, else
/// `("<not set>",)`.
pub(crate) fn snapshot_optional(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    if typ.is_true()? {
        snapshot_value(py, typ, refs)
    } else {
        Ok(Some(tuple_from(
            py,
            vec![PyString::new(py, "<not set>").into()],
        )))
    }
}

/// `encode_optional_str`: None -> "<None>", else the value itself.
fn encode_optional_str(py: Python<'_>, value: &PyAny) -> PyResult<PyObject> {
    if value.is_none() {
        Ok(PyString::new(py, "<None>").into())
    } else {
        Ok(value.into())
    }
}

fn snapshot_unbound(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let name = typ.getattr("name")?;
    let optional = typ.getattr("optional")?;
    let empty_tuple_index = typ.getattr("empty_tuple_index")?;
    let args = match snapshot_types(py, typ.getattr("args")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "UnboundType").into(),
            name.into(),
            optional.into(),
            empty_tuple_index.into(),
            args,
        ],
    )))
}

fn snapshot_instance(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let info = typ.getattr("type")?;
    let fullname = encode_optional_str(py, info.getattr("fullname")?)?;
    let args = match snapshot_types(py, typ.getattr("args")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let last_known_value = typ.getattr("last_known_value")?;
    let lkv_snapshot = if last_known_value.is_none() {
        tuple_from(py, vec![PyString::new(py, "None").into()])
    } else {
        match snapshot_value(py, last_known_value, refs)? {
            Some(v) => v,
            None => return Ok(None),
        }
    };
    let extra_attrs = typ.getattr("extra_attrs")?;
    let extra_attrs_snapshot = if extra_attrs.is_true()? {
        match snapshot_extra_attrs(py, extra_attrs, refs)? {
            Some(v) => v,
            None => return Ok(None),
        }
    } else {
        PyTuple::empty(py).into()
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "Instance").into(),
            fullname,
            args,
            lkv_snapshot,
            extra_attrs_snapshot,
        ],
    )))
}

/// Instance `extra_attrs`: `(tuple(sorted(attrs.items())), tuple(immutable))`.
fn snapshot_extra_attrs(
    py: Python<'_>,
    extra_attrs: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let attrs = extra_attrs.getattr("attrs")?;
    let attrs_dict = attrs.downcast::<PyDict>()?;
    let mut pairs: Vec<PyObject> = Vec::with_capacity(attrs_dict.len());
    for (key, value) in attrs_dict.iter() {
        let value_snapshot = match snapshot_value(py, value, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        pairs.push(tuple_from(py, vec![key.into(), value_snapshot]));
    }
    // Python does the sort; keys are unique so tuple comparison is moot.
    let sorted_pairs = refs.sorted.call1((PyList::new(py, &pairs),))?;
    let pairs_tuple = sequence_to_tuple(py, sorted_pairs)?;
    // `tuple(extra_attrs.immutable)` preserves the live set's iteration order.
    let immutable_tuple = sequence_to_tuple(py, extra_attrs.getattr("immutable")?)?;
    Ok(Some(tuple_from(py, vec![pairs_tuple, immutable_tuple])))
}

fn snapshot_type_var(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let id = typ.getattr("id")?;
    let values = match snapshot_types(py, typ.getattr("values")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let upper_bound = match snapshot_value(py, typ.getattr("upper_bound")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let default = match snapshot_value(py, typ.getattr("default")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "TypeVar").into(),
            typ.getattr("name")?.into(),
            typ.getattr("fullname")?.into(),
            id.getattr("raw_id")?.into(),
            id.getattr("meta_level")?.into(),
            values,
            upper_bound,
            default,
            typ.getattr("variance")?.into(),
        ],
    )))
}

fn snapshot_param_spec(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let id = typ.getattr("id")?;
    let upper_bound = match snapshot_value(py, typ.getattr("upper_bound")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let default = match snapshot_value(py, typ.getattr("default")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let prefix = match snapshot_value(py, typ.getattr("prefix")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "ParamSpec").into(),
            id.getattr("raw_id")?.into(),
            id.getattr("meta_level")?.into(),
            typ.getattr("flavor")?.into(),
            upper_bound,
            default,
            prefix,
        ],
    )))
}

fn snapshot_type_var_tuple(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let id = typ.getattr("id")?;
    let upper_bound = match snapshot_value(py, typ.getattr("upper_bound")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let default = match snapshot_value(py, typ.getattr("default")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "TypeVarTupleType").into(),
            id.getattr("raw_id")?.into(),
            id.getattr("meta_level")?.into(),
            upper_bound,
            default,
        ],
    )))
}

fn snapshot_parameters(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let arg_types = match snapshot_types(py, typ.getattr("arg_types")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let arg_names = encoded_names_tuple(py, typ.getattr("arg_names")?)?;
    let arg_kinds = arg_kind_values_tuple(py, typ.getattr("arg_kinds")?)?;
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "Parameters").into(),
            arg_types,
            arg_names,
            arg_kinds,
        ],
    )))
}

fn snapshot_callable(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    // Generic callables need normalize_callable_variables (expand_type);
    // the Python visitor handles them.
    if typ.call_method0("is_generic")?.is_true()? {
        return Ok(None);
    }
    let arg_types = match snapshot_types(py, typ.getattr("arg_types")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let ret_type = match snapshot_value(py, typ.getattr("ret_type")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let arg_names = encoded_names_tuple(py, typ.getattr("arg_names")?)?;
    let arg_kinds = arg_kind_values_tuple(py, typ.getattr("arg_kinds")?)?;
    let is_type_obj = typ.call_method0("is_type_obj")?;
    let variables = match snapshot_types(py, typ.getattr("variables")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let type_guard = match snapshot_optional(py, typ.getattr("type_guard")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let type_is = match snapshot_optional(py, typ.getattr("type_is")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let instance_type = match snapshot_optional(py, typ.getattr("instance_type")?, refs)? {
        Some(v) => v,
        None => return Ok(None),
    };
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "CallableType").into(),
            arg_types,
            ret_type,
            arg_names,
            arg_kinds,
            is_type_obj.into(),
            typ.getattr("is_ellipsis_args")?.into(),
            variables,
            typ.getattr("is_bound")?.into(),
            type_guard,
            type_is,
            instance_type,
        ],
    )))
}

fn snapshot_typed_dict(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    // Python: tuple((key, snapshot_type(item_type)) for ... in items.items()).
    let items = typ.getattr("items")?;
    let items_dict = items.downcast::<PyDict>()?;
    let mut pairs: Vec<PyObject> = Vec::with_capacity(items_dict.len());
    for (key, value) in items_dict.iter() {
        let value_snapshot = match snapshot_value(py, value, refs)? {
            Some(v) => v,
            None => return Ok(None),
        };
        pairs.push(tuple_from(py, vec![key.into(), value_snapshot]));
    }
    let items_tuple = PyTuple::new(py, &pairs).into();
    let required = sorted_tuple(py, typ.getattr("required_keys")?, refs)?;
    let readonly = sorted_tuple(py, typ.getattr("readonly_keys")?, refs)?;
    Ok(Some(tuple_from(
        py,
        vec![
            PyString::new(py, "TypedDictType").into(),
            items_tuple,
            required,
            readonly,
            typ.getattr("is_closed")?.into(),
        ],
    )))
}

fn snapshot_union(
    py: Python<'_>,
    typ: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<Option<PyObject>> {
    let items = typ.getattr("items")?;
    let mut snapshots: Vec<PyObject> = Vec::new();
    for item in items.iter()? {
        match snapshot_value(py, item?, refs)? {
            Some(value) => snapshots.push(value),
            None => return Ok(None),
        }
    }
    // Python: `{snapshot_type(item) for item in items}` then sorted().
    let seen = PySet::empty(py)?;
    for snapshot in &snapshots {
        seen.add(snapshot)?;
    }
    let normalized = refs.sorted.call1((seen,))?;
    let normalized_tuple = sequence_to_tuple(py, normalized)?;
    Ok(Some(tuple_from(
        py,
        vec![PyString::new(py, "UnionType").into(), normalized_tuple],
    )))
}

/// `tuple(sorted(values))` via Python's own sort (set iteration order does
/// not survive a Rust-side sort of the live object).
pub(crate) fn sorted_tuple(
    py: Python<'_>,
    values: &PyAny,
    refs: &SnapshotRefs<'_>,
) -> PyResult<PyObject> {
    let sorted = refs.sorted.call1((values,))?;
    sequence_to_tuple(py, sorted)
}

/// `tuple(encode_optional_str(name) for name in seq)`.
fn encoded_names_tuple(py: Python<'_>, seq: &PyAny) -> PyResult<PyObject> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        out.push(encode_optional_str(py, item?)?);
    }
    Ok(PyTuple::new(py, &out).into())
}

/// `tuple(k.value for k in seq)` for the ArgKind sequence.
fn arg_kind_values_tuple(py: Python<'_>, seq: &PyAny) -> PyResult<PyObject> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        out.push(item?.getattr("value")?.into());
    }
    Ok(PyTuple::new(py, &out).into())
}

/// Build a Python tuple from an iterable Python object.
fn sequence_to_tuple(py: Python<'_>, seq: &PyAny) -> PyResult<PyObject> {
    let mut out: Vec<PyObject> = Vec::new();
    for item in seq.iter()? {
        out.push(item?.into());
    }
    Ok(PyTuple::new(py, &out).into())
}

/// Build a Python tuple from already-built elements.
pub(crate) fn tuple_from(py: Python<'_>, elements: Vec<PyObject>) -> PyObject {
    PyTuple::new(py, &elements).into()
}
