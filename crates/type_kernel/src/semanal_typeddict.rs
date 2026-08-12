//! Native port of pure helper functions from `mypy/semanal_typeddict.py`
//! and `mypy/semanal_namedtuple.py` (Issue #532).
//!
//! These are the pure, non-mutating helpers used by `TypedDictAnalyzer` and
//! `NamedTupleAnalyzer`. Each mirrors the Python implementation and operates
//! on live Python type objects via PyO3, following the same strangler-fig
//! pattern as `semanal_visitor` and `semanal_classprop`.
//!
//! Ported functions:
//! - `extract_meta_info` — unwrap `RequiredType` / `ReadOnlyType` wrappers.
//! - `check_namedtuple_field_name` — validate NamedTuple field names.
//! - `NAMEDTUPLE_PROHIBITED_NAMES` — prohibited attribute names constant.
//! - `primary_source` — select primary FieldSource from a list.
//! - `verify_requiredness_compatibility` — requiredness conflict messages.
//! - `verify_field_against_closed_bases` — closed-base violation messages.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple, PyType};

/// `mypy.semanal_namedtuple.NAMEDTUPLE_PROHIBITED_NAMES` (lines 77-90).
///
/// Matches `_prohibited` in typing.py, plus `__annotations__`.
const NAMEDTUPLE_PROHIBITED_NAMES: &[&str] = &[
    "__new__",
    "__init__",
    "__slots__",
    "__getnewargs__",
    "_fields",
    "_field_defaults",
    "_field_types",
    "_make",
    "_replace",
    "_asdict",
    "_source",
    "__annotations__",
];

// ---------------------------------------------------------------------------
// extract_meta_info
// ---------------------------------------------------------------------------

