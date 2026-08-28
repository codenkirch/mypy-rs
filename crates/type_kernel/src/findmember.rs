//! Port of the `find_member` name-resolution prelude (issue #1074).
//!
//! `mypy.subtypes.find_member` (subtypes.py:2006) resolves a protocol
//! member by name before handing off to the heavy checkmember tail. The
//! ported head is the pure decision part: the `info.get(name)` miss
//! path, the `__getattribute__` / `__getattr__` scan, and the
//! `fallback_to_any` / `meta_fallback_to_any` / `extra_attrs` verdicts.
//! The `type_checker is None` gate and the `MemberContext` tail stay
//! Python.
//!
//! Live-PyO3-object seam in the `rust_is_final_enum_value` /
//! `rust_always_returns_none` shape: isinstance-free attribute and
//! method reads, zero wire bytes. Rust returns one of four tags
//! (PROCEED / ANY_SPECIAL_FORM / EXTRA_ATTR / NOT_FOUND); on
//! EXTRA_ATTR, Python fetches `itype.extra_attrs.attrs[name]` itself so
//! the live `Type` object never crosses the seam. PROCEED falls through
//! to the untouched pure-Python tail.
//!
//! Strangler-fig contract: any unreadable attribute or a failed method
//! call defers (`None`), and the shim re-runs the pure-Python body.

use pyo3::prelude::*;

/// Tag 0: the member resolved (direct hit or a custom accessor was
/// found); Python runs the untouched checkmember tail.
pub(crate) const TAG_PROCEED: i64 = 0;
/// Tag 1: the miss verdict is `AnyType(TypeOfAny.special_form)`.
const TAG_ANY_SPECIAL_FORM: i64 = 1;
/// Tag 2: the name lives in `itype.extra_attrs.attrs`; Python fetches
/// the attr (the Type must not cross the seam).
const TAG_EXTRA_ATTR: i64 = 2;
/// Tag 3: the member is not found; Python returns None.
const TAG_NOT_FOUND: i64 = 3;

/// Pure decision core of the `find_member` prelude (subtypes.py:2029-2047).
/// `node_found` is the `info.get(name)` hit; `dunder_scan_allowed` folds
/// the four-part gate (`name not in [...] and not is_operator and not
/// class_obj and itype.extra_attrs is None`); `custom_accessor_found` is
/// the `__getattribute__` / `__getattr__` scan outcome; `fallback_to_any`
/// and `meta_fallback_to_any` are the TypeInfo flags (the meta flag is
/// only consulted when `class_obj`, folded in by the caller);
/// `extra_attr_hit` is `name in itype.extra_attrs.attrs`.
fn find_member_prelude_decision(
    node_found: bool,
    dunder_scan_allowed: bool,
    custom_accessor_found: bool,
    fallback_to_any: bool,
    meta_fallback_to_any: bool,
    extra_attr_hit: bool,
) -> i64 {
    if node_found || (dunder_scan_allowed && custom_accessor_found) {
        return TAG_PROCEED;
    }
    if fallback_to_any || meta_fallback_to_any {
        return TAG_ANY_SPECIAL_FORM;
    }
    if extra_attr_hit {
        return TAG_EXTRA_ATTR;
    }
    TAG_NOT_FOUND
}

