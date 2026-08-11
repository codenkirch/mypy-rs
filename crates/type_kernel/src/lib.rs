//! Native type-kernel seam for mypy.
//!
//! This crate ports pure `mypy.types` visitors onto a PyO3 extension that
//! walks live Python `Type` objects. Each visitor returns `None` for any type
//! class it does not handle, so the Python caller falls back to the pure-Python
//! visitor — the strangler-fig per-call gate. No behavior changes ship unless
//! `Options.native_type_kernel` is set, and even then unsupported cases
//! degrade gracefully.
//!
//! Stages:
//!   * **Stage 1** (`erase::erase_type`): mirrors `EraseTypeVisitor`. Proves
//!     the seam end-to-end with the smallest surface area.
//!   * **Stage 2** (`lkv::remove_instance_last_known_values`): mirrors
//!     `LastKnownValueEraser`. Broadens Rust coverage of the visitor dispatch
//!     on a hot path (checker, expression checker, binder).
//!   * **Stage 3a** (`wire::read_type_to_str`): a Rust-owned `Type` enum +
//!     binary wire-format reader, parity-tested but not yet wired to any
//!     visitor. Foundation for Stage 3c (`is_subtype`).
//!   * **Stage 3b** (`typeinfo::build_resolver` +
//!     `typeinfo::read_type_to_str_with_resolver`): freezes the live Python
//!     `TypeInfo` graph into a snapshot keyed by `fullname`, closing the
//!     Stage 3a deferred renderings (prefix-strip, enum/bytes literal,
//!     `[()]` variadic-tuple). Foundation for Stage 3c (`is_subtype`).
//!   * **Stage 3c / M8a** (`typeinfo::build_native_resolver` +
//!     `typeinfo::read_type_to_str_with_native_resolver`): enriches the
//!     snapshot with `bases`, `tuple_type`,
//!     `type_var_tuple_prefix/suffix`, `type_vars_with_variance`, and
//!     adds a `TypeAliasResolver` for `TypeAliasType` expansion. The
//!     `NativeTypeResolver` `#[pyclass]` holds both resolvers in Rust
//!     for zero-FFI-per-lookup access by Stage 3c `is_subtype`.
//!   * **Stage 4** (`argmap::rust_map_actuals_to_formals`): ports the pure
//!     `mypy.argmap.map_actuals_to_formals` binding step from `check_call`.
//!     Handles non-star actuals; returns `None` for star actuals so Python
//!     re-runs the function with the `actual_arg_type` callback. Foundation
//!     for the `rust_check_call` kernel.
//!
//! Shared infrastructure (`TypeRefs` class cache, `fallback_sentinel`/
//! `is_fallback`, `make_any`) lives in `refs` and is reused by both stages.
//! See `docs/rust-migration-strangler.md` ("Milestone 3/4/5 (Phase 4)") for the
//! full staging roadmap.

mod aliases;
mod applytype;
mod argapprox;
mod argmap;
mod astwire;
mod attrs;
mod cache;
mod callable_compat;
mod checkcall;
mod checker_helpers;
mod checker_stmts;
mod checker_visitor;
mod checkexpr_argcheck;
mod checkexpr_argcount;
mod checkexpr_functions;
mod checkmember;
mod checkoperator;
mod checkpattern;
mod checkstrformat;
mod constraints;
mod constraints_filter;
mod constraints_helpers;
mod dataclasses;
mod erase;
mod erase_typevars;
mod errors;
mod expand;
mod expandtype;
mod freshen;
mod generators;
mod lkv;
mod maptype;
mod meet;
mod messages;
mod mro;
mod operators;
mod overload;
mod plugin_helpers;
mod plugin_hooks;
mod refs;
mod semanal_algebra;
mod semanal_shared;
mod semanal_visitor;
mod serverdeps;
mod setops;
mod solve;
mod stubgen;
mod subtypes;
mod suggestions;
mod traverser;
mod typeanal_queries;
mod typeinfo;
mod typeops;
mod types_impl;
mod visitor;
mod wire;

use pyo3::prelude::*;