/// `mypy.semanal_typeddict.TypedDictAnalyzer.extract_meta_info`
/// (semanal_typeddict.py:548-576).
///
/// Unwraps `RequiredType` and `ReadOnlyType` metadata wrappers from a `Type`.
/// Returns `(unwrapped_type, is_required, is_readonly)`.
/// `is_required` is `None` when no `RequiredType` wrapper was seen.
///
/// Does NOT emit nesting errors (those need `SemanticAnalyzer.fail` context);
/// only unwraps the type layers.
#[pyfunction]
pub(crate) fn rust_extract_meta_info(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<(PyObject, Option<bool>, bool)> {
    let types_mod = py.import("mypy.types")?;
    let required_cls: &PyType = types_mod.getattr("RequiredType")?.downcast()?;
    let readonly_cls: &PyType = types_mod.getattr("ReadOnlyType")?.downcast()?;

    let mut current = typ;
    let mut is_required: Option<bool> = None;
    let mut readonly = false;

    while current.is_instance(required_cls)? || current.is_instance(readonly_cls)? {
        if current.is_instance(required_cls)? {
            let required_val = current.getattr("required")?;
            is_required = Some(required_val.is_true()?);
            current = current.getattr("item")?;
        }
        if current.is_instance(readonly_cls)? {
            readonly = true;
            current = current.getattr("item")?;
        }
    }

    Ok((current.into_py(py), is_required, readonly))
}

// ---------------------------------------------------------------------------
// check_namedtuple_field_name
// ---------------------------------------------------------------------------

/// `mypy.semanal_namedtuple.NamedTupleAnalyzer.check_namedtuple_field_name`
/// (semanal_namedtuple.py:666-676).
///
/// Returns `None` for valid field names, `Some(error_message)` for invalid.
/// Checks: duplicate (in `seen_names`), valid identifier, no leading
/// underscore, not a Python keyword.
#[pyfunction]
pub(crate) fn rust_check_namedtuple_field_name(
    py: Python<'_>,
    field: &str,
    seen_names: &PyAny,
) -> PyResult<Option<String>> {
    // Duplicate check: `field in seen_names`.
    let in_seen: bool = seen_names.contains(field)?;
    if in_seen {
        return Ok(Some(format!("has duplicate field name \"{}\"", field)));
    }

    // isidentifier check via Python str method.
    let field_py = PyString::new(py, field);
    let is_id = field_py.call_method0("isidentifier")?;
    if !is_id.is_true()? {
        return Ok(Some(format!(
            "field name \"{}\" is not a valid identifier",
            field
        )));
    }

    if field.starts_with('_') {
        return Ok(Some(format!(
            "field name \"{}\" starts with an underscore",
            field
        )));
    }

    // keyword.iskeyword check.
    let keyword_mod = py.import("keyword")?;
    let is_keyword = keyword_mod.call_method1("iskeyword", (field,))?;
    if is_keyword.is_true()? {
        return Ok(Some(format!("field name \"{}\" is a keyword", field)));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// namedtuple_prohibited_names
// ---------------------------------------------------------------------------

/// `mypy.semanal_namedtuple.NAMEDTUPLE_PROHIBITED_NAMES` — return the
/// prohibited-names tuple as a Python tuple of strings.
#[pyfunction]
pub(crate) fn rust_namedtuple_prohibited_names(py: Python<'_>) -> PyResult<PyObject> {
    let py_strings: Vec<&PyAny> = NAMEDTUPLE_PROHIBITED_NAMES
        .iter()
        .map(|s| PyString::new(py, s) as &PyAny)
        .collect();
    Ok(PyTuple::new(py, &py_strings).into())
}

// ---------------------------------------------------------------------------
// primary_source
// ---------------------------------------------------------------------------

/// `mypy.semanal_typeddict.TypedDictAnalyzer.primary_source`
/// (semanal_typeddict.py:261-270).
///
/// Selects the primary source from a reverse-ordered list of `FieldSource`
/// objects. If the last source has no base, return it. Otherwise find the
/// last non-readonly source, or fall back to the last source.
#[pyfunction]
pub(crate) fn rust_primary_source(py: Python<'_>, sources: &PyAny) -> PyResult<PyObject> {
    let sources_list = sources.downcast::<PyList>()?;
    let len = sources_list.len();
    if len == 0 {
        return Ok(py.None());
    }

    let last = sources_list.get_item(len - 1)?;
    let last_base = last.getattr("base")?;
    if last_base.is_none() {
        return Ok(last.into());
    }

    // Walk reversed, find first non-readonly (i.e. last non-readonly).
    for i in (0..len).rev() {
        let src = sources_list.get_item(i)?;
        let is_readonly = src.getattr("is_readonly")?;
        if !is_readonly.is_true()? {
            return Ok(src.into());
        }
    }

    // Fall back to last.
    Ok(last.into())
}

// ---------------------------------------------------------------------------
// verify_requiredness_compatibility
// ---------------------------------------------------------------------------

/// `mypy.semanal_typeddict.TypedDictAnalyzer.verify_requiredness_compatibility`
/// (semanal_typeddict.py:272-303).
///
/// Returns `None` (no error) or `Some(error_message)`.
/// `source` has `.is_required`, `.is_readonly`, `.base` (with `.name`).
/// `primary_source_base` may be None.
#[pyfunction]
pub(crate) fn rust_verify_requiredness_compatibility(
    _py: Python<'_>,
    field_name: &str,
    source: &PyAny,
    is_required: bool,
    primary_source_base: &PyAny,
) -> PyResult<Option<String>> {
    let source_base = source.getattr("base")?;
    let source_base_name: String = source_base.getattr("name")?.extract()?;

    let source_is_required = source.getattr("is_required")?.is_true()?;
    let source_is_readonly = source.getattr("is_readonly")?.is_true()?;

    if source_is_required && !is_required {
        if primary_source_base.is_none() {
            return Ok(Some(format!(
                "Field \"{}\" is required in base class \"{}\"",
                field_name, source_base_name
            )));
        }
        let primary_name: String = primary_source_base.getattr("name")?.extract()?;
        return Ok(Some(format!(
            "Field \"{}\" is required in base class \"{}\" but can \
             be deleted in base class \"{}\"",
            field_name, source_base_name, primary_name
        )));
    }

    if !source_is_required && !source_is_readonly && is_required {
        if primary_source_base.is_none() {
            return Ok(Some(format!(
                "Field \"{}\" can be deleted in base class \"{}\"",
                field_name, source_base_name
            )));
        }
        let primary_name: String = primary_source_base.getattr("name")?.extract()?;
        return Ok(Some(format!(
            "Field \"{}\" is required in base class \"{}\" but can \
             be deleted in base class \"{}\"",
            field_name, primary_name, source_base_name
        )));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// verify_field_against_closed_bases
// ---------------------------------------------------------------------------

/// `mypy.semanal_typeddict.TypedDictAnalyzer.verify_field_against_closed_bases`
/// (semanal_typeddict.py:305-327).
///
/// `closed_bases` is a list of `(TypeInfo, field_names_collection)` tuples.
/// Returns a list of error messages (empty if no errors).
/// `primary_source_base` may be None.
#[pyfunction]
pub(crate) fn rust_verify_field_against_closed_bases(
    _py: Python<'_>,
    field_name: &str,
    closed_bases: &PyAny,
    primary_source_base: &PyAny,
) -> PyResult<Vec<String>> {
    let bases_list = closed_bases.downcast::<PyList>()?;
    let mut errors: Vec<String> = Vec::new();

    for entry in bases_list.iter() {
        let tuple = entry.downcast::<PyTuple>()?;
        let closed_base_type = tuple.get_item(0)?;
        let closed_base_fields = tuple.get_item(1)?;

        // `field_name in closed_base_fields`
        let in_fields: bool = closed_base_fields.contains(field_name)?;
        if in_fields {
            continue;
        }

        let closed_name: String = closed_base_type.getattr("name")?.extract()?;

        if !primary_source_base.is_none() {
            let primary_name: String = primary_source_base.getattr("name")?.extract()?;
            errors.push(format!(
                "Cannot extend closed base class \"{}\" with field \
                 \"{}\" from base class \"{}\"",
                closed_name, field_name, primary_name
            ));
        } else {
            errors.push(format!(
                "Cannot extend closed base class \"{}\" with new \
                 field \"{}\"",
                closed_name, field_name
            ));
        }
    }

    Ok(errors)
}
