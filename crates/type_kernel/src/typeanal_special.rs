#![allow(non_local_definitions)]

//! Native port of the special-form dispatch classifier of
//! `mypy.typeanal.TypeAnalyser.try_analyze_special_unbound_type`
//! (typeanal.py:931-1141).
//!
//! The method is the elif-chain that binds an unbound type whose resolved
//! fullname is a magic special-form name (`builtins.None`, `typing.Any`,
//! `typing.Union`, `Tuple`, `ClassVar`, `Required`, ...). Almost every
//! branch's *decision* is a pure function of the fullname plus a handful of
//! scalar facts (argument count, `empty_tuple_index`, analyzer flags, the
//! configured Python version). This module owns that decision table and
//! returns a single branch tag; the Python shim applies the side effects
//! (error messages, object construction, `anal_type`/`anal_array`
//! recursion) exactly as the original body does.
//!
//! Rust returns `None` (defer) on any path where the decision is not pure
//! or the branch needs recursive type analysis the shim does not route:
//! the tuple full form (`tuple_type`), the `typing.Union` gold path
//! (`make_union` over `anal_array` results), `Optional`'s gold path,
//! `analyze_callable_type`, the `Type[...]`/`TypeForm[...]` one-arg and
//! multi-arg tails, the `ClassVar` one-arg tail, `Annotated`'s gold path,
//! the `Required`/`NotRequired`/`ReadOnly` gold paths, and plain
//! (non-special) names. The deferred branches keep their full pure-Python
//! body, honoring the strangler-fig per-call gate.
//!
//! The `Literal`, `TypeGuard`, `TypeIs` and `Unpack` families are
//! classified by family membership, with the gold paths still routed by
//! the shim: `Literal` always defers to `analyze_literal_type`, the two
//! is-check families fold into the bool-alias branch (tag-name variants
//! `TAG_NAME_TYPEGUARD` / `TAG_NAME_TYPEIS`), and `Unpack` classifies
//! the two pure error branches (`TAG_UNPACK_ARG_ERR` /
//! `TAG_UNPACK_POS_ERR`) plus the gold path (`TAG_UNPACK_DEFER`, whose
//! only non-pure step is the `allow_type_var_tuple` mutation the shim
//! applies around the same `anal_type` recursion). The whole `Self`
//! family (needs the live `api.type`, and there is no pure sub-branch:
//! the args-error falls through to the gold body) defers.
//!
//! Since #775, the bool-alias `TypeGuard`/`TypeIs` filtering and the
//! `Unpack` branch tags are gated on pure arity facts; the shim passes
//! the membership booleans and `allow_unpack` so the classifier can
//! return the tags without touching live `t.args` contents.
//!
//! The fullname membership sets are passed from Python as booleans (the
//! shim computes the tuple membership with the live `mypy.types.*_NAMES`
//! constants, so a Rust copy of the sets cannot drift).

use pyo3::prelude::*;

