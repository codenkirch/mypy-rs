//! Stage 4 seam: argument-to-formal binding for `check_call`.
//!
//! Ports `mypy.argmap.map_actuals_to_formals` (argmap.py:27-122) to Rust. The
//! Python function is a pure free function: no `self`, no plugin hooks, no
//! constraint solver. Its only external coupling is the `actual_arg_type`
//! callback, invoked solely in the `ARG_STAR` and `ARG_STAR2` branches to
//! decide tuple/TypedDict unpacking.
//!
//! This port handles every non-star-actual call (ARG_POS, ARG_NAMED,
//! ARG_NAMED_OPT actuals against any formal kinds) and returns `None` for any
//! call with an ARG_STAR or ARG_STAR2 actual, so Python re-runs the full
//! function including the callback. The `return None -> fall through` contract
//! mirrors `erase::erase_type` (Stage 1) and the Stage 3c subtype/join/meet
//! visitors: no behavior change unless `Options.native_type_kernel` is set,
//! and even then unsupported cases degrade gracefully.

use pyo3::prelude::*;

// ArgKind integer values, mirroring `mypy.nodes.ARG_*` (nodes.py:2480-2517).
// The wire format and the Python shim both pass `int(ArgKind.value)`.
const ARG_POS: i64 = 0;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

fn is_star(kind: i64) -> bool {
    kind == ARG_STAR || kind == ARG_STAR2
}

fn is_named(kind: i64) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT
}

