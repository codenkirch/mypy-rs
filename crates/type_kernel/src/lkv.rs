//! Stage 2: `remove_instance_last_known_values` — mirrors
//! `mypy.erasetype.LastKnownValueEraser` (a `TypeTranslator`).
//!
//! Walks a live Python `mypy.types.Type` and strips `Instance.last_known_value`,
//! recursing on children. Returns `None` for any type class Rust does not
//! handle, so the Python caller falls back to the pure-Python visitor (the
//! strangler-fig per-call gate).
//!
//! Three overrides of `LastKnownValueEraser`:
//!   * `visit_instance`: strip `last_known_value`, recurse args.
//!   * `visit_type_alias_type`: identity (aliases can't contain literal values).
//!   * `visit_union_type`: translate items, then dedup `Instance` items with
//!     the same fullname via `make_simplified_union`.
//!
//! All other types use the `TypeTranslator` defaults (recursive translation
//! of children, identity on leaves), implemented here directly.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use crate::refs::{fallback_sentinel, is_fallback, is_instance, TypeRefs};

/// Translate a single `Type` through the `LastKnownValueEraser` logic.
///
/// Returns `None` (the fallback sentinel) for cases Rust does not handle —
/// the Python caller falls back to the pure-Python visitor.
fn lkv_translate_one(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    // --- Leaf types: TypeTranslator defaults are identity ---
    // AnyType, NoneType, UninhabitedType, ErasedType, DeletedType,
    // TypeVarType, ParamSpecType, TypeVarTupleType, PartialType, UnboundType
    // — all return `t` unchanged.
    if is_instance(obj, refs.any_type)
        || is_instance(obj, refs.none_type)
        || is_instance(obj, refs.uninhabited_type)
        || is_instance(obj, refs.deleted_type)
        || is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.type_var_tuple_type)
        || is_instance(obj, refs.literal_type)
    {
        return Ok(obj.into());
    }

    // --- TypeAliasType: return as-is (no recursion into alias target) ---
    if is_instance(obj, refs.type_alias_type) {
        return Ok(obj.into());
    }

    // --- Instance: the core override ---
    // Python:
    //   if not t.last_known_value and not t.args:
    //       return t
    //   return t.copy_modified(args=[a.accept(self) for a in t.args],
    //                          last_known_value=None)
    if is_instance(obj, refs.instance) {
        return lkv_visit_instance(py, obj, refs);
    }

    // --- UnionType: the dedup override ---
    // Python calls super().visit_union_type (translate all items), then
    // merges Instance items with the same fullname via make_simplified_union.
    if is_instance(obj, refs.union_type) {
        return lkv_visit_union(py, obj, refs);
    }

    // --- CallableType: TypeTranslator default ---
    // copy_modified(arg_types=translated, ret_type=translated)
    if is_instance(obj, refs.callable_type) {
        return lkv_visit_callable(py, obj, refs);
    }

    // --- TupleType: TypeTranslator default ---
    // TupleType(translated items, translated partial_fallback, line, column)
    if is_instance(obj, refs.tuple_type) {
        return lkv_visit_tuple(py, obj, refs);
    }

    // --- Overloaded: TypeTranslator default ---
    // Overloaded(items=[translated items])
    if is_instance(obj, refs.overloaded) {
        return lkv_visit_overloaded(py, obj, refs);
    }

    // --- TypeType: TypeTranslator default ---
    // TypeType.make_normalized(translated item, line, column, is_type_form)
    if is_instance(obj, refs.type_type) {
        return lkv_visit_type_type(py, obj, refs);
    }

    // --- LiteralType: TypeTranslator default ---
    // LiteralType(value, translated fallback, line, column)
    if is_instance(obj, refs.literal_type) {
        return lkv_visit_literal(py, obj, refs);
    }

    // --- UnpackType: TypeTranslator default ---
    // UnpackType(t.type.accept(self))
    if is_instance(obj, refs.unpack_type) {
        return lkv_visit_unpack(py, obj, refs);
    }

    // --- TypedDictType, Parameters, and anything else: fall back ---
    // TypedDictType has complex dict construction + caching; Parameters is
    // rare in this context. Fall back to Python for correctness.
    fallback_sentinel(py)
}