// Branch tags handed to the Python shim. Each maps to exactly one terminal
// branch of `try_analyze_special_unbound_type`; the comment cites the
// typeanal.py line and the Python-side effect the shim must apply. Plain
// names (not special forms) return `None` from the classifier and keep the
// full pure-Python body.
const TAG_NONE_TYPE: i64 = 2; // 936-937 NoneType()
const TAG_ANY_TYPE: i64 = 3; // 938-939 AnyType(explicit)
const TAG_NEVER: i64 = 4; // 1044-1045 UninhabitedType()
const TAG_FINAL_ERROR: i64 = 5; // 940-954 error Any (flags only)
const TAG_TUPLE_LOOKUP_DEFER: i64 = 6; // 958-964 sym lookup missing/placeholder
const TAG_TUPLE_BARE: i64 = 7; // 965-968 named_type(tuple, [omitted Any])
const TAG_TUPLE_ELLIPSIS: i64 = 8; // 969-973 named_type(tuple, [anal arg])
const TAG_TUPLE_FULL_DEFER: i64 = 9; // 974-976 tuple_type construction
const TAG_UNION_DEFER: i64 = 11; // 1028-1030 make_union over anal_array
const TAG_OPTIONAL_ARG_ERR: i64 = 12; // 981-985 arity != 1 -> fail + Any(from_error)
const TAG_OPTIONAL_DEFER: i64 = 13; // 986-987 make_optional_type
const TAG_CALLABLE_DEFER: i64 = 14; // 988-989 analyze_callable_type
const TAG_TYPE_BARE_ANY: i64 = 15; // 1042-1045 typing.Type bare -> TypeType(Any)
const TAG_TYPE_BARE_NONE: i64 = 16; // 1046-1049 builtins.type bare -> None (#9476)
const TAG_TYPE_ONE_ARG: i64 = 17; // 999-1009 one arg, make_normalized
const TAG_TYPE_ARG_ERR: i64 = 18; // 1000-1003 arity != 1 -> fail + Any(from_error)
const TAG_TYPEFORM_BARE: i64 = 19; // 1011-1013 TypeType(Any, is_type_form)
const TAG_TYPEFORM_DEFER: i64 = 20; // 1014-1020 make_normalized + arity check
const TAG_CLASSVAR_ZERO: i64 = 21; // 1036-1037 Any(from_omitted_generics)
const TAG_CLASSVAR_DEFER: i64 = 22; // 1038-1043 one arg or arity error
const TAG_ANNOTATED_ARG_ERR: i64 = 23; // 1049-1056 arity < 2 -> fail + Any(from_error)
const TAG_ANNOTATED_DEFER: i64 = 24; // 1057-1059 anal_type
const TAG_REQUIRED_BAD_CTX: i64 = 25; // 1061-1067 flag off -> fail + Any(from_error)
const TAG_REQUIRED_ARG_ERR: i64 = 26; // 1068-1072 arity != 1 -> fail + Any(from_error)
const TAG_REQUIRED_DEFER: i64 = 27; // 1073-1075 gold path
const TAG_NOTREQUIRED_BAD_CTX: i64 = 28; // 1077-1083 flag off -> fail + Any(from_error)
const TAG_NOTREQUIRED_ARG_ERR: i64 = 29; // 1084-1088 arity != 1 -> fail + Any(from_error)
const TAG_NOTREQUIRED_DEFER: i64 = 30; // 1089-1091 gold path
const TAG_READONLY_BAD_CTX: i64 = 31; // 1093-1099 flag off -> fail + Any(from_error)
const TAG_READONLY_ARG_ERR: i64 = 32; // 1100-1104 arity != 1 -> fail + Any(from_error)
const TAG_READONLY_DEFER: i64 = 33; // 1105 gold path
const TAG_LITERAL_DEFER: i64 = 34; // 1108-1109 analyze_literal_type
const TAG_NAME_TYPEGUARD: i64 = 35; // 1168-1173 TypeGuard/TypeIs is-check
const TAG_NAME_TYPEIS: i64 = 36; // 1168-1173 TypeGuard/TypeIs is-check
const TAG_UNPACK_ARG_ERR: i64 = 37; // 1174-1177 arity != 1 -> fail + Any(from_error)
const TAG_UNPACK_POS_ERR: i64 = 38; // 1178-1180 !allow_unpack -> fail + Any(from_error)
const TAG_UNPACK_DEFER: i64 = 39; // 1181-1186 gold path (mutates allow_type_var_tuple)
const TAG_NOT_SPECIAL: i64 = 40; // tail: a plain name; Python skips the elif-chain

