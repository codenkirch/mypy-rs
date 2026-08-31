//! Native port of the metaclass resolution decision heads of
//! `mypy.semanal.SemanticAnalyzer` (issue #1037):
//! `get_declared_metaclass` (semanal.py:3767-3835) and
//! `recalculate_metaclass` (semanal.py:3837-3856).
//!
//! `get_declared_metaclass` runs a strictly sequential gate chain over the
//! metaclass expression's resolved symbol: dynamic name (not a
//! Name/Member chain), failed lookup, `Var` with an `Any` proper type,
//! `PlaceholderNode` deferral, type-alias unwrap, non-TypeInfo or
//! tuple-named class, and non-metaclass class. Rust owns that classification;
//! Python keeps every side effect (the four `self.fail` calls,
//! `lookup_qualified` itself, the alias unwrap feeding `meta_info`, and the
//! `fill_typevars` Instance construction). The shim early-returns the common
//! `metaclass_expr is None` case before calling Rust.
//!
//! `recalculate_metaclass` is the per-class tail: after Python writes
//! `declared_metaclass` / `metaclass_type` (via the live
//! `calculate_metaclass_type`), Rust folds the protocol-MRO scan and the
//! enum scan into one exclusive 4-way tag. Python keeps the
//! `named_type_or_none("abc.ABCMeta")` write and the
//! `is_enum = True` / "Enum class cannot be generic" fail.
//!
//! A `None` result means an unreadable fact and the pure-Python body runs
//! unchanged.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};

use crate::checker_visitor::rust_typeinfo_is_metaclass;
use crate::wire::{read_type, ReadBuffer, Type};

/// Decision tags for `get_declared_metaclass`, mirroring the sequential
/// gate chain (semanal.py:3778-3835):
/// - `META_OK`: valid metaclass TypeInfo; Python does `fill_typevars`
///   and returns `(inst, False, False)`.
/// - `META_DYNAMIC`: metaclass name not representable; Python fails
///   'Dynamic metaclass not supported for "..."', returns `(None, False, True)`.
/// - `META_NAME_ERROR`: lookup failed (name error handled elsewhere);
///   returns `(None, False, True)`.
/// - `META_ANY`: `Var` symbol with an `Any` proper type; Python fails
///   'Class cannot use "..." as a metaclass' only under
///   `disallow_subclassing_any`, returns `(None, False, True)`.
/// - `META_DEFER`: `PlaceholderNode` symbol; returns `(None, True, False)`.
/// - `META_INVALID`: symbol not a TypeInfo (or a tuple-named class);
///   Python fails 'Invalid metaclass "..."', returns `(None, False, False)`.
/// - `META_NOT_METACLASS`: class does not inherit from `type`; Python fails
///   'Metaclasses not inheriting from "type" are not supported',
///   returns `(None, False, False)`.
pub(crate) const META_OK: i64 = 1;
pub(crate) const META_DYNAMIC: i64 = 2;
pub(crate) const META_NAME_ERROR: i64 = 3;
pub(crate) const META_ANY: i64 = 4;
pub(crate) const META_DEFER: i64 = 5;
pub(crate) const META_INVALID: i64 = 6;
pub(crate) const META_NOT_METACLASS: i64 = 7;

/// Decision tags for `recalculate_metaclass` (semanal.py:3837-3856):
/// - `RECALC_OK`: nothing to do.
/// - `RECALC_ABCMETA`: protocol in the MRO with no/default metaclass;
///   Python runs `named_type_or_none("abc.ABCMeta", [])` and installs it.
/// - `RECALC_IS_ENUM`: metaclass has the `enum.EnumMeta` base;
///   Python sets `is_enum = True`.
/// - `RECALC_ENUM_GENERIC_FAIL`: same plus a generic class def; Python also
///   fails "Enum class cannot be generic".
pub(crate) const RECALC_OK: i64 = 0;
pub(crate) const RECALC_ABCMETA: i64 = 1;
pub(crate) const RECALC_IS_ENUM: i64 = 2;
pub(crate) const RECALC_ENUM_GENERIC_FAIL: i64 = 3;