/// LastKnownValueEraser.visit_instance: strip last_known_value, recurse args.
fn lkv_visit_instance(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let has_lkv = !obj.getattr("last_known_value")?.is_none();
    let args = obj.getattr("args")?;

    // args is typed as tuple[Type, ...] but may arrive as a list in some
    // code paths; collect into a Vec to handle both uniformly. Treat
    // anything else as a fallback.
    let args_vec: Vec<&PyAny> = if let Ok(list) = args.downcast::<PyList>() {
        list.iter().collect()
    } else if let Ok(tuple) = args.downcast::<PyTuple>() {
        tuple.iter().collect()
    } else {
        return fallback_sentinel(py);
    };

    if !has_lkv && args_vec.is_empty() {
        return Ok(obj.into());
    }

    // Recurse on each arg. If any falls back, the whole instance falls back.
    // The Python visitor uses copy_modified(args=[a.accept(self) for a in t.args])
    // — it passes a list regardless of the input tuple/list shape.
    let mut translated_args: Vec<PyObject> = Vec::with_capacity(args_vec.len());
    for arg in &args_vec {
        let translated = lkv_translate_one(py, arg, refs)?;
        if is_fallback(&translated, py) {
            return fallback_sentinel(py);
        }
        translated_args.push(translated);
    }

    let kwargs = PyDict::new(py);
    kwargs.set_item("args", PyList::new(py, &translated_args))?;
    kwargs.set_item("last_known_value", py.None())?;
    let copy_modified = obj.getattr("copy_modified")?;
    let result = copy_modified.call((), Some(kwargs))?;
    Ok(result.into())
}

/// LastKnownValueEraser.visit_union_type: translate items, then dedup
/// Instance items with the same fullname via make_simplified_union.
fn lkv_visit_union(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;

    // Translate all items (TypeTranslator.visit_union_type default).
    let mut translated: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated.push(t);
    }

    // Python logic:
    //   instances = [item for item in new.items if isinstance(get_proper_type(item), Instance)]
    //   if len(instances) > 1:
    //       ... group by fullname, merge groups >1 via make_simplified_union ...
    //   return new  (if <=1 instance)
    //
    // We replicate the dedup: collect proper-type Instance items with no args,
    // group by fullname, and call make_simplified_union for groups with >1.

    let types_mod = py.import("mypy.types")?;
    let get_proper_type = types_mod.getattr("get_proper_type")?;

    // First pass: resolve proper types and check if dedup is needed.
    let proper: Vec<PyObject> = translated
        .iter()
        .map(|t| get_proper_type.call1((t,)).map(|r| r.into()))
        .collect::<PyResult<Vec<_>>>()?;

    // Count Instance items (with no args) per fullname.
    let mut groups_by_name: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    let mut fullnames: Vec<Option<String>> = Vec::with_capacity(proper.len());
    for p in &proper {
        let is_inst = is_instance(p.as_ref(py), refs.instance);
        if is_inst {
            // args is typed as tuple[Type, ...] but may be a list; accept both.
            let args_obj = p.as_ref(py).getattr("args")?;
            let no_args = if let Ok(list) = args_obj.downcast::<PyList>() {
                list.is_empty()
            } else if let Ok(tuple) = args_obj.downcast::<PyTuple>() {
                tuple.is_empty()
            } else {
                // Unexpected args type — be conservative, skip grouping.
                false
            };
            if no_args {
                let fullname_obj = p
                    .as_ref(py)
                    .getattr("type")?
                    .getattr("fullname")?;
                let fullname: String = fullname_obj
                    .downcast::<PyString>()?
                    .to_str()?
                    .to_string();
                fullnames.push(Some(fullname.clone()));
                let idx = fullnames.len() - 1;
                groups_by_name
                    .entry(fullname)
                    .or_default()
                    .push(idx);
                continue;
            }
        }
        fullnames.push(None);
    }

    // If no group has >1 member, no dedup needed — construct the union.
    let needs_dedup = groups_by_name.values().any(|v| v.len() > 1);
    if !needs_dedup {
        let translated_list = PyList::new(py, &translated);
        let result = types_mod
            .getattr("UnionType")?
            .getattr("make_union")?
            .call1((translated_list,))?;
        return Ok(result.into());
    }

    // Dedup: build merged list, calling make_simplified_union for groups.
    let typeops = py.import("mypy.typeops")?;
    let make_simplified = typeops.getattr("make_simplified_union")?;

    // Track which indices have been consumed by a merge.
    let mut consumed: Vec<bool> = vec![false; translated.len()];
    let mut merged: Vec<PyObject> = Vec::with_capacity(translated.len());

    for (i, p) in proper.iter().enumerate() {
        if consumed[i] {
            continue;
        }
        match &fullnames[i] {
            None => {
                // Not an Instance with no args — keep original translated item.
                merged.push(translated[i].clone_ref(py));
            }
            Some(name) => {
                let group = groups_by_name.get(name).unwrap();
                if group.len() <= 1 {
                    // Single instance — keep as-is (use proper type).
                    merged.push(p.clone_ref(py));
                } else {
                    // Merge the group via make_simplified_union.
                    let group_items: Vec<PyObject> = group
                        .iter()
                        .map(|&idx| {
                            consumed[idx] = true;
                            proper[idx].clone_ref(py)
                        })
                        .collect();
                    let group_list = PyList::new(py, &group_items);
                    let simplified = make_simplified.call1((group_list,))?;
                    merged.push(simplified.into());
                }
            }
        }
    }

    let merged_list = PyList::new(py, &merged);
    let result = types_mod
        .getattr("UnionType")?
        .getattr("make_union")?
        .call1((merged_list,))?;
    Ok(result.into())
}