/// `try_analyze_special_unbound_type` classifier. Mirrors the branch order
/// of typeanal.py:936-1141 and returns the terminal branch tag; `None`
/// defers to the pure-Python body.
///
/// Facts (all scalars / strings, no live type objects):
/// - `fullname`: the resolved magic fullname (`typing.Tuple` for both
///   `typing.Tuple` and `typing_extensions.Tuple`).
/// - `arg_count`, `empty_tuple_index`: from `t`.
/// - `allow_typed_dict_special_forms`: the analyzer flag gating the
///   `Required`/`NotRequired`/`ReadOnly` "bad context" error.
/// - `tuple_missing_or_placeholder`: the `builtins.tuple` symbol lookup for
///   the Tuple family failed (missing or a `PlaceholderNode`).
/// - `tuple_ellipsis_form`: `len(t.args) == 2 and isinstance(t.args[1],
///   EllipsisType)` computed on the live `t` (the Tuple[T, ...] form).
/// - `not_in_*`: the fullname is not in the corresponding
///   `mypy.types.*_NAMES` tuple / `SELF_TYPE_NAMES` set (computed
///   Python-side).
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub(crate) fn rust_classify_special_unbound(
    fullname: String,
    arg_count: i64,
    empty_tuple_index: bool,
    allow_typed_dict_special_forms: bool,
    tuple_missing_or_placeholder: bool,
    tuple_ellipsis_form: bool,
    not_in_final: bool,
    not_in_tuple: bool,
    not_in_type: bool,
    not_in_typeform: bool,
    not_in_classvar: bool,
    not_in_never: bool,
    not_in_annotated: bool,
    not_in_required: bool,
    not_in_notrequired: bool,
    not_in_readonly: bool,
    not_in_literal: bool,
    not_in_unpack: bool,
    not_in_self: bool,
    allow_unpack: bool,
) -> PyResult<Option<i64>> {
    // builtins.None (typeanal.py:936-937).
    if fullname == "builtins.None" {
        return Ok(Some(TAG_NONE_TYPE));
    }
    // typing.Any (typeanal.py:938-939).
    if fullname == "typing.Any" {
        return Ok(Some(TAG_ANY_TYPE));
    }
    // Final (typeanal.py:940-954): always a from_error Any; the shim applies
    // the prohibiting-context / allow_final message.
    if !not_in_final {
        return Ok(Some(TAG_FINAL_ERROR));
    }
    // Tuple (typeanal.py:955-976).
    if !not_in_tuple {
        if tuple_missing_or_placeholder {
            // 958-964: lookup missing/placeholder -> record_incomplete_ref /
            // fail + Any(special_form). Decision is pure; the message
            // depends on api.is_incomplete_namespace, which the shim holds.
            return Ok(Some(TAG_TUPLE_LOOKUP_DEFER));
        }
        if tuple_ellipsis_form {
            return Ok(Some(TAG_TUPLE_ELLIPSIS));
        }
        if arg_count == 0 && !empty_tuple_index {
            // Bare 'Tuple' is same as 'tuple'.
            return Ok(Some(TAG_TUPLE_BARE));
        }
        // Tuple[()] (empty_tuple_index) and Tuple[T, U, ...] (any other
        // arity) -> the full form.
        return Ok(Some(TAG_TUPLE_FULL_DEFER));
    }
    // typing.Union (typeanal.py:1028-1030): no arity check in the original;
    // make_union over anal_array.
    if fullname == "typing.Union" {
        return Ok(Some(TAG_UNION_DEFER));
    }
    // typing.Optional (typeanal.py:980-987).
    if fullname == "typing.Optional" {
        return Ok(Some(if arg_count == 1 {
            TAG_OPTIONAL_DEFER
        } else {
            TAG_OPTIONAL_ARG_ERR
        }));
    }
    // typing.Callable (typeanal.py:988-989): analyze_callable_type is not
    // pure, always defer.
    if fullname == "typing.Callable" {
        return Ok(Some(TAG_CALLABLE_DEFER));
    }
    // typing.Type / type (typeanal.py:990-1009).
    if !not_in_type {
        return Ok(Some(if arg_count == 0 {
            if fullname == "typing.Type" {
                TAG_TYPE_BARE_ANY
            } else {
                TAG_TYPE_BARE_NONE
            }
        } else if arg_count == 1 {
            TAG_TYPE_ONE_ARG
        } else {
            TAG_TYPE_ARG_ERR
        }));
    }
    // TypeForm (typeanal.py:1010-1020).
    if !not_in_typeform {
        return Ok(Some(if arg_count == 0 {
            TAG_TYPEFORM_BARE
        } else {
            TAG_TYPEFORM_DEFER
        }));
    }
    // ClassVar (typeanal.py:1021-1043).
    if !not_in_classvar {
        return Ok(Some(if arg_count == 0 {
            TAG_CLASSVAR_ZERO
        } else {
            TAG_CLASSVAR_DEFER
        }));
    }
    // Never (typeanal.py:1044-1045).
    if !not_in_never {
        return Ok(Some(TAG_NEVER));
    }
    // typing.Literal (typeanal.py:1046-1047): the gold path is
    // analyze_literal_type (recursive, side-effect-bound) which stays in
    // Python; the family membership is decidable from the fullname.
    if !not_in_literal {
        return Ok(Some(TAG_LITERAL_DEFER));
    }
    // Annotated (typeanal.py:1048-1059).
    if !not_in_annotated {
        return Ok(Some(if arg_count < 2 {
            TAG_ANNOTATED_ARG_ERR
        } else {
            TAG_ANNOTATED_DEFER
        }));
    }
    // Required / NotRequired / ReadOnly (typeanal.py:1060-1105).
    let (bad_ctx_tag, arg_err_tag, defer_tag) = if !not_in_required {
        (
            TAG_REQUIRED_BAD_CTX,
            TAG_REQUIRED_ARG_ERR,
            TAG_REQUIRED_DEFER,
        )
    } else if !not_in_notrequired {
        (
            TAG_NOTREQUIRED_BAD_CTX,
            TAG_NOTREQUIRED_ARG_ERR,
            TAG_NOTREQUIRED_DEFER,
        )
    } else if !not_in_readonly {
        (
            TAG_READONLY_BAD_CTX,
            TAG_READONLY_ARG_ERR,
            TAG_READONLY_DEFER,
        )
    } else {
        // TypeGuard / TypeIs / Unpack / Self (typeanal.py:1168-1202) and
        // the non-special tail. Unpack is fully classified: the pure
        // error branches plus the gold path tag, whose only non-pure step
        // is the allow_type_var_tuple mutation the shim applies.
        if !not_in_unpack {
            if arg_count != 1 {
                return Ok(Some(TAG_UNPACK_ARG_ERR));
            }
            if !allow_unpack {
                return Ok(Some(TAG_UNPACK_POS_ERR));
            }
            return Ok(Some(TAG_UNPACK_DEFER));
        }
        // Self (typeanal.py:1188-1205): state-dependent (api.type,
        // has_base, prohibit_self_type), always defer to Python.
        if !not_in_self {
            return Ok(None);
        }
        return Ok(classify_tail(&fullname));
    };
    let tag = if !allow_typed_dict_special_forms {
        bad_ctx_tag
    } else if arg_count != 1 {
        arg_err_tag
    } else {
        defer_tag
    };
    Ok(Some(tag))
}

