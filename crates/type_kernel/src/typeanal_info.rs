#![allow(non_local_definitions)]

//! Native port of the *decision front* of
//! `mypy.typeanal.TypeAnalyser.analyze_type_with_type_info`
//! (typeanal.py:1166-1289).
//!
//! The method binds an unbound type that resolved to a `TypeInfo` node. The
//! Python shim runs the deprecation check itself, then calls this classifier;
//! it settles a small fixed set of branches over the node + argument-list
//! facts:
//!
//! - `tuple[...]` with arguments -> a `TupleType` (typeanal.py:1176-1178);
//! - `librt.vecs.vec` with an invalid item type -> `Any(from_error)`
//!   (typeanal.py:1204-1207);
//! - named tuple / tuple-alias tail (typeanal.py:1228-1253);
//! - TypedDict tail (typeanal.py:1254-1279);
//! - `types.NoneType` -> an error + `NoneType` (typeanal.py:1281-1287);
//! - anything else -> a plain `Instance` (typeanal.py:1289).
//!
//! Rust receives raw node facts (fullname, argument count, which of
//! `tuple_type` / `special_alias` / `typeddict_type` are set) and returns a
//! single branch tag. Python applies the side effects and builds the result
//! object for the two tags it executes inline; every other tag falls through
//! to the original pure-Python body, so error messages and construction stay
//! single-sourced and parity is trivial for the non-inline branches.

use pyo3::prelude::*;

/// Branch tags handed to the Python shim. Each maps to a terminal branch of
/// `analyze_type_with_type_info`; the comment cites the typeanal.py line.
const TAG_TUPLE: i64 = 1; // 1176-1178 tuple[...] with args -> TupleType
const TAG_VEC: i64 = 2; // 1204-1207 librt.vecs.vec bad item -> Any(from_error)
const TAG_TUPLE_TAIL: i64 = 3; // 1228-1253 named-tuple base, no alias
const TAG_TUPLE_TAIL_ALIAS: i64 = 4; // 1232-1250 tuple-alias base
const TAG_TYPEDDICT_TAIL: i64 = 5; // 1254-1279 TypedDict base, no alias
const TAG_TYPEDDICT_TAIL_ALIAS: i64 = 6; // 1258-1275 TypedDict-alias base
const TAG_NONE_TYPE: i64 = 7; // 1281-1287 types.NoneType -> error + NoneType
const TAG_INSTANCE: i64 = 8; // 1289 plain Instance

