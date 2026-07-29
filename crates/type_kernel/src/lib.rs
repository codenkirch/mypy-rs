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
//!     `typeinfo::read_type_to_str_with_native_resolver` +
//!     `aliases::build_alias_resolver`): enriches the snapshot with
//!     `bases`, `tuple_type`, `type_var_tuple_prefix/suffix`,
//!     `type_vars_with_variance`, and adds a `TypeAliasResolver` for
//!     `TypeAliasType` expansion. The `NativeTypeResolver` `#[pyclass]`
//!     holds both resolvers in Rust for zero-FFI-per-lookup access by
//!     Stage 3c `is_subtype`.
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
mod argmap;
mod astwire;
mod checker_algebra;
mod checker_engine;
mod checker_extra;
mod checker_full;
mod checkexpr_algebra;
mod checkexpr_core;
mod checkexpr_engine;
mod checkexpr_evaluator;
mod checkexpr_full;
mod checkexpr_functions;
mod checkmember;
mod checkpattern;
mod checkstrformat;
mod constraints;
mod erase;
mod erase_typevars;
mod errors;
mod expandtype;
mod lkv;
mod messages;
mod mro;
mod mypyc_port;
mod nodes;
mod operators;
mod plugin_hooks;
mod refs;
mod semanal_algebra;
mod semanal_core;
mod semanal_full;
mod semanal_pass;
mod setops;
mod subtypes;
mod traverser;
mod typeanal;
mod typeinfo;
mod typeops;
mod types_codec;
mod types_engine;
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
        lkv::remove_instance_last_known_values,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(wire::read_type_to_str, module)?)?;
    module.add_function(wrap_pyfunction!(wire::round_trip_type_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(typeinfo::build_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeinfo::read_type_to_str_with_resolver,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(aliases::build_alias_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(typeinfo::build_native_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeinfo::read_type_to_str_with_native_resolver,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_subtype, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_join, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_meet, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_join_types, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_meet_types, module)?)?;
    module.add_function(wrap_pyfunction!(
        argmap::rust_map_actuals_to_formals,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(mro::rust_linearize_hierarchy, module)?)?;
    module.add_function(wrap_pyfunction!(expandtype::rust_expand_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_make_simplified_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_simple_literal_type, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_is_simple_literal, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_true_only, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_false_only, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_true_or_false, module)?)?;
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
        checkexpr_functions::rust_flatten_types_if_tuple,
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
    module.add_function(wrap_pyfunction!(
        errors::rust_format_messages_default,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints::rust_infer_constraints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(messages::rust_format_key_list, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkstrformat::rust_is_numeric_format_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(nodes::rust_is_builtin_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_member_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_is_self_match_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkpattern::rust_is_non_sequence_match_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_codec::rust_is_valid_type_codec_tag,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_algebra::rust_is_type_overlap,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal::rust_is_type_constructor,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeanal::rust_is_self_type_name, module)?)?;
    module.add_function(wrap_pyfunction!(typeanal::rust_make_optional_type, module)?)?;
    module.add_function(wrap_pyfunction!(typeanal::rust_has_explicit_any, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeanal::rust_has_any_from_unimported_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal::rust_collect_all_inner_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeanal::rust_unknown_unpack, module)?)?;
    module.add_function(wrap_pyfunction!(typeanal::rust_find_self_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_algebra::rust_allow_fast_container_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_algebra::rust_has_erased_component,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_algebra::rust_replace_callable_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_algebra::rust_all_same_types,
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
        mypyc_port::rust_can_subclass_builtin,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_check_call_dispatch,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_engine::rust_infer_binary_op_simple,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_algebra::rust_clean_up_type_aliases,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_core::rust_analyze_symbol_table_entry,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_core::rust_check_overload_call_core,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_evaluator::rust_evaluate_binary_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_pass::rust_run_semanal_pass_check,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_engine::rust_types_engine_classify_tag,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_engine::rust_checker_engine_evaluate_binop,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_full::rust_full_type_check_statement,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_full::rust_full_semanal_analyze_symbol,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_full::rust_full_checkexpr_eval_binop,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_extra::rust_extra_type_check_statement,
        module
    )?)?;
    module.add_class::<plugin_hooks::PluginHookRegistry>()?;
    module.add_class::<typeinfo::NativeTypeResolver>()?;
    Ok(())
}