// Implicit-tuple message tags for the Python shim (visit_tuple_type,
// typeanal.py:2041-2058). OK takes the normal reconstruction path;
// EMPTY/SINGLE/MULTI select the one-of-three suggestion note.
const TAG_TUPLE_OK: i64 = 0; // normal path: named_type + anal_array
const TAG_TUPLE_EMPTY: i64 = 1; // len(items) == 0 -> Tuple[()] suggestion
const TAG_TUPLE_SINGLE: i64 = 2; // len(items) == 1 -> spurious comma
const TAG_TUPLE_MULTI: i64 = 3; // len(items) > 1 -> Tuple[T1, ..., Tn]

/// `visit_tuple_type` implicit-tuple message-arbitration classifier.
/// Mirrors the branch order of typeanal.py:2041-2058: the error head fires
/// only when `t.implicit` is set and `allow_tuple_literal` is off; inside
/// the head the note is chosen by `len(t.items)`. All three facts are
/// scalars, so the classifier never defers: every (implicit,
/// allow_tuple_literal, items_len) triple maps to exactly one tag, and
/// `None` is unreachable (kept as the exception-only deferral shape).
#[pyfunction]
pub(crate) fn rust_classify_tuple_type_implicit(
    implicit: bool,
    allow_tuple_literal: bool,
    items_len: usize,
) -> PyResult<Option<i64>> {
    if !(implicit && !allow_tuple_literal) {
        return Ok(Some(TAG_TUPLE_OK));
    }
    let tag = if items_len == 0 {
        TAG_TUPLE_EMPTY
    } else if items_len == 1 {
        TAG_TUPLE_SINGLE
    } else {
        TAG_TUPLE_MULTI
    };
    Ok(Some(tag))
}

// TypeGuard/TypeIs argument tags for the Python shim (anal_type_guard_arg /
// anal_type_is_arg, typeanal.py:2009-2033). NOT_GUARD lets the Python
// wrapper return None; FAIL/RECURSE select the shim's side effects.
const TAG_GUARD_NOT_GUARD: i64 = 0; // fullname not in the family -> Python returns None
const TAG_GUARD_FAIL: i64 = 1; // arity != 1 -> fail(VALID_TYPE) + Any(from_error)
const TAG_GUARD_RECURSE: i64 = 2; // arity == 1 -> anal_type(t.args[0])