/// Rust port of `map_actuals_to_formals` (argmap.py:27-122).
///
/// Returns `None` for any call with an ARG_STAR or ARG_STAR2 actual (those
/// branches need the `actual_arg_type` callback, deferred to a follow-on),
/// or for any unexpected actual kind (lets Python raise its own internal
/// error). For all non-star actuals the result is identical to the Python
/// function, including the positional-overflow drop and the named-routes-to-
/// ARG_STAR2 fallback.
#[pyfunction]
pub fn rust_map_actuals_to_formals(
    actual_kinds: Vec<i64>,
    actual_names: Vec<Option<String>>,
    formal_kinds: Vec<i64>,
    formal_names: Vec<Option<String>>,
) -> Option<Vec<Vec<i64>>> {
    // Star actuals need the `actual_arg_type` callback; defer to Python.
    if actual_kinds
        .iter()
        .any(|&k| k == ARG_STAR || k == ARG_STAR2)
    {
        return None;
    }
    let nformals = formal_kinds.len();
    let mut formal_to_actual: Vec<Vec<i64>> = vec![Vec::new(); nformals];
    let mut fi: usize = 0;
    for (ai, &actual_kind) in actual_kinds.iter().enumerate() {
        if actual_kind == ARG_POS {
            if fi < nformals {
                if !is_star(formal_kinds[fi]) {
                    formal_to_actual[fi].push(ai as i64);
                    fi += 1;
                } else if formal_kinds[fi] == ARG_STAR {
                    formal_to_actual[fi].push(ai as i64);
                }
                // ARG_STAR2 formal with a positional actual: drop (mirrors Python).
            }
            // Too many positional args: drop (mirrors Python).
        } else if is_named(actual_kind) {
            // Python asserts `actual_names is not None` for named kinds. If
            // the name is missing, fall through so Python raises the same
            // error.
            let name = actual_names.get(ai).and_then(|n| n.as_deref())?;
            if let Some(idx) = formal_names.iter().position(|n| n.as_deref() == Some(name)) {
                if formal_kinds[idx] != ARG_STAR {
                    formal_to_actual[idx].push(ai as i64);
                } else if let Some(s2) = formal_kinds.iter().position(|&k| k == ARG_STAR2) {
                    formal_to_actual[s2].push(ai as i64);
                }
                // Named actual matched an ARG_STAR formal with no ARG_STAR2: drop.
            } else if let Some(s2) = formal_kinds.iter().position(|&k| k == ARG_STAR2) {
                formal_to_actual[s2].push(ai as i64);
            }
            // Named actual with no matching formal and no ARG_STAR2: drop.
        } else {
            // ARG_OPT actuals are unreachable in mypy (Python asserts in the
            // `else` branch). Fall through to let Python raise the error.
            return None;
        }
    }
    // The ambiguous-kwargs pass only runs for ARG_STAR2 actuals, already
    // filtered out above, so no deferred pass is needed here.
    Some(formal_to_actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(ks: &[i64]) -> Vec<i64> {
        ks.to_vec()
    }

    fn names(ns: &[Option<&str>]) -> Vec<Option<String>> {
        ns.iter().map(|s| s.map(String::from)).collect()
    }

    // Positional actuals.

    #[test]
    fn test_pos_to_pos() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_pos_to_star_formal() {
        // Two positional actuals into a single ARG_STAR formal: both stack.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS, ARG_POS]),
            names(&[None, None]),
            kinds(&[ARG_STAR]),
            names(&[None]),
        );
        assert_eq!(r, Some(vec![vec![0, 1]]));
    }

    #[test]
    fn test_pos_overflow_dropped() {
        // Second positional with no formal to bind: dropped.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS, ARG_POS]),
            names(&[None, None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_pos_into_star2_dropped() {
        // Positional actual into an ARG_STAR2 formal: neither branch fires.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS]),
            names(&[None]),
            kinds(&[ARG_STAR2]),
            names(&[None]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_pos_skips_star_formal_then_binds() {
        // ARG_STAR formal consumes the first positional without advancing fi;
        // the next positional has no further formal and is dropped.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS, ARG_POS]),
            names(&[None, None]),
            kinds(&[ARG_STAR]),
            names(&[None]),
        );
        assert_eq!(r, Some(vec![vec![0, 1]]));
    }

    // Named actuals.

    #[test]
    fn test_named_to_named() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[Some("x")]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_named_opt_to_named() {
        // ARG_NAMED_OPT actual binds to a same-named formal.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED_OPT]),
            names(&[Some("x")]),
            kinds(&[ARG_NAMED_OPT]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_named_to_star2_when_no_formal_match() {
        // Named actual with no matching formal routes to ARG_STAR2.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[Some("z")]),
            kinds(&[ARG_POS, ARG_STAR2]),
            names(&[Some("x"), None]),
        );
        assert_eq!(r, Some(vec![vec![], vec![0]]));
    }

    #[test]
    fn test_named_not_found_no_star2_dropped() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[Some("z")]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_named_matches_star_formal_routes_to_star2() {
        // Name matches an ARG_STAR formal: first condition false, elif fires.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[Some("x")]),
            kinds(&[ARG_STAR, ARG_STAR2]),
            names(&[Some("x"), None]),
        );
        assert_eq!(r, Some(vec![vec![], vec![0]]));
    }

    #[test]
    fn test_named_matches_star_formal_no_star2_dropped() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[Some("x")]),
            kinds(&[ARG_STAR]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_multiple_named() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED, ARG_NAMED]),
            names(&[Some("x"), Some("y")]),
            kinds(&[ARG_POS, ARG_POS]),
            names(&[Some("x"), Some("y")]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![1]]));
    }

    #[test]
    fn test_pos_then_named() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS, ARG_NAMED]),
            names(&[None, Some("y")]),
            kinds(&[ARG_POS, ARG_POS]),
            names(&[Some("x"), Some("y")]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![1]]));
    }

    // Star-actual fallback (return None).

    #[test]
    fn test_returns_none_for_star_actual() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_STAR]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_returns_none_for_star2_actual() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_STAR2]),
            names(&[None]),
            kinds(&[ARG_STAR2]),
            names(&[None]),
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_returns_none_for_mixed_star() {
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_POS, ARG_STAR2]),
            names(&[None, None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, None);
    }

    // Edge cases.

    #[test]
    fn test_empty_caller() {
        let r = rust_map_actuals_to_formals(
            kinds(&[]),
            names(&[]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_empty_callee() {
        // No formals: every actual is dropped.
        let r =
            rust_map_actuals_to_formals(kinds(&[ARG_POS]), names(&[None]), kinds(&[]), names(&[]));
        assert_eq!(r, Some(vec![]));
    }

    #[test]
    fn test_named_missing_name_falls_through() {
        // Named kind with no name entry: Python would assert; fall through.
        let r = rust_map_actuals_to_formals(
            kinds(&[ARG_NAMED]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, None);
    }
}
