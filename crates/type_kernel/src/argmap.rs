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

use crate::wire::{read_type, ReadBuffer, Type};

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

/// `ArgKind.is_named(star=True)`: named or `**kwargs` (ARG_STAR2).
fn is_named_or_star2(kind: i64) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT || kind == ARG_STAR2
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

/// Rust port of `map_formals_to_actuals` (argmap.py:167-183).
///
/// Computes the forward mapping via `rust_map_actuals_to_formals`, then
/// reverses it into `actual_to_formal` (indexed by actual argument index,
/// each entry the list of formals that actual binds to). Deferral contract
/// matches the forward function: returns `None` for any star actual
/// (needs the callback) or unexpected actual kind.
#[pyfunction]
pub fn rust_map_formals_to_actuals(
    actual_kinds: Vec<i64>,
    actual_names: Vec<Option<String>>,
    formal_kinds: Vec<i64>,
    formal_names: Vec<Option<String>>,
) -> Option<Vec<Vec<i64>>> {
    let formal_to_actual = rust_map_actuals_to_formals(
        actual_kinds.clone(),
        actual_names,
        formal_kinds,
        formal_names,
    )?;
    let mut actual_to_formal: Vec<Vec<i64>> = vec![Vec::new(); actual_kinds.len()];
    for (formal, actuals) in formal_to_actual.iter().enumerate() {
        for &actual in actuals {
            actual_to_formal[actual as usize].push(formal as i64);
        }
    }
    Some(actual_to_formal)
}

/// Rust port of `map_actuals_to_formals` for calls with star actuals
/// (argmap.py:96-140 plus the ambiguous-kwargs fill at 142-163).
///
/// The non-star branches share logic with `rust_map_actuals_to_formals`;
/// the star branches additionally need the actual type per star actual to
/// decide tuple/TypedDict unpacking. Python serializes each star actual's
/// type on the wire (`actual_types[ai]`), so this fn can inspect
/// TupleType/TypedDictType structurally without any type identity round-trip
/// (M5 wire-safety rule: Rust only validates; the caller keeps the original
/// Python objects). Returns `None` to defer to Python when a star actual
/// lacks a wire type or the kinds are unexpected, matching the fall-through
/// contract of the non-star fn.
#[pyfunction]
pub fn rust_map_actuals_to_formals_with_types(
    actual_kinds: Vec<i64>,
    actual_names: Vec<Option<String>>,
    formal_kinds: Vec<i64>,
    formal_names: Vec<Option<String>>,
    actual_types: Vec<Option<Vec<u8>>>,
) -> Option<Vec<Vec<i64>>> {
    let nformals = formal_kinds.len();
    let mut formal_to_actual: Vec<Vec<i64>> = vec![Vec::new(); nformals];
    let mut ambiguous_actual_kwargs: Vec<i64> = Vec::new();
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
            }
        } else if actual_kind == ARG_STAR {
            // Python: `actualt = get_proper_type(actual_arg_type(ai))`.
            let blob = actual_types.get(ai).and_then(|b| b.as_deref())?;
            let actualt = read_type_lone(blob)?;
            match &actualt {
                Type::TupleType { items, .. } => {
                    // A tuple actual maps to a fixed number of formals.
                    for _ in 0..items.len() {
                        if fi >= nformals {
                            break;
                        }
                        if formal_kinds[fi] != ARG_STAR2 {
                            formal_to_actual[fi].push(ai as i64);
                        } else {
                            break;
                        }
                        if formal_kinds[fi] != ARG_STAR {
                            fi += 1;
                        }
                    }
                }
                _ => {
                    // Assume iterable; if not, an error surfaces later.
                    while fi < nformals {
                        if is_named_or_star2(formal_kinds[fi]) {
                            break;
                        }
                        formal_to_actual[fi].push(ai as i64);
                        if formal_kinds[fi] == ARG_STAR {
                            break;
                        }
                        fi += 1;
                    }
                }
            }
        } else if is_named(actual_kind) {
            let name = actual_names.get(ai).and_then(|n| n.as_deref())?;
            if let Some(idx) = formal_names.iter().position(|n| n.as_deref() == Some(name)) {
                if formal_kinds[idx] != ARG_STAR {
                    formal_to_actual[idx].push(ai as i64);
                } else if let Some(s2) = formal_kinds.iter().position(|&k| k == ARG_STAR2) {
                    formal_to_actual[s2].push(ai as i64);
                }
            } else if let Some(s2) = formal_kinds.iter().position(|&k| k == ARG_STAR2) {
                formal_to_actual[s2].push(ai as i64);
            }
        } else if actual_kind == ARG_STAR2 {
            let blob = actual_types.get(ai).and_then(|b| b.as_deref())?;
            let actualt = read_type_lone(blob)?;
            match &actualt {
                Type::TypedDictType { items, .. } => {
                    for (name, _) in items {
                        if let Some(idx) = formal_names
                            .iter()
                            .position(|n| n.as_deref() == Some(name.as_str()))
                        {
                            formal_to_actual[idx].push(ai as i64);
                        } else if let Some(s2) = formal_kinds.iter().position(|&k| k == ARG_STAR2) {
                            formal_to_actual[s2].push(ai as i64);
                        }
                    }
                }
                _ => {
                    // We don't know which **kwargs the caller provides; defer.
                    ambiguous_actual_kwargs.push(ai as i64);
                }
            }
        } else {
            // ARG_OPT actuals are unreachable in calls.
            return None;
        }
    }
    if !ambiguous_actual_kwargs.is_empty() {
        // Assume the ambiguous kwargs fill the remaining arguments.
        let unmatched_formals: Vec<usize> = (0..nformals)
            .filter(|&fi| {
                // `formal_names[fi]` is falsy for None or "" (Python truthiness).
                let name_truthy = formal_names
                    .get(fi)
                    .and_then(|n| n.as_deref())
                    .is_some_and(|n| !n.is_empty());
                (name_truthy
                    && (formal_to_actual[fi].is_empty()
                        || actual_kinds[formal_to_actual[fi][0] as usize] == ARG_STAR)
                    && formal_kinds[fi] != ARG_STAR)
                    || formal_kinds[fi] == ARG_STAR2
            })
            .collect();
        for &ai in &ambiguous_actual_kwargs {
            for &fi in &unmatched_formals {
                formal_to_actual[fi].push(ai);
            }
        }
    }
    Some(formal_to_actual)
}

