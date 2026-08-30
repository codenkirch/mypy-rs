//! Native port of the *decision front* of
//! `mypy.typeanal.TypeAnalyser.visit_unbound_type_nonoptional`
//! (typeanal.py:310-482).
//!
//! This is the dispatch hub for every unbound type resolution. The method
//! resolves a symbol, then, before it reaches the "back" (special unbound
//! types, `TypeAlias` expansion, `TypeInfo` analysis,
//! `analyze_unbound_type_without_type_info`), it settles a fixed set of
//! node-family branches:
//!
//! - unresolved symbol / `node is None` internal error;
//! - `PlaceholderNode` (cyclic / incomplete references);
//! - `ParamSpecExpr` (bound, unbound, args, component-literal);
//! - `TypeVarExpr` (generic-alias guard, erased, bound construct);
//! - `TypeVarTupleExpr` (generic-alias guard, unbound, unpack-nesting,
//!   bound construct).
//!
//! Rust receives raw node *facts* (booleans / ints / a string set) computed
//! by the Python shim and returns a single branch tag. Python applies the
//! side effects and builds the result object for that tag. Returning `None`
//! (defer) falls through to the full pure-Python body, keeping error
//! messages and construction single-sourced.
//!
//! The hook path (`plugin.get_type_analyze_hook`, typeanal.py:356-358) and
//! every non-front node kind (Var, TypeAlias, TypeInfo, function, ...) are
//! NOT classifiable from facts alone, so they defer to Python. The
//! `tvar_scope.get_binding` lookup and the "typevar params contain a
//! placeholder -> api.defer()" pre-check stay Python-side; the shim passes
//! them back as facts and re-applies the pre-check deferral only when Rust
//! actually decides a front branch. An unbound, non-alias TypeVarExpr
//! decides natively under `allow_unbound_tvars` (the without-info back
//! returns the raw type); its fail tail (arg re-analysis + messages)
//! defers.

use pyo3::prelude::*;

/// Node-kind discriminator for the resolved `sym.node`, encoded so the
/// priority order of the body is preserved in a single int:
/// - `-1` sym is None (typeanal.py:547)
/// - `0`  node is None (typeanal.py:352)
/// - `1`  PlaceholderNode (typeanal.py:322)
/// - `2`  ParamSpecExpr (typeanal.py:369)
/// - `3`  TypeVarExpr (typeanal.py:401/418/425)
/// - `4`  TypeVarTupleExpr (typeanal.py:433/444)
/// - `5`  anything else (Var, TypeAlias, TypeInfo, ...) -> defer
const NODE_KIND_SYM_NONE: i64 = -1;
const NODE_KIND_NODE_NONE: i64 = 0;
const NODE_KIND_PLACEHOLDER: i64 = 1;
const NODE_KIND_PARAM_SPEC: i64 = 2;
const NODE_KIND_TYPE_VAR: i64 = 3;
const NODE_KIND_TYPE_VAR_TUPLE: i64 = 4;