/// Live-object read of the prelude facts, mirroring the Python branch
/// order (subtypes.py:2029-2047). Errors propagate to the
/// `#[pyfunction]` boundary, which maps them to a deferral.
fn find_member_prelude_inner(
    name: &str,
    itype: &PyAny,
    is_operator: bool,
    class_obj: bool,
) -> PyResult<i64> {
    let info = itype.getattr("type")?;
    let sym = info.call_method1("get", (name,))?;
    let node_found = !sym.is_none() && !sym.getattr("node")?.is_none();
    if node_found {
        return Ok(TAG_PROCEED);
    }
    let extra_attrs = itype.getattr("extra_attrs")?;
    let dunder_scan_allowed = name != "__getattr__"
        && name != "__setattr__"
        && name != "__getattribute__"
        && !is_operator
        && !class_obj
        && extra_attrs.is_none();
    let custom_accessor_found = if dunder_scan_allowed {
        let mut found = false;
        for method_name in ["__getattribute__", "__getattr__"] {
            let method = info.call_method1("get_method", (method_name,))?;
            if !method.is_none() {
                let fullname: String = method.getattr("info")?.getattr("fullname")?.extract()?;
                if fullname != "builtins.object" {
                    found = true;
                    break;
                }
            }
        }
        found
    } else {
        false
    };
    let fallback_to_any: bool = info.getattr("fallback_to_any")?.extract()?;
    // `class_obj and info.meta_fallback_to_any` short-circuits in Python;
    // only read the flag when class_obj is set.
    let meta_fallback_to_any: bool = if class_obj {
        info.getattr("meta_fallback_to_any")?.extract()?
    } else {
        false
    };
    let extra_attr_hit = if !extra_attrs.is_none() {
        extra_attrs.getattr("attrs")?.contains(name)?
    } else {
        false
    };
    Ok(find_member_prelude_decision(
        node_found,
        dunder_scan_allowed,
        custom_accessor_found,
        fallback_to_any,
        meta_fallback_to_any,
        extra_attr_hit,
    ))
}

/// Name-resolution prelude of `mypy.subtypes.find_member` (#1074).
/// Reads the live `Instance` (`type`/`extra_attrs`) and its `TypeInfo`
/// (`get`, `get_method`, `fallback_to_any`, `meta_fallback_to_any`) via
/// PyO3 and returns a tag; the shim applies the Python-side effects.
/// Any unreadable fact defers (`None`) so the shim re-runs the
/// pure-Python body.
#[pyfunction]
#[pyo3(signature = (name, itype, is_operator, class_obj))]
pub(crate) fn rust_classify_find_member(
    name: &str,
    itype: &PyAny,
    is_operator: bool,
    class_obj: bool,
) -> PyResult<Option<i64>> {
    Ok(find_member_prelude_inner(name, itype, is_operator, class_obj).ok())
}

#[cfg(test)]
mod findmember_tests {
    use super::{
        find_member_prelude_decision, TAG_ANY_SPECIAL_FORM, TAG_EXTRA_ATTR, TAG_NOT_FOUND,
        TAG_PROCEED,
    };

    #[test]
    fn test_direct_hit_proceeds() {
        assert_eq!(
            find_member_prelude_decision(true, false, false, false, false, false),
            TAG_PROCEED
        );
    }

    #[test]
    fn test_direct_hit_beats_fallback() {
        assert_eq!(
            find_member_prelude_decision(true, false, false, true, false, false),
            TAG_PROCEED
        );
    }

    #[test]
    fn test_custom_getattribute_proceeds() {
        assert_eq!(
            find_member_prelude_decision(false, true, true, false, false, false),
            TAG_PROCEED
        );
    }

    #[test]
    fn test_plain_miss_not_found() {
        assert_eq!(
            find_member_prelude_decision(false, true, false, false, false, false),
            TAG_NOT_FOUND
        );
    }

    #[test]
    fn test_scan_disallowed_operator_miss_not_found() {
        // Operator-mode access skips the dunder scan; extra_attrs is
        // None, so a plain miss is NOT_FOUND.
        assert_eq!(
            find_member_prelude_decision(false, false, false, false, false, false),
            TAG_NOT_FOUND
        );
    }

    #[test]
    fn test_scan_disallowed_extra_attr_hit() {
        assert_eq!(
            find_member_prelude_decision(false, false, false, false, false, true),
            TAG_EXTRA_ATTR
        );
    }

    #[test]
    fn test_fallback_to_any() {
        assert_eq!(
            find_member_prelude_decision(false, true, false, true, false, false),
            TAG_ANY_SPECIAL_FORM
        );
    }

    #[test]
    fn test_meta_fallback_to_any() {
        assert_eq!(
            find_member_prelude_decision(false, false, false, false, true, false),
            TAG_ANY_SPECIAL_FORM
        );
    }

    #[test]
    fn test_fallback_beats_extra_attr() {
        assert_eq!(
            find_member_prelude_decision(false, false, false, true, false, true),
            TAG_ANY_SPECIAL_FORM
        );
    }
}
