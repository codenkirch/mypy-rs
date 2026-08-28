//! Port of `ExpressionChecker.lookup_definer` (mypy/checkexpr.py:5862-5876):
//! the class in `typ.type.mro` that actually defines `attr_name`.
//!
//! Live-PyO3-object seam in the `rust_is_magic_base` /
//! `rust_always_returns_none` shape: attribute reads over the live
//! `Instance`, zero wire bytes. The walk reads `typ.type.mro` and returns
//! the first `cls` whose `names.get(attr_name)` is present, in MRO order.
//! Both call sites run inside `check_op_reversible`, i.e. twice per
//! non-shortcut binary/comparison op between instances.
//!
//! Strangler-fig contract: a found verdict is `Some(Some(fullname))`, a
//! not-found verdict is `Some(None)`, and any unreadable attribute
//! (`typ.type`, an MRO entry, its `names`, `fullname`) defers (`None`),
//! so the shim re-runs the untouched pure-Python body.

use pyo3::prelude::*;
use pyo3::types::PyAny;

/// The fold over per-class facts, kept separate from the PyO3 entry so
/// the MRO ordering is unit-testable without a Python runtime. Each
/// entry is `(symbol_present, fullname)` for one `cls` in `mro`; `None`
/// is an unreadable entry, which defers the whole walk (Python would
/// have raised mid-loop, so the shim falls back to the pure body).
fn lookup_definer_fold(
    entries: impl Iterator<Item = Option<(bool, String)>>,
) -> Option<Option<String>> {
    for entry in entries {
        match entry {
            None => return None,
            Some((true, fullname)) => return Some(Some(fullname)),
            Some((false, _)) => {}
        }
    }
    Some(None)
}

/// `ExpressionChecker.lookup_definer` (mypy/checkexpr.py:5862-5876) as a
/// `#[pyfunction]`. `typ` is the live `Instance`. Returns
/// `Some(Some(fullname))` when a class in the MRO defines `attr_name`,
/// `Some(None)` when none does, and defers (`None`) on any unreadable
/// fact so the shim falls back to the pure-Python walk.
#[pyfunction]
pub(crate) fn rust_lookup_definer(
    typ: &PyAny,
    attr_name: &str,
) -> PyResult<Option<Option<String>>> {
    let result = lookup_definer_inner(typ, attr_name);
    Ok(result.unwrap_or(None))
}

fn lookup_definer_inner(typ: &PyAny, attr_name: &str) -> PyResult<Option<Option<String>>> {
    let info = typ.getattr("type")?;
    let mro = info.getattr("mro")?;
    let entries = mro.iter()?.map(|cls| {
        let cls = match cls {
            Ok(c) => c,
            Err(_) => return None,
        };
        let names = match cls.getattr("names") {
            Ok(n) => n,
            Err(_) => return None,
        };
        let sym = match names.call_method1("get", (attr_name,)) {
            Ok(s) => s,
            Err(_) => return None,
        };
        // Python tests truthiness of the node; a SymbolTableNode is
        // always truthy, so the None check is exact.
        let present = !sym.is_none();
        let fullname = match cls.getattr("fullname") {
            Ok(f) => f,
            Err(_) => return None,
        };
        match fullname.extract::<String>() {
            Ok(f) => Some((present, f)),
            Err(_) => None,
        }
    });
    Ok(lookup_definer_fold(entries))
}

#[cfg(test)]
mod lookup_definer_tests {
    use super::lookup_definer_fold;

    #[test]
    fn test_fold_empty_mro_is_not_found() {
        assert_eq!(
            lookup_definer_fold(std::iter::empty()),
            Some(None::<String>)
        );
    }

    #[test]
    fn test_fold_miss_is_not_found() {
        let entries = vec![Some((false, "mod.A".to_string()))];
        assert_eq!(
            lookup_definer_fold(entries.into_iter()),
            Some(None::<String>)
        );
    }

    #[test]
    fn test_fold_first_hit_in_mro_order() {
        // Base defines it, subclass does not: the base wins.
        let entries = vec![
            Some((false, "mod.B".to_string())),
            Some((true, "mod.A".to_string())),
        ];
        assert_eq!(
            lookup_definer_fold(entries.into_iter()),
            Some(Some("mod.A".to_string()))
        );
    }

    #[test]
    fn test_fold_override_beats_base() {
        // Subclass overrides: the first MRO entry with the attr wins.
        let entries = vec![
            Some((true, "mod.B".to_string())),
            Some((true, "mod.A".to_string())),
        ];
        assert_eq!(
            lookup_definer_fold(entries.into_iter()),
            Some(Some("mod.B".to_string()))
        );
    }

    #[test]
    fn test_fold_miss_then_hit() {
        let entries = vec![
            Some((false, "mod.C".to_string())),
            Some((false, "mod.B".to_string())),
            Some((true, "mod.A".to_string())),
        ];
        assert_eq!(
            lookup_definer_fold(entries.into_iter()),
            Some(Some("mod.A".to_string()))
        );
    }

    #[test]
    fn test_fold_unreadable_entry_defers() {
        // A mid-walk deferral aborts the walk; Python re-runs the body.
        let entries = vec![Some((false, "mod.B".to_string())), None];
        assert_eq!(lookup_definer_fold(entries.into_iter()), None);
    }
}