/// Branch tags handed to the Python shim. Each maps to exactly one terminal
/// branch of `visit_unbound_type_nonoptional`; the comment cites the
/// typeanal.py line and the Python-side effect the shim must apply.
const TAG_PH_BECOMES_FINAL: i64 = 1; // 325-327 cannot_resolve_type + Any(from_error)
const TAG_PH_BECOMES_DEFER: i64 = 2; // 328-331 api.defer + PlaceholderType
const TAG_PH_BECOMES_RECORD: i64 = 3; // 331 api.record_incomplete_ref + PlaceholderType
const TAG_PH_PLAIN_FINAL: i64 = 4; // 345-347 cannot_resolve_type + Any(from_error)
const TAG_PH_PLAIN_RECORD: i64 = 5; // 349-351 record_incomplete_ref + Any(special_form)
const TAG_SYM_NONE: i64 = 6; // 547 Any(special_form)
const TAG_NODE_NONE: i64 = 7; // 352-354 fail(Internal error) + Any(special_form)
const TAG_PSPEC_UNBOUND_TVAR: i64 = 8; // 371-372 return t
const TAG_PSPEC_NOT_DECLARED: i64 = 9; // 374-375 fail + Any(from_error)
const TAG_PSPEC_UNBOUND: i64 = 10; // 377 fail + Any(from_error)
const TAG_PSPEC_ARGS_COMPONENT: i64 = 11; // 381-384 + 385-389 fail x2 + Any(from_error)
const TAG_PSPEC_ARGS: i64 = 12; // 381-384 fail + ParamSpecType
const TAG_PSPEC_COMPONENT: i64 = 13; // 385-389 fail + Any(from_error)
const TAG_PSPEC_OK: i64 = 14; // 391-400 ParamSpecType
const TAG_TVAR_ALIAS_NOT_DECLARED: i64 = 15; // 407-413 fail + Any(from_error)
const TAG_TVAR_ALIAS_BOUND: i64 = 16; // 414-415 fail + Any(from_error)
const TAG_TVAR_ERASED: i64 = 17; // 418-424 Any(from_error) (no fail)
const TAG_TVAR_ARGS: i64 = 18; // 427-429 fail + copy_modified
const TAG_TVAR_OK: i64 = 19; // 432 copy_modified
const TAG_TVT_ALIAS_NOT_DECLARED: i64 = 20; // 438-439 fail + Any(from_error)
const TAG_TVT_ALIAS_BOUND: i64 = 21; // 440-441 fail + Any(from_error)
const TAG_TVT_UNBOUND_TVAR: i64 = 22; // 446-447 return t
const TAG_TVT_NOT_DECLARED: i64 = 23; // 448-451 fail + Any(from_error)
const TAG_TVT_UNBOUND: i64 = 24; // 456 fail + Any(from_error)
const TAG_TVT_NESTING: i64 = 25; // 460-465 fail + Any(from_error)
const TAG_TVT_ARGS: i64 = 26; // 467-470 fail + TypeVarTupleType
const TAG_TVT_OK: i64 = 27; // 473-482 TypeVarTupleType

// Unbound non-alias TypeVarExpr under allow_unbound_tvars, mirroring the
// PSPEC/TVT arms; decided from the already-passed allow_unbound_tvars fact.
const TAG_TVAR_UNBOUND: i64 = 28; // 1731-1736 (without-info back) return t

