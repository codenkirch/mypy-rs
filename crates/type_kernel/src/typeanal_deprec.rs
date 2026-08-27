#![allow(non_local_definitions)]

//! Native port of the deprecation-warn arbitration head of
//! `mypy.typeanal.TypeAnalyser.check_and_warn_deprecated`
//! (typeanal.py:1466-1483, issue #1002).

use pyo3::prelude::*;

// Result tags handed to the Python shim. The shim applies the note/fail
// side effect; SILENT means the original body would have done nothing.
const TAG_DEPRECATED_SILENT: i64 = 0;
const TAG_DEPRECATED_NOTE: i64 = 1;
const TAG_DEPRECATED_FAIL: i64 = 2;

/// `check_and_warn_deprecated` arbitration classifier. Mirrors the gate
/// chain of typeanal.py:1469-1483 and returns the outcome tag; never
/// defers (every fact is a scalar or string, so `Some(tag)` always).
///
/// Facts (all scalars / strings, no live type objects):
/// - `deprecated`: `info.deprecated` (None or empty means no warning).
/// - `is_typeshed_stub`: `self.is_typeshed_stub`.
/// - `api_type_fullname`: `self.api.type.fullname` or None.
/// - `info_fullname` / `info_name`: `info.fullname` / `info.name`.
/// - `deprecated_calls_exclude`: `self.options.deprecated_calls_exclude`.
/// - `report_deprecated_as_note`: the note-vs-fail option flag.
/// - `import_from_names`: flattened first names of the module's
///   ImportFrom nodes (the shim flattens `cur_mod_node.imports`).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    deprecated,
    is_typeshed_stub,
    api_type_fullname,
    info_fullname,
    info_name,
    deprecated_calls_exclude,
    report_deprecated_as_note,
    import_from_names,
))]
pub(crate) fn rust_classify_check_warn_deprecated(
    deprecated: Option<String>,
    is_typeshed_stub: bool,
    api_type_fullname: Option<String>,
    info_fullname: String,
    info_name: String,
    deprecated_calls_exclude: Vec<String>,
    report_deprecated_as_note: bool,
    import_from_names: Vec<String>,
) -> PyResult<Option<i64>> {
    // `(deprecated := info.deprecated)` is falsy for None and "".
    if !deprecated.as_deref().is_some_and(|d| !d.is_empty()) {
        return Ok(Some(TAG_DEPRECATED_SILENT));
    }
    if is_typeshed_stub {
        return Ok(Some(TAG_DEPRECATED_SILENT));
    }
    // Same-class exemption: `self.api.type.fullname == info.fullname`.
    if api_type_fullname.as_deref() == Some(info_fullname.as_str()) {
        return Ok(Some(TAG_DEPRECATED_SILENT));
    }
    // Prefix-exclusion list: fullname == p or fullname.startswith(p + ".").
    let excluded = deprecated_calls_exclude
        .iter()
        .any(|p| info_fullname == *p || info_fullname.starts_with(&format!("{p}.")));
    if excluded {
        return Ok(Some(TAG_DEPRECATED_SILENT));
    }
    // ImportFrom-presence scan: a plain `for ... break / else` in Python,
    // so a matching import suppresses the warning entirely.
    if import_from_names.contains(&info_name) {
        return Ok(Some(TAG_DEPRECATED_SILENT));
    }
    // Note-vs-fail by report_deprecated_as_note; the shim emits with the
    // live `info.deprecated` string and the DEPRECATED error code.
    let tag = if report_deprecated_as_note {
        TAG_DEPRECATED_NOTE
    } else {
        TAG_DEPRECATED_FAIL
    };
    Ok(Some(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn classify(
        deprecated: Option<&str>,
        is_typeshed_stub: bool,
        api_type_fullname: Option<&str>,
        info_fullname: &str,
        info_name: &str,
        deprecated_calls_exclude: &[&str],
        report_deprecated_as_note: bool,
        import_from_names: &[&str],
    ) -> Option<i64> {
        rust_classify_check_warn_deprecated(
            deprecated.map(str::to_string),
            is_typeshed_stub,
            api_type_fullname.map(str::to_string),
            info_fullname.to_string(),
            info_name.to_string(),
            deprecated_calls_exclude
                .iter()
                .map(|s| s.to_string())
                .collect(),
            report_deprecated_as_note,
            import_from_names.iter().map(|s| s.to_string()).collect(),
        )
        .unwrap()
    }

    #[test]
    fn test_no_deprecated_string_is_silent() {
        assert_eq!(
            classify(None, false, None, "mod.C", "C", &[], true, &[]),
            Some(TAG_DEPRECATED_SILENT)
        );
        assert_eq!(
            classify(Some(""), false, None, "mod.C", "C", &[], true, &[]),
            Some(TAG_DEPRECATED_SILENT)
        );
    }

    #[test]
    fn test_typeshed_stub_is_silent() {
        assert_eq!(
            classify(Some("use B"), true, None, "mod.C", "C", &[], true, &[]),
            Some(TAG_DEPRECATED_SILENT)
        );
    }

    #[test]
    fn test_same_class_is_exempt() {
        assert_eq!(
            classify(
                Some("use C"),
                false,
                Some("mod.C"),
                "mod.C",
                "C",
                &[],
                true,
                &[]
            ),
            Some(TAG_DEPRECATED_SILENT)
        );
    }

    #[test]
    fn test_other_class_is_not_exempt() {
        assert_eq!(
            classify(
                Some("use C"),
                false,
                Some("mod.D"),
                "mod.C",
                "C",
                &[],
                true,
                &[]
            ),
            Some(TAG_DEPRECATED_NOTE)
        );
    }

    #[test]
    fn test_exclude_exact_and_prefix() {
        assert_eq!(
            classify(Some("x"), false, None, "mod.C", "C", &["mod.C"], true, &[]),
            Some(TAG_DEPRECATED_SILENT)
        );
        assert_eq!(
            classify(
                Some("x"),
                false,
                None,
                "mod.C.inner",
                "inner",
                &["mod.C"],
                true,
                &[]
            ),
            Some(TAG_DEPRECATED_SILENT)
        );
    }

    #[test]
    fn test_exclude_prefix_requires_dot_boundary() {
        // "mod.CX" must not match an exclusion of "mod.C".
        assert_eq!(
            classify(
                Some("x"),
                false,
                None,
                "mod.CX",
                "CX",
                &["mod.C"],
                true,
                &[]
            ),
            Some(TAG_DEPRECATED_NOTE)
        );
    }

    #[test]
    fn test_import_from_suppresses_warning() {
        assert_eq!(
            classify(Some("x"), false, None, "mod.C", "C", &[], true, &["C", "D"]),
            Some(TAG_DEPRECATED_SILENT)
        );
    }

    #[test]
    fn test_import_from_other_name_warns() {
        assert_eq!(
            classify(Some("x"), false, None, "mod.C", "C", &[], true, &["D"]),
            Some(TAG_DEPRECATED_NOTE)
        );
    }

    #[test]
    fn test_note_vs_fail_by_option() {
        assert_eq!(
            classify(Some("x"), false, None, "mod.C", "C", &[], true, &[]),
            Some(TAG_DEPRECATED_NOTE)
        );
        assert_eq!(
            classify(Some("x"), false, None, "mod.C", "C", &[], false, &[]),
            Some(TAG_DEPRECATED_FAIL)
        );
    }
}
