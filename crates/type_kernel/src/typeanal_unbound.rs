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
//!   the `type` special form;
//! - an unbound type variable is kept as-is when the caller permits it
//!   (generic type alias right-hand sides);
//! - an enum member used inside `Literal[...]` becomes a `LiteralType`.
//!
//! The message tail (three error families + notes + the `anal_array` args
//! rewrite) stays Python-owned. Rust applies the ordered decision table
//! over the raw node facts supplied by the Python shim and returns a
//! branch tag; `None` (defer) falls through to the full pure-Python body,
//! keeping message side effects single-sourced. The `TypeType[Any]`
//! special form also defers: the `AnyType(TypeOfAny.from_another_any)`
//! it maps to carries a `source_any` pointer Python must construct.

use pyo3::prelude::*;

/// Branch tags handed to the Python shim:
/// - `UNIMPORTED_ANY`: Var with an Any type -> `AnyType(from_unimported_type,
///   missing_import_name=typ.missing_import_name)`.
/// - `SPECIAL_ANY`: Var typed `Instance("builtins.type")` under
///   `allow_type_any` -> `AnyType(TypeOfAny.special_form)`.
/// - `TVAR`: unbound type variable allowed -> return `t` unchanged.
/// - `LITERAL`: enum member inside `Literal[...]` -> `LiteralType(value,
///   fallback=Instance(info, [], line, column), line, column)`.
const TAG_UNIMPORTED_ANY: i64 = 1;
const TAG_SPECIAL_ANY: i64 = 2;
const TAG_TVAR: i64 = 3;
const TAG_LITERAL: i64 = 4;

/// `analyze_unbound_type_without_type_info` classifier — mirrors the branch
/// order of typeanal.py:1002-1062.
///
/// The Python shim computes the raw node facts (cheap `isinstance` /
/// attribute reads on the live `sym.node`) and passes them as booleans;
/// Rust owns the priority-ordered decision table. Precedence mirrors the
/// Python body exactly: Var-Any first, then the `allow_type_any` special
/// forms, then the unbound type variable, then the enum member.
///
/// Returns `None` (defer) for every path where Python would emit an error
/// or construct an object Rust cannot (TypeType[Any] -> from_another_any,
/// raw enum value error, invalid-reference message tail).
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
) -> PyResult<Option<i64>> {
    // Option 1 (typeanal.py:1002-1006): a Var typed Any is an alias for
    // Any in a type context.
    if is_var_any {
        return Ok(Some(TAG_UNIMPORTED_ANY));
    }
    // Option 1.5 (typeanal.py:1007-1013): under allow_type_any, a Var
    // typed `type` is the special form. The TypeType[Any] case defers:
    // the resulting AnyType carries a source_any Python must build.
    if allow_type_any {
        if is_type_instance {
            return Ok(Some(TAG_SPECIAL_ANY));
        }
        if is_type_type_any {
            return Ok(None);
        }
    }
    // Option 2 (typeanal.py:1015-1021): an unbound type variable is kept
    // when the caller permits it (generic alias right-hand sides).
    if allow_unbound_tvars && unbound_tvar {
        return Ok(Some(TAG_TVAR));
    }
    // Option 3 (typeanal.py:1023-1046): an enum member is only a
    // LiteralType inside Literal[...]; outside that context Python emits
    // the raw-enum-value error, so defer.
    if is_enum_member {
        return if defining_literal {
            Ok(Some(TAG_LITERAL))
        } else {
            Ok(None)
        };
    }
    // None of the above: the message tail stays Python-owned.
    Ok(None)
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
    ) -> Option<i64> {
        rust_analyze_unbound_without_info(
            is_var_any,
            allow_type_any,
            is_type_instance,
            is_type_type_any,
            unbound_tvar,
            allow_unbound_tvars,
            is_enum_member,
            defining_literal,
        )
        .unwrap()
    }

    #[test]
    fn var_any_wins_over_enum() {
        // A Var typed Any is an alias for Any even if it would otherwise
        // be an enum member.
        assert_eq!(
            classify(true, true, false, false, false, false, true, true),
            Some(1)
        );
    }

    #[test]
    fn type_instance_under_allow_type_any() {
        assert_eq!(
            classify(false, true, true, false, false, false, false, false),
            Some(2)
        );
    }

    #[test]
    fn type_instance_ignored_without_allow_type_any() {
        assert_eq!(
            classify(false, false, true, false, false, false, false, false),
            None
        );
    }

    #[test]
    fn type_type_any_defers() {
        assert_eq!(
            classify(false, true, false, true, false, false, false, false),
            None
        );
    }

    #[test]
    fn unbound_tvar_requires_both_flags() {
        assert_eq!(
            classify(false, false, false, false, true, true, false, false),
            Some(3)
        );
        assert_eq!(
            classify(false, false, false, false, true, false, false, false),
            None
        );
    }

    #[test]
    fn enum_member_requires_literal_context() {
        assert_eq!(
            classify(false, false, false, false, false, false, true, true),
            Some(4)
        );
        assert_eq!(
            classify(false, false, false, false, false, false, true, false),
            None
        );
    }

    #[test]
    fn plain_reference_defers() {
        assert_eq!(
            classify(false, false, false, false, false, false, false, false),
            None
        );
    }
}
