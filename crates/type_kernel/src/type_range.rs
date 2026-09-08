//! `TypeChecker.get_type_range_of_type` leaf-decision port (mypy.checker,
//! issue #1464 C1).
//!
//! The Python method (checker.py:10396) is a 9-way dispatch over one proper
//! type: the `TypeVarType` upper-bound unroll and the `UnionType` item fold
//! are structural recursion and stay Python-side. The *leaf* decision (what
//! branch a non-union, non-typevar proper type hits) is ported here: Rust
//! reads the live proper type via PyO3 (`FunctionLike` + `is_type_obj`,
//! `TypeType` item shape, `AnyType`, and `Instance` fullnames), returns a
//! branch tag, and the Python shim applies the side effects
//! (`fill_typevars_with_any` / `erase_typevars` — both already native — and
//! the `is_subtype(builtins.type, typ)` gate) and keeps the pure-Python
//! leaf body as the fallback.
//!
//! Strangler-fig contract: `None` defers to Python. The only deferral is an
//! unreadable attribute on the live type. Every reachable leaf branch is
//! classified; the `builtins.type` `is_subtype` gate rides the `REST` tag
//! and runs Python-side (it is already native via the subtype resolver).

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyType};

/// Decision tags for `get_type_range_of_type`; must match
/// `NATIVE_TYPE_RANGE_*` in mypy/checker.py.
pub const TYPE_RANGE_FN_TYPEOBJ: i64 = 1;
pub const TYPE_RANGE_TYPETYPE: i64 = 2;
pub const TYPE_RANGE_ANY: i64 = 3;
pub const TYPE_RANGE_BUILTINS_TYPE: i64 = 4;
pub const TYPE_RANGE_TYPES_UNION: i64 = 5;
pub const TYPE_RANGE_SPECIAL_FORM: i64 = 6;
pub const TYPE_RANGE_REST: i64 = 7;

/// Which leaf class the proper type is. Mutually exclusive in practice (a
/// proper type is exactly one of these kinds), so the pure decision can
/// branch on a single kind instead of a bool forest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    FunctionLike,
    TypeType,
    Any,
    Instance,
    Other,
}

/// Pure leaf decision mirroring the checker.py:10405-10440 branch order.
/// Returns `(tag, typetype_is_upper_bound)`. The `typetype_is_upper_bound`
/// value is only meaningful for the `TYPE_RANGE_TYPETYPE` tag.
pub fn classify_type_range(
    kind: LeafKind,
    is_type_obj: bool,
    item_is_none: bool,
    item_is_final: bool,
    fullname: &str,
    args_len: usize,
) -> (i64, bool) {
    match kind {
        // D: FunctionLike and is_type_obj. A non-type-object FunctionLike
        // reaches the is_subtype gate (REST).
        LeafKind::FunctionLike => (
            if is_type_obj {
                TYPE_RANGE_FN_TYPEOBJ
            } else {
                TYPE_RANGE_REST
            },
            false,
        ),
        // E: TypeType — the is_upper_bound arbitration (item NoneType or
        // Instance is_final) collapses to `not item_is_none and not
        // item_is_final`, mirroring the two `is_upper_bound = False` writes.
        LeafKind::TypeType => (TYPE_RANGE_TYPETYPE, !item_is_none && !item_is_final),
        // F: AnyType.
        LeafKind::Any => (TYPE_RANGE_ANY, false),
        // G/H/I: Instance fullnames.
        LeafKind::Instance => match fullname {
            "builtins.type" => (TYPE_RANGE_BUILTINS_TYPE, false),
            "types.UnionType" => (
                if args_len > 0 {
                    TYPE_RANGE_TYPES_UNION
                } else {
                    TYPE_RANGE_REST
                },
                false,
            ),
            "typing._SpecialForm" => (TYPE_RANGE_SPECIAL_FORM, false),
            _ => (TYPE_RANGE_REST, false),
        },
        // Any other proper type (UninhabitedType, TupleType, Literal, ...)
        // reaches the is_subtype gate.
        LeafKind::Other => (TYPE_RANGE_REST, false),
    }
}

