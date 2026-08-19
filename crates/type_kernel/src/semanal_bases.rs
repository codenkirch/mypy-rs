//! Native port of the base-class classification front of
//! `mypy.semanal.SemanticAnalyzer.clean_up_bases_and_infer_type_variables`
//! (semanal.py:2709-2804).
//!
//! The function visits the base expressions of a class definition and
//! decides which ones are `Generic[...]` / `Protocol[...]` type-variable
//! declarations (removed, their type vars declared for the class) and which
//! are bare `Protocol` bases (removed, class becomes a protocol). The Rust
//! port owns that per-base decision given the resolved symbol facts the
//! Python shim already computed (it performs the same `lookup_qualified`
//! calls in the same order, so lookup side effects are identical). Python
//! keeps every side effect: the `analyze_type_expr` / `expr_to_unanalyzed_type`
//! preprocessing, the tvar extraction loop, the error messages, the
//! `removed_base_type_exprs` bookkeeping, and the `tvar_defs_from_tvars`
//! binding. A `None` result means the shim could not supply clean facts and
//! the pure-Python checks run unchanged.

use pyo3::prelude::*;

/// Action tags handed to the Python shim, index-aligned with the base list:
/// - `KEEP`: not a Generic/Protocol declaration; nothing to remove.
/// - `GENERIC`: `typing.Generic` (with or without args); removed, its type
///   vars are declared for the class, `is_protocol` unchanged.
/// - `PROTOCOL_GENERIC`: `typing.Protocol` / `typing_extensions.Protocol`
///   with args; removed, its type vars declared, `is_protocol` set.
/// - `BARE_PROTOCOL`: a Protocol name without args; removed, `is_protocol`
///   set, no type vars.
pub(crate) const ACTION_KEEP: i64 = 1;
pub(crate) const ACTION_GENERIC: i64 = 2;
pub(crate) const ACTION_PROTOCOL_GENERIC: i64 = 3;
pub(crate) const ACTION_BARE_PROTOCOL: i64 = 4;

/// The special `Generic` name is a fixed constant, mirroring
/// `sym.node.fullname == "typing.Generic"` in semanal.py:2824. The
/// `PROTOCOL_NAMES` set travels from `mypy.types` so the names stay
/// single-sourced in Python.
const GENERIC_FULLNAME: &str = "typing.Generic";

/// `clean_up_bases_and_infer_type_variables` per-base classifier — mirrors
/// the branch order of semanal.py:2817-2843 + 2768-2774.
///
/// `fullname` is the resolved `sym.node.fullname` for an `UnboundType` base
/// (or `None` when the lookup failed / the node is missing). `has_args` is
/// `bool(base.args)`. The shim only calls Rust for `UnboundType` bases whose
/// translation succeeded; every other base is skipped or kept on the Python
/// side.
///
/// Branch order mirrors Python exactly: `typing.Generic` wins first (bare
/// `Generic` is still a declaration, semanal.py:2824), then `Protocol`
/// names split on whether args are present (`Protocol[...]` declares type
/// vars, bare `Protocol` only flags the class as a protocol).
#[pyfunction]
#[pyo3(signature = (fullname, in_protocol_names, has_args))]
pub(crate) fn rust_clean_up_bases(
    fullname: Option<String>,
    in_protocol_names: bool,
    has_args: bool,
) -> PyResult<i64> {
    match fullname.as_deref() {
        Some(GENERIC_FULLNAME) => Ok(ACTION_GENERIC),
        Some(_) if in_protocol_names => Ok(if has_args {
            ACTION_PROTOCOL_GENERIC
        } else {
            ACTION_BARE_PROTOCOL
        }),
        Some(_) | None => Ok(ACTION_KEEP),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(fullname: Option<&str>, in_protocol_names: bool, has_args: bool) -> i64 {
        rust_clean_up_bases(fullname.map(str::to_string), in_protocol_names, has_args).unwrap()
    }

    #[test]
    fn plain_base_kept() {
        assert_eq!(classify(Some("mod.Bar"), false, false), ACTION_KEEP);
    }

    #[test]
    fn generic_base_with_args_kept() {
        // `class B(A[int])`: A[int] is not a Generic/Protocol declaration.
        assert_eq!(classify(Some("mod.A"), false, true), ACTION_KEEP);
    }

    #[test]
    fn generic_name_wins_even_without_args() {
        // Bare `Generic` is still removed (semanal.py:2824 first clause).
        assert_eq!(
            classify(Some("typing.Generic"), false, false),
            ACTION_GENERIC
        );
    }

    #[test]
    fn generic_with_args() {
        assert_eq!(
            classify(Some("typing.Generic"), false, true),
            ACTION_GENERIC
        );
    }

    #[test]
    fn protocol_with_args_declares_tvars() {
        assert_eq!(
            classify(Some("typing.Protocol"), true, true),
            ACTION_PROTOCOL_GENERIC
        );
    }

    #[test]
    fn bare_protocol_removed_no_tvars() {
        assert_eq!(
            classify(Some("typing_extensions.Protocol"), true, false),
            ACTION_BARE_PROTOCOL
        );
    }

    #[test]
    fn unresolved_symbol_kept() {
        // Missing node / failed lookup: neither declaration fires.
        assert_eq!(classify(None, false, true), ACTION_KEEP);
        assert_eq!(classify(None, false, false), ACTION_KEEP);
    }

    #[test]
    fn unrelated_name_kept() {
        assert_eq!(classify(Some("mod.NotProtocol"), false, true), ACTION_KEEP);
    }
}