/// TypeTranslator.visit_callable_type default: copy_modified with translated
/// arg_types and ret_type. variables are passed through unchanged
/// (translate_variables returns them as-is).
fn lkv_visit_callable(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let arg_types = obj.getattr("arg_types")?.downcast::<PyList>()?;
    let mut translated_args: Vec<PyObject> = Vec::with_capacity(arg_types.len());
    for arg in arg_types.iter() {
        let t = lkv_translate_one(py, arg, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_args.push(t);
    }

    let ret_type = obj.getattr("ret_type")?;
    let translated_ret = lkv_translate_one(py, ret_type, refs)?;
    if is_fallback(&translated_ret, py) {
        return fallback_sentinel(py);
    }

    // instance_type: translate if present (TypeTranslator default).
    let instance_type = obj.getattr("instance_type")?;
    let translated_instance_type: Option<PyObject> = if !instance_type.is_none() {
        let t = lkv_translate_one(py, instance_type, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        Some(t)
    } else {
        None
    };

    let kwargs = PyDict::new(py);
    kwargs.set_item("arg_types", PyList::new(py, &translated_args))?;
    kwargs.set_item("ret_type", &translated_ret)?;
    if let Some(it) = &translated_instance_type {
        kwargs.set_item("instance_type", it)?;
    }
    let copy_modified = obj.getattr("copy_modified")?;
    let result = copy_modified.call((), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_tuple_type default: TupleType(translated items,
/// translated partial_fallback, line, column).
fn lkv_visit_tuple(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;
    let mut translated_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_items.push(t);
    }

    let partial_fallback = obj.getattr("partial_fallback")?;
    let translated_fallback = lkv_translate_one(py, partial_fallback, refs)?;
    if is_fallback(&translated_fallback, py) {
        return fallback_sentinel(py);
    }

    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let items_list = PyList::new(py, &translated_items);
    let result = refs.tuple_type.call1((items_list, translated_fallback, line, column))?;
    Ok(result.into())
}

/// TypeTranslator.visit_overloaded default: Overloaded(items=[translated items]).
fn lkv_visit_overloaded(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let items = obj.getattr("items")?.downcast::<PyList>()?;
    let mut translated_items: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let t = lkv_translate_one(py, item, refs)?;
        if is_fallback(&t, py) {
            return fallback_sentinel(py);
        }
        translated_items.push(t);
    }
    let items_list = PyList::new(py, &translated_items);
    let kwargs = PyDict::new(py);
    kwargs.set_item("items", items_list)?;
    let result = refs.overloaded.call((), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_type_type default: TypeType.make_normalized(
/// translated item, line, column, is_type_form).
fn lkv_visit_type_type(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let item = obj.getattr("item")?;
    let translated = lkv_translate_one(py, item, refs)?;
    if is_fallback(&translated, py) {
        return fallback_sentinel(py);
    }
    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let is_type_form = obj.getattr("is_type_form")?;
    let make_normalized = refs.type_type.getattr("make_normalized")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("line", line)?;
    kwargs.set_item("column", column)?;
    kwargs.set_item("is_type_form", is_type_form)?;
    let result = make_normalized.call((translated,), Some(kwargs))?;
    Ok(result.into())
}

/// TypeTranslator.visit_literal_type default: LiteralType(value,
/// translated fallback, line, column).
fn lkv_visit_literal(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let value = obj.getattr("value")?;
    let fallback = obj.getattr("fallback")?;
    let translated_fallback = lkv_translate_one(py, fallback, refs)?;
    if is_fallback(&translated_fallback, py) {
        return fallback_sentinel(py);
    }
    let line = obj.getattr("line")?;
    let column = obj.getattr("column")?;
    let result = refs.literal_type.call1((value, translated_fallback, line, column))?;
    Ok(result.into())
}

/// TypeTranslator.visit_unpack_type default: UnpackType(t.type.accept(self)).
fn lkv_visit_unpack(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> PyResult<PyObject> {
    let typ = obj.getattr("type")?;
    let translated = lkv_translate_one(py, typ, refs)?;
    if is_fallback(&translated, py) {
        return fallback_sentinel(py);
    }
    let result = refs.unpack_type.call1((translated,))?;
    Ok(result.into())
}

/// Native `remove_instance_last_known_values(typ) -> Type | None`.
///
/// Returns `None` when the Rust path does not handle `typ` or one of its
/// sub-components; the Python caller falls back to the pure-Python
/// `LastKnownValueEraser`. Stage 2 of the type-kernel migration.
#[pyfunction]
pub(crate) fn remove_instance_last_known_values(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<PyObject> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return fallback_sentinel(py),
    };
    lkv_translate_one(py, typ, &refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stage 2 parity: `remove_instance_last_known_values` on an Instance with
    /// a `last_known_value` strips it, matching the Python visitor. Compares
    /// Rust output against `mypy.erasetype.remove_instance_last_known_values`.
    #[test]
    fn lkv_strips_last_known_value_from_instance() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.erasetype import remove_instance_last_known_values as py_lkv
fx = TypeFixture(COVARIANT)
# fx.lit1_inst is an Instance with a last_known_value (Literal[1]).
typ = fx.lit1_inst
expected = str(py_lkv(typ))
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let expected: String = locals
                .get_item("expected")
                .unwrap()
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let typ = locals.get_item("typ").unwrap().unwrap();
            let result = super::remove_instance_last_known_values(py, typ).unwrap();
            assert!(!result.is_none(py), "Rust path should not fall back here");
            let builtins = py.import("builtins").unwrap();
            let result_str: String = builtins
                .getattr("str")
                .unwrap()
                .call1((&result,))
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            assert_eq!(result_str, expected);
        });
    }

    /// Stage 2 parity: union dedup — `make_union([lit1_inst, lit2_inst, lit4_inst])`
    /// collapses to a single Instance after LKV erasure, matching the Python path.
    #[test]
    fn lkv_merges_union_of_same_fullname_instances() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
from mypy.test.typefixture import TypeFixture
from mypy.nodes import COVARIANT
from mypy.types import UnionType
from mypy.erasetype import remove_instance_last_known_values as py_lkv
fx = TypeFixture(COVARIANT)
typ = UnionType.make_union([fx.lit1_inst, fx.lit2_inst, fx.lit4_inst])
expected = str(py_lkv(typ))
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let expected: String = locals
                .get_item("expected")
                .unwrap()
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let typ = locals.get_item("typ").unwrap().unwrap();
            let result = super::remove_instance_last_known_values(py, typ).unwrap();
            assert!(!result.is_none(py), "Rust path should not fall back here");
            let builtins = py.import("builtins").unwrap();
            let result_str: String = builtins
                .getattr("str")
                .unwrap()
                .call1((&result,))
                .unwrap()
                .downcast::<PyString>()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            assert_eq!(result_str, expected);
        });
    }
}