/// Fetch a class from `mypy.types`.
fn types_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.types")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// `#[pyfunction]` entry. Reads the live proper type (already
/// `get_proper_type`'d, `TypeVarType`-unrolled, and non-`UnionType` by the
/// Python shim) and returns `(tag, typetype_is_upper_bound)`, deferring
/// (`None`) on an unreadable attribute so the shim re-runs the pure-Python
/// leaf body.
#[pyfunction]
pub(crate) fn rust_classify_type_range(
    py: Python<'_>,
    typ: &PyAny,
) -> PyResult<Option<(i64, bool)>> {
    let classify: PyResult<Option<(i64, bool)>> = (|| {
        let fl_cls = types_class(py, "FunctionLike")?;
        let tt_cls = types_class(py, "TypeType")?;
        let any_cls = types_class(py, "AnyType")?;
        let inst_cls = types_class(py, "Instance")?;
        let none_cls = types_class(py, "NoneType")?;

        if typ.is_instance(fl_cls)? {
            let is_to = typ.call_method0("is_type_obj")?.extract::<bool>()?;
            return Ok(Some(classify_type_range(
                LeafKind::FunctionLike,
                is_to,
                false,
                false,
                "",
                0,
            )));
        }
        if typ.is_instance(tt_cls)? {
            let item = typ.getattr("item")?;
            let item_is_none = item.is_instance(none_cls)?;
            let item_is_final = if item.is_instance(inst_cls)? {
                item.getattr("type")?
                    .getattr("is_final")?
                    .extract::<bool>()?
            } else {
                false
            };
            return Ok(Some(classify_type_range(
                LeafKind::TypeType,
                false,
                item_is_none,
                item_is_final,
                "",
                0,
            )));
        }
        if typ.is_instance(any_cls)? {
            return Ok(Some(classify_type_range(
                LeafKind::Any,
                false,
                false,
                false,
                "",
                0,
            )));
        }
        if typ.is_instance(inst_cls)? {
            let fullname: String = typ.getattr("type")?.getattr("fullname")?.extract()?;
            let args_len: usize = typ.getattr("args")?.len()?;
            return Ok(Some(classify_type_range(
                LeafKind::Instance,
                false,
                false,
                false,
                &fullname,
                args_len,
            )));
        }
        Ok(Some(classify_type_range(
            LeafKind::Other,
            false,
            false,
            false,
            "",
            0,
        )))
    })();
    // Contract (#1466): an unreadable attribute defers (None), other PyErrs
    // stay visible so genuine kernel bugs surface in the parity tests.
    match classify {
        Ok(v) => Ok(v),
        Err(e) if e.is_instance_of::<PyAttributeError>(py) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod type_range_tests {
    use super::*;

    fn c(
        kind: LeafKind,
        is_type_obj: bool,
        item_is_none: bool,
        item_is_final: bool,
        fullname: &str,
        args_len: usize,
    ) -> (i64, bool) {
        classify_type_range(
            kind,
            is_type_obj,
            item_is_none,
            item_is_final,
            fullname,
            args_len,
        )
    }

    #[test]
    fn test_functionlike_typeobj() {
        assert_eq!(
            c(LeafKind::FunctionLike, true, false, false, "", 0),
            (TYPE_RANGE_FN_TYPEOBJ, false)
        );
    }

    #[test]
    fn test_functionlike_non_typeobj_is_rest() {
        assert_eq!(
            c(LeafKind::FunctionLike, false, false, false, "", 0),
            (TYPE_RANGE_REST, false)
        );
    }

    #[test]
    fn test_typetype_upper_by_default() {
        assert_eq!(
            c(LeafKind::TypeType, false, false, false, "", 0),
            (TYPE_RANGE_TYPETYPE, true)
        );
    }

    #[test]
    fn test_typetype_none_item_lower() {
        assert_eq!(
            c(LeafKind::TypeType, false, true, false, "", 0),
            (TYPE_RANGE_TYPETYPE, false)
        );
    }

    #[test]
    fn test_typetype_final_instance_lower() {
        assert_eq!(
            c(LeafKind::TypeType, false, false, true, "", 0),
            (TYPE_RANGE_TYPETYPE, false)
        );
    }

    #[test]
    fn test_any() {
        assert_eq!(
            c(LeafKind::Any, false, false, false, "", 0),
            (TYPE_RANGE_ANY, false)
        );
    }

    #[test]
    fn test_builtins_type() {
        assert_eq!(
            c(LeafKind::Instance, false, false, false, "builtins.type", 0),
            (TYPE_RANGE_BUILTINS_TYPE, false)
        );
    }

    #[test]
    fn test_types_union_with_args() {
        assert_eq!(
            c(
                LeafKind::Instance,
                false,
                false,
                false,
                "types.UnionType",
                2
            ),
            (TYPE_RANGE_TYPES_UNION, false)
        );
    }

    #[test]
    fn test_types_union_no_args_is_rest() {
        assert_eq!(
            c(
                LeafKind::Instance,
                false,
                false,
                false,
                "types.UnionType",
                0
            ),
            (TYPE_RANGE_REST, false)
        );
    }

    #[test]
    fn test_special_form() {
        assert_eq!(
            c(
                LeafKind::Instance,
                false,
                false,
                false,
                "typing._SpecialForm",
                0
            ),
            (TYPE_RANGE_SPECIAL_FORM, false)
        );
    }

    #[test]
    fn test_other_instance_is_rest() {
        assert_eq!(
            c(LeafKind::Instance, false, false, false, "mymod.Klass", 0),
            (TYPE_RANGE_REST, false)
        );
    }

    #[test]
    fn test_other_kind_is_rest() {
        assert_eq!(
            c(LeafKind::Other, false, false, false, "", 0),
            (TYPE_RANGE_REST, false)
        );
    }
}