/// Parse a single wire type blob; on any decode failure, defer to Python.
fn read_type_lone(blob: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(blob);
    read_type(&mut buf, None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn types_blob(t: &Type) -> Vec<Option<Vec<u8>>> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok();
        vec![Some(buf.into_bytes())]
    }

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

    // Reverse mapping (map_formals_to_actuals).

    #[test]
    fn test_reverse_pos_to_pos() {
        // One positional actual binds to one formal; reverse maps actual 0 -> [formal 0].
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_reverse_pos_to_star_formal() {
        // Two positional actuals into one ARG_STAR formal: each actual lists formal 0.
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS, ARG_POS]),
            names(&[None, None]),
            kinds(&[ARG_STAR]),
            names(&[None]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![0]]));
    }

    #[test]
    fn test_reverse_pos_overflow_dropped() {
        // Second positional with no formal: it has an empty actual_to_formal slot.
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS, ARG_POS]),
            names(&[None, None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![]]));
    }

    #[test]
    fn test_reverse_pos_into_star2_dropped() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS]),
            names(&[None]),
            kinds(&[ARG_STAR2]),
            names(&[None]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_reverse_named_to_named() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_NAMED]),
            names(&[Some("x")]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![0]]));
    }

    #[test]
    fn test_reverse_named_to_star2_when_no_formal_match() {
        // Named actual with no formal match routes to ARG_STAR2 (formal 1).
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_NAMED]),
            names(&[Some("z")]),
            kinds(&[ARG_POS, ARG_STAR2]),
            names(&[Some("x"), None]),
        );
        assert_eq!(r, Some(vec![vec![1]]));
    }

    #[test]
    fn test_reverse_named_not_found_no_star2_dropped() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_NAMED]),
            names(&[Some("z")]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![vec![]]));
    }

    #[test]
    fn test_reverse_multiple_named() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_NAMED, ARG_NAMED]),
            names(&[Some("x"), Some("y")]),
            kinds(&[ARG_POS, ARG_POS]),
            names(&[Some("x"), Some("y")]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![1]]));
    }

    #[test]
    fn test_reverse_pos_then_named() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS, ARG_NAMED]),
            names(&[None, Some("y")]),
            kinds(&[ARG_POS, ARG_POS]),
            names(&[Some("x"), Some("y")]),
        );
        assert_eq!(r, Some(vec![vec![0], vec![1]]));
    }

    #[test]
    fn test_reverse_returns_none_for_star_actual() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_STAR]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, None);
    }

    #[test]
    fn test_reverse_empty_caller() {
        // No actuals: empty actual_to_formal even with a formal present.
        let r = rust_map_formals_to_actuals(
            kinds(&[]),
            names(&[]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, Some(vec![]));
    }

    #[test]
    fn test_reverse_empty_callee() {
        // No formals: every actual has an empty list.
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_POS, ARG_NAMED]),
            names(&[None, Some("y")]),
            kinds(&[]),
            names(&[]),
        );
        assert_eq!(r, Some(vec![vec![], vec![]]));
    }

    #[test]
    fn test_reverse_named_missing_name_falls_through() {
        let r = rust_map_formals_to_actuals(
            kinds(&[ARG_NAMED]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
        );
        assert_eq!(r, None);
    }

    // Star actuals (with wire type blobs).

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn object_type() -> Type {
        Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tuple2_type() -> Type {
        Type::TupleType {
            partial_fallback: Box::new(object_type()),
            items: vec![any_type(), any_type()],
            implicit: false,
        }
    }

    fn list_type() -> Type {
        Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![any_type()],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn test_star_tuple_to_fixed_formals() {
        // *args: tuple[int, str] against (int, str, str): binds first two.
        let r = rust_map_actuals_to_formals_with_types(
            kinds(&[ARG_STAR]),
            names(&[None]),
            kinds(&[ARG_POS, ARG_POS, ARG_POS]),
            names(&[None, None, None]),
            types_blob(&tuple2_type()),
        );
        assert_eq!(r, Some(vec![vec![0], vec![0], vec![]]));
    }

    #[test]
    fn test_star_iterable_while_loop() {
        // *args: list (iterable) against (int, *int): binds both to first, then star.
        let lst = list_type();
        let r = rust_map_actuals_to_formals_with_types(
            kinds(&[ARG_STAR]),
            names(&[None]),
            kinds(&[ARG_POS, ARG_STAR]),
            names(&[Some("x"), None]),
            types_blob(&lst),
        );
        assert_eq!(r, Some(vec![vec![0], vec![0]]));
    }

    #[test]
    fn test_star2_typeddict_routes_names() {
        // **kwargs: TypedDict{x:int, y:str} against (x: int, **kwargs).
        let td = Type::TypedDictType {
            fallback: Box::new(object_type()),
            items: vec![("x".to_string(), any_type()), ("y".to_string(), any_type())],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let r = rust_map_actuals_to_formals_with_types(
            kinds(&[ARG_STAR2]),
            names(&[None]),
            kinds(&[ARG_POS, ARG_STAR2]),
            names(&[Some("x"), None]),
            types_blob(&td),
        );
        // x routes to formal 0, y has no formal match so routes to ARG_STAR2 (1).
        assert_eq!(r, Some(vec![vec![0], vec![0]]));
    }

    #[test]
    fn test_star2_non_typeddict_ambiguous_fill() {
        // **kwargs: list (not a TypedDict): ambiguous; fills all unmatched formals.
        let lst = list_type();
        let r = rust_map_actuals_to_formals_with_types(
            kinds(&[ARG_STAR2]),
            names(&[None]),
            kinds(&[ARG_POS, ARG_POS]),
            names(&[Some("x"), Some("y")]),
            types_blob(&lst),
        );
        // Both formals are named with no match yet, so both get the ambiguous actual.
        assert_eq!(r, Some(vec![vec![0], vec![0]]));
    }

    #[test]
    fn test_star_missing_type_blob_defer() {
        // Star actual without a wire type: fall through to Python (None).
        let r = rust_map_actuals_to_formals_with_types(
            kinds(&[ARG_STAR]),
            names(&[None]),
            kinds(&[ARG_POS]),
            names(&[Some("x")]),
            vec![None],
        );
        assert_eq!(r, None);
    }
}