/// `analyze_type_with_type_info` decision classifier. Mirrors the branch
/// order of typeanal.py:1176-1289 exactly and returns the terminal branch
/// tag; `None` (defer) falls through to the full pure-Python body.
///
/// Facts (all scalars / strings, no live type objects):
/// - `fullname`: `info.fullname` (the `builtins.tuple` / `librt.vecs.vec` /
///   `types.NoneType` special-cases).
/// - `args_len`: `len(args)` (the tuple-with-args gate).
/// - `tuple_type_not_none` / `special_alias_not_none` / `typeddict_type_not_none`:
///   which of `info.tuple_type` / `info.special_alias` / `info.typeddict_type`
///   are set (nodes.py:3964-3967).
///
/// The Python shim acts inline only on `TAG_TUPLE` and `TAG_NONE_TYPE`; the
/// remaining tags all re-run the body (the body re-derives the vec check,
/// the argument-count validation, and the tuple/typeddict tails from the
/// same live objects, so no side effect runs twice).
#[pyfunction]
pub(crate) fn rust_classify_type_with_info(
    fullname: String,
    args_len: i64,
    tuple_type_not_none: bool,
    special_alias_not_none: bool,
    typeddict_type_not_none: bool,
) -> PyResult<Option<i64>> {
    // Tuple with arguments (typeanal.py:1176-1178): before everything else.
    if args_len > 0 && fullname == "builtins.tuple" {
        return Ok(Some(TAG_TUPLE));
    }
    // librt.vecs.vec (typeanal.py:1204-1207): the item-type validity check
    // itself is already native (`rust_check_vec_type_args`); the shim runs
    // the original body, which re-derives the check.
    if fullname == "librt.vecs.vec" {
        return Ok(Some(TAG_VEC));
    }
    // Named-tuple / tuple-alias tail (typeanal.py:1228-1253).
    if tuple_type_not_none {
        return Ok(Some(if special_alias_not_none {
            TAG_TUPLE_TAIL_ALIAS
        } else {
            TAG_TUPLE_TAIL
        }));
    }
    // TypedDict tail (typeanal.py:1254-1279).
    if typeddict_type_not_none {
        return Ok(Some(if special_alias_not_none {
            TAG_TYPEDDICT_TAIL_ALIAS
        } else {
            TAG_TYPEDDICT_TAIL
        }));
    }
    // types.NoneType (typeanal.py:1281-1287): the shim fails + builds
    // NoneType inline.
    if fullname == "types.NoneType" {
        return Ok(Some(TAG_NONE_TYPE));
    }
    // Plain Instance (typeanal.py:1289).
    Ok(Some(TAG_INSTANCE))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(fullname, args_len, tuple, special_alias, typeddict)`.
    /// Keeps the five-fact calls readable.
    fn classify(
        fullname: &str,
        args_len: i64,
        tuple: bool,
        special_alias: bool,
        typeddict: bool,
    ) -> Option<i64> {
        rust_classify_type_with_info(
            fullname.to_string(),
            args_len,
            tuple,
            special_alias,
            typeddict,
        )
        .unwrap()
    }

    #[test]
    fn tuple_with_args_wins_over_tails() {
        // tuple[...] with arguments is a TupleType even if the class also
        // carries the tail markers the later branches test.
        assert_eq!(
            classify("builtins.tuple", 2, true, false, true),
            Some(TAG_TUPLE)
        );
    }

    #[test]
    fn bare_tuple_no_args_is_instance() {
        // args_len == 0 defeats the tuple-with-args gate; the class is a
        // plain generic instance.
        assert_eq!(
            classify("builtins.tuple", 0, false, false, false),
            Some(TAG_INSTANCE)
        );
    }

    #[test]
    fn vec_is_its_own_tag() {
        assert_eq!(
            classify("librt.vecs.vec", 0, false, false, false),
            Some(TAG_VEC)
        );
    }

    #[test]
    fn named_tuple_tail_splits_on_alias() {
        assert_eq!(
            classify("mod.Point", 0, true, false, false),
            Some(TAG_TUPLE_TAIL)
        );
        assert_eq!(
            classify("mod.Point", 0, true, true, false),
            Some(TAG_TUPLE_TAIL_ALIAS)
        );
    }

    #[test]
    fn tuple_tail_beats_typeddict_tail() {
        // typeanal.py:1228 tests tuple_type before 1254 tests typeddict_type.
        assert_eq!(
            classify("mod.X", 0, true, false, true),
            Some(TAG_TUPLE_TAIL)
        );
    }

    #[test]
    fn typeddict_tail_splits_on_alias() {
        assert_eq!(
            classify("mod.TD", 0, false, false, true),
            Some(TAG_TYPEDDICT_TAIL)
        );
        assert_eq!(
            classify("mod.TD", 0, false, true, true),
            Some(TAG_TYPEDDICT_TAIL_ALIAS)
        );
    }

    #[test]
    fn none_type_beats_instance() {
        assert_eq!(
            classify("types.NoneType", 0, false, false, false),
            Some(TAG_NONE_TYPE)
        );
    }

    #[test]
    fn plain_reference_is_instance() {
        assert_eq!(
            classify("mod.UserClass", 0, false, false, false),
            Some(TAG_INSTANCE)
        );
    }

    #[test]
    fn generic_reference_is_instance() {
        assert_eq!(
            classify("mod.UserClass", 1, false, false, false),
            Some(TAG_INSTANCE)
        );
    }
}
