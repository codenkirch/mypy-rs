#![allow(non_local_definitions)]

//! Native port of the pure classification front of
//! `mypy.typeanal.TypeAnalyser.analyze_unbound_type_without_type_info`
//! (typeanal.py:986-1107).
//!
//! The function figures out what an unbound type that does not resolve to
//! a TypeInfo node means:
//!
//! - a `Var` typed `Any` is an alias for `Any` in type context;
//! - under `allow_type_any`, a `Var` typed `Instance("builtins.type")` is
//!   the `type` special form and `TypeType[Any]` maps to
//!   `AnyType(from_another_any)`;
//! - an unbound type variable is kept as-is when the caller permits it
//!   (generic type alias right-hand sides);
//! - an enum member used inside `Literal[...]` becomes a `LiteralType`;
//! - every remaining shape falls into the message tail: `Variable ... is
//!   not valid as a type`, the function/module families, the rejected
//!   unbound type variable (with the PEP 695 name-not-defined variant),
//!   the raw-enum-value error, and `Cannot interpret reference ...`.

use pyo3::prelude::*;

/// Branch tags handed to the Python shim:
/// - `UNIMPORTED_ANY`: Var with an Any type -> `AnyType(from_unimported_type,
///   missing_import_name=typ.missing_import_name)`.
/// - `SPECIAL_ANY`: Var typed `Instance("builtins.type")` under
///   `allow_type_any` -> `AnyType(TypeOfAny.special_form)`.
/// - `TYPE_TYPE_ANY`: Var typed `TypeType[Any]` under `allow_type_any` ->
///   `AnyType(from_another_any, source_any=typ.item)` (live object).
/// - `TVAR`: unbound type variable allowed -> return `t` unchanged.
/// - `TVAR_PEP695`: unbound PEP 695 type parameter -> `Name "T" is not
///   defined` (NAME_DEFINED).
/// - `TVAR_HINTS`: unbound classic type variable -> `Type variable "T" is
///   unbound` + the two Generic/Protocol hints.
/// - `LITERAL`: enum member inside `Literal[...]` -> `LiteralType(value,
///   fallback=Instance(info, [], line, column), line, column)`.
/// - `ENUM_RAW`: enum member outside `Literal[...]` -> raw-enum-value
///   error + `AnyType(from_error)`.
/// - `TAIL_*`: the message tail of the pure body; `TAIL_FUNC` carries one
///   note tag (FUNC_NOTE_ANY / FUNC_NOTE_CALLABLE / FUNC_NOTE_CALLBACK).
const TAG_UNIMPORTED_ANY: i64 = 1;
const TAG_SPECIAL_ANY: i64 = 2;
const TAG_TVAR: i64 = 3;
const TAG_LITERAL: i64 = 4;
const TAG_TYPE_TYPE_ANY: i64 = 5;
const TAG_TVAR_HINTS: i64 = 6;
const TAG_TVAR_PEP695: i64 = 7;
const TAG_ENUM_RAW: i64 = 8;
const TAG_TAIL_VAR: i64 = 9;
const TAG_TAIL_FUNC: i64 = 10;
const TAG_TAIL_MODULE: i64 = 11;
const TAG_TAIL_OTHER: i64 = 12;
const NOTE_FUNC_ANY: i64 = 1;
const NOTE_FUNC_CALLABLE: i64 = 2;
const NOTE_FUNC_CALLBACK: i64 = 3;