/// Pure decision core of the `get_declared_metaclass` gate chain. PyO3-free
/// so the decision table is unit-tested directly. The shim supplies:
/// `mc_name` (None when the expression is not a Name/Member chain),
/// `sym_missing` (the `lookup_qualified` result was None), the
/// Var/Placeholder isinstance facts (None when unreadable), `var_any`
/// (whether the Var symbol's proper type is `AnyType`; None when the wire
/// bytes are undecodable), and the three metaclass-info facts (None when
/// unreadable). Branch order mirrors Python exactly.
#[allow(clippy::too_many_arguments)]
fn classify_declared_metaclass_inner(
    mc_name: Option<&str>,
    sym_missing: bool,
    sym_is_var: Option<bool>,
    sym_is_placeholder: Option<bool>,
    var_any: Option<bool>,
    meta_is_typeinfo: Option<bool>,
    meta_has_tuple_type: Option<bool>,
    meta_is_metaclass: Option<bool>,
) -> Option<i64> {
    if mc_name.is_none() {
        return Some(META_DYNAMIC);
    }
    if sym_missing {
        return Some(META_NAME_ERROR);
    }
    if sym_is_var? {
        // `isinstance(sym.node, Var) and isinstance(get_proper_type(...), AnyType)`;
        // a Var symbol that is not Any falls through to the TypeInfo gate and
        // fails there (a Var is never a TypeInfo).
        if var_any? {
            return Some(META_ANY);
        }
    }
    if sym_is_placeholder? {
        return Some(META_DEFER);
    }
    let is_typeinfo = meta_is_typeinfo?;
    // Python short-circuits: `tuple_type` is only read on a TypeInfo.
    if !is_typeinfo {
        return Some(META_INVALID);
    }
    if meta_has_tuple_type? {
        return Some(META_INVALID);
    }
    if !meta_is_metaclass? {
        return Some(META_NOT_METACLASS);
    }
    Some(META_OK)
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `SemanticAnalyzer.get_declared_metaclass` decision head (semanal.py:3767).
/// The shim performs `lookup_qualified` (Python state), the alias unwrap into
/// `meta_info` (pure reads mirroring semanal.py:3808-3816), and the one wire
/// serialization of a Var symbol's proper type. Rust owns the gate chain:
/// the Var-Any arm decodes the wire bytes, the metaclass checks read the
/// live TypeInfo via PyO3 (`tuple_type`, `is_metaclass` via
/// `rust_typeinfo_is_metaclass`). Python applies the four `self.fail` calls,
/// the `disallow_subclassing_any` option gate, and `fill_typevars`.
/// Defers (`None`) on any unreadable attribute or undecodable wire bytes.
#[pyfunction]
#[pyo3(signature = (mc_name, sym_node, var_type_wire, meta_info))]
pub(crate) fn rust_classify_declared_metaclass(
    py: Python<'_>,
    mc_name: Option<String>,
    sym_node: Option<&PyAny>,
    var_type_wire: Option<&[u8]>,
    meta_info: Option<&PyAny>,
) -> PyResult<Option<i64>> {
    let nodes_mod = py.import("mypy.nodes")?;
    let var_cls: &PyType = nodes_mod.getattr("Var")?.downcast()?;
    let placeholder_cls: &PyType = nodes_mod.getattr("PlaceholderNode")?.downcast()?;

    let sym_missing = sym_node.is_none();
    let sym_is_var = match sym_node {
        Some(node) => node.is_instance(var_cls).ok(),
        None => Some(false),
    };
    let sym_is_placeholder = match sym_node {
        Some(node) => node.is_instance(placeholder_cls).ok(),
        None => Some(false),
    };
    // The shim passes None wire bytes when the Var has no type at all
    // (`get_proper_type(None)` is not `AnyType`); garbage bytes defer.
    let var_any = if sym_is_var == Some(true) {
        match var_type_wire {
            None => Some(false),
            Some(bytes) => match decode_type(bytes) {
                Some(Type::AnyType { .. }) => Some(true),
                Some(_) => Some(false),
                None => None,
            },
        }
    } else {
        Some(false)
    };
    let (meta_is_typeinfo, meta_has_tuple_type, meta_is_metaclass) = match meta_info {
        Some(info) => {
            let typeinfo_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;
            let is_typeinfo = match info.is_instance(typeinfo_cls) {
                Ok(b) => b,
                Err(_) => return Ok(None),
            };
            // Python short-circuits: `tuple_type` / `is_metaclass()` are only
            // read on a TypeInfo; a Var or PlaceholderNode symbol fails at
            // the isinstance gate without touching further attributes.
            if is_typeinfo {
                let has_tuple = match info.getattr("tuple_type") {
                    Ok(t) => !t.is_none(),
                    Err(_) => return Ok(None),
                };
                let is_meta = match rust_typeinfo_is_metaclass(py, info, false) {
                    Ok(b) => b,
                    Err(_) => return Ok(None),
                };
                (Some(true), Some(has_tuple), Some(is_meta))
            } else {
                (Some(false), None, None)
            }
        }
        None => (None, None, None),
    };
    Ok(classify_declared_metaclass_inner(
        mc_name.as_deref(),
        sym_missing,
        sym_is_var,
        sym_is_placeholder,
        var_any,
        meta_is_typeinfo,
        meta_has_tuple_type,
        meta_is_metaclass,
    ))
}

/// Pure decision core of the `recalculate_metaclass` tail. The arms are
/// exclusive by construction: when the ABCMeta replacement fires, the
/// (possibly skipped) replacement installs `abc.ABCMeta` — never an
/// enum-metaclass — or leaves a `None`/`builtins.type` metaclass in place
/// when the named type is unavailable, so the enum scan never fires on the
/// same class. Branch order mirrors Python: the protocol-MRO block runs
/// before the enum scan, which reads the post-replacement metaclass.
fn classify_recalculate_metaclass_inner(
    any_protocol_mro: bool,
    meta_present: bool,
    meta_is_builtins_type: Option<bool>,
    meta_is_enum: Option<bool>,
    type_vars_nonempty: bool,
) -> Option<i64> {
    if any_protocol_mro && (!meta_present || meta_is_builtins_type?) {
        return Some(RECALC_ABCMETA);
    }
    if !meta_present {
        return Some(RECALC_OK);
    }
    match meta_is_enum? {
        true if type_vars_nonempty => Some(RECALC_ENUM_GENERIC_FAIL),
        true => Some(RECALC_IS_ENUM),
        false => Some(RECALC_OK),
    }
}

/// `SemanticAnalyzer.recalculate_metaclass` decision head (semanal.py:3837).
/// The shim performs the two unconditional writes (`declared_metaclass`,
/// `metaclass_type = calculate_metaclass_type()`) before calling; Rust owns
/// the protocol-MRO scan plus the enum scan. Python keeps
/// `named_type_or_none("abc.ABCMeta")`, the `is_enum = True` write, and the
/// "Enum class cannot be generic" fail. Defers (`None`) on any unreadable
/// attribute (in practice never: every field is a plain scalar read).
#[pyfunction]
#[pyo3(signature = (defn))]
pub(crate) fn rust_classify_recalculate_metaclass(defn: &PyAny) -> PyResult<Option<i64>> {
    let info = match defn.getattr("info") {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    let mut any_protocol_mro = false;
    match info.getattr("mro") {
        Ok(mro) => match mro.downcast::<PyList>() {
            Ok(mro_list) => {
                for cls in mro_list.iter() {
                    match cls.getattr("is_protocol").and_then(|v| v.is_true()) {
                        Ok(true) => {
                            any_protocol_mro = true;
                            break;
                        }
                        Ok(false) => {}
                        Err(_) => return Ok(None),
                    }
                }
            }
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    }
    let meta = match info.getattr("metaclass_type") {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    let meta_present = !meta.is_none();
    let meta_is_builtins_type = if meta_present {
        match meta.getattr("type").and_then(|t| t.getattr("fullname")) {
            Ok(f) => match f.extract::<String>() {
                Ok(s) => Some(s == "builtins.type"),
                Err(_) => None,
            },
            Err(_) => None,
        }
    } else {
        Some(false)
    };
    let meta_is_enum = if meta_present {
        match meta
            .getattr("type")
            .and_then(|t| t.call_method1("has_base", ("enum.EnumMeta",)))
        {
            Ok(b) => b.is_true().ok(),
            Err(_) => None,
        }
    } else {
        Some(false)
    };
    let type_vars_nonempty = match defn.getattr("type_vars").and_then(|t| t.is_true()) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok(classify_recalculate_metaclass_inner(
        any_protocol_mro,
        meta_present,
        meta_is_builtins_type,
        meta_is_enum,
        type_vars_nonempty,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn classify_declared(
        mc_name: Option<&str>,
        sym_missing: bool,
        sym_is_var: Option<bool>,
        sym_is_placeholder: Option<bool>,
        var_any: Option<bool>,
        meta_is_typeinfo: Option<bool>,
        meta_has_tuple_type: Option<bool>,
        meta_is_metaclass: Option<bool>,
    ) -> Option<i64> {
        classify_declared_metaclass_inner(
            mc_name,
            sym_missing,
            sym_is_var,
            sym_is_placeholder,
            var_any,
            meta_is_typeinfo,
            meta_has_tuple_type,
            meta_is_metaclass,
        )
    }

    #[test]
    fn dynamic_metaclass_tag() {
        // Not a Name/Member chain: `class C(metaclass=f(x))`.
        assert_eq!(
            classify_declared(None, false, None, None, None, None, None, None),
            Some(META_DYNAMIC)
        );
    }

    #[test]
    fn name_error_tag() {
        assert_eq!(
            classify_declared(Some("M"), true, None, None, None, None, None, None),
            Some(META_NAME_ERROR)
        );
    }

    #[test]
    fn any_var_tag_and_option_is_python_side() {
        // The option split lives in Python; the tag is the same either way.
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(true),
                Some(false),
                Some(true),
                None,
                None,
                None
            ),
            Some(META_ANY)
        );
    }

    #[test]
    fn var_without_any_falls_to_invalid() {
        // A Var metaclass whose type is not Any is not a TypeInfo -> INVALID.
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(true),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                None
            ),
            Some(META_INVALID)
        );
    }

    #[test]
    fn var_without_type_is_not_any() {
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(true),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                None
            ),
            Some(META_INVALID)
        );
    }

    #[test]
    fn placeholder_defers() {
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(false),
                Some(true),
                None,
                None,
                None,
                None
            ),
            Some(META_DEFER)
        );
    }

    #[test]
    fn non_typeinfo_symbol_is_invalid() {
        // An unwrapped TypeAlias or other node -> INVALID.
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(false),
                Some(false),
                None,
                Some(false),
                Some(false),
                None
            ),
            Some(META_INVALID)
        );
    }

    #[test]
    fn tuple_named_class_is_invalid() {
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(false),
                Some(false),
                None,
                Some(true),
                Some(true),
                Some(true)
            ),
            Some(META_INVALID)
        );
    }

    #[test]
    fn non_metaclass_class_tag() {
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(false),
                Some(false),
                None,
                Some(true),
                Some(false),
                Some(false)
            ),
            Some(META_NOT_METACLASS)
        );
    }

    #[test]
    fn valid_metaclass_ok() {
        assert_eq!(
            classify_declared(
                Some("M"),
                false,
                Some(false),
                Some(false),
                None,
                Some(true),
                Some(false),
                Some(true)
            ),
            Some(META_OK)
        );
    }

    #[test]
    fn unreadable_facts_defer() {
        assert!(classify_declared(
            Some("M"),
            false,
            None,
            Some(false),
            None,
            Some(true),
            Some(false),
            Some(true)
        )
        .is_none());
        assert!(classify_declared(
            Some("M"),
            false,
            Some(true),
            Some(false),
            None,
            Some(true),
            Some(false),
            Some(true)
        )
        .is_none());
        assert!(classify_declared(
            Some("M"),
            false,
            Some(false),
            Some(false),
            None,
            Some(true),
            None,
            Some(true)
        )
        .is_none());
        assert!(classify_declared(
            Some("M"),
            false,
            Some(false),
            Some(false),
            None,
            Some(true),
            Some(false),
            None
        )
        .is_none());
    }

    fn classify_recalc(
        any_protocol_mro: bool,
        meta_present: bool,
        meta_is_builtins_type: Option<bool>,
        meta_is_enum: Option<bool>,
        type_vars_nonempty: bool,
    ) -> Option<i64> {
        classify_recalculate_metaclass_inner(
            any_protocol_mro,
            meta_present,
            meta_is_builtins_type,
            meta_is_enum,
            type_vars_nonempty,
        )
    }

    #[test]
    fn recalc_plain_class_ok() {
        assert_eq!(
            classify_recalc(false, false, None, None, false),
            Some(RECALC_OK)
        );
        assert_eq!(
            classify_recalc(false, true, Some(false), Some(false), false),
            Some(RECALC_OK)
        );
    }

    #[test]
    fn recalc_protocol_mro_sets_abcmeta() {
        assert_eq!(
            classify_recalc(true, false, None, None, false),
            Some(RECALC_ABCMETA)
        );
        assert_eq!(
            classify_recalc(true, true, Some(true), None, false),
            Some(RECALC_ABCMETA)
        );
    }

    #[test]
    fn recalc_protocol_with_real_metaclass_skips_abc() {
        // Protocol MRO but an explicit non-type metaclass: ABC block skips,
        // enum scan reads the unchanged metaclass.
        assert_eq!(
            classify_recalc(true, true, Some(false), Some(false), false),
            Some(RECALC_OK)
        );
    }

    #[test]
    fn recalc_enum_metaclass() {
        assert_eq!(
            classify_recalc(false, true, Some(false), Some(true), false),
            Some(RECALC_IS_ENUM)
        );
    }

    #[test]
    fn recalc_enum_generic_fails() {
        assert_eq!(
            classify_recalc(false, true, Some(false), Some(true), true),
            Some(RECALC_ENUM_GENERIC_FAIL)
        );
    }

    #[test]
    fn recalc_unreadable_enum_base_defers() {
        assert!(classify_recalc(false, true, Some(false), None, false).is_none());
    }

    #[test]
    fn recalc_unreadable_builtins_type_defers_only_on_protocol_mro() {
        // Without a protocol in the MRO the builtins.type fact is unused.
        assert_eq!(
            classify_recalc(false, true, None, Some(false), false),
            Some(RECALC_OK)
        );
        assert!(classify_recalc(true, true, None, Some(false), false).is_none());
    }

    #[test]
    fn recalc_constants_are_distinct() {
        assert_ne!(RECALC_OK, RECALC_ABCMETA);
        assert_ne!(RECALC_OK, RECALC_IS_ENUM);
        assert_ne!(RECALC_OK, RECALC_ENUM_GENERIC_FAIL);
        assert_ne!(RECALC_ABCMETA, RECALC_IS_ENUM);
        assert_ne!(RECALC_IS_ENUM, RECALC_ENUM_GENERIC_FAIL);
    }

    #[test]
    fn meta_constants_are_distinct() {
        let tags = [
            META_OK,
            META_DYNAMIC,
            META_NAME_ERROR,
            META_ANY,
            META_DEFER,
            META_INVALID,
            META_NOT_METACLASS,
        ];
        for (i, a) in tags.iter().enumerate() {
            for b in &tags[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