/// `anal_type_guard_arg` / `anal_type_is_arg` classifier (typeanal.py
/// 2009-2033). Mirrors the two-step decision: family membership by the
/// `is_typeis` flag (TypeGuard vs TypeIs name-sets), then the arity gate.
/// All facts are scalars (the shim precomputes the fullname via
/// `lookup_qualified`; the `isinstance(t, UnboundType)` check stays
/// Python-side), so the classifier never defers: every (fullname,
/// args_len, is_typeis) triple maps to exactly one tag, and `None` is
/// unreachable (kept as the exception-only deferral shape).
#[pyfunction]
pub(crate) fn rust_classify_type_guard_arg(
    fullname: String,
    args_len: usize,
    is_typeis: bool,
) -> PyResult<Option<i64>> {
    let in_family = if is_typeis {
        fullname == "typing.TypeIs" || fullname == "typing_extensions.TypeIs"
    } else {
        fullname == "typing.TypeGuard" || fullname == "typing_extensions.TypeGuard"
    };
    if !in_family {
        return Ok(Some(TAG_GUARD_NOT_GUARD));
    }
    if args_len != 1 {
        return Ok(Some(TAG_GUARD_FAIL));
    }
    Ok(Some(TAG_GUARD_RECURSE))
}

/// Classify the tail of `try_analyze_special_unbound_type`
/// (typeanal.py:1168-1202): the TypeGuard/TypeIs is-check families, the
/// Unpack and Self special forms, and the non-special tail.
fn classify_tail(fullname: &str) -> Option<i64> {
    match fullname {
        "typing.TypeGuard" | "typing_extensions.TypeGuard" => Some(TAG_NAME_TYPEGUARD),
        "typing.TypeIs" | "typing_extensions.TypeIs" => Some(TAG_NAME_TYPEIS),
        // Any other name is not a special form; the shim then skips the
        // elif-chain (which would fall through to None anyway).
        _ => Some(TAG_NOT_SPECIAL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Facts {
        fullname: String,
        arg_count: i64,
        empty_tuple_index: bool,
        allow_typed_dict_special_forms: bool,
        tuple_missing_or_placeholder: bool,
        tuple_ellipsis_form: bool,
        allow_unpack: bool,
    }

    impl Default for Facts {
        fn default() -> Self {
            Facts {
                fullname: "mod.SomeName".to_string(),
                arg_count: 0,
                empty_tuple_index: false,
                allow_typed_dict_special_forms: false,
                tuple_missing_or_placeholder: false,
                tuple_ellipsis_form: false,
                allow_unpack: false,
            }
        }
    }

    /// Build the per-family membership booleans exactly as the Python shim
    /// does with the live `mypy.types.*_NAMES` tuples. The sets are
    /// duplicated here inline; a divergence would fail the parity suites.
    fn classify(f: &Facts) -> Option<i64> {
        rust_classify_special_unbound(
            f.fullname.clone(),
            f.arg_count,
            f.empty_tuple_index,
            f.allow_typed_dict_special_forms,
            f.tuple_missing_or_placeholder,
            f.tuple_ellipsis_form,
            !(f.fullname == "typing.Final" || f.fullname == "typing_extensions.Final"),
            !(f.fullname == "builtins.tuple" || f.fullname == "typing.Tuple"),
            !(f.fullname == "builtins.type" || f.fullname == "typing.Type"),
            !(f.fullname == "typing.TypeForm" || f.fullname == "typing_extensions.TypeForm"),
            !(f.fullname == "typing.ClassVar"),
            !(f.fullname == "typing.NoReturn"
                || f.fullname == "typing_extensions.NoReturn"
                || f.fullname == "mypy_extensions.NoReturn"
                || f.fullname == "typing.Never"
                || f.fullname == "typing_extensions.Never"),
            !(f.fullname == "typing.Annotated" || f.fullname == "typing_extensions.Annotated"),
            f.fullname != "typing.Required" && f.fullname != "typing_extensions.Required",
            f.fullname != "typing.NotRequired" && f.fullname != "typing_extensions.NotRequired",
            f.fullname != "typing.ReadOnly" && f.fullname != "typing_extensions.ReadOnly",
            !(f.fullname == "typing.Literal" || f.fullname == "typing_extensions.Literal"),
            !(f.fullname == "typing.Unpack" || f.fullname == "typing_extensions.Unpack"),
            f.fullname != "typing.Self" && f.fullname != "typing_extensions.Self",
            f.allow_unpack,
        )
        .unwrap()
    }

    #[test]
    fn plain_name_is_not_special() {
        // A plain (non-special) name is now classified: the Python shim
        // skips the whole elif-chain (which would return None anyway).
        assert_eq!(classify(&Facts::default()), Some(TAG_NOT_SPECIAL));
    }

    #[test]
    fn self_defers() {
        // Self is state-dependent (api.type, has_base, prohibit_self_type);
        // Rust must defer so the pure-Python body runs unchanged.
        let f = Facts {
            fullname: "typing.Self".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), None);
    }

    #[test]
    fn none_type() {
        let f = Facts {
            fullname: "builtins.None".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NONE_TYPE));
    }

    #[test]
    fn any_type() {
        let f = Facts {
            fullname: "typing.Any".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_ANY_TYPE));
    }

    #[test]
    fn final_is_always_error_any() {
        let f = Facts {
            fullname: "typing.Final".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_FINAL_ERROR));
    }

    #[test]
    fn final_ext_is_always_error_any() {
        let f = Facts {
            fullname: "typing_extensions.Final".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_FINAL_ERROR));
    }

    #[test]
    fn tuple_bare() {
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_BARE));
    }

    #[test]
    fn tuple_ellipsis() {
        // Tuple[T, ...]: arg_count 2, second arg is EllipsisType (the shim
        // computes tuple_ellipsis_form on the live t).
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            arg_count: 2,
            tuple_ellipsis_form: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_ELLIPSIS));
    }

    #[test]
    fn tuple_full_non_ellipsis_defers() {
        // Tuple[int, str]: arg_count 2 but no EllipsisType -> full form.
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            arg_count: 2,
            tuple_ellipsis_form: false,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_FULL_DEFER));
    }

    #[test]
    fn tuple_fixed_arity_defers() {
        // Tuple[int, str] has arg_count 2 with empty_tuple_index False; the
        // search hits the "other arity" tail -> full form.
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_FULL_DEFER));
    }

    #[test]
    fn tuple_empty_index_is_full_form() {
        // Tuple[()] -> arg_count 1, empty_tuple_index True -> the Python
        // conditional `len(t.args) == 0 and not t.empty_tuple_index` fails,
        // so it falls to the full form.
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            arg_count: 1,
            empty_tuple_index: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_FULL_DEFER));
    }

    #[test]
    fn tuple_missing_lookup_defers() {
        let f = Facts {
            fullname: "typing.Tuple".to_string(),
            tuple_missing_or_placeholder: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TUPLE_LOOKUP_DEFER));
    }

    #[test]
    fn union_single_arg_defers() {
        // The original Union branch has no arity check; any arity defers to
        // make_union over anal_array.
        let f = Facts {
            fullname: "typing.Union".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_UNION_DEFER));
    }

    #[test]
    fn union_gold_path_defers() {
        let f = Facts {
            fullname: "typing.Union".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_UNION_DEFER));
    }

    #[test]
    fn optional_arity_error() {
        let f = Facts {
            fullname: "typing.Optional".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_OPTIONAL_ARG_ERR));
    }

    #[test]
    fn optional_gold_path_defers() {
        let f = Facts {
            fullname: "typing.Optional".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_OPTIONAL_DEFER));
    }

    #[test]
    fn callable_defers() {
        let f = Facts {
            fullname: "typing.Callable".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_CALLABLE_DEFER));
    }

    #[test]
    fn type_bare_any() {
        // typing.Type bare -> TypeType(Any) (#9476 only forces None for
        // builtins.type, which keeps 'type' from collapsing to object).
        let f = Facts {
            fullname: "typing.Type".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPE_BARE_ANY));
    }

    #[test]
    fn type_bare_none_9476() {
        let f = Facts {
            fullname: "builtins.type".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPE_BARE_NONE));
    }

    #[test]
    fn type_one_arg() {
        let f = Facts {
            fullname: "typing.Type".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPE_ONE_ARG));
    }

    #[test]
    fn type_arity_error() {
        let f = Facts {
            fullname: "typing.Type".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPE_ARG_ERR));
    }

    #[test]
    fn typeform_bare() {
        let f = Facts {
            fullname: "typing.TypeForm".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPEFORM_BARE));
    }

    #[test]
    fn typeform_one_arg_defers() {
        let f = Facts {
            fullname: "typing.TypeForm".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_TYPEFORM_DEFER));
    }

    #[test]
    fn classvar_zero() {
        let f = Facts {
            fullname: "typing.ClassVar".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_CLASSVAR_ZERO));
    }

    #[test]
    fn classvar_one_arg_defers() {
        let f = Facts {
            fullname: "typing.ClassVar".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_CLASSVAR_DEFER));
    }

    #[test]
    fn classvar_multi_arg_defers() {
        let f = Facts {
            fullname: "typing.ClassVar".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_CLASSVAR_DEFER));
    }

    #[test]
    fn never_tag() {
        let f = Facts {
            fullname: "typing.Never".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NEVER));
    }

    #[test]
    fn never_noreturn_tag() {
        let f = Facts {
            fullname: "typing.NoReturn".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NEVER));
    }

    #[test]
    fn annotate_arity_error() {
        let f = Facts {
            fullname: "typing.Annotated".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_ANNOTATED_ARG_ERR));
    }

    #[test]
    fn annotate_gold_path_defers() {
        let f = Facts {
            fullname: "typing_extensions.Annotated".to_string(),
            arg_count: 2,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_ANNOTATED_DEFER));
    }

    #[test]
    fn required_bad_ctx() {
        let f = Facts {
            fullname: "typing.Required".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_REQUIRED_BAD_CTX));
    }

    #[test]
    fn required_arg_err() {
        let f = Facts {
            fullname: "typing.Required".to_string(),
            arg_count: 2,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_REQUIRED_ARG_ERR));
    }

    #[test]
    fn required_defer() {
        let f = Facts {
            fullname: "typing.Required".to_string(),
            arg_count: 1,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_REQUIRED_DEFER));
    }

    #[test]
    fn notrequired_bad_ctx() {
        let f = Facts {
            fullname: "typing_extensions.NotRequired".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NOTREQUIRED_BAD_CTX));
    }

    #[test]
    fn notrequired_arg_err() {
        let f = Facts {
            fullname: "typing.NotRequired".to_string(),
            arg_count: 0,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NOTREQUIRED_ARG_ERR));
    }

    #[test]
    fn notrequired_defer() {
        let f = Facts {
            fullname: "typing.NotRequired".to_string(),
            arg_count: 1,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NOTREQUIRED_DEFER));
    }

    #[test]
    fn readonly_bad_ctx() {
        let f = Facts {
            fullname: "typing.ReadOnly".to_string(),
            arg_count: 1,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_READONLY_BAD_CTX));
    }

    #[test]
    fn readonly_arg_err() {
        let f = Facts {
            fullname: "typing.ReadOnly".to_string(),
            arg_count: 2,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_READONLY_ARG_ERR));
    }

    #[test]
    fn readonly_defer() {
        let f = Facts {
            fullname: "typing.ReadOnly".to_string(),
            arg_count: 1,
            allow_typed_dict_special_forms: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_READONLY_DEFER));
    }

    #[test]
    fn literal_defers_tag() {
        // Literal always defers to analyze_literal_type; the tag tells the
        // shim which branch to run.
        let f = Facts {
            fullname: "typing.Literal".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_LITERAL_DEFER));
    }

    #[test]
    fn typeguard_tags() {
        let f = Facts {
            fullname: "typing.TypeGuard".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NAME_TYPEGUARD));
    }

    #[test]
    fn typeguard_ext_tags() {
        let f = Facts {
            fullname: "typing_extensions.TypeGuard".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NAME_TYPEGUARD));
    }

    #[test]
    fn typeis_tags() {
        let f = Facts {
            fullname: "typing_extensions.TypeIs".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_NAME_TYPEIS));
    }

    #[test]
    fn unpack_arg_count_error() {
        // Unpack[int, str] -> arity != 1 -> from_error Any; only the two
        // error branches of the Unpack family are classified.
        let f = Facts {
            fullname: "typing.Unpack".to_string(),
            arg_count: 2,
            allow_unpack: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_UNPACK_ARG_ERR));
    }

    #[test]
    fn unpack_not_in_variadic_position_error() {
        let f = Facts {
            fullname: "typing_extensions.Unpack".to_string(),
            arg_count: 1,
            allow_unpack: false,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_UNPACK_POS_ERR));
    }

    #[test]
    fn unpack_gold_path_tags_defer() {
        // Unpack[int] in a variadic position: the gold path tag routes the
        // shim to the exact original body (mutates allow_type_var_tuple
        // around anal_type).
        let f = Facts {
            fullname: "typing.Unpack".to_string(),
            arg_count: 1,
            allow_unpack: true,
            ..Default::default()
        };
        assert_eq!(classify(&f), Some(TAG_UNPACK_DEFER));
    }

    #[test]
    fn self_type_defers() {
        // Every Self branch needs the live api.type / self_type, and the
        // args-error falls through to the gold body; nothing is decidable.
        let f = Facts {
            fullname: "typing_extensions.Self".to_string(),
            ..Default::default()
        };
        assert_eq!(classify(&f), None);
    }
}