/// `analyze_unbound_type_without_type_info` classifier — mirrors the branch
/// order of typeanal.py:1002-1107.
///
/// The Python shim computes the raw node facts (cheap `isinstance` /
/// attribute reads on the live `sym.node`) and passes them as booleans/
/// scalars; Rust owns the priority-ordered decision table, including the
/// message-tail family selection. Precedence mirrors the Python body
/// exactly: Var-Any first, then the `allow_type_any` special forms, then
/// the unbound type variable, then the enum member, then the tail kinds in
/// the body's elif order (Var / function-symbol / module / other).
///
/// `tail_kind` mirrors the body's message kind (0 Var, 1
/// `SYMBOL_FUNCBASE_TYPES or Decorator`, 2 MypyFile, 3 other) and `name`
/// is the already-resolved reference name used for the function-note
/// membership checks (`builtins.any` beats `typing.Any`,
/// `builtins.callable` beats `typing.Callable`).
///
/// Never defers on well-formed facts: every reachable branch is decided.
/// `Ok(None)` (defer to the pure-Python body) only covers undecidable
/// facts, which the shim represents by not calling the seam.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub(crate) fn rust_analyze_unbound_without_info(
    is_var_any: bool,
    allow_type_any: bool,
    is_type_instance: bool,
    is_type_type_any: bool,
    unbound_tvar: bool,
    allow_unbound_tvars: bool,
    is_enum_member: bool,
    defining_literal: bool,
    is_new_style: bool,
    tail_kind: i64,
    name: &str,
) -> PyResult<Option<(i64, Vec<i64>)>> {
    // Option 1 (typeanal.py:1002-1006): a Var typed Any is an alias for
    // Any in a type context.
    if is_var_any {
        return Ok(Some((TAG_UNIMPORTED_ANY, Vec::new())));
    }
    // Option 1.5 (typeanal.py:1007-1013): under allow_type_any, a Var
    // typed `type` is the special form; TypeType[Any] maps to
    // from_another_any with the live source Any (rebuilt shim-side).
    if allow_type_any {
        if is_type_instance {
            return Ok(Some((TAG_SPECIAL_ANY, Vec::new())));
        }
        if is_type_type_any {
            return Ok(Some((TAG_TYPE_TYPE_ANY, Vec::new())));
        }
    }
    // Option 2 (typeanal.py:1015-1021): an unbound type variable is kept when
    // the caller permits it (generic alias RHS); when rejected it reaches the
    // tail's type-variable branch (PEP 695: "Name is not defined", classic: unbound).
    if unbound_tvar {
        if allow_unbound_tvars {
            return Ok(Some((TAG_TVAR, Vec::new())));
        }
        if is_new_style {
            return Ok(Some((TAG_TVAR_PEP695, Vec::new())));
        }
        return Ok(Some((TAG_TVAR_HINTS, Vec::new())));
    }
    // Option 3 (typeanal.py:1023-1046): an enum member is only a
    // LiteralType inside Literal[...]; outside that context it is the
    // raw-enum-value error (emission stays Python-owned).
    if is_enum_member {
        if defining_literal {
            return Ok(Some((TAG_LITERAL, Vec::new())));
        }
        return Ok(Some((TAG_ENUM_RAW, Vec::new())));
    }
    // Message tail (typeanal.py:1071-1107): classification only, all
    // fail/note emission rebuilds from the live symbols in Python.
    match tail_kind {
        1 => {
            let notes = vec![match name {
                "builtins.any" => NOTE_FUNC_ANY,
                "builtins.callable" => NOTE_FUNC_CALLABLE,
                _ => NOTE_FUNC_CALLBACK,
            }];
            Ok(Some((TAG_TAIL_FUNC, notes)))
        }
        0 => Ok(Some((TAG_TAIL_VAR, Vec::new()))),
        2 => Ok(Some((TAG_TAIL_MODULE, Vec::new()))),
        _ => Ok(Some((TAG_TAIL_OTHER, Vec::new()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn classify(
        is_var_any: bool,
        allow_type_any: bool,
        is_type_instance: bool,
        is_type_type_any: bool,
        unbound_tvar: bool,
        allow_unbound_tvars: bool,
        is_enum_member: bool,
        defining_literal: bool,
        is_new_style: bool,
        tail_kind: i64,
        name: &str,
    ) -> Option<(i64, Vec<i64>)> {
        rust_analyze_unbound_without_info(
            is_var_any,
            allow_type_any,
            is_type_instance,
            is_type_type_any,
            unbound_tvar,
            allow_unbound_tvars,
            is_enum_member,
            defining_literal,
            is_new_style,
            tail_kind,
            name,
        )
        .unwrap()
    }

    #[test]
    fn var_any_wins_over_everything() {
        // A Var typed Any is an alias for Any even if it would otherwise
        // be an enum member, and never reaches the tail.
        assert_eq!(
            classify(true, true, false, false, false, false, true, true, false, 1, "x"),
            Some((1, vec![]))
        );
        assert_eq!(
            classify(true, false, false, false, true, true, false, false, true, 3, "T"),
            Some((1, vec![]))
        );
    }

    #[test]
    fn type_instance_under_allow_type_any() {
        assert_eq!(
            classify(false, true, true, false, false, false, false, false, false, 3, "x"),
            Some((2, vec![]))
        );
    }

    #[test]
    fn type_instance_ignored_without_allow_type_any() {
        assert_eq!(
            classify(false, false, true, false, false, false, false, false, false, 3, "x"),
            Some((12, vec![]))
        );
    }

    #[test]
    fn type_type_any_under_allow_type_any() {
        assert_eq!(
            classify(false, true, false, true, false, false, false, false, false, 0, "x"),
            Some((5, vec![]))
        );
    }

    #[test]
    fn type_type_any_ignored_without_allow_type_any() {
        assert_eq!(
            classify(false, false, false, true, false, false, false, false, false, 0, "x"),
            Some((9, vec![]))
        );
    }

    #[test]
    fn unbound_tvar_allowed() {
        assert_eq!(
            classify(false, false, false, false, true, true, false, false, false, 3, "T"),
            Some((3, vec![]))
        );
    }

    #[test]
    fn unbound_tvar_rejected_variants() {
        // PEP 695 parameters report "Name is not defined"; classic
        // parameters report unbound (hints are Python-side strings).
        assert_eq!(
            classify(false, false, false, false, true, false, false, false, true, 3, "m.T"),
            Some((7, vec![]))
        );
        assert_eq!(
            classify(false, false, false, false, true, false, false, false, false, 3, "m.T"),
            Some((6, vec![]))
        );
    }

    #[test]
    fn enum_member_variants() {
        assert_eq!(
            classify(false, false, false, false, false, false, true, true, false, 0, "RED"),
            Some((4, vec![]))
        );
        assert_eq!(
            classify(false, false, false, false, false, false, true, false, false, 0, "RED"),
            Some((8, vec![]))
        );
    }

    #[test]
    fn tail_kind_precedence_matches_body_elif_chain() {
        assert_eq!(
            classify(false, false, false, false, false, false, false, false, false, 0, "x"),
            Some((9, vec![]))
        );
        assert_eq!(
            classify(false, false, false, false, false, false, false, false, false, 1, "x"),
            Some((10, vec![3]))
        );
        assert_eq!(
            classify(false, false, false, false, false, false, false, false, false, 2, "x"),
            Some((11, vec![]))
        );
        assert_eq!(
            classify(false, false, false, false, false, false, false, false, false, 9, "x"),
            Some((12, vec![]))
        );
    }

    #[test]
    fn func_note_membership_uses_resolved_name() {
        assert_eq!(
            classify(
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                1,
                "builtins.any"
            ),
            Some((10, vec![1]))
        );
        assert_eq!(
            classify(
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                1,
                "builtins.callable"
            ),
            Some((10, vec![2]))
        );
        assert_eq!(
            classify(false, false, false, false, false, false, false, false, false, 1, "any"),
            Some((10, vec![3]))
        );
    }
}