/// `visit_unbound_type_nonoptional` front classifier. Mirrors the branch
/// order of typeanal.py:310-482 exactly and returns the terminal branch
/// tag; `None` defers to the pure-Python body.
///
/// Facts (all scalars / strings, no live type objects):
/// - `node_kind`: the NODE_KIND_* discriminator for `sym.node`.
/// - `placeholder_becomes_typeinfo`: `node.becomes_typeinfo`.
/// - `final_iteration` / `allow_placeholder`: analyzer + api flags.
/// - `has_hook`: `plugin.get_type_analyze_hook(node.fullname) is not None`.
/// - `tvar_def_exists` / `tvar_def_in_allowed` / `tvar_def_erased`: the
///   binding from `tvar_scope.get_binding` (Python-side) and its membership
///   in `allowed_alias_tvars` / `erase_tvar_defs`.
/// - `placeholder_in_tvar_params`: a typevar param (bound/default/values)
///   is a `PlaceholderType` -> the pre-check `api.defer()`.
/// - `allow_unbound_tvars` / `defining_alias` / `defining_literal`.
/// - `param_spec_name_set` / `allow_param_spec_literals` / `has_args`.
/// - `alias_type_params_names` + `tname`: for `not_declared_in_type_params`.
/// - `allow_type_var_tuple` / `nesting_level`: unpack-nesting check.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (
    node_kind,
    placeholder_becomes_typeinfo,
    final_iteration,
    allow_placeholder,
    has_hook,
    tvar_def_exists,
    tvar_def_in_allowed,
    tvar_def_erased,
    placeholder_in_tvar_params,
    allow_unbound_tvars,
    defining_alias,
    defining_literal,
    param_spec_name_set,
    allow_param_spec_literals,
    has_args,
    alias_type_params_names,
    tname,
    allow_type_var_tuple,
    nesting_level,
))]
pub(crate) fn rust_classify_unbound_front(
    node_kind: i64,
    placeholder_becomes_typeinfo: bool,
    final_iteration: bool,
    allow_placeholder: bool,
    has_hook: bool,
    tvar_def_exists: bool,
    tvar_def_in_allowed: bool,
    tvar_def_erased: bool,
    placeholder_in_tvar_params: bool,
    allow_unbound_tvars: bool,
    defining_alias: bool,
    defining_literal: bool,
    param_spec_name_set: bool,
    allow_param_spec_literals: bool,
    has_args: bool,
    alias_type_params_names: Option<Vec<String>>,
    tname: String,
    allow_type_var_tuple: i64,
    nesting_level: i64,
) -> PyResult<Option<i64>> {
    // `placeholder_in_tvar_params` is a Python-side pre-check side effect,
    // not a terminal branch; it is accepted but never decides a tag here.
    // The shim re-applies the deferral when Rust picks a front branch.
    let _ = placeholder_in_tvar_params;

    // Unresolved symbol (typeanal.py:547): the whole body is skipped.
    if node_kind == NODE_KIND_SYM_NONE {
        return Ok(Some(TAG_SYM_NONE));
    }
    // PlaceholderNode (typeanal.py:322-351): handled before node-None and
    // before the hook, so it takes precedence over both.
    if node_kind == NODE_KIND_PLACEHOLDER {
        return Ok(Some(if placeholder_becomes_typeinfo {
            if final_iteration {
                TAG_PH_BECOMES_FINAL
            } else if allow_placeholder {
                TAG_PH_BECOMES_DEFER
            } else {
                TAG_PH_BECOMES_RECORD
            }
        } else if final_iteration {
            TAG_PH_PLAIN_FINAL
        } else {
            TAG_PH_PLAIN_RECORD
        }));
    }
    // node is None (typeanal.py:352-354): internal error, before the hook.
    if node_kind == NODE_KIND_NODE_NONE {
        return Ok(Some(TAG_NODE_NONE));
    }
    // Plugin hook (typeanal.py:356-358): Python must run the hook body,
    // regardless of node kind.
    if has_hook {
        return Ok(None);
    }
    let not_declared = match &alias_type_params_names {
        None => false,
        Some(names) => !names.iter().any(|n| n == &tname),
    };
    match node_kind {
        // ParamSpecExpr (typeanal.py:369-400).
        NODE_KIND_PARAM_SPEC => {
            if !tvar_def_exists {
                if allow_unbound_tvars {
                    return Ok(Some(TAG_PSPEC_UNBOUND_TVAR));
                }
                return Ok(Some(if defining_alias && not_declared {
                    TAG_PSPEC_NOT_DECLARED
                } else {
                    TAG_PSPEC_UNBOUND
                }));
            }
            let component_bad = param_spec_name_set && !allow_param_spec_literals;
            if component_bad {
                return Ok(Some(if has_args {
                    TAG_PSPEC_ARGS_COMPONENT
                } else {
                    TAG_PSPEC_COMPONENT
                }));
            }
            Ok(Some(if has_args {
                TAG_PSPEC_ARGS
            } else {
                TAG_PSPEC_OK
            }))
        }
        // TypeVarExpr (typeanal.py:401-432).
        NODE_KIND_TYPE_VAR => {
            if defining_alias && !defining_literal && (!tvar_def_exists || !tvar_def_in_allowed) {
                return Ok(Some(if not_declared {
                    TAG_TVAR_ALIAS_NOT_DECLARED
                } else {
                    TAG_TVAR_ALIAS_BOUND
                }));
            }
            if tvar_def_exists && tvar_def_erased {
                return Ok(Some(TAG_TVAR_ERASED));
            }
            if tvar_def_exists {
                return Ok(Some(if has_args { TAG_TVAR_ARGS } else { TAG_TVAR_OK }));
            }
            // No binding and not a defining-alias error: the body's
            // without-info back returns the raw type under
            // allow_unbound_tvars; the fail tail still defers.
            if allow_unbound_tvars {
                return Ok(Some(TAG_TVAR_UNBOUND));
            }
            Ok(None)
        }
        // TypeVarTupleExpr (typeanal.py:433-482).
        NODE_KIND_TYPE_VAR_TUPLE => {
            if tvar_def_exists && defining_alias && !tvar_def_in_allowed {
                return Ok(Some(if not_declared {
                    TAG_TVT_ALIAS_NOT_DECLARED
                } else {
                    TAG_TVT_ALIAS_BOUND
                }));
            }
            if !tvar_def_exists {
                if allow_unbound_tvars {
                    return Ok(Some(TAG_TVT_UNBOUND_TVAR));
                }
                return Ok(Some(if defining_alias && not_declared {
                    TAG_TVT_NOT_DECLARED
                } else {
                    TAG_TVT_UNBOUND
                }));
            }
            if allow_type_var_tuple != nesting_level {
                return Ok(Some(TAG_TVT_NESTING));
            }
            Ok(Some(if has_args { TAG_TVT_ARGS } else { TAG_TVT_OK }))
        }
        // Var, TypeAlias, TypeInfo, function, ...: not classifiable from
        // facts alone (hook/special dispatch), defer to Python.
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Facts {
        node_kind: i64,
        placeholder_becomes_typeinfo: bool,
        final_iteration: bool,
        allow_placeholder: bool,
        has_hook: bool,
        tvar_def_exists: bool,
        tvar_def_in_allowed: bool,
        tvar_def_erased: bool,
        placeholder_in_tvar_params: bool,
        allow_unbound_tvars: bool,
        defining_alias: bool,
        defining_literal: bool,
        param_spec_name_set: bool,
        allow_param_spec_literals: bool,
        has_args: bool,
        alias_type_params_names: Option<Vec<String>>,
        tname: String,
        allow_type_var_tuple: i64,
        nesting_level: i64,
    }

    impl Facts {
        fn new(node_kind: i64) -> Self {
            Facts {
                node_kind,
                placeholder_becomes_typeinfo: false,
                final_iteration: false,
                allow_placeholder: false,
                has_hook: false,
                tvar_def_exists: false,
                tvar_def_in_allowed: false,
                tvar_def_erased: false,
                placeholder_in_tvar_params: false,
                allow_unbound_tvars: false,
                defining_alias: false,
                defining_literal: false,
                param_spec_name_set: false,
                allow_param_spec_literals: false,
                has_args: false,
                alias_type_params_names: None,
                tname: "T".to_string(),
                allow_type_var_tuple: -1,
                nesting_level: 0,
            }
        }
    }

    fn classify(f: &Facts) -> Option<i64> {
        rust_classify_unbound_front(
            f.node_kind,
            f.placeholder_becomes_typeinfo,
            f.final_iteration,
            f.allow_placeholder,
            f.has_hook,
            f.tvar_def_exists,
            f.tvar_def_in_allowed,
            f.tvar_def_erased,
            f.placeholder_in_tvar_params,
            f.allow_unbound_tvars,
            f.defining_alias,
            f.defining_literal,
            f.param_spec_name_set,
            f.allow_param_spec_literals,
            f.has_args,
            f.alias_type_params_names.clone(),
            f.tname.clone(),
            f.allow_type_var_tuple,
            f.nesting_level,
        )
        .unwrap()
    }

    #[test]
    fn unresolved_symbol_returns_any_special_form() {
        assert_eq!(
            classify(&Facts::new(NODE_KIND_SYM_NONE)),
            Some(TAG_SYM_NONE)
        );
    }

    // --- PlaceholderNode ---

    #[test]
    fn placeholder_becomes_typeinfo_final_iteration_errors() {
        let mut f = Facts::new(NODE_KIND_PLACEHOLDER);
        f.placeholder_becomes_typeinfo = true;
        f.final_iteration = true;
        assert_eq!(classify(&f), Some(TAG_PH_BECOMES_FINAL));
    }

    #[test]
    fn placeholder_becomes_typeinfo_allows_defer() {
        let mut f = Facts::new(NODE_KIND_PLACEHOLDER);
        f.placeholder_becomes_typeinfo = true;
        f.allow_placeholder = true;
        assert_eq!(classify(&f), Some(TAG_PH_BECOMES_DEFER));
    }

    #[test]
    fn placeholder_becomes_typeinfo_records_incomplete_ref() {
        let mut f = Facts::new(NODE_KIND_PLACEHOLDER);
        f.placeholder_becomes_typeinfo = true;
        assert_eq!(classify(&f), Some(TAG_PH_BECOMES_RECORD));
    }

    #[test]
    fn placeholder_plain_final_iteration_errors() {
        let mut f = Facts::new(NODE_KIND_PLACEHOLDER);
        f.final_iteration = true;
        assert_eq!(classify(&f), Some(TAG_PH_PLAIN_FINAL));
    }

    #[test]
    fn placeholder_plain_records_incomplete_ref() {
        assert_eq!(
            classify(&Facts::new(NODE_KIND_PLACEHOLDER)),
            Some(TAG_PH_PLAIN_RECORD)
        );
    }

    #[test]
    fn placeholder_beats_hook() {
        // Placeholder is decided before the hook check (typeanal.py:322 < 356).
        let mut f = Facts::new(NODE_KIND_PLACEHOLDER);
        f.has_hook = true;
        assert_eq!(classify(&f), Some(TAG_PH_PLAIN_RECORD));
    }

    // --- node is None ---

    #[test]
    fn node_none_is_internal_error() {
        assert_eq!(
            classify(&Facts::new(NODE_KIND_NODE_NONE)),
            Some(TAG_NODE_NONE)
        );
    }

    // --- hook defers for non-front kinds ---

    #[test]
    fn hook_defers_param_spec() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.has_hook = true;
        assert_eq!(classify(&f), None);
    }

    #[test]
    fn other_kind_defers() {
        assert_eq!(classify(&Facts::new(5)), None);
    }

    // --- ParamSpecExpr ---

    #[test]
    fn pspec_unbound_tvar_returns_t() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.allow_unbound_tvars = true;
        assert_eq!(classify(&f), Some(TAG_PSPEC_UNBOUND_TVAR));
    }

    #[test]
    fn pspec_unbound_not_declared() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["X".to_string()]);
        assert_eq!(classify(&f), Some(TAG_PSPEC_NOT_DECLARED));
    }

    #[test]
    fn pspec_unbound_plain() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["T".to_string()]);
        assert_eq!(classify(&f), Some(TAG_PSPEC_UNBOUND));
    }

    #[test]
    fn pspec_args_component_both_errors() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.tvar_def_exists = true;
        f.has_args = true;
        f.param_spec_name_set = true;
        assert_eq!(classify(&f), Some(TAG_PSPEC_ARGS_COMPONENT));
    }

    #[test]
    fn pspec_args_only() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.tvar_def_exists = true;
        f.has_args = true;
        assert_eq!(classify(&f), Some(TAG_PSPEC_ARGS));
    }

    #[test]
    fn pspec_component_only() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.tvar_def_exists = true;
        f.param_spec_name_set = true;
        assert_eq!(classify(&f), Some(TAG_PSPEC_COMPONENT));
    }

    #[test]
    fn pspec_ok() {
        let mut f = Facts::new(NODE_KIND_PARAM_SPEC);
        f.tvar_def_exists = true;
        assert_eq!(classify(&f), Some(TAG_PSPEC_OK));
    }

    // --- TypeVarExpr ---

    #[test]
    fn typevar_alias_not_declared() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["X".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVAR_ALIAS_NOT_DECLARED));
    }

    #[test]
    fn typevar_alias_bound() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["T".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVAR_ALIAS_BOUND));
    }

    #[test]
    fn typevar_alias_skipped_when_defining_literal() {
        // defining_literal disables the alias guard (typeanal.py:401-406).
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.defining_alias = true;
        f.defining_literal = true;
        assert_eq!(classify(&f), None);
    }

    #[test]
    fn typevar_erased() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.tvar_def_exists = true;
        f.tvar_def_erased = true;
        assert_eq!(classify(&f), Some(TAG_TVAR_ERASED));
    }

    #[test]
    fn typevar_args() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.tvar_def_exists = true;
        f.tvar_def_in_allowed = true;
        f.has_args = true;
        assert_eq!(classify(&f), Some(TAG_TVAR_ARGS));
    }

    #[test]
    fn typevar_ok() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.tvar_def_exists = true;
        f.tvar_def_in_allowed = true;
        assert_eq!(classify(&f), Some(TAG_TVAR_OK));
    }

    #[test]
    fn typevar_unbound_fail_tail_defers() {
        // No binding, not allowed: the body's without-info tail re-analyzes
        // args and emits fail/notes, so Rust defers (typeanal.py:1769-1839).
        assert_eq!(classify(&Facts::new(NODE_KIND_TYPE_VAR)), None);
    }

    #[test]
    fn typevar_unbound_allow_returns_t() {
        // Unbound tvar in an allowed context -> raw t via the without-info
        // back's Option 2 (typeanal.py:1731-1736).
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.allow_unbound_tvars = true;
        assert_eq!(classify(&f), Some(TAG_TVAR_UNBOUND));
    }

    #[test]
    fn typevar_unbound_allow_defining_literal() {
        // defining_literal skips the alias guard; the unbound back still
        // returns t under allow_unbound_tvars.
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.allow_unbound_tvars = true;
        f.defining_literal = true;
        assert_eq!(classify(&f), Some(TAG_TVAR_UNBOUND));
    }

    #[test]
    fn typevar_alias_error_beats_unbound_allow() {
        // The alias-guard error fires even when allow_unbound_tvars is set,
        // because the body checks the alias arm first (typeanal.py:412-428).
        let mut f = Facts::new(NODE_KIND_TYPE_VAR);
        f.allow_unbound_tvars = true;
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["X".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVAR_ALIAS_NOT_DECLARED));
    }

    // --- TypeVarTupleExpr ---

    #[test]
    fn tvartuple_alias_not_declared() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.tvar_def_exists = true;
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["X".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVT_ALIAS_NOT_DECLARED));
    }

    #[test]
    fn tvartuple_alias_bound() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.tvar_def_exists = true;
        f.defining_alias = true;
        f.tname = "Ts".to_string();
        f.alias_type_params_names = Some(vec!["Ts".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVT_ALIAS_BOUND));
    }

    #[test]
    fn tvartuple_unbound_tvar_returns_t() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.allow_unbound_tvars = true;
        assert_eq!(classify(&f), Some(TAG_TVT_UNBOUND_TVAR));
    }

    #[test]
    fn tvartuple_unbound_not_declared() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.defining_alias = true;
        f.alias_type_params_names = Some(vec!["X".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVT_NOT_DECLARED));
    }

    #[test]
    fn tvartuple_unbound_plain() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.defining_alias = true;
        f.tname = "Ts".to_string();
        f.alias_type_params_names = Some(vec!["Ts".to_string()]);
        assert_eq!(classify(&f), Some(TAG_TVT_UNBOUND));
    }

    #[test]
    fn tvartuple_nesting_mismatch() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.tvar_def_exists = true;
        f.allow_type_var_tuple = 1;
        f.nesting_level = 2;
        assert_eq!(classify(&f), Some(TAG_TVT_NESTING));
    }

    #[test]
    fn tvartuple_args() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.tvar_def_exists = true;
        f.allow_type_var_tuple = 0;
        f.has_args = true;
        assert_eq!(classify(&f), Some(TAG_TVT_ARGS));
    }

    #[test]
    fn tvartuple_ok() {
        let mut f = Facts::new(NODE_KIND_TYPE_VAR_TUPLE);
        f.tvar_def_exists = true;
        f.allow_type_var_tuple = 0;
        assert_eq!(classify(&f), Some(TAG_TVT_OK));
    }
}
