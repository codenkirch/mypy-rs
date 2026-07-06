//! Shared infrastructure for the type-kernel visitors.
//!
//! `TypeRefs` caches the resolved `mypy.types` class objects once per call
//! (PyO3 cell borrow is cheap; the GIL-bound reference is what we hold), so
//! both visitors (`erase::erase_type`, `lkv::remove_instance_last_known_values`)
//! dispatch by `isinstance` against the same class set without re-importing.
//!
//! `fallback_sentinel` / `is_fallback` implement the strangler-fig per-call
//! gate: a visitor returns Python `None` for any type it does not handle, and
//! the Python caller falls back to the pure-Python visitor.

use pyo3::prelude::*;
use pyo3::types::PyType;

/// Cache of Python constructors and class objects, looked up once per call.
pub(crate) struct TypeRefs<'py> {
    /// mypy.types.AnyType
    pub(crate) any_type: &'py PyType,
    /// mypy.types.NoneType
    pub(crate) none_type: &'py PyType,
    /// mypy.types.UninhabitedType
    pub(crate) uninhabited_type: &'py PyType,
    /// mypy.types.TypeVarType
    pub(crate) type_var_type: &'py PyType,
    /// mypy.types.ParamSpecType
    pub(crate) param_spec_type: &'py PyType,
    /// mypy.types.TypeVarTupleType
    pub(crate) type_var_tuple_type: &'py PyType,
    /// mypy.types.UnpackType
    pub(crate) unpack_type: &'py PyType,
    /// mypy.types.LiteralType
    pub(crate) literal_type: &'py PyType,
    /// mypy.types.DeletedType
    pub(crate) deleted_type: &'py PyType,
    /// mypy.types.Instance
    pub(crate) instance: &'py PyType,
    /// mypy.types.CallableType
    pub(crate) callable_type: &'py PyType,
    /// mypy.types.UnionType
    pub(crate) union_type: &'py PyType,
    /// mypy.types.TupleType
    pub(crate) tuple_type: &'py PyType,
    /// mypy.types.TypeType
    pub(crate) type_type: &'py PyType,
    /// mypy.types.Overloaded
    pub(crate) overloaded: &'py PyType,
    /// mypy.types.TypedDictType
    pub(crate) typed_dict_type: &'py PyType,
    /// mypy.types.TypeAliasType
    pub(crate) type_alias_type: &'py PyType,
    /// mypy.types.TypeOfAny (the IntEnum)
    pub(crate) type_of_any: &'py PyType,
}

impl<'py> TypeRefs<'py> {
    pub(crate) fn try_new(py: Python<'py>) -> PyResult<Self> {
        let types_mod = py.import("mypy.types")?;
        macro_rules! class {
            ($name:literal) => {{
                types_mod.getattr($name)?.downcast::<PyType>()?
            }};
        }
        Ok(TypeRefs {
            any_type: class!("AnyType"),
            none_type: class!("NoneType"),
            uninhabited_type: class!("UninhabitedType"),
            type_var_type: class!("TypeVarType"),
            param_spec_type: class!("ParamSpecType"),
            type_var_tuple_type: class!("TypeVarTupleType"),
            unpack_type: class!("UnpackType"),
            literal_type: class!("LiteralType"),
            deleted_type: class!("DeletedType"),
            instance: class!("Instance"),
            callable_type: class!("CallableType"),
            union_type: class!("UnionType"),
            tuple_type: class!("TupleType"),
            type_type: class!("TypeType"),
            overloaded: class!("Overloaded"),
            typed_dict_type: class!("TypedDictType"),
            type_alias_type: class!("TypeAliasType"),
            type_of_any: class!("TypeOfAny"),
        })
    }
}

/// Look up `TypeOfAny.special_form` once per call. Cheap, but avoids repeated
/// attribute resolution in the hot path.
pub(crate) fn type_of_any_special_form(type_of_any: &PyType) -> PyResult<PyObject> {
    // TypeOfAny is an IntEnum; special_form is an int value, not a callable.
    Ok(type_of_any.getattr("special_form")?.into())
}

/// Construct `AnyType(TypeOfAny.special_form)` with no source/missing-import.
pub(crate) fn make_any(_py: Python<'_>, refs: &TypeRefs<'_>) -> PyResult<PyObject> {
    let type_of_any = type_of_any_special_form(refs.type_of_any)?;
    Ok(refs.any_type.call1((type_of_any,))?.into())
}

/// Return true if `obj` is an instance of `cls` (PyType check).
pub(crate) fn is_instance(obj: &PyAny, cls: &PyType) -> bool {
    obj.is_instance(cls).unwrap_or(false)
}

/// Build the fallback sentinel as a Python `None`. The Python caller treats
/// `None` as "Rust did not handle this; use the Python visitor".
pub(crate) fn fallback_sentinel(py: Python<'_>) -> PyResult<PyObject> {
    Ok(py.None())
}

/// True if `obj` is the Python `None` sentinel (our fallback marker).
pub(crate) fn is_fallback(obj: &PyObject, py: Python<'_>) -> bool {
    obj.is(&py.None())
}
