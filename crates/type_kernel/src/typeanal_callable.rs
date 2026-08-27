#![allow(non_local_definitions)]

//! Native port of the two-level dispatch head of
//! `mypy.typeanal.TypeAnalyser.analyze_callable_type`
//! (typeanal.py:2330).
//!
//! The method dispatches on `len(t.args)` (0 = bare `Callable`, 2 =
//! normal form, anything else = invalid arity) and, inside the `2`
//! arm, on the kind of `t.args[0]` (`TypeList` -> `Callable[[...], RET]`,
//! `EllipsisType` -> `Callable[..., RET]`, anything else -> the
//! ParamSpec `Callable[P, RET]` form). The invalid-arity message also
//! branches on `options.disallow_any_generics`.
//!
//! Every branch's *decision* is a pure function of four scalar facts
//! (`arg_count`, `arg0_is_type_list`, `arg0_is_ellipsis`,
//! `disallow_any_generics`), so Rust owns the whole decision table and
//! returns a single branch tag; the Python shim applies the side
//! effects (object construction, `tvar_scope` entry, the
//! `analyze_callable_args*` variants, `fail`/`note` emission) exactly
//! as the original body does. Rust returns `None` (defer) only on an
//! unreadable `t.args` (not a list), which never happens for a real
//! `UnboundType` but keeps the seam safe for test doubles.

use pyo3::prelude::*;

// Branch tags handed to the Python shim. Each maps to exactly one
// terminal branch of `analyze_callable_type`; the comment cites the
// typeanal.py line and the Python-side effect the shim must apply.
const TAG_BARE_CALLABLE: i64 = 0; // 2333-2335 Callable[..., Any]
const TAG_TYPE_LIST: i64 = 1; // 2337-2344 Callable[[ARG, ...], RET]
const TAG_ELLIPSIS: i64 = 2; // 2345-2350 Callable[..., RET]
const TAG_PARAMSPEC: i64 = 3; // 2351-2376 Callable[P, RET]
const TAG_INVALID_DISALLOW: i64 = 4; // 2377-2379 disallow_any_generics
const TAG_INVALID_ALLOW: i64 = 5; // 2380-2382 allow any generics

/// `analyze_callable_type` dispatch classifier. Mirrors the branch order
/// of typeanal.py:2330-2382 and returns the terminal branch tag; `None`
/// defers to the pure-Python body (gate off or unreadable `t.args`).
///
/// Facts (all scalars, no live type objects):
/// - `arg_count`: `len(t.args)`.
/// - `arg0_is_type_list`: `isinstance(t.args[0], TypeList)` (only
///   consulted when `arg_count == 2`).
/// - `arg0_is_ellipsis`: `isinstance(t.args[0], EllipsisType)` (only
///   consulted when `arg_count == 2`).
/// - `disallow_any_generics`: `self.options.disallow_any_generics`,
///   selecting the invalid-arity message.
#[pyfunction]
pub(crate) fn rust_classify_analyze_callable_type(
    arg_count: i64,
    arg0_is_type_list: bool,
    arg0_is_ellipsis: bool,
    disallow_any_generics: bool,
) -> PyResult<Option<i64>> {
    if arg_count == 0 {
        return Ok(Some(TAG_BARE_CALLABLE));
    }
    if arg_count == 2 {
        if arg0_is_type_list {
            return Ok(Some(TAG_TYPE_LIST));
        }
        if arg0_is_ellipsis {
            return Ok(Some(TAG_ELLIPSIS));
        }
        return Ok(Some(TAG_PARAMSPEC));
    }
    Ok(Some(if disallow_any_generics {
        TAG_INVALID_DISALLOW
    } else {
        TAG_INVALID_ALLOW
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(
        arg_count: i64,
        arg0_is_type_list: bool,
        arg0_is_ellipsis: bool,
        disallow_any_generics: bool,
    ) -> Option<i64> {
        rust_classify_analyze_callable_type(
            arg_count,
            arg0_is_type_list,
            arg0_is_ellipsis,
            disallow_any_generics,
        )
        .unwrap()
    }

    #[test]
    fn test_bare_callable() {
        assert_eq!(classify(0, false, false, false), Some(TAG_BARE_CALLABLE));
    }

    #[test]
    fn test_type_list() {
        assert_eq!(classify(2, true, false, false), Some(TAG_TYPE_LIST));
    }

    #[test]
    fn test_ellipsis() {
        assert_eq!(classify(2, false, true, false), Some(TAG_ELLIPSIS));
    }

    #[test]
    fn test_paramspec() {
        assert_eq!(classify(2, false, false, false), Some(TAG_PARAMSPEC));
    }

    #[test]
    fn test_invalid_arity_allow() {
        assert_eq!(classify(1, false, false, false), Some(TAG_INVALID_ALLOW));
        assert_eq!(classify(3, false, false, false), Some(TAG_INVALID_ALLOW));
    }

    #[test]
    fn test_invalid_arity_disallow() {
        assert_eq!(classify(1, false, false, true), Some(TAG_INVALID_DISALLOW));
        assert_eq!(classify(3, false, false, true), Some(TAG_INVALID_DISALLOW));
    }

    #[test]
    fn test_type_list_wins_over_ellipsis() {
        // TypeList is checked first in the original elif-chain.
        assert_eq!(classify(2, true, true, false), Some(TAG_TYPE_LIST));
    }

    #[test]
    fn test_invalid_arity_ignores_arg0_facts() {
        // arg0 facts only matter when arg_count == 2.
        assert_eq!(classify(1, true, true, false), Some(TAG_INVALID_ALLOW));
    }
}