#[cfg(test)]
mod tuple_implicit_tests {
    use super::*;

    fn classify(implicit: bool, allow_tuple_literal: bool, items_len: usize) -> Option<i64> {
        rust_classify_tuple_type_implicit(implicit, allow_tuple_literal, items_len).unwrap()
    }

    #[test]
    fn not_implicit_is_ok() {
        assert_eq!(classify(false, false, 2), Some(TAG_TUPLE_OK));
    }

    #[test]
    fn implicit_with_allowed_literal_is_ok() {
        assert_eq!(classify(true, true, 0), Some(TAG_TUPLE_OK));
    }

    #[test]
    fn implicit_empty_items() {
        assert_eq!(classify(true, false, 0), Some(TAG_TUPLE_EMPTY));
    }

    #[test]
    fn implicit_single_item() {
        assert_eq!(classify(true, false, 1), Some(TAG_TUPLE_SINGLE));
    }

    #[test]
    fn implicit_many_items() {
        assert_eq!(classify(true, false, 3), Some(TAG_TUPLE_MULTI));
    }
}

#[cfg(test)]
mod type_guard_arg_tests {
    use super::*;

    fn classify(fullname: &str, args_len: usize, is_typeis: bool) -> Option<i64> {
        rust_classify_type_guard_arg(fullname.to_string(), args_len, is_typeis).unwrap()
    }