/// PyO3 module entry point: registers the visitor functions (Stages 1/2)
/// and the parity-only wire readers (Stages 3a/3b) + the Stage 3c M8a
/// native resolver.
#[pymodule]
fn type_kernel(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(erase::erase_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        erase::shallow_erase_type_for_equality,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        lkv::remove_instance_last_known_values,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(cache::rust_read_cache_meta, module)?)?;
    module.add_function(wrap_pyfunction!(cache::rust_read_cache_meta_ex, module)?)?;
    module.add_function(wrap_pyfunction!(wire::read_type_to_str, module)?)?;
    module.add_function(wrap_pyfunction!(typeinfo::build_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeinfo::read_type_to_str_with_resolver,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeinfo::build_native_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeinfo::read_type_to_str_with_native_resolver,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_subtype, module)?)?;
    // Issue #465: pure-computation helpers from subtypes.py.
    module.add_function(wrap_pyfunction!(
        subtypes::rust_has_underscore_prefix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_erased_instance, module)?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_try_restrict_literal_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_more_precise, module)?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_equivalent, module)?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_same_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        callable_compat::rust_callables_compatible,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_overlapping_types, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_narrow_declared_type, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_join, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_meet, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_join_types, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_meet_types, module)?)?;
    module.add_function(wrap_pyfunction!(
        argmap::rust_map_actuals_to_formals,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        argmap::rust_map_formals_to_actuals,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        argmap::rust_map_actuals_to_formals_with_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(expand::rust_expand_actual_type, module)?)?;
    module.add_function(wrap_pyfunction!(mro::rust_linearize_hierarchy, module)?)?;
    module.add_function(wrap_pyfunction!(expandtype::rust_expand_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        expandtype::rust_expand_type_by_instance,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        freshen::rust_freshen_all_functions_type_vars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_make_simplified_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_simple_literal_type, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_is_simple_literal, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_is_literal_type_like,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_try_getting_str_literals_from_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_try_getting_int_literals_from_type,
        module
    )?)?;
    // Issue #425: maptype nominal supertype mapping (M8d).
    module.add_function(wrap_pyfunction!(
        maptype::rust_map_instance_to_supertype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        maptype::rust_class_derivation_paths,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        maptype::rust_map_instance_to_direct_supertypes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_try_getting_bool_literals_from_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_try_getting_instance_fallback,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_true_only, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_false_only, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_true_or_false, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_try_expanding_sum_type_to_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_separate_union_literals,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_get_type_vars, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_erase_to_bound, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_tuple_fallback, module)?)?;
    module.add_function(wrap_pyfunction!(operators::rust_operator_tables, module)?)?;
    module.add_function(wrap_pyfunction!(
        erase_typevars::rust_erase_typevars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        erase_typevars::rust_replace_meta_vars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_has_type_vars, module)?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_has_recursive_types, module)?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_is_literal_type, module)?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_is_unannotated_any, module)?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_remove_dups, module)?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_type_vars_as_args, module)?)?;
    module.add_function(wrap_pyfunction!(
        visitor::rust_callable_with_ellipsis,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_find_unpack_in_list, module)?)?;
    module.add_function(wrap_pyfunction!(
        visitor::rust_split_with_prefix_and_suffix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        visitor::rust_flatten_nested_unions,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        visitor::rust_flatten_nested_tuples,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(visitor::rust_copy_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        applytype::rust_apply_generic_arguments,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(applytype::rust_has_no_typevars, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_any_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_uninhabited_component,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_ambiguous_uninhabited_component,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_allow_fast_container_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_bytes_component,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_bool_item,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_non_empty_tuple,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_coroutine_decorator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_async_def,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_duplicate_mapping,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_typed_callable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_private,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_operator_method,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_are_argument_counts_overlapping,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_type_type_context,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_try_getting_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_string_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_untyped_decorator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_typeddict_type_context,
        module
    )?)?;
    // M8c: visit_conditional_expr / visit_star_expr helpers.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_conditional_expr_join,
        module
    )?)?;
    // Issue #385: container-literal fast paths.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_container_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_tuple_context_matches,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_build_tuple_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_analyze_cond_branch,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_star_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_method_fullname,
        module
    )?)?;
    // Issue #432: overload-ambiguity approximate-similarity.
    module.add_function(wrap_pyfunction!(
        argapprox::rust_arg_approximate_similarity,
        module
    )?)?;
    // Issue #434: generator/coroutine return-type helpers.
    module.add_function(wrap_pyfunction!(
        generators::rust_is_generator_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        generators::rust_is_async_generator_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        generators::rust_get_generator_yield_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        generators::rust_get_generator_receive_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        generators::rust_get_generator_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        generators::rust_get_coroutine_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors::rust_format_messages_default,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints::rust_infer_constraints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints::rust_infer_constraints_full,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_helpers::rust_select_trivial,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_helpers::rust_exclude_non_meta_vars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_helpers::rust_is_similar_constraints,
        module
    )?)?;
    // Issue #474: pure constraint-list filtering functions.
    module.add_function(wrap_pyfunction!(
        constraints_filter::rust_skip_reverse_union_constraints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_filter::rust_filter_imprecise_kinds,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_filter::rust_is_type_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_filter::rust_unwrap_type_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_filter::rust_infer_directed_arg_constraints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkcall::rust_classify_call, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkcall::rust_calibrate_type_obj_return,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkcall::rust_normalize_callable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkcall::rust_real_union, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkcall::rust_possible_none_type_var_overlap,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkcall::rust_solve_generic_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkcall::rust_check_callable_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        overload::rust_check_overload_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_type_requires_usage,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_is_unreachable_map,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checker_stmts::rust_stmt_outcome, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_argcheck::rust_check_arguments,
        module
    )?)?;
    // Issue #473: check_argument_count + check_call_expr_with_callee_type
    // pure dispatch (decision records, no message emission).
    module.add_function(wrap_pyfunction!(
        checkexpr_argcount::rust_check_argument_count,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_argcount::rust_check_call_expr_callable_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_argcount::rust_should_dispatch_union_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_with_exit_suppresses,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_try_handler_union,
        module
    )?)?;
    // Issue #347: checker narrowing helpers. narrow_declared_type is already
    // registered via meet::rust_narrow_declared_type (the authoritative
    // meet.py seam); the other four defer (None) and exist only as
    // entry-points so Python can call through the gate.
    module.add_function(wrap_pyfunction!(checker_stmts::rust_narrow_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_infer_value_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_find_isinstance_join,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_partial_type_inference,
        module
    )?)?;
    // Issue #387: identity-equality narrowing. Returns (if, else) type blobs
    // or None to defer to the pure-Python path.
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_narrow_type_by_identity_equality,
        module
    )?)?;
    // Issue #445: is_valid_inferred_type pure boolean query.
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_is_valid_inferred_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_true_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_false_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_literal_none,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_literal_not_implemented,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checker_visitor::rust_is_static, module)?)?;
    module.add_function(wrap_pyfunction!(checker_visitor::rust_is_property, module)?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_settable_property,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_custom_settable_property,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_can_have_shared_disjoint_base,
        module
    )?)?;
    // Issue #458: pure checkexpr visitor identity/constant methods.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_temp_node,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_promote_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_paramspec_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_type_var_tuple_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_newtype_expr,
        module
    )?)?;
    // Issue #457: Node object-model pure predicates from mypy/nodes.py.
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_func_has_self_or_cls_argument,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_func_item_is_dynamic,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_decorator_is_dynamic,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_overloaded_is_dynamic,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_typeinfo_is_generic,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_typeinfo_is_metaclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_typeinfo_has_base,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(solve::rust_solve_one, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_is_trivial_bound, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_find_linear, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_join_sorted_key, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_get_vars, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_is_callable_protocol, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_solve_dependent, module)?)?;
    module.add_function(wrap_pyfunction!(solve::rust_solve_constraints, module)?)?;
    module.add_function(wrap_pyfunction!(
        solve::rust_infer_function_type_arguments,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(messages::rust_format_key_list, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_quote_type_string, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_capitalize, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_pretty_seq, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_format_string_list, module)?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_format_item_name_list,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_wrong_type_arg_count,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(messages::rust_strip_quotes, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_extract_type, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_variance_string, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_format_type, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_format_type_bare, module)?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_format_type_distinctly,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_append_invariance_notes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_append_numbers_notes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(messages::rust_append_union_note, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_pretty_callable, module)?)?;
    // Callable name helpers — ports callable_name and for_function.
    module.add_function(wrap_pyfunction!(messages::rust_callable_name, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_for_function, module)?)?;
    // Issue #358: dmypy server pure helper — count_stats from mypy/util.py
    module.add_function(wrap_pyfunction!(messages::rust_count_stats, module)?)?;
    // Issue #438: pure string-message generators from mypy/messages.py
    module.add_function(wrap_pyfunction!(messages::rust_too_few_arguments, module)?)?;
    module.add_function(wrap_pyfunction!(messages::rust_too_many_arguments, module)?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_too_many_positional_arguments,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_missing_named_argument,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_unexpected_keyword_argument_for_function,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(messages::rust_invalid_index_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_wrong_number_values_to_unpack,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_undefined_in_superclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_signatures_incompatible,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_signature_incompatible_with_supertype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_is_numeric_format_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkpattern::rust_is_uninhabited, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_get_match_arg_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkpattern::rust_get_type_range, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_should_self_match,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_can_match_sequence,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_contract_starred_pattern_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_expand_starred_pattern_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_construct_sequence_child,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_parse_conversion_specifiers,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_find_non_escaped_targets,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_parse_format_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_parse_placeholder_format,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_analyze_conversion_specifiers,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_has_return_statement,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_has_str_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_has_yield_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_has_yield_from_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_has_await_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_return_statements,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_yield_expressions,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_yield_from_expressions,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_name_and_member_expressions,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_algebra::rust_make_any_non_explicit,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_algebra::rust_make_any_non_unimported,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_algebra::rust_replace_implicit_first_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_refers_to_fullname,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_refers_to_class_or_function,
        module
    )?)?;
    // Issue #391: additional pure semanal helpers (is_init_only, erase_func_annotations,
    // get_deprecated, get_name_repr_of_expr) are registered below alongside the
    // already-existing is_trivial_body, find_duplicate, etc.
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_trivial_body,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_find_duplicate,
        module
    )?)?;
    // Issue #391: additional pure semanal pure helpers.
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_init_only,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_erase_func_annotations,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_get_deprecated,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_get_name_repr_of_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_valid_replacement,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_same_symbol,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_names_modified_in_lvalue,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_names_modified_by_assignment,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_remove_imported_names_from_symtable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_apply_semantic_analyzer_patches,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_decorators,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_visitor::rust_lookup, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_imports,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_member_resolution,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_var_is_typing_special_form,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_same_var_from_getattr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_get_typevarlike_declaration,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_visitor::rust_parse_bool, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_mangled_global,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_initial_mangled_global,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_final_redefinition,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_can_possibly_be_typevarlike_declaration,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_can_possibly_be_type_form,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_visitor::rust_is_type_ref, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_can_be_type_alias,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_check_typevarlike_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_extract_typevarlike_name,
        module
    )?)?;
    // semanal_shared.py + sharedparse.py pure helpers.
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_special_function_elide_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_argument_elide_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_set_callable_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_has_placeholder,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_calculate_tuple_fallback,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_shared::rust_find_dataclass_transform_spec,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_has_explicit_any,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_has_any_from_unimported_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_collect_all_inner_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_make_optional_type,
        module
    )?)?;
    // Hot path: mirrors TypeAnalyser.anal_type for already-bound types
    // (Instance, Callable, TypeVar, Tuple, etc.). Returns None for types
    // needing semantic context, matching Python's deferral semantics.
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_type_analyze,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkmember::rust_bind_self_fast, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_classify_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_instance_fallback,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkmember::rust_has_operator, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_meta_has_operator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_defined_in_superclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_instance_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_union_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_none_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_typeddict_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_enum_class_attribute_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_descriptor_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkoperator::rust_check_operator,
        module
    )?)?;
    module.add_class::<plugin_hooks::PluginHookRegistry>()?;
    module.add_function(wrap_pyfunction!(
        plugin_hooks::rust_resolve_plugin_hook,
        module
    )?)?;
    module.add_class::<typeinfo::NativeTypeResolver>()?;
    module.add_function(wrap_pyfunction!(suggestions::rust_best_matches, module)?)?;
    module.add_function(wrap_pyfunction!(suggestions::rust_pretty_seq, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_get_type_triggers,
        module
    )?)?;
    // M354: pure server trigger/target computation
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_compute_wildcard_triggers,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_compute_target_modules,
        module
    )?)?;
    // M388: pure server update helpers (dedupe_modules, get_sources, message extraction)
    module.add_function(wrap_pyfunction!(serverdeps::rust_dedupe_modules, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_get_module_to_path_map,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(serverdeps::rust_get_sources, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_extract_fnam_from_message,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_extract_possible_fnam_from_message,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_sort_messages_preserving_file_order,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(stubgen::rust_stubgen_render, module)?)?;
    module.add_function(wrap_pyfunction!(
        stubgen::rust_stubgen_render_type_args,
        module
    )?)?;
    // Issue #392: stubgen pure collectors.
    module.add_function(wrap_pyfunction!(stubgen::rust_get_assigned_names, module)?)?;
    module.add_function(wrap_pyfunction!(stubgen::rust_is_none_expr, module)?)?;
    module.add_function(wrap_pyfunction!(
        stubgen::rust_is_pybind11_overloaded_function_docstring,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        stubgen::rust_method_name_sort_key,
        module
    )?)?;
    // Attrs plugin transform (Issue #357): seam function for class decoration.
    module.add_function(wrap_pyfunction!(attrs::rust_transform_attrs, module)?)?;
    module.add_function(wrap_pyfunction!(attrs::rust_serialize_fields, module)?)?;
    // Stage 30: dataclasses plugin transform seam (Issue #356). Computes
    // the `__init__` argument names/kinds from serialized field metadata;
    // Python validates and applies the AST mutation.
    module.add_function(wrap_pyfunction!(
        dataclasses::rust_dataclass_transform,
        module
    )?)?;
    // Issue #393: sibling seam for `__post_init__` (InitVar fields only).
    module.add_function(wrap_pyfunction!(
        dataclasses::rust_dataclass_post_init_transform,
        module
    )?)?;
    // Issue #394: pure plugin common helpers.
    module.add_function(wrap_pyfunction!(
        plugin_helpers::rust_find_shallow_matching_overload_item,
        module
    )?)?;
    // Issue #389: dmypy_server pure helpers — plain-record shuffling.
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_process_start_options,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_ignore_suppressed_imports,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(serverdeps::rust_get_meminfo, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_response_metadata,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_find_all_sources_in_build,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_add_all_sources_to_changed,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(serverdeps::rust_fix_module_deps, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_filter_out_missing_top_level_packages,
        module
    )?)?;
    // Issue #456: pure Type object-model methods (can_be_true/false_default,
    // CallableType accessors, TupleType/UnionType length).
    module.add_function(wrap_pyfunction!(
        types_impl::rust_can_be_true_default,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_can_be_false_default,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_min_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_is_var_arg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_is_kw_arg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_max_possible_positional_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_is_generic,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(types_impl::rust_tuple_length, module)?)?;
    module.add_function(wrap_pyfunction!(types_impl::rust_union_length, module)?)?;
    // Issue #477: checker narrowing + type-validation pure helpers.
    module.add_function(wrap_pyfunction!(
        checker_helpers::rust_custom_special_method,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_helpers::rust_has_custom_eq_checks,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_helpers::rust_restrict_subtype_away,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_helpers::rust_join_type_list,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_helpers::rust_get_protocol_member,
        module
    )?)?;
    Ok(())
}