    #[test]
    fn guard_one_arg_recurses() {
        assert_eq!(
            classify("typing.TypeGuard", 1, false),
            Some(TAG_GUARD_RECURSE)
        );
    }

    #[test]
    fn guard_ext_one_arg_recurses() {
        assert_eq!(
            classify("typing_extensions.TypeGuard", 1, false),
            Some(TAG_GUARD_RECURSE)
        );
    }

    #[test]
    fn guard_zero_args_fails() {
        assert_eq!(classify("typing.TypeGuard", 0, false), Some(TAG_GUARD_FAIL));
    }

    #[test]
    fn guard_two_args_fails() {
        assert_eq!(
            classify("typing_extensions.TypeGuard", 2, false),
            Some(TAG_GUARD_FAIL)
        );
    }

    #[test]
    fn typeis_one_arg_recurses() {
        assert_eq!(classify("typing.TypeIs", 1, true), Some(TAG_GUARD_RECURSE));
    }

    #[test]
    fn typeis_ext_one_arg_recurses() {
        assert_eq!(
            classify("typing_extensions.TypeIs", 1, true),
            Some(TAG_GUARD_RECURSE)
        );
    }

    #[test]
    fn typeis_zero_args_fails() {
        assert_eq!(classify("typing.TypeIs", 0, true), Some(TAG_GUARD_FAIL));
    }

    #[test]
    fn typeis_two_args_fails() {
        assert_eq!(classify("typing.TypeIs", 3, true), Some(TAG_GUARD_FAIL));
    }

    #[test]
    fn non_guard_fullname_is_not_guard() {
        assert_eq!(
            classify("mod.NotAGuard", 1, false),
            Some(TAG_GUARD_NOT_GUARD)
        );
        assert_eq!(
            classify("mod.NotAGuard", 0, true),
            Some(TAG_GUARD_NOT_GUARD)
        );
    }

    #[test]
    fn guard_fullname_with_typeis_flag_is_not_guard() {
        // The name-sets are disjoint: a TypeGuard fullname with the TypeIs
        // flag is outside the TypeIs family (and vice versa).
        assert_eq!(
            classify("typing.TypeGuard", 1, true),
            Some(TAG_GUARD_NOT_GUARD)
        );
        assert_eq!(
            classify("typing.TypeIs", 1, false),
            Some(TAG_GUARD_NOT_GUARD)
        );
    }

    #[test]
    fn other_special_form_is_not_guard() {
        assert_eq!(
            classify("typing.Optional", 1, false),
            Some(TAG_GUARD_NOT_GUARD)
        );
    }
}
